//! Synthetic pressure only. Reports measurements without approving physical-device G02.
use crate::browser::{FixtureClient, request, require, sample, tunnel_url};
use df_protocol::transport_fixture::Sample;
use df_rpc_bridge::{BrowserConnection, ConnectionSnapshot, RPC_MESSAGE_BYTES};
use futures::{
    FutureExt,
    channel::oneshot,
    future::{Either, join_all, select},
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
fn display_callback_capacity(document: &Document, text: &str) {
    if let Some(element) = document.get_element_by_id("callback-capacity-report") {
        element.set_text_content(Some(text));
    }
}
fn display_connection_credit(document: &Document, text: &str) {
    if let Some(element) = document.get_element_by_id("connection-credit-report") {
        element.set_text_content(Some(text));
    }
}

struct CreditPoint {
    elapsed_ms: f64,
    snapshot: ConnectionSnapshot,
    wasm_buffer_bytes: usize,
}
fn credit_point(connection: &BrowserConnection, started: Instant) -> Result<CreditPoint, String> {
    Ok(CreditPoint {
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        snapshot: connection.snapshot().map_err(|error| error.to_string())?,
        wasm_buffer_bytes: memory_bytes()?,
    })
}
fn credit_line(label: &str, point: &CreditPoint) -> String {
    let credit = &point.snapshot.receive_credit;
    format!(
        "{label} at {:.1}ms: connection available={} DATA={} WINDOW_UPDATE={} peak_grant={} peak_held={}; callback bytes={}/{} items={}/{} retained Vec capacity={}/{}; WASM buffer={} bytes\n",
        point.elapsed_ms,
        credit.available,
        credit.data_bytes,
        credit.update_bytes,
        credit.peak_grant,
        credit.peak_held,
        point.snapshot.callback_bytes.current,
        point.snapshot.callback_bytes.peak,
        point.snapshot.callback_items.current,
        point.snapshot.callback_items.peak,
        point.snapshot.browser_callback_vec_capacity.current,
        point.snapshot.browser_callback_vec_capacity.peak,
        point.wasm_buffer_bytes,
    )
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
async fn probe(
    kind: &str,
) -> Result<(ConnectionSnapshot, ConnectionSnapshot, tonic::Code, f64), String> {
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
                && snapshot.websocket_receive.peak == df_rpc_bridge::MESSAGE_BYTES + 1
                && snapshot.browser_callback_vec_capacity.total == 0
                && snapshot.callback_bytes.total == 0,
            "oversized engine receipt was not actually rejected before Rust copying",
        )?;
    }
    if kind == "header" {
        require(
            snapshot.received_frames >= 2 && elapsed_ms < 1000.0,
            "header-list probe ended through server timeout rather than bounded stream rejection",
        )?;
    }
    if kind == "control-rate" {
        require(
            snapshot.rejected
                && snapshot.received_control_frames == 100
                && snapshot.received_frames == 101
                && snapshot.incoming_control_window.len() == 100
                && snapshot
                    .rejected_control
                    .is_some_and(|frame| frame.kind == 4),
            "exact 101st incoming control did not trigger adapter rate rejection",
        )?;
    }
    connection.close();
    TimeoutFuture::new(20).await;
    let closed = connection.snapshot().map_err(|error| error.to_string())?;
    require(
        closed.closed
            && closed.callback_bytes.current == 0
            && closed.callback_items.current == 0
            && closed.browser_callback_vec_capacity.current == 0,
        "malicious probe left browser callback ownership after close",
    )?;
    Ok((snapshot, closed, status, elapsed_ms))
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
    if slow.is_err() || paced.is_err() || bulk.is_err() || latencies.is_err() || frames.is_err() {
        let pressure_snapshot = connection.snapshot().map_err(|error| error.to_string())?;
        connection.close();
        TimeoutFuture::new(20).await;
        let closed_snapshot = connection.snapshot().map_err(|error| error.to_string())?;
        let report = format!(
            "FAIL · unchanged simultaneous pressure workload\nSlow: {:?}; paced: {:?}; bulk: {:?}; unary: {:?}; animation: {:?}\nBuild: {}\nIncoming controls admitted: {}; outgoing controls observed: {}\nIncoming rolling frame type/stream/relative-ns observations: {:?}\nRejected incoming control: {:?}\nPressure snapshot: {:?}\nPost-close snapshot: {:?}\nThe five original malicious probes and new 101-control probe did not execute after pressure failure. G02 and control-rate repair remain unqualified.\n",
            slow.as_ref().err(),
            paced.as_ref().err(),
            bulk.as_ref().err(),
            latencies.as_ref().err(),
            frames.as_ref().err(),
            crate::BUILD_ID,
            pressure_snapshot.received_control_frames,
            pressure_snapshot.sent_control_frames,
            pressure_snapshot.incoming_control_window,
            pressure_snapshot.rejected_control,
            pressure_snapshot,
            closed_snapshot,
        );
        display(document, &report);
        save_report("qualification", &report).await?;
        return Err(
            "unchanged simultaneous pressure workload failed; retained frame and cleanup evidence"
                .to_owned(),
        );
    }
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
        closed.closed
            && closed.callback_bytes.current == 0
            && closed.callback_items.current == 0
            && closed.browser_callback_vec_capacity.current == 0,
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
    for kind in [
        "websocket",
        "frame",
        "header",
        "continuation",
        "flood",
        "control-rate",
    ] {
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
async fn run_callback_capacity(document: &Document) -> Result<(), String> {
    display_callback_capacity(
        document,
        "RUNNING · eight slow replies and one same-connection unary",
    );
    let memory_before = memory_bytes()?;
    let (connection, channel) = BrowserConnection::connect(&tunnel_url("/tunnel")?)
        .await
        .map_err(|error| error.to_string())?;
    let initial = connection.snapshot().map_err(|error| error.to_string())?;
    let mut slow_client =
        FixtureClient::new(channel.clone()).max_decoding_message_size(RPC_MESSAGE_BYTES);
    let mut unary_client = FixtureClient::new(channel).max_decoding_message_size(RPC_MESSAGE_BYTES);
    let slow = async {
        let mut stream = slow_client
            .server_stream(request(Sample {
                sequence: 8,
                payload: b"pressure:slow".to_vec(),
                ..sample(0)
            })?)
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        let mut payload_bytes = 0;
        for sequence in 0..8 {
            let message = stream
                .message()
                .await
                .map_err(|error| error.to_string())?
                .ok_or("slow stream ended before eight replies")?;
            require(
                message.sequence == sequence
                    && message.payload.len() == 49_152
                    && message.payload.iter().all(|byte| *byte == 0x5a),
                "slow stream response changed",
            )?;
            payload_bytes += message.payload.len();
            let progress = connection.snapshot().map_err(|error| error.to_string())?;
            display_callback_capacity(
                document,
                &format!(
                    "RUNNING · slow replies {}/8; payload {} bytes; retained callback Vec capacity {} bytes; peak {} bytes",
                    sequence + 1,
                    payload_bytes,
                    progress.browser_callback_vec_capacity.current,
                    progress.browser_callback_vec_capacity.peak,
                ),
            );
            TimeoutFuture::new(20).await;
        }
        require(
            stream
                .message()
                .await
                .map_err(|error| error.to_string())?
                .is_none(),
            "slow stream returned an extra reply",
        )?;
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
            "slow stream terminal trailers missing",
        )?;
        Ok::<usize, String>(payload_bytes)
    };
    let unary = async {
        TimeoutFuture::new(25).await;
        let response = unary_client
            .unary(request(sample(77))?)
            .await
            .map_err(|error| error.to_string())?;
        require(
            response
                .metadata()
                .get("fixture-server")
                .is_some_and(|value| value == "native-tonic"),
            "same-connection unary metadata missing",
        )?;
        let output = response.into_inner();
        require(
            output.sequence == 77 && output.payload == b"synthetic protobuf",
            "same-connection unary response changed",
        )
    };
    let (slow_result, unary_result) = futures::join!(slow, unary);
    let payload_bytes = slow_result?;
    unary_result?;
    let peak = connection.snapshot().map_err(|error| error.to_string())?;
    connection.close();
    TimeoutFuture::new(20).await;
    let closed = connection.snapshot().map_err(|error| error.to_string())?;
    require(
        closed.closed
            && closed.callback_bytes.current == 0
            && closed.callback_items.current == 0
            && closed.browser_callback_vec_capacity.current == 0,
        "slow observation retained callback owners after close",
    )?;
    let (oversized, oversized_closed, status, elapsed_ms) = probe("websocket").await?;
    require(
        oversized.websocket_receive.peak == 262_145
            && oversized.browser_callback_vec_capacity.total == 0
            && oversized_closed.browser_callback_vec_capacity.current == 0,
        "oversized ArrayBuffer copied into a Rust callback Vec",
    )?;
    let user_agent = web_sys::window()
        .ok_or("window unavailable")?
        .navigator()
        .user_agent()
        .map_err(|_| "user agent unavailable")?;
    let report = format!(
        "PASS · eight exact 49,152-byte slow responses ({payload_bytes} bytes), generated terminal trailers and one same-connection unary response\nInitial snapshot: {initial:?}\nPeak/finished snapshot: {peak:?}\nClosed snapshot: {closed:?}\nPASS · oversized 262,145-byte engine ArrayBuffer rejected before Uint8Array::to_vec; copied callback Vec capacity total={} bytes, callback queue total={} bytes; RPC status {status:?} in {elapsed_ms:.3}ms\nOversized snapshot: {oversized:?}\nOversized closed snapshot: {oversized_closed:?}\nWASM linear memory buffer length before={} after={} bytes (whole instance, not live heap)\nBrowser: {user_agent}\nBuild: {}\nScope: callback Vec capacity is original Rust Vec::capacity at the copy boundary, retained through Bytes aliases. It excludes engine/pre-callback storage, allocator and Bytes metadata, h2 buffers and other allocations. Native capacity is unobserved/not applicable. D03/full G02, total 8MiB per-connection allocation, reserved control queue/fairness, physical phones and audio remain pending.\n",
        oversized.browser_callback_vec_capacity.total,
        oversized.callback_bytes.total,
        memory_before,
        memory_bytes()?,
        crate::BUILD_ID,
    );
    display_callback_capacity(document, &report);
    save_report("callback-capacity", &report).await
}

async fn consume_credit_stream(
    index: usize,
    mut stream: tonic::Streaming<Sample>,
) -> Result<usize, String> {
    let mut bytes = 0;
    for sequence in 0..8 {
        let message = stream
            .message()
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("credit stream {index} ended before eight replies"))?;
        require(
            message.sequence == sequence
                && message.payload.len() == 49_152
                && message.payload.iter().all(|byte| *byte == 0x5a),
            "credit stream sequence or payload changed",
        )?;
        bytes += message.payload.len();
    }
    require(
        stream
            .message()
            .await
            .map_err(|error| error.to_string())?
            .is_none(),
        "credit stream returned an extra reply",
    )?;
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
        "credit stream terminal trailers missing",
    )?;
    Ok(bytes)
}

