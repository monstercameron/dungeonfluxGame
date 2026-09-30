use axum::{
    Router,
    extract::{State, WebSocketUpgrade},
    http::HeaderMap,
    response::{Html, IntoResponse},
    routing::get,
};
use df_observe::{FixtureTelemetry, OperationContext};
use df_protocol::transport_fixture::{
    Sample,
    sample::Behavior,
    transport_fixture_server::{TransportFixture, TransportFixtureServer},
};
use df_rpc_bridge::{
    CONCURRENT_STREAMS, FRAME_BYTES, MESSAGE_BYTES, RPC_MESSAGE_BYTES, TunnelStream,
    accept_websocket,
};
use futures::Stream;
use std::{
    io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};
use tokio::sync::{Semaphore, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Code, Request, Response, Status, Streaming, metadata::MetadataMap};
use tower_http::services::ServeDir;

type ResponseStream = Pin<Box<dyn Stream<Item = Result<Sample, Status>> + Send>>;
#[derive(Default)]
struct Statistics {
    active: AtomicU32,
    cancelled: AtomicU32,
    completed: AtomicU32,
    deadlines: AtomicU32,
}
struct CallScope {
    statistics: Arc<Statistics>,
    span: df_observe::OperationSpan,
    status: Code,
    complete: bool,
}
impl CallScope {
    fn finish(&mut self) {
        self.complete = true;
        self.status = Code::Ok;
    }
}
impl CallScope {
    fn finish_with(&mut self, status: Code) {
        self.complete = true;
        self.status = status;
    }
}
impl Drop for CallScope {
    fn drop(&mut self) {
        self.statistics.active.fetch_sub(1, Ordering::Relaxed);
        let status = if self.complete {
            self.statistics.completed.fetch_add(1, Ordering::Relaxed);
            "complete"
        } else {
            self.statistics.cancelled.fetch_add(1, Ordering::Relaxed);
            "cancelled"
        };
        self.span.finish(&format!("{status}:{:?}", self.status), 0);
    }
}
#[derive(Clone)]
struct FixtureService {
    statistics: Arc<Statistics>,
}
impl FixtureService {
    fn scope<T>(&self, request: &Request<T>, method: &'static str) -> CallScope {
        self.statistics.active.fetch_add(1, Ordering::Relaxed);
        if request.metadata().get("grpc-timeout").is_some() {
            self.statistics.deadlines.fetch_add(1, Ordering::Relaxed);
        }
        let trace_parent = request
            .metadata()
            .get("traceparent")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned();
        CallScope {
            statistics: self.statistics.clone(),
            span: df_observe::begin(
                &OperationContext {
                    trace_parent,
                    build: crate::BUILD_ID.to_owned(),
                },
                method,
            ),
            status: Code::Cancelled,
            complete: false,
        }
    }
}
fn terminal(code: Code) -> Status {
    let mut metadata = MetadataMap::new();
    metadata.insert(
        "fixture-terminal",
        tonic::metadata::MetadataValue::from_static("observed"),
    );
    Status::with_metadata(code, "synthetic terminal", metadata)
}
fn check(sample: &Sample) -> Result<(), Status> {
    if sample.behavior == Behavior::Reject as i32 {
        return Err(terminal(Code::InvalidArgument));
    }
    Ok(())
}
#[tonic::async_trait]
impl TransportFixture for FixtureService {
    async fn unary(&self, request: Request<Sample>) -> Result<Response<Sample>, Status> {
        let mut scope = self.scope(&request, "fixture.unary");
        let sample = request.into_inner();
        if let Err(error) = check(&sample) {
            scope.finish_with(error.code());
            return Err(error);
        }
        if sample.behavior == Behavior::Wait as i32 {
            return futures::future::pending().await;
        }
        let sample = if sample.payload == b"stats" {
            Sample {
                sequence: self.statistics.cancelled.load(Ordering::Relaxed),
                payload: vec![],
                behavior: Behavior::Echo as i32,
                active_calls: self.statistics.active.load(Ordering::Relaxed),
                deadline_calls: self.statistics.deadlines.load(Ordering::Relaxed),
            }
        } else {
            sample
        };
        let mut response = Response::new(sample);
        response.metadata_mut().insert(
            "fixture-server",
            tonic::metadata::MetadataValue::from_static("native-tonic"),
        );
        scope.finish();
        Ok(response)
    }
    type ServerStreamStream = ResponseStream;
    async fn server_stream(
        &self,
        request: Request<Sample>,
    ) -> Result<Response<Self::ServerStreamStream>, Status> {
        let mut scope = self.scope(&request, "fixture.server_stream");
        let sample = request.into_inner();
        if let Err(status) = check(&sample) {
            scope.finish_with(status.code());
            return Err(status);
        }
        let stream = futures::stream::unfold(
            (0, sample, scope),
            |(sequence, sample, mut scope)| async move {
                if sample.behavior == Behavior::Wait as i32 && sequence > 0 {
                    return futures::future::pending().await;
                }
                if sequence < 3 {
                    return Some((
                        Ok(Sample {
                            sequence,
                            ..sample.clone()
                        }),
                        (sequence + 1, sample, scope),
                    ));
                }
                if sequence == 3 {
                    scope.finish();
                    return Some((Err(terminal(Code::Ok)), (4, sample, scope)));
                }
                None
            },
        );
        Ok(Response::new(Box::pin(stream)))
    }
    async fn client_stream(
        &self,
        request: Request<Streaming<Sample>>,
    ) -> Result<Response<Sample>, Status> {
        let mut scope = self.scope(&request, "fixture.client_stream");
        let mut incoming = request.into_inner();
        let mut count = 0;
        while let Some(sample) = incoming.message().await? {
            if let Err(status) = check(&sample) {
                scope.finish_with(status.code());
                return Err(status);
            }
            count += 1;
            if count > 16 {
                scope.finish_with(Code::ResourceExhausted);
                return Err(terminal(Code::ResourceExhausted));
            }
        }
        scope.finish();
        Ok(Response::new(Sample {
            sequence: count,
            payload: b"half-close observed".to_vec(),
            behavior: Behavior::Echo as i32,
            ..Sample::default()
        }))
    }
    type BidiStream = ResponseStream;
    async fn bidi(
        &self,
        request: Request<Streaming<Sample>>,
    ) -> Result<Response<Self::BidiStream>, Status> {
        let scope = self.scope(&request, "fixture.bidi");
        let stream = futures::stream::unfold(
            (request.into_inner(), scope, 0, false),
            |(mut incoming, mut scope, count, finished)| async move {
                if finished {
                    return None;
                }
                match incoming.message().await {
                    Ok(Some(sample)) if count < 16 => {
                        let result = check(&sample).map(|()| sample);
                        let finished = result.is_err();
                        if let Err(status) = &result {
                            scope.finish_with(status.code());
                        }
                        Some((result, (incoming, scope, count + 1, finished)))
                    }
                    Ok(Some(_)) => {
                        scope.finish_with(Code::ResourceExhausted);
                        Some((
                            Err(terminal(Code::ResourceExhausted)),
                            (incoming, scope, count, true),
                        ))
                    }
                    Ok(None) => {
                        scope.finish();
                        Some((Err(terminal(Code::Ok)), (incoming, scope, count, true)))
                    }
                    Err(status) => Some((Err(status), (incoming, scope, count, true))),
                }
            },
        );
        Ok(Response::new(Box::pin(stream)))
    }
}
#[derive(Clone)]
struct PreviewState {
    incoming: mpsc::Sender<Result<TunnelStream, io::Error>>,
    telemetry: Arc<FixtureTelemetry>,
    statistics: Arc<Statistics>,
    connections: Arc<Semaphore>,
}
async fn websocket(
    State(state): State<PreviewState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> axum::response::Response {
    if headers.get("origin").and_then(|value| value.to_str().ok()) != Some("http://127.0.0.1:43180")
    {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "synthetic loopback origin required",
        )
            .into_response();
    }
    let Ok(permit) = state.connections.clone().try_acquire_owned() else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "fixture connection limit reached",
        )
            .into_response();
    };
    upgrade
        .max_message_size(MESSAGE_BYTES)
        .max_frame_size(MESSAGE_BYTES)
        .on_upgrade(move |socket| async move {
            let context = OperationContext {
                trace_parent: String::new(),
                build: "S00-experimental".to_owned(),
            };
            let result = accept_websocket(socket, state.incoming).await;
            drop(permit);
            df_observe::record(
                &context,
                "bridge.connection",
                if result.is_ok() { "closed" } else { "failed" },
                0,
            );
        })
}
async fn health(State(state): State<PreviewState>) -> impl IntoResponse {
    match state.telemetry.count() {
        Ok(spans) => format!(
            "S00 experimental; active={}; completed={}; cancelled={}; otel_spans={spans}; durable_export=false; dropped_spans={}; deadline_headers={}",
            state.statistics.active.load(Ordering::Relaxed),
            state.statistics.completed.load(Ordering::Relaxed),
            state.statistics.cancelled.load(Ordering::Relaxed),
            state.telemetry.dropped(),
            state.statistics.deadlines.load(Ordering::Relaxed)
        ),
        Err(error) => format!("telemetry failure: {error}"),
    }
}
async fn fixture_telemetry(State(state): State<PreviewState>) -> impl IntoResponse {
    match state.telemetry.snapshot() {
        Ok(spans) => spans,
        Err(error) => format!("telemetry export failure: {error}"),
    }
}
const HTML: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>DungeonFlux S00 transport laboratory</title><style>body{margin:0;background:#10141c;color:#e6eaf3;font:17px system-ui,sans-serif}main{max-width:880px;margin:64px auto;padding:0 24px}h1{font-size:38px;line-height:1.1}p{color:#b7c3d4;line-height:1.5}button{background:#88dfb5;color:#10141c;border:0;border-radius:8px;font:600 17px system-ui;padding:14px 22px;cursor:pointer}button:disabled{opacity:.5}pre{white-space:pre-wrap;padding:20px;background:#1a2331;line-height:1.7;border-radius:10px}.tag{color:#88dfb5;letter-spacing:2px;font-size:13px}a{color:#88dfb5}</style></head><body><main><div class="tag">DUNGEONFLUX / EXPERIMENTAL S00</div><h1>Rust transport laboratory</h1><p>Generated protobuf calls travel as native HTTP/2 gRPC bytes over one binary WebSocket. The browser driver and this fixture interface are Rust compiled to single-threaded WebAssembly.</p><p>This starts the execution foundation. Gameplay, production authentication, durable telemetry and physical device qualification remain pending.</p><button id="run" disabled>Loading Rust/WASM…</button><pre id="results" role="status" aria-live="polite">Loading generated WebAssembly bindings…</pre><p><a href="/fixture-health">Native fixture diagnostics</a></p><p id="build"></p></main><script type="module">import init from '/pkg/df_tools.js';await init();</script></body></html>"#;

/// Start only a synthetic loopback preview. The caller owns process and output directory.
pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let web_root = std::env::args()
        .nth(1)
        .ok_or("generated web directory argument required")?;
    let telemetry = Arc::new(FixtureTelemetry::default());
    let statistics = Arc::new(Statistics::default());
    let (sender, receiver) = mpsc::channel(4);
    let service = TransportFixtureServer::new(FixtureService {
        statistics: statistics.clone(),
    })
    .max_decoding_message_size(RPC_MESSAGE_BYTES)
    .max_encoding_message_size(RPC_MESSAGE_BYTES);
    let grpc = tonic::transport::Server::builder()
        .initial_stream_window_size(MESSAGE_BYTES as u32)
        .initial_connection_window_size(df_rpc_bridge::RECEIVE_BYTES as u32)
        .max_frame_size(FRAME_BYTES as u32)
        .http2_max_header_list_size(FRAME_BYTES as u32)
        .max_concurrent_streams(CONCURRENT_STREAMS)
        .add_service(service)
        .serve_with_incoming(ReceiverStream::new(receiver));
    let state = PreviewState {
        incoming: sender,
        telemetry: telemetry.clone(),
        statistics,
        connections: Arc::new(Semaphore::new(4)),
    };
    let router = Router::new()
        .route("/", get(|| async { Html(HTML) }))
        .route("/tunnel", get(websocket))
        .route("/fixture-health", get(health))
        .route("/fixture-telemetry", get(fixture_telemetry))
        .nest_service("/pkg", ServeDir::new(web_root))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:43180").await?;
    println!(
        "DungeonFlux S00 fixture ready at http://127.0.0.1:43180 · build {}",
        crate::BUILD_ID
    );
    tokio::select! { result = grpc => result?, result = axum::serve(listener, router) => result?, result = tokio::signal::ctrl_c() => result? }
    telemetry.shutdown().map_err(io::Error::other)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    #[tokio::test]
    async fn rejected_unary_preserves_terminal_metadata() {
        let service = FixtureService {
            statistics: Arc::new(Statistics::default()),
        };
        let error = service
            .unary(Request::new(Sample {
                sequence: 0,
                payload: vec![],
                behavior: Behavior::Reject as i32,
                ..Sample::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(error.code(), Code::InvalidArgument);
        assert_eq!(
            error.metadata().get("fixture-terminal").unwrap(),
            "observed"
        );
    }
    #[tokio::test]
    async fn cancellation_releases_stream_owner() {
        let statistics = Arc::new(Statistics::default());
        let service = FixtureService {
            statistics: statistics.clone(),
        };
        let mut stream = service
            .server_stream(Request::new(Sample {
                sequence: 0,
                payload: vec![],
                behavior: Behavior::Wait as i32,
                ..Sample::default()
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(stream.next().await.unwrap().is_ok());
        drop(stream);
        assert_eq!(statistics.active.load(Ordering::Relaxed), 0);
        assert_eq!(statistics.cancelled.load(Ordering::Relaxed), 1);
    }
}
