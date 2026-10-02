use axum::{
    Router,
    extract::{RawQuery, State, WebSocketUpgrade},
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
    CONCURRENT_STREAMS, ConnectionMetrics, FRAME_BYTES, MESSAGE_BYTES, RPC_MESSAGE_BYTES,
    TunnelStream, accept_websocket_measured,
};
use futures::{Stream, StreamExt};
use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    sync::{
        Arc, Mutex,
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
    payload_bytes: usize,
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
        self.span
            .finish(&format!("{status}:{:?}", self.status), self.payload_bytes);
    }
}
#[derive(Clone)]
struct FixtureService {
    statistics: Arc<Statistics>,
    telemetry: Arc<FixtureTelemetry>,
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
            span: self.telemetry.begin(
                &OperationContext {
                    trace_parent,
                    build: crate::BUILD_ID.to_owned(),
                },
                method,
            ),
            status: Code::Cancelled,
            complete: false,
            payload_bytes: 0,
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
        scope.payload_bytes += sample.payload.len();
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
        scope.payload_bytes += sample.payload.len();
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
        scope.payload_bytes += sample.payload.len();
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
                let pressure = sample.payload.starts_with(b"pressure:");
                let limit = if pressure {
                    sample.sequence.clamp(1, 128)
                } else {
                    3
                };
                if sequence < limit {
                    let output = if pressure {
                        let paced = sample.payload == b"pressure:paced";
                        if paced {
                            tokio::time::sleep(std::time::Duration::from_millis(8)).await;
                        }
                        Sample {
                            sequence,
                            payload: vec![0x5a; if paced { 8 * 1024 } else { 48 * 1024 }],
                            ..Sample::default()
                        }
                    } else {
                        Sample {
                            sequence,
                            ..sample.clone()
                        }
                    };
                    scope.payload_bytes += output.payload.len();
                    return Some((Ok(output), (sequence + 1, sample, scope)));
                }
                if sequence == limit {
                    scope.finish();
                    return Some((Err(terminal(Code::Ok)), (limit + 1, sample, scope)));
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
            scope.payload_bytes += sample.payload.len();
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
        scope.payload_bytes += b"half-close observed".len();
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
                        scope.payload_bytes += sample.payload.len();
                        let result = check(&sample).map(|()| {
                            scope.payload_bytes += sample.payload.len();
                            sample
                        });
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
    resources: Arc<Mutex<VecDeque<Arc<ConnectionMetrics>>>>,
    origin: Arc<str>,
    reports: Arc<Mutex<ReportState>>,
}
struct ReportState {
    slots: [String; 4],
    credit_order: Option<(u64, u8)>,
}
impl ReportState {
    fn write(&mut self, index: usize, report: String, order: Option<(u64, u8)>) -> bool {
        if index == 3 {
            let Some(order) = order else {
                return false;
            };
            if self.credit_order.is_some_and(|current| order <= current) {
                return false;
            }
            self.credit_order = Some(order);
        }
        self.slots[index] = report;
        true
    }
}
fn credit_report_order(query: Option<&str>) -> Option<(u64, u8)> {
    let (generation, phase) = query?.split_once("&phase=")?;
    let generation = generation.strip_prefix("generation=")?.parse().ok()?;
    let phase = phase.parse().ok()?;
    (generation > 0 && phase <= 2).then_some((generation, phase))
}
async fn websocket(
    State(state): State<PreviewState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> axum::response::Response {
    if headers.get("origin").and_then(|value| value.to_str().ok()) != Some(state.origin.as_ref()) {
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
    let metrics = Arc::new(ConnectionMetrics::new(false));
    match state.resources.lock() {
        Ok(mut resources) => {
            if resources.len() >= 20 {
                if let Some(index) = resources.iter().position(|value| {
                    value
                        .snapshot()
                        .is_ok_and(|value| value.closed && value.pipe_owners == 0)
                }) {
                    resources.remove(index);
                } else {
                    return (
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        "metrics owner capacity reached",
                    )
                        .into_response();
                }
            }
            resources.push_back(metrics.clone());
        }
        Err(_) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "metrics unavailable",
            )
                .into_response();
        }
    }
    upgrade
        .read_buffer_size(FRAME_BYTES)
        .write_buffer_size(FRAME_BYTES)
        .max_write_buffer_size(MESSAGE_BYTES)
        .max_message_size(MESSAGE_BYTES)
        .max_frame_size(MESSAGE_BYTES)
        .on_upgrade(move |socket| async move {
            let context = OperationContext {
                trace_parent: String::new(),
                build: crate::BUILD_ID.to_owned(),
            };
            let result = accept_websocket_measured(socket, state.incoming, metrics.clone()).await;
            drop(permit);
            let mut span = state.telemetry.begin(&context, "bridge.connection");
            match metrics.snapshot() {
                Ok(value) => span.finish(
                    if result.is_ok() { "closed" } else { "failed" },
                    value.received_bytes + value.sent_bytes,
                ),
                Err(_) => span.finish_unmeasured("measurement_failed"),
            }
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
async fn fixture_resources(State(state): State<PreviewState>) -> impl IntoResponse {
    match state.resources.lock() {
        Ok(resources) => resources
            .iter()
            .map(|metrics| format!("{:?}", metrics.snapshot()))
            .collect::<Vec<_>>()
            .join("\n"),
        Err(error) => format!("resource snapshot failure: {error}"),
    }
}
fn report_index(kind: &str) -> Option<usize> {
    match kind {
        "semantics" => Some(0),
        "qualification" => Some(1),
        "callback-capacity" => Some(2),
        "connection-credit" => Some(3),
        _ => None,
    }
}
async fn read_report(
    State(state): State<PreviewState>,
    axum::extract::Path(kind): axum::extract::Path<String>,
) -> axum::response::Response {
    let Some(index) = report_index(&kind) else {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    };
    match state.reports.lock() {
        Ok(reports) => reports.slots[index].clone().into_response(),
        Err(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
async fn write_report(
    State(state): State<PreviewState>,
    axum::extract::Path(kind): axum::extract::Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    report: String,
) -> axum::response::Response {
    if headers.get("origin").and_then(|value| value.to_str().ok()) != Some(state.origin.as_ref()) {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    let Some(index) = report_index(&kind) else {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    };
    if report.len() > 64 * 1024 || report.is_empty() {
        return axum::http::StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let order = if index == 3 {
        match credit_report_order(query.as_deref()) {
            Some(order) => Some(order),
            None => return axum::http::StatusCode::BAD_REQUEST.into_response(),
        }
    } else {
        None
    };
    match state.reports.lock() {
        Ok(mut reports) => {
            if reports.write(index, report, order) {
                axum::http::StatusCode::NO_CONTENT.into_response()
            } else {
                axum::http::StatusCode::CONFLICT.into_response()
            }
        }
        Err(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
// Finite synthetic malicious server inputs. No user/provider data or unbounded sender.
async fn malicious(
    State(state): State<PreviewState>,
    axum::extract::Path(kind): axum::extract::Path<String>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> axum::response::Response {
    if headers.get("origin").and_then(|value| value.to_str().ok()) != Some(state.origin.as_ref()) {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    let Ok(permit) = state.connections.clone().try_acquire_owned() else {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    upgrade
        .read_buffer_size(FRAME_BYTES)
        .write_buffer_size(FRAME_BYTES)
        .max_write_buffer_size(MESSAGE_BYTES * 2)
        .max_message_size(MESSAGE_BYTES)
        .max_frame_size(MESSAGE_BYTES)
        .on_upgrade(move |mut socket| async move {
            let _permit = permit;
            // Wait for the real h2 client's preface before emitting valid SETTINGS followed by abuse.
            if tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
                .await
                .is_err()
            {
                return;
            }
            let mut wire = vec![0, 0, 0, 4, 0, 0, 0, 0, 0];
            match kind.as_str() {
                "websocket" => {
                    wire.resize(MESSAGE_BYTES + 1, 0);
                }
                "frame" => {
                    wire.extend([0, 0x40, 1, 1, 4, 0, 0, 0, 1]);
                    wire.resize(wire.len() + FRAME_BYTES + 1, 0);
                }
                "header" => {
                    // Small HPACK block, decoded list exceeds the advertised 16KiB limit.
                    let mut block = vec![0x88];
                    for _ in 0..300 {
                        block.extend([0, 1, b'x', 32]);
                        block.extend([b'a'; 32]);
                    }
                    let length = block.len();
                    wire.extend([0, (length >> 8) as u8, length as u8, 1, 4, 0, 0, 0, 1]);
                    wire.extend(block);
                }
                "continuation" => {
                    wire.extend([0, 0, 1, 1, 0, 0, 0, 0, 1, 0x82]);
                    for _ in 0..128 {
                        wire.extend([0, 0, 1, 9, 0, 0, 0, 0, 1, 0x82]);
                    }
                }
                "flood" => {
                    for _ in 0..2049 {
                        wire.extend([0, 0, 0, 4, 0, 0, 0, 0, 0]);
                    }
                }
                "control-rate" => {
                    // The prefix already contains initial SETTINGS: exactly 100
                    // more controls make total frame 101 the first rejection.
                    for _ in 0..100 {
                        wire.extend([0, 0, 0, 4, 1, 0, 0, 0, 0]);
                    }
                }
                _ => return,
            }
            if tokio::time::timeout(
                std::time::Duration::from_secs(2),
                socket.send(axum::extract::ws::Message::Binary(wire.into())),
            )
            .await
            .is_ok_and(|result| result.is_ok())
            {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                    while socket.next().await.is_some() {}
                })
                .await;
            }
        })
}
const HTML: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>DungeonFlux S00 transport laboratory</title><style>body{margin:0;background:#10141c;color:#e6eaf3;font:17px system-ui,sans-serif}main{max-width:880px;margin:64px auto;padding:0 24px}h1{font-size:38px;line-height:1.1}p{color:#b7c3d4;line-height:1.5}button{background:#88dfb5;color:#10141c;border:0;border-radius:8px;font:600 17px system-ui;padding:14px 22px;cursor:pointer}button:disabled{opacity:.5}pre{white-space:pre-wrap;padding:20px;background:#1a2331;line-height:1.7;border-radius:10px}.tag{color:#88dfb5;letter-spacing:2px;font-size:13px}a{color:#88dfb5}</style></head><body><main><div class="tag">DUNGEONFLUX / EXPERIMENTAL S00</div><h1>Rust transport laboratory</h1><p>Generated protobuf calls travel as native HTTP/2 gRPC bytes over one binary WebSocket. The browser driver and this fixture interface are Rust compiled to single-threaded WebAssembly.</p><p>This starts the execution foundation. Gameplay, production authentication, durable telemetry and physical device qualification remain pending.</p><button id="run" disabled>Loading Rust/WASM…</button><button id="qualify" disabled>Loading qualification…</button><button id="callback-capacity" disabled>Loading callback observation…</button><button id="connection-credit" disabled>Loading connection-credit observation…</button><pre id="connection-credit-report" role="status" aria-live="polite">Connection-credit stall observation has not run.</pre><pre id="callback-capacity-report" role="status" aria-live="polite">Callback capacity observation has not run.</pre><pre id="qualification" role="status" aria-live="polite">Desktop pressure qualification has not run.</pre><pre id="results" role="status" aria-live="polite">Loading generated WebAssembly bindings…</pre><p><a href="/fixture-health">Native fixture diagnostics</a></p><p id="build"></p></main><script type="module">import init from '/pkg/df_tools.js';await init();</script></body></html>"#;

/// Start only a synthetic loopback preview. The caller owns process and output directory.
pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let preview_configuration = crate::preview::PreviewConfiguration::from_environment()?;
    let web_root = std::env::args()
        .nth(1)
        .ok_or("generated web directory argument required")?;
    let port: u16 = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "43180".to_owned())
        .parse()?;
    if port == 0 {
        return Err("explicit nonzero preview port required".into());
    }
    let telemetry = Arc::new(FixtureTelemetry::default());
    let statistics = Arc::new(Statistics::default());
    let (sender, receiver) = mpsc::channel(4);
    let service = TransportFixtureServer::new(FixtureService {
        statistics: statistics.clone(),
        telemetry: telemetry.clone(),
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
        resources: Arc::new(Mutex::new(VecDeque::new())),
        reports: Arc::new(Mutex::new(ReportState {
            slots: std::array::from_fn(|_| String::new()),
            credit_order: None,
        })),
        origin: format!("http://127.0.0.1:{port}").into(),
    };
    let router = Router::new()
        .route("/", get(|| async { Html(HTML) }))
        .route("/tunnel", get(websocket))
        .route("/fixture-health", get(health))
        .route("/fixture-telemetry", get(fixture_telemetry))
        .route("/fixture-resources", get(fixture_resources))
        .route(
            "/fixture-report/{kind}",
            get(read_report).post(write_report),
        )
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
        .route("/malicious/{kind}", get(malicious))
        .nest_service("/pkg", ServeDir::new(&web_root))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let registration = match preview_configuration {
        Some(configuration) => Some(
            configuration
                .register(web_root.into(), listener.local_addr()?)
                .await?,
        ),
        None => None,
    };
    println!(
        "DungeonFlux S00 fixture ready at http://127.0.0.1:{port} · build {}",
        crate::BUILD_ID
    );
    // Leaving this scope drops both serving futures and the owned listener before
    // the capability is consumed. Errors also take the explicit release path.
    let server_result: Result<(), Box<dyn std::error::Error>> = {
        tokio::select! {
            result = grpc => result.map_err(Into::into),
            result = axum::serve(listener, router) => result.map_err(Into::into),
            result = tokio::signal::ctrl_c() => result.map_err(Into::into),
        }
    };
    let release_result = match registration {
        Some(registration) => registration.release_after_listener_stopped().await,
        None => Ok(()),
    };
    let telemetry_result = telemetry.shutdown();
    if server_result.is_err() || release_result.is_err() || telemetry_result.is_err() {
        return Err(io::Error::other(format!(
            "fixture shutdown: server={server_result:?}; ownership={release_result:?}; telemetry={telemetry_result:?}"
        )).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use df_protocol::transport_fixture::transport_fixture_client::TransportFixtureClient;
    use futures::StreamExt;

    #[test]
    fn credit_report_rejects_late_run_and_late_success_after_deadline() {
        let mut reports = ReportState {
            slots: std::array::from_fn(|_| String::new()),
            credit_order: None,
        };
        assert_eq!(
            credit_report_order(Some("generation=10&phase=0")),
            Some((10, 0))
        );
        assert_eq!(credit_report_order(Some("generation=10&phase=3")), None);
        assert!(reports.write(3, "starting".to_owned(), Some((10, 0))));
        assert!(reports.write(3, "deadline".to_owned(), Some((10, 2))));
        assert!(!reports.write(3, "late success".to_owned(), Some((10, 1))));
        assert_eq!(reports.slots[3], "deadline");
        assert!(reports.write(3, "new run".to_owned(), Some((11, 0))));
        assert!(!reports.write(3, "late old run".to_owned(), Some((10, 2))));
        assert_eq!(reports.slots[3], "new run");
    }
    #[tokio::test]
    async fn rejected_unary_preserves_terminal_metadata() {
        let service = FixtureService {
            statistics: Arc::new(Statistics::default()),
            telemetry: Arc::new(FixtureTelemetry::default()),
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
            telemetry: Arc::new(FixtureTelemetry::default()),
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

    #[tokio::test]
    async fn native_rpc_modes_record_on_the_service_owner() {
        let telemetry = Arc::new(FixtureTelemetry::default());
        let statistics = Arc::new(Statistics::default());
        let service = TransportFixtureServer::new(FixtureService {
            statistics: statistics.clone(),
            telemetry: telemetry.clone(),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(service)
                .serve_with_shutdown(address, async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });
        let endpoint = format!("http://{address}");
        let channel = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match tonic::transport::Endpoint::from_shared(endpoint.clone())
                    .unwrap()
                    .connect()
                    .await
                {
                    Ok(channel) => break channel,
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .unwrap();
        let mut client = TransportFixtureClient::new(channel);
        let sample = || Sample {
            sequence: 0,
            payload: b"owner".to_vec(),
            behavior: Behavior::Echo as i32,
            ..Sample::default()
        };

        client.unary(Request::new(sample())).await.unwrap();
        let mut server_stream = client
            .server_stream(Request::new(sample()))
            .await
            .unwrap()
            .into_inner();
        while server_stream.message().await.unwrap().is_some() {}
        let client_input = futures::stream::iter([sample(), sample()]);
        client
            .client_stream(Request::new(client_input))
            .await
            .unwrap();
        let bidi_input = futures::stream::iter([sample(), sample()]);
        let mut bidi = client
            .bidi(Request::new(bidi_input))
            .await
            .unwrap()
            .into_inner();
        while bidi.message().await.unwrap().is_some() {}

        let mut cancelled = client
            .server_stream(Request::new(Sample {
                behavior: Behavior::Wait as i32,
                ..sample()
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(cancelled.message().await.unwrap().is_some());
        drop(cancelled);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while statistics.cancelled.load(Ordering::Relaxed) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let spans = telemetry.snapshot().unwrap();
        for name in [
            "fixture.unary",
            "fixture.server_stream",
            "fixture.client_stream",
            "fixture.bidi",
        ] {
            assert!(spans.contains(name), "missing {name}: {spans}");
        }
        assert!(spans.contains("cancelled"), "missing cancellation: {spans}");
        assert_eq!(telemetry.count().unwrap(), 5);
        let _ = shutdown_tx.send(());
        server.await.unwrap();
    }
}
