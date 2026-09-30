use df_observe::OperationContext;
use df_protocol::transport_fixture::{
    Sample, sample::Behavior, transport_fixture_client::TransportFixtureClient,
};
use df_rpc_bridge::{BrowserChannel, BrowserConnection, RPC_MESSAGE_BYTES};
use futures::{
    FutureExt, SinkExt,
    channel::mpsc,
    future::{Either, select},
};
use gloo_timers::future::TimeoutFuture;
use std::{cell::Cell, rc::Rc};
use tonic::{Code, Request, Status};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, HtmlButtonElement};

type FixtureClient = TransportFixtureClient<BrowserChannel>;
fn sample(sequence: u32) -> Sample {
    Sample {
        sequence,
        payload: b"synthetic protobuf".to_vec(),
        behavior: Behavior::Echo as i32,
        ..Sample::default()
    }
}
fn request<T>(body: T) -> Result<Request<T>, String> {
    let mut request = Request::new(body);
    request.metadata_mut().insert(
        "traceparent",
        "00-11111111111111111111111111111111-2222222222222222-01"
            .parse()
            .map_err(|error: tonic::metadata::errors::InvalidMetadataValue| error.to_string())?,
    );
    Ok(request)
}
fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned())
    }
}
fn show(document: &Document, text: &str) {
    if let Some(element) = document.get_element_by_id("results") {
        element.set_text_content(Some(text));
    }
}
fn append(document: &Document, lines: &mut String, line: &str) {
    lines.push_str(line);
    lines.push('\n');
    show(document, lines);
}
async fn finish(mut stream: tonic::Streaming<Sample>, expected: &[u32]) -> Result<(), String> {
    let mut sequences = vec![];
    while let Some(sample) = stream.message().await.map_err(|error| error.to_string())? {
        sequences.push(sample.sequence);
    }
    require(sequences == expected, "stream order or count mismatch")?;
    let trailers = stream
        .trailers()
        .await
        .map_err(|error| error.to_string())?
        .ok_or("terminal trailers missing")?;
    require(
        trailers
            .get("fixture-terminal")
            .is_some_and(|value| value == "observed"),
        "custom terminal trailer missing",
    )
}
async fn run(document: Document) -> Result<(), String> {
    let mut lines = String::new();
    append(
        &document,
        &mut lines,
        "CONNECTING · binary WebSocket / native HTTP/2",
    );
    let (connection, channel) = BrowserConnection::connect("ws://127.0.0.1:43180/tunnel")
        .await
        .map_err(|error| error.to_string())?;
    let mut client = FixtureClient::new(channel)
        .max_decoding_message_size(RPC_MESSAGE_BYTES)
        .max_encoding_message_size(RPC_MESSAGE_BYTES * 2);
    let reply = client
        .unary(request(sample(7))?)
        .await
        .map_err(|error| error.to_string())?;
    require(
        reply
            .metadata()
            .get("fixture-server")
            .is_some_and(|value| value == "native-tonic"),
        "native response metadata missing",
    )?;
    require(reply.into_inner().sequence == 7, "unary mismatch")?;
    append(&document, &mut lines, "PASS · Unary + response metadata");
    let stream = client
        .server_stream(request(sample(0))?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    finish(stream, &[0, 1, 2]).await?;
    append(
        &document,
        &mut lines,
        "PASS · Server streaming + ordered DATA + OK trailers",
    );
    let response = client
        .client_stream(request(futures::stream::iter([
            sample(0),
            sample(1),
            sample(2),
        ]))?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    require(
        response.sequence == 3 && response.payload == b"half-close observed",
        "client half-close not observed",
    )?;
    append(
        &document,
        &mut lines,
        "PASS · Client streaming + request half-close",
    );
    let (mut sender, receiver) = mpsc::channel(1);
    sender
        .send(sample(10))
        .await
        .map_err(|error| error.to_string())?;
    let mut bidi = client
        .bidi(request(receiver)?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    require(
        bidi.message()
            .await
            .map_err(|error| error.to_string())?
            .is_some_and(|sample| sample.sequence == 10),
        "bidi did not respond before half-close",
    )?;
    sender
        .send(sample(11))
        .await
        .map_err(|error| error.to_string())?;
    drop(sender);
    finish(bidi, &[11]).await?;
    append(
        &document,
        &mut lines,
        "PASS · Bidirectional streaming + response before half-close",
    );
    let reject = Sample {
        behavior: Behavior::Reject as i32,
        ..sample(0)
    };
    let status = client
        .unary(request(reject)?)
        .await
        .err()
        .ok_or("rejection was accepted")?;
    require(
        status.code() == Code::InvalidArgument
            && status.metadata().get("fixture-terminal").is_some(),
        "error status/trailer mismatch",
    )?;
    append(
        &document,
        &mut lines,
        "PASS · INVALID_ARGUMENT + error trailers",
    );
    rejection_modes(&mut client).await?;
    append(
        &document,
        &mut lines,
        "PASS · Error statuses + trailers in all four RPC modes",
    );
    let waiting = Sample {
        behavior: Behavior::Wait as i32,
        ..sample(0)
    };
    let baseline_stats = client
        .unary(request(Sample {
            payload: b"stats".to_vec(),
            ..sample(0)
        })?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    let baseline = baseline_stats.sequence;
    let mut stream = client
        .server_stream(request(waiting.clone())?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    require(
        stream
            .message()
            .await
            .map_err(|error| error.to_string())?
            .is_some(),
        "cancellable stream never started",
    )?;
    drop(stream);
    wait_for_cancellation(&mut client, baseline + 1).await?;
    append(
        &document,
        &mut lines,
        "PASS · Cancellation resets stream + server owner released",
    );
    let mut deadline_request = request(waiting)?;
    deadline_request.set_timeout(std::time::Duration::from_millis(40));
    let (deadline_status, deadline_source) = match select(
        client.unary(deadline_request).boxed_local(),
        TimeoutFuture::new(40).boxed_local(),
    )
    .await
    {
        Either::Left((result, _)) => (
            result.err().ok_or("waiting RPC unexpectedly completed")?,
            "native terminal status",
        ),
        Either::Right(((), rpc)) => {
            drop(rpc);
            (
                Status::deadline_exceeded("browser monotonic deadline elapsed"),
                "local browser deadline",
            )
        }
    };
    require(
        matches!(
            deadline_status.code(),
            Code::DeadlineExceeded | Code::Cancelled
        ),
        "deadline not enforced",
    )?;
    wait_for_cancellation(&mut client, baseline + 2).await?;
    let deadline_stats = client
        .unary(request(Sample {
            payload: b"stats".to_vec(),
            ..sample(0)
        })?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    require(
        deadline_stats.deadline_calls > baseline_stats.deadline_calls,
        "native grpc-timeout metadata was not observed",
    )?;
    append(
        &document,
        &mut lines,
        &format!(
            "PASS · Deadline ({deadline_source}: {:?}) + native grpc-timeout + cleanup",
            deadline_status.code()
        ),
    );
    let oversized = Sample {
        payload: vec![0; RPC_MESSAGE_BYTES + 1],
        ..sample(0)
    };
    let status = client
        .unary(request(oversized)?)
        .await
        .err()
        .ok_or("oversized message was accepted")?;
    require(
        status.code() == Code::OutOfRange || status.code() == Code::ResourceExhausted,
        "bounded rejection status mismatch",
    )?;
    append(
        &document,
        &mut lines,
        "PASS · Oversized protobuf rejected at native decoding boundary",
    );
    let mut other_client = client.clone();
    let stream = client
        .server_stream(request(sample(0))?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    let unary = other_client.unary(request(sample(99))?);
    let (stream_result, unary_result) = futures::join!(finish(stream, &[0, 1, 2]), unary);
    stream_result?;
    require(
        unary_result
            .map_err(|error| error.to_string())?
            .into_inner()
            .sequence
            == 99,
        "simultaneous unary failed",
    )?;
    append(
        &document,
        &mut lines,
        "PASS · Simultaneous streams share the same connection",
    );
    connection.close();
    let context = OperationContext {
        trace_parent: "00-11111111111111111111111111111111-2222222222222222-01".to_owned(),
        build: "S00-experimental".to_owned(),
    };
    df_observe::record(&context, "browser.fixture", "complete", 0);
    append(
        &document,
        &mut lines,
        "COMPLETE · 10 transport checks passed\nG02 INCONCLUSIVE · physical devices / adversarial receive memory pending",
    );
    Ok(())
}
async fn wait_for_cancellation(client: &mut FixtureClient, expected: u32) -> Result<(), String> {
    for _ in 0..100 {
        let stats = client
            .unary(request(Sample {
                payload: b"stats".to_vec(),
                ..sample(0)
            })?)
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        if stats.sequence >= expected && stats.active_calls == 1 {
            return Ok(());
        }
        TimeoutFuture::new(5).await;
    }
    Err("server cancellation cleanup not observed within 500ms".to_owned())
}
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("document unavailable"))?;
    let button = document
        .get_element_by_id("run")
        .ok_or_else(|| JsValue::from_str("fixture button unavailable"))?
        .dyn_into::<HtmlButtonElement>()?;
    button.set_disabled(false);
    button.set_text_content(Some("Run transport checks"));
    show(
        &document,
        "READY · Rust/WASM initialized. Run all four RPC modes and failure checks.",
    );
    if let Some(build) = document.get_element_by_id("build") {
        build.set_text_content(Some(&format!("Candidate: START-S00-001-a1 · Rust 1.98.1 · tonic 0.14.6 · h2 0.4.19 + browser clock · build {}", crate::BUILD_ID)));
    }
    let active = Rc::new(Cell::new(false));
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
            let timed = select(
                run(document.clone()).boxed_local(),
                TimeoutFuture::new(15000).boxed_local(),
            )
            .await;
            let result = match timed {
                Either::Left((result, _)) => result,
                Either::Right(((), future)) => {
                    drop(future);
                    Err("fixture exceeded its 15-second run deadline".to_owned())
                }
            };
            if let Err(error) = result {
                show(
                    &document,
                    &format!("FAIL · {error}\nG02 INCONCLUSIVE · fixture did not complete"),
                );
            }
            active.set(false);
            button.set_disabled(false);
            button.set_text_content(Some("Run transport checks again"));
        });
    }) as Box<dyn FnMut()>);
    button.set_onclick(Some(callback.as_ref().unchecked_ref()));
    // Page-lifetime owner: browser drops the document and WASM instance together on navigation.
    callback.forget();
    Ok(())
}

async fn rejection_modes(client: &mut FixtureClient) -> Result<(), String> {
    let rejected = Sample {
        behavior: Behavior::Reject as i32,
        ..sample(0)
    };
    let server_error = client
        .server_stream(request(rejected.clone())?)
        .await
        .err()
        .ok_or("server stream rejection accepted")?;
    let client_error = client
        .client_stream(request(futures::stream::iter([rejected.clone()]))?)
        .await
        .err()
        .ok_or("client stream rejection accepted")?;
    let mut bidi = client
        .bidi(request(futures::stream::iter([rejected]))?)
        .await
        .map_err(|error| error.to_string())?
        .into_inner();
    let bidi_error = bidi
        .message()
        .await
        .err()
        .ok_or("bidi rejection accepted")?;
    for error in [server_error, client_error, bidi_error] {
        require(
            error.code() == Code::InvalidArgument
                && error
                    .metadata()
                    .get("fixture-terminal")
                    .is_some_and(|value| value == "observed"),
            "stream rejection status or trailers mismatch",
        )?;
    }
    Ok(())
}
