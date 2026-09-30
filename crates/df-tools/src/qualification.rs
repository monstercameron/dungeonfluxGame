//! Synthetic pressure only. Reports measurements without approving physical-device G02.
use crate::browser::{FixtureClient, request, require, sample, tunnel_url};
use df_protocol::transport_fixture::Sample;
use df_rpc_bridge::{BrowserConnection, ConnectionSnapshot, RPC_MESSAGE_BYTES};
use futures::{
    FutureExt,
    channel::oneshot,
    future::{Either, select},
};
use gloo_timers::future::TimeoutFuture;
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Document, HtmlButtonElement};
use web_time::Instant;

fn memory_bytes() -> Result<usize, String> {
    let memory = wasm_bindgen::memory()
        .dyn_into::<js_sys::WebAssembly::Memory>()
        .map_err(|_| "WASM memory unavailable")?;
    let buffer = memory
        .buffer()
        .dyn_into::<js_sys::ArrayBuffer>()
        .map_err(|_| "WASM memory buffer unavailable")?;
    Ok(buffer.byte_length() as usize)
}
struct AnimationFrame {
    id: i32,
    _callback: Closure<dyn FnMut(f64)>,
}
impl Drop for AnimationFrame {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            let _ = window.cancel_animation_frame(self.id);
        }
    }
}
async fn frame() -> Result<f64, String> {
    let (sender, receiver) = oneshot::channel();
    let mut sender = Some(sender);
    let callback = Closure::wrap(Box::new(move |time| {
        if let Some(sender) = sender.take() {
            let _ = sender.send(time);
        }
    }) as Box<dyn FnMut(f64)>);
    let id = web_sys::window()
        .ok_or("window unavailable")?
        .request_animation_frame(callback.as_ref().unchecked_ref())
        .map_err(|_| "animation frame unavailable")?;
    let _owner = AnimationFrame {
        id,
        _callback: callback,
    };
    receiver
        .await
        .map_err(|_| "animation frame owner dropped".to_owned())
}
async fn heartbeat(done: Rc<Cell<bool>>) -> Result<Vec<f64>, String> {
    let mut previous = frame().await?;
    let mut gaps = vec![];
    while !done.get() {
        let current = frame().await?;
        gaps.push(current - previous);
        previous = current;
    }
    Ok(gaps)
}
fn percentile(samples: &[f64], percentile: usize) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[(sorted.len() * percentile)
        .div_ceil(100)
        .saturating_sub(1)
        .min(sorted.len() - 1)]
}
fn display(document: &Document, text: &str) {
    if let Some(element) = document.get_element_by_id("qualification") {
        element.set_text_content(Some(text));
    }
}
#[derive(Debug)]
struct StreamObservation {
    payload_bytes: usize,
    memory_peak: usize,
    first_message_ms: f64,
    arrival_gaps_ms: Vec<f64>,
}
async fn consume(
    mut client: FixtureClient,
    kind: &str,
    count: u32,
    pause: u32,
    connection: &BrowserConnection,
) -> Result<StreamObservation, String> {
    let started = Instant::now();
    let mut stream = client
        .server_stream(request(Sample {
            sequence: count,
            payload: format!("pressure:{kind}").into_bytes(),
            ..sample(0)
        })?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    let mut bytes = 0;
    let mut messages = 0;
    let mut memory_peak = memory_bytes()?;
    let mut first_message_ms = None;
    let mut previous = None;
    let mut arrival_gaps_ms = vec![];
    while let Some(message) = stream.message().await.map_err(|error| error.to_string())? {
        let arrived_ms = started.elapsed().as_secs_f64() * 1000.0;
        if let Some(previous) = previous {
            arrival_gaps_ms.push(arrived_ms - previous);
        } else {
            first_message_ms = Some(arrived_ms);
        }
        previous = Some(arrived_ms);
        require(message.sequence == messages, "pressure sequence mismatch")?;
        bytes += message.payload.len();
        messages += 1;
        memory_peak = memory_peak.max(memory_bytes()?);
        let snapshot = connection.snapshot().map_err(|error| error.to_string())?;
        require(
            snapshot.envelope_within_limit()
                && snapshot.callback_bytes.current <= 1024 * 1024
                && snapshot.callback_items.current <= 256,
            "owned resource bound exceeded",
        )?;
        if pause > 0 {
            TimeoutFuture::new(pause).await;
        }
    }
    require(messages == count, "pressure stream silently lost messages")?;
    require(
        stream
            .trailers()
            .await
            .map_err(|error| error.to_string())?
            .is_some_and(|trailers| {
                trailers
                    .get("fixture-terminal")
                    .is_some_and(|value| value == "observed")
            }),
        "pressure terminal trailers missing",
    )?;
    Ok(StreamObservation {
        payload_bytes: bytes,
        memory_peak,
        first_message_ms: first_message_ms.ok_or("pressure stream never produced a message")?,
        arrival_gaps_ms,
    })
}
async fn small_calls(mut client: FixtureClient, count: usize) -> Result<Vec<f64>, String> {
    let mut latencies = vec![];
    for sequence in 0..count {
        let start = Instant::now();
        let output = client
            .unary(request(sample(sequence as u32))?)
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        require(
            output.sequence == sequence as u32,
            "small RPC mismatch under pressure",
        )?;
        latencies.push(start.elapsed().as_secs_f64() * 1000.0);
        TimeoutFuture::new(10).await;
    }
    Ok(latencies)
}
async fn stats(client: &mut FixtureClient) -> Result<Sample, String> {
    client
        .unary(request(Sample {
            payload: b"stats".to_vec(),
            ..sample(0)
        })?)
        .await
        .map(|response| response.into_inner())
        .map_err(|error| error.to_string())
}
async fn probe(kind: &str) -> Result<(ConnectionSnapshot, tonic::Code, f64), String> {
    let (connection, channel) =
        BrowserConnection::connect(&tunnel_url(&format!("/malicious/{kind}"))?)
            .await
            .map_err(|error| error.to_string())?;
    let mut client = FixtureClient::new(channel).max_decoding_message_size(RPC_MESSAGE_BYTES);
    let started = Instant::now();
    let result = select(
        client.unary(request(sample(0))?).boxed_local(),
        TimeoutFuture::new(2500).boxed_local(),
    )
    .await;
    let status = match result {
        Either::Left((result, _)) => result
            .err()
            .ok_or("malicious peer returned accepted RPC")?
            .code(),
        Either::Right(((), rpc)) => {
            drop(rpc);
            return Err(format!(
                "malicious {kind} peer did not terminate within 2500ms"
            ));
        }
    };
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    TimeoutFuture::new(10).await;
    let snapshot = connection.snapshot().map_err(|error| error.to_string())?;
    require(
        snapshot.rejected || snapshot.driver_failed || kind == "header",
        "no controlled adapter/HTTP2 rejection observed",
    )?;
    if kind == "websocket" {
        require(
            snapshot.rejected
                && snapshot.websocket_receive.peak == df_rpc_bridge::MESSAGE_BYTES + 1,
            "oversized engine receipt was not actually rejected before Rust copying",
        )?;
    }
    if kind == "header" {
        require(
            snapshot.received_frames >= 2 && elapsed_ms < 1000.0,
            "header-list probe ended through server timeout rather than bounded stream rejection",
        )?;
    }
    connection.close();
    Ok((snapshot, status, elapsed_ms))
}
async fn run(document: &Document) -> Result<(), String> {
    display(
        document,
        "RUNNING · simultaneous slow consumer / paced synthetic media / bulk / small RPC",
    );
    let memory_before = memory_bytes()?;
    let cold_start = Instant::now();
    let (connection, channel) = BrowserConnection::connect(&tunnel_url("/tunnel")?)
        .await
        .map_err(|error| error.to_string())?;
    let cold_ms = cold_start.elapsed().as_secs_f64() * 1000.0;
    let mut client = FixtureClient::new(channel).max_decoding_message_size(RPC_MESSAGE_BYTES);
    let baseline = stats(&mut client).await?;
    let warm = small_calls(client.clone(), 30).await?;
    let done = Rc::new(Cell::new(false));
    let pressure_done = done.clone();
    let pressure = async {
        let result = futures::join!(
            consume(client.clone(), "slow", 64, 20, &connection),
            consume(client.clone(), "paced", 80, 0, &connection),
            consume(client.clone(), "bulk", 128, 0, &connection),
            small_calls(client.clone(), 60)
        );
        pressure_done.set(true);
        result
    };
    let ((slow, paced, bulk, latencies), frames) = futures::join!(pressure, heartbeat(done));
    let slow = slow?;
    let paced = paced?;
    let bulk = bulk?;
    let latencies = latencies?;
    let frames = frames?;
    require(
        !frames.is_empty(),
        "no animation frame responsiveness observations",
    )?;
    let snapshot = connection.snapshot().map_err(|error| error.to_string())?;
    require(
        snapshot.receive_credit.data_bytes > 8 * 1024 * 1024
            && snapshot.receive_credit.update_bytes > 0
            && snapshot.decode_yields > 0,
        "pressure did not exercise DATA credit and time slicing",
    )?;
    // Drop an active server-owned stream and require the server's real counter to return.
    let mut waiting = client
        .server_stream(request(Sample {
            behavior: df_protocol::transport_fixture::sample::Behavior::Wait as i32,
            ..sample(0)
        })?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    require(
        waiting
            .message()
            .await
            .map_err(|error| error.to_string())?
            .is_some(),
        "cancellation stream never started",
    )?;
    drop(waiting);
    let mut cleaned = false;
    for _ in 0..100 {
        let current = stats(&mut client).await?;
        if current.active_calls == 1 && current.sequence > baseline.sequence {
            cleaned = true;
            break;
        }
        TimeoutFuture::new(5).await;
    }
    require(
        cleaned,
        "server work failed to terminate after cancellation",
    )?;
    connection.close();
    TimeoutFuture::new(20).await;
    let closed = connection.snapshot().map_err(|error| error.to_string())?;
    require(
        closed.closed && closed.callback_bytes.current == 0 && closed.callback_items.current == 0,
        "browser callbacks/queues remained after owner cancellation",
    )?;
    let mut text = format!(
        "PASS · desktop pressure streams complete with generated OK trailers\nSlow: {} bytes; paced media: {} bytes; bulk: {} bytes\nCold connection: {cold_ms:.3}ms\nWarm RPC p95/p99: {:.3}/{:.3}ms\nPressure RPC p95/p99: {:.3}/{:.3}ms\nAnimation gaps p95/p99: {:.3}/{:.3}ms; frames={}\nWASM linear memory allocated: before={memory_before}, peak={}, after={} bytes (not live heap or engine memory)\nPressure resource snapshot: {snapshot:?}\nPASS · cancellation/server owner cleanup + browser callback cleanup\nClosed snapshot: {closed:?}\nWarm samples ms: {warm:?}\nPressure samples ms: {latencies:?}\nAnimation gap samples ms: {frames:?}\n",
        slow.payload_bytes,
        paced.payload_bytes,
        bulk.payload_bytes,
        percentile(&warm, 95),
        percentile(&warm, 99),
        percentile(&latencies, 95),
        percentile(&latencies, 99),
        percentile(&frames, 95),
        percentile(&frames, 99),
        frames.len(),
        slow.memory_peak
            .max(paced.memory_peak)
            .max(bulk.memory_peak),
        memory_bytes()?
    );
    text.push_str(&format!(
        "Paced first message: {:.3}ms; arrival interval p95/p99: {:.3}/{:.3}ms; samples ms: {:?}\n",
        paced.first_message_ms,
        percentile(&paced.arrival_gaps_ms, 95),
        percentile(&paced.arrival_gaps_ms, 99),
        paced.arrival_gaps_ms
    ));
    display(document, &text);
    for kind in ["websocket", "frame", "header", "continuation", "flood"] {
        let result = probe(kind).await?;
        text.push_str(&format!(
            "PASS · malicious {kind} controlled rejection: {result:?}\n"
        ));
        display(document, &text);
    }
    let user_agent = web_sys::window()
        .ok_or("window unavailable")?
        .navigator()
        .user_agent()
        .map_err(|_| "user agent unavailable")?;
    text.push_str(&format!("Engine identity: {user_agent}\nBuild: {}\nG02 INCONCLUSIVE · required physical iOS/Android, network matrix, whole-process CPU/memory and browser pre-callback allocations remain unqualified. Local latency observations have no approved production budget.\n", crate::BUILD_ID));
    display(document, &text);
    save_report("qualification", &text).await?;
    Ok(())
}
pub(super) fn install(document: &Document, active: Rc<Cell<bool>>) -> Result<(), JsValue> {
    let button = document
        .get_element_by_id("qualify")
        .ok_or_else(|| JsValue::from_str("qualification button unavailable"))?
        .dyn_into::<HtmlButtonElement>()?;
    button.set_disabled(false);
    button.set_text_content(Some("Run desktop pressure qualification"));
    let document = document.clone();
    let callback_button = button.clone();
    let callback = Closure::wrap(Box::new(move || {
        if active.replace(true) {
            return;
        }
        callback_button.set_disabled(true);
        let document = document.clone();
        let button = callback_button.clone();
        let active = active.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let outcome = select(
                run(&document).boxed_local(),
                TimeoutFuture::new(30000).boxed_local(),
            )
            .await;
            let result = match outcome {
                Either::Left((result, _)) => result,
                Either::Right(((), owned)) => {
                    drop(owned);
                    Err("qualification exceeded owned 30-second deadline".to_owned())
                }
            };
            if let Err(error) = result {
                display(
                    &document,
                    &format!("FAIL · {error}\nG02 INCONCLUSIVE · incomplete qualification"),
                );
            }
            button.set_disabled(false);
            active.set(false);
        });
    }) as Box<dyn FnMut()>);
    button.set_onclick(Some(callback.as_ref().unchecked_ref()));
    // Owned by this document/WASM instance for its page lifetime.
    callback.forget();
    Ok(())
}

pub(super) async fn save_report(kind: &str, text: &str) -> Result<(), String> {
    let options = web_sys::RequestInit::new();
    options.set_method("POST");
    options.set_body(&JsValue::from_str(text));
    let window = web_sys::window().ok_or("window unavailable")?;
    let response = wasm_bindgen_futures::JsFuture::from(
        window.fetch_with_str_and_init(&format!("/fixture-report/{kind}"), &options),
    )
    .await
    .map_err(|_| "loopback evidence report failed")?
    .dyn_into::<web_sys::Response>()
    .map_err(|_| "loopback evidence response unavailable")?;
    require(
        response.status() == 204,
        "loopback evidence report rejected",
    )
}