async fn observe_connection_credit(
    document: &Document,
    connection: &BrowserConnection,
    channel: df_rpc_bridge::BrowserChannel,
    report: &mut String,
) -> Result<String, String> {
    let started = Instant::now();
    let baseline = credit_point(connection, started)?;
    report.push_str(&credit_line("Baseline", &baseline));
    let opened = join_all((0..4).map(|_| {
        let mut client =
            FixtureClient::new(channel.clone()).max_decoding_message_size(RPC_MESSAGE_BYTES);
        async move {
            client
                .server_stream(request(Sample {
                    sequence: 8,
                    payload: b"pressure:slow".to_vec(),
                    ..sample(0)
                })?)
                .await
                .map(|response| response.into_inner())
                .map_err(|error| error.to_string())
        }
    }))
    .await;
    let mut streams = Vec::with_capacity(4);
    for opened_stream in opened {
        streams.push(opened_stream?);
    }
    // Retain all four response bodies without polling message() until sampling ends.
    let mut held = Vec::with_capacity(4);
    held.push(credit_point(connection, started)?);
    for _ in 0..3 {
        TimeoutFuture::new(250).await;
        held.push(credit_point(connection, started)?);
    }
    for (index, point) in held.iter().enumerate() {
        report.push_str(&credit_line(&format!("Held sample {index}"), point));
        require(
            point.snapshot.envelope_within_limit()
                && point.snapshot.callback_bytes.current <= 1024 * 1024
                && point.snapshot.callback_items.current <= 256
                && !point.snapshot.rejected
                && !point.snapshot.driver_failed,
            "held connection exceeded existing resource/protocol bounds",
        )?;
    }
    display_connection_credit(document, report);
    let last = held.last().ok_or("held credit sample missing")?;
    let previous = &held[held.len() - 2];
    let before_previous = &held[held.len() - 3];
    let stalled_data = last.snapshot.receive_credit.data_bytes
        == previous.snapshot.receive_credit.data_bytes
        && previous.snapshot.receive_credit.data_bytes
            == before_previous.snapshot.receive_credit.data_bytes
        && last.snapshot.receive_credit.data_bytes > baseline.snapshot.receive_credit.data_bytes;
    let stalled_updates = last.snapshot.receive_credit.update_bytes
        == previous.snapshot.receive_credit.update_bytes
        && previous.snapshot.receive_credit.update_bytes
            == before_previous.snapshot.receive_credit.update_bytes;
    let observed_exhaustion = stalled_data
        && stalled_updates
        && last.snapshot.receive_credit.available == 0
        && previous.snapshot.receive_credit.available == 0;
    let credit_result = if observed_exhaustion {
        "PASS · directly observed zero connection DATA credit and no DATA/WINDOW_UPDATE progress across two 250ms held intervals"
    } else {
        "INCONCLUSIVE · held snapshots do not establish connection-credit exhaustion; remaining credit, stream-level credit, scheduling, and pre-callback allocation cannot be attributed from this connection snapshot"
    };
    report.push_str(credit_result);
    report.push('\n');

    let mut unary_client = FixtureClient::new(channel).max_decoding_message_size(RPC_MESSAGE_BYTES);
    let unary_started = Instant::now();
    let unary = async move {
        let response = unary_client
            .unary(request(sample(77))?)
            .await
            .map_err(|error| error.to_string())?;
        require(
            response
                .metadata()
                .get("fixture-server")
                .is_some_and(|value| value == "native-tonic"),
            "same-connection unary metadata missing",
        )?;
        let output = response.into_inner();
        require(
            output.sequence == 77 && output.payload == b"synthetic protobuf",
            "same-connection unary response changed",
        )?;
        Ok::<f64, String>(unary_started.elapsed().as_secs_f64() * 1000.0)
    };
    let (early_unary, pending_unary) =
        match select(unary.boxed_local(), TimeoutFuture::new(200).boxed_local()).await {
            Either::Left((result, _)) => (Some(result), None),
            Either::Right(((), pending)) => (None, Some(pending)),
        };
    report.push_str(if early_unary.is_some() {
        "Unary completed while stream bodies remained unpolled.\n"
    } else {
        "Unary still pending after 200ms with stream bodies unpolled; resuming streams.\n"
    });
    display_connection_credit(document, report);
    let resumed = join_all(
        streams
            .into_iter()
            .enumerate()
            .map(|(index, stream)| consume_credit_stream(index, stream)),
    );
    let unary_done = async move {
        match pending_unary {
            Some(pending) => pending.await,
            None => early_unary.ok_or("unary result missing".to_owned())?,
        }
    };
    let (stream_results, unary_result) = futures::join!(resumed, unary_done);
    let mut payload_bytes = 0;
    for stream_result in stream_results {
        payload_bytes += stream_result?;
    }
    require(
        payload_bytes == 32 * 49_152,
        "credit stream byte count changed",
    )?;
    let unary_ms = unary_result?;
    let resumed_point = credit_point(connection, started)?;
    report.push_str(&credit_line("Resumed", &resumed_point));
    require(
        resumed_point.snapshot.envelope_within_limit()
            && !resumed_point.snapshot.rejected
            && !resumed_point.snapshot.driver_failed,
        "resumed connection exceeded existing resource/protocol bounds",
    )?;
    require(
        resumed_point.snapshot.receive_credit.data_bytes > last.snapshot.receive_credit.data_bytes
            && resumed_point.snapshot.receive_credit.update_bytes
                > last.snapshot.receive_credit.update_bytes,
        "resumption lacked directly observed connection DATA and WINDOW_UPDATE progress",
    )?;
    report.push_str(&format!(
        "PASS · four streams, 32 exact replies, {payload_bytes} payload bytes, all terminal trailers; same-connection unary metadata/response in {unary_ms:.1}ms; directly observed resumed DATA/WINDOW_UPDATE progress.\n"
    ));
    Ok(credit_result.to_owned())
}

async fn save_credit_report(generation: u64, phase: u8, report: &str) -> Result<(), String> {
    save_report(
        &format!("connection-credit?generation={generation}&phase={phase}"),
        report,
    )
    .await
}

async fn require_stale_credit_report_rejected(
    generation: u64,
    expected_report: &str,
) -> Result<(), String> {
    let window = web_sys::window().ok_or("window unavailable")?;
    let older_generation = generation
        .checked_sub(1)
        .filter(|value| *value > 0)
        .ok_or("report generation has no older valid generation")?;
    for (stale_generation, stale_phase) in [(generation, 0), (older_generation, 2)] {
        let options = web_sys::RequestInit::new();
        options.set_method("POST");
        options.set_body(&JsValue::from_str(
            "STALE · must not replace current report",
        ));
        let response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str_and_init(
            &format!(
                "/fixture-report/connection-credit?generation={stale_generation}&phase={stale_phase}"
            ),
            &options,
        ))
        .await
        .map_err(|_| "stale report request failed")?
        .dyn_into::<web_sys::Response>()
        .map_err(|_| "stale report response unavailable")?;
        require(
            response.status() == 409,
            "stale report was not refused with HTTP 409",
        )?;
    }
    let response = wasm_bindgen_futures::JsFuture::from(
        window.fetch_with_str("/fixture-report/connection-credit"),
    )
    .await
    .map_err(|_| "stored report request failed")?
    .dyn_into::<web_sys::Response>()
    .map_err(|_| "stored report response unavailable")?;
    require(response.status() == 200, "stored report GET failed")?;
    let stored = wasm_bindgen_futures::JsFuture::from(
        response
            .text()
            .map_err(|_| "stored report body unavailable")?,
    )
    .await
    .map_err(|_| "stored report body read failed")?
    .as_string()
    .ok_or("stored report was not text")?;
    require(
        stored == expected_report,
        "stale publication changed the stored report",
    )
}

async fn observe_browser_connection_drop() -> Result<String, String> {
    let (connection, channel) = BrowserConnection::connect(&tunnel_url("/tunnel")?)
        .await
        .map_err(|error| error.to_string())?;
    let mut monitor =
        FixtureClient::new(channel.clone()).max_decoding_message_size(RPC_MESSAGE_BYTES);
    let baseline = stats(&mut monitor).await?;
    let mut waiting_client =
        FixtureClient::new(channel).max_decoding_message_size(RPC_MESSAGE_BYTES);
    let waiting = waiting_client
        .unary(request(Sample {
            behavior: df_protocol::transport_fixture::sample::Behavior::Wait as i32,
            ..sample(0)
        })?)
        .boxed_local();
    let pending = match select(waiting, TimeoutFuture::new(40).boxed_local()).await {
        Either::Left((Ok(_), _)) => {
            return Err("waiting generated unary succeeded before owner drop".to_owned());
        }
        Either::Left((Err(error), _)) => {
            return Err(format!(
                "waiting generated unary failed before owner drop: {error}"
            ));
        }
        Either::Right(((), pending)) => pending,
    };
    let mut active_observed = false;
    for _ in 0..20 {
        if stats(&mut monitor).await?.active_calls > baseline.active_calls {
            active_observed = true;
            break;
        }
        TimeoutFuture::new(20).await;
    }
    require(
        active_observed,
        "waiting generated unary never became active on server",
    )?;
    let before_drop = connection.snapshot().map_err(|error| error.to_string())?;
    drop(connection);
    let terminal = match select(pending, TimeoutFuture::new(1000).boxed_local()).await {
        Either::Left((result, _)) => result
            .err()
            .ok_or("waiting generated unary succeeded after owner drop")?,
        Either::Right(((), pending)) => {
            drop(pending);
            return Err("waiting generated unary remained pending after owner drop".to_owned());
        }
    };
    let (replacement, replacement_channel) = BrowserConnection::connect(&tunnel_url("/tunnel")?)
        .await
        .map_err(|error| error.to_string())?;
    let mut replacement_client =
        FixtureClient::new(replacement_channel).max_decoding_message_size(RPC_MESSAGE_BYTES);
    let mut released = None;
    for _ in 0..30 {
        let point = stats(&mut replacement_client).await?;
        if point.sequence > baseline.sequence && point.active_calls <= baseline.active_calls {
            released = Some(point);
            break;
        }
        TimeoutFuture::new(20).await;
    }
    let released = released.ok_or("dropped connection did not release server call")?;
    let reply = replacement_client
        .unary(request(sample(9001))?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    require(
        reply.sequence == 9001,
        "replacement connection unary did not recover",
    )?;
    replacement.close();
    Ok(format!(
        "PASS · real generated Wait unary active on server, owner dropped implicitly, pending future returned typed {terminal:?} before 1000ms; cancellation count {}→{}, active calls {}→{}, replacement connection unary recovered. Pre-drop snapshot: {before_drop:?}\n",
        baseline.sequence, released.sequence, baseline.active_calls, released.active_calls,
    ))
}

async fn run_connection_credit(document: &Document, generation: u64) -> Result<(), String> {
    let running_report = "RUNNING · connection-credit observation admitted";
    save_credit_report(generation, 0, running_report).await?;
    display_connection_credit(document, "RUNNING · opening four pressure:slow streams");
    let (connection, channel) = BrowserConnection::connect(&tunnel_url("/tunnel")?)
        .await
        .map_err(|error| error.to_string())?;
    let closed_channel = channel.clone();
    let mut observations = String::new();
    let outcome =
        observe_connection_credit(document, &connection, channel, &mut observations).await;
    connection.close();
    TimeoutFuture::new(20).await;
    let closed = connection.snapshot().map_err(|error| error.to_string());
    let cleaned = closed.as_ref().is_ok_and(|snapshot| {
        snapshot.closed
            && snapshot.callback_bytes.current == 0
            && snapshot.callback_items.current == 0
            && snapshot.browser_callback_vec_capacity.current == 0
    });
    require(cleaned, "explicit close did not release browser callbacks")?;
    let mut closed_client =
        FixtureClient::new(closed_channel).max_decoding_message_size(RPC_MESSAGE_BYTES);
    let closed_status = match select(
        closed_client.unary(request(sample(9002))?).boxed_local(),
        TimeoutFuture::new(1000).boxed_local(),
    )
    .await
    {
        Either::Left((result, _)) => result
            .err()
            .ok_or("generated unary succeeded after explicit close")?,
        Either::Right(((), pending)) => {
            drop(pending);
            return Err("generated unary remained pending after explicit close".to_owned());
        }
    };
    let drop_observation = observe_browser_connection_drop().await?;
    require_stale_credit_report_rejected(generation, running_report).await?;
    let status = match outcome {
        Ok(credit_result) => credit_result,
        Err(error) => format!("FAIL · {error}"),
    };
    let browser = web_sys::window()
        .ok_or("window unavailable")?
        .navigator()
        .user_agent()
        .map_err(|_| "user agent unavailable")?;
    let report = format!(
        "{status}\n{observations}Callback cleanup observed: {cleaned}\nPost-close snapshot: {closed:?}\nPASS · post-explicit-close generated unary returned typed {closed_status:?} before 1000ms.\n{drop_observation}PASS · stale phase/generation POST returned HTTP 409 and GET preserved the admitted report.\nBrowser: {browser}\nBuild: {}\nReport generation: {generation}\nScope: connection wire DATA credit is not stream credit or memory allocation. WASM buffer length is the whole allocated linear memory, not live heap or browser-engine storage. Hostile over-credit bursts, full G02/D03, total 8 MiB, pre-callback allocation, 128-frame/256 KiB control queue and fairness, physical phones, audio, gameplay and production remain pending.\n",
        crate::BUILD_ID,
    );
    save_credit_report(generation, 1, &report).await?;
    require_stale_credit_report_rejected(generation, &report).await?;
    display_connection_credit(document, &report);
    Ok(())
}

pub(super) fn install(document: &Document, active: Rc<Cell<bool>>) -> Result<(), JsValue> {
    let button = document
        .get_element_by_id("qualify")
        .ok_or_else(|| JsValue::from_str("qualification button unavailable"))?
        .dyn_into::<HtmlButtonElement>()?;
    button.set_disabled(false);
    button.set_text_content(Some("Run desktop pressure qualification"));
    let capacity_document = document.clone();
    let capacity_active = active.clone();
    let credit_document = document.clone();
    let credit_active = active.clone();
    let credit_generation = Rc::new(Cell::new(0u64));
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
                let retained_failure = document
                    .get_element_by_id("qualification")
                    .and_then(|element| element.text_content())
                    .is_some_and(|text| {
                        text.starts_with("FAIL · unchanged simultaneous pressure workload")
                    });
                if !retained_failure {
                    display(
                        &document,
                        &format!("FAIL · {error}\nG02 INCONCLUSIVE · incomplete qualification"),
                    );
                }
            }
            button.set_disabled(false);
            active.set(false);
        });
    }) as Box<dyn FnMut()>);
    button.set_onclick(Some(callback.as_ref().unchecked_ref()));
    // Owned by this document/WASM instance for its page lifetime.
    callback.forget();
    let capacity_button = capacity_document
        .get_element_by_id("callback-capacity")
        .ok_or_else(|| JsValue::from_str("callback capacity button unavailable"))?
        .dyn_into::<HtmlButtonElement>()?;
    capacity_button.set_disabled(false);
    capacity_button.set_text_content(Some("Run receive allocation observation"));
    let callback_button = capacity_button.clone();
    let capacity_callback = Closure::wrap(Box::new(move || {
        if capacity_active.replace(true) {
            return;
        }
        callback_button.set_disabled(true);
        let document = capacity_document.clone();
        let button = callback_button.clone();
        let active = capacity_active.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let outcome = select(
                run_callback_capacity(&document).boxed_local(),
                TimeoutFuture::new(30000).boxed_local(),
            )
            .await;
            let result = match outcome {
                Either::Left((result, _)) => result,
                Either::Right(((), owned)) => {
                    drop(owned);
                    Err(
                        "callback capacity observation exceeded owned 30-second deadline"
                            .to_owned(),
                    )
                }
            };
            if let Err(error) = result {
                let report = format!(
                    "FAIL · {error}\nBuild: {}\nCallback capacity and G02 qualification remain unverified.\n",
                    crate::BUILD_ID
                );
                display_callback_capacity(&document, &report);
                let _ = save_report("callback-capacity", &report).await;
            }
            button.set_disabled(false);
            active.set(false);
        });
    }) as Box<dyn FnMut()>);
    capacity_button.set_onclick(Some(capacity_callback.as_ref().unchecked_ref()));
    // Owned by this document/WASM instance for its page lifetime.
    capacity_callback.forget();
    let credit_button = credit_document
        .get_element_by_id("connection-credit")
        .ok_or_else(|| JsValue::from_str("connection credit button unavailable"))?
        .dyn_into::<HtmlButtonElement>()?;
    credit_button.set_disabled(false);
    credit_button.set_text_content(Some("Run connection-credit stall observation"));
    let callback_button = credit_button.clone();
    let credit_callback = Closure::wrap(Box::new(move || {
        if credit_active.replace(true) {
            return;
        }
        let generation =
            (js_sys::Date::now() as u64).max(credit_generation.get().saturating_add(1));
        if generation <= credit_generation.get() {
            display_connection_credit(&credit_document, "FAIL · report generation exhausted");
            credit_active.set(false);
            return;
        }
        credit_generation.set(generation);
        callback_button.set_disabled(true);
        let document = credit_document.clone();
        let button = callback_button.clone();
        let active = credit_active.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let outcome = select(
                run_connection_credit(&document, generation).boxed_local(),
                TimeoutFuture::new(29000).boxed_local(),
            )
            .await;
            let result = match outcome {
                Either::Left((result, _)) => result,
                Either::Right(((), owned)) => {
                    drop(owned);
                    Err(
                        "connection-credit observation reached its owned 30-second deadline"
                            .to_owned(),
                    )
                }
            };
            if let Err(error) = result {
                let report = format!(
                    "FAIL · {error}\nBuild: {}\nConnection-credit attribution remains unverified. Hostile over-credit bursts, full G02/D03, total 8 MiB, pre-callback allocation, 128-frame/256 KiB control queue and fairness, physical phones, audio, gameplay and production remain pending. The run owner and its streams were dropped.\n",
                    crate::BUILD_ID,
                );
                display_connection_credit(&document, &report);
                let _ = select(
                    save_credit_report(generation, 2, &report).boxed_local(),
                    TimeoutFuture::new(1000).boxed_local(),
                )
                .await;
            }
            button.set_disabled(false);
            active.set(false);
        });
    }) as Box<dyn FnMut()>);
    credit_button.set_onclick(Some(credit_callback.as_ref().unchecked_ref()));
    // Owned by this document/WASM instance for its page lifetime.
    credit_callback.forget();
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
