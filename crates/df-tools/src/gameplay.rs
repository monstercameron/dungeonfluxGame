//! A finite, explicitly local gameplay slice. Authentication is preconfigured development
//! membership, not production bootstrap. The action and its outcome use generated RPC.
mod actor;
mod model;
mod qualification;
mod wire;

use axum::{
    Router,
    extract::{State, WebSocketUpgrade},
    http::HeaderMap,
    response::{Html, IntoResponse},
    routing::get,
};
use df_observe::OperationContext;
use df_persistence::local_demo_scope::{LocalDemoAuthority, LocalDemoRole, LocalDemoScopeIssuer};
use df_persistence::{
    NativeCodecLimits, NativeRepositoryOptions, NativeTransactionBounds, PostgresRepository,
};
use df_protocol::common as rpc;
use df_rpc_bridge::{
    CONCURRENT_STREAMS, FRAME_BYTES, MESSAGE_BYTES, NativeAdmission, NativeIncoming,
};
use df_session::inbox::{InboxHandle, bounded_inbox};
use df_session::submission::DurableOwner;
use futures::Stream;
use std::{io, num::NonZeroUsize, pin::Pin, sync::Arc, time::Duration};
use tokio::sync::{Semaphore, oneshot, watch};
use tonic::{Request, Response, Status};

#[derive(Clone)]
struct Service {
    actor: InboxHandle<actor::Call>,
    updates: watch::Receiver<df_model::checkpoint::Checkpoint>,
    streams: Arc<Semaphore>,
}
fn credential<T>(request: &Request<T>) -> Result<[u8; 32], Status> {
    let value = request
        .metadata()
        .get("x-df-local-binding")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| Status::unauthenticated("local binding required"))?;
    if value.len() != 64 {
        return Err(Status::unauthenticated("local binding invalid"));
    }
    let mut result = [0; 32];
    for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let digit = |byte| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let high =
            digit(pair[0]).ok_or_else(|| Status::unauthenticated("local binding invalid"))?;
        let low = digit(pair[1]).ok_or_else(|| Status::unauthenticated("local binding invalid"))?;
        result[index] = (high << 4) | low;
    }
    Ok(result)
}
impl Service {
    async fn view(
        &self,
        credential: [u8; 32],
        request: rpc::WatchViewRequest,
    ) -> Result<(LocalDemoRole, rpc::ViewMessage), Status> {
        let (reply, wait) = oneshot::channel();
        self.actor
            .try_submit(actor::Call::View {
                credential,
                request,
                reply,
            })
            .map_err(|_| Status::resource_exhausted("gameplay actor queue full"))?;
        tokio::time::timeout(Duration::from_secs(5), wait)
            .await
            .map_err(|_| Status::deadline_exceeded("gameplay view deadline"))?
            .map_err(|_| Status::unavailable("gameplay actor stopped"))?
    }
}
#[tonic::async_trait]
impl rpc::action_service_server::ActionService for Service {
    async fn submit(
        &self,
        request: Request<rpc::SubmitActionRequest>,
    ) -> Result<Response<rpc::SubmitActionResponse>, Status> {
        let credential = credential(&request)?;
        let request = request.into_inner();
        let (reply, wait) = oneshot::channel();
        self.actor
            .try_submit(actor::Call::Submit {
                credential,
                request,
                reply,
            })
            .map_err(|_| Status::resource_exhausted("gameplay actor queue full"))?;
        let outcome = tokio::time::timeout(Duration::from_secs(5), wait)
            .await
            .map_err(|_| Status::deadline_exceeded("retain your gameplay operation for lookup"))?
            .map_err(|_| Status::unavailable("gameplay actor stopped"))??;
        Ok(Response::new(outcome))
    }
}
type ViewStream = Pin<Box<dyn Stream<Item = Result<rpc::ViewMessage, Status>> + Send>>;
#[tonic::async_trait]
impl rpc::session_service_server::SessionService for Service {
    type WatchStream = ViewStream;
    async fn watch(
        &self,
        request: Request<rpc::WatchViewRequest>,
    ) -> Result<Response<ViewStream>, Status> {
        let credential = credential(&request)?;
        let request = request.into_inner();
        let permit = self
            .streams
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::resource_exhausted("gameplay watch limit"))?;
        let mut updates = self.updates.clone();
        updates.borrow_and_update();
        let (_, first) = self.view(credential, request.clone()).await?;
        let service = self.clone();
        let stream = futures::stream::unfold(
            (Some(first), updates, service, request, credential, permit),
            |(first, mut updates, service, request, credential, permit)| async move {
                let next = if let Some(first) = first {
                    Ok(first)
                } else {
                    if updates.changed().await.is_err() {
                        return None;
                    }
                    service
                        .view(credential, request.clone())
                        .await
                        .map(|(_, view)| view)
                };
                let failed = next.is_err();
                Some((
                    next,
                    (
                        None,
                        if failed {
                            let (_, closed) = watch::channel(updates.borrow().clone());
                            closed
                        } else {
                            updates
                        },
                        service,
                        request,
                        credential,
                        permit,
                    ),
                ))
            },
        );
        Ok(Response::new(Box::pin(stream)))
    }
}
#[derive(Clone)]
struct PageState {
    incoming: NativeAdmission,
    connections: Arc<Semaphore>,
    player: [u8; 32],
    display: [u8; 32],
    glue: bytes::Bytes,
    wasm: bytes::Bytes,
    art: bytes::Bytes,
}
async fn socket(
    State(state): State<PageState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> axum::response::Response {
    if headers.get("origin").and_then(|v| v.to_str().ok()) != Some("http://127.0.0.1:63309") {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    let Ok(permit) = state.connections.clone().try_acquire_owned() else {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(admission) = state.incoming.try_reserve() else {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    upgrade
        .read_buffer_size(FRAME_BYTES)
        .write_buffer_size(FRAME_BYTES)
        .max_write_buffer_size(MESSAGE_BYTES)
        .max_message_size(MESSAGE_BYTES)
        .max_frame_size(MESSAGE_BYTES)
        .on_upgrade(move |socket| async move {
            let metrics = Arc::new(df_rpc_bridge::ConnectionMetrics::new(false));
            let result = admission.accept_websocket_measured(socket, metrics).await;
            let context = OperationContext {
                trace_parent: String::new(),
                build: crate::BUILD_ID.to_owned(),
            };
            let mut span = df_observe::begin(&context, "gameplay.local_connection");
            span.finish_unmeasured(if result.is_ok() {
                "connection_closed"
            } else {
                "connection_failed"
            });
            drop(permit);
        })
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
async fn glue(State(state): State<PageState>) -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/javascript")],
        state.glue,
    )
}
async fn wasm(State(state): State<PageState>) -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/wasm")],
        state.wasm,
    )
}
async fn art(State(state): State<PageState>) -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "image/png")], state.art)
}
fn bounded_asset(path: std::path::PathBuf, maximum: u64) -> Result<bytes::Bytes, io::Error> {
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(io::Error::other("owned asset bound refused"));
    }
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(file, maximum + 1), &mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(io::Error::other("owned asset bound refused"));
    }
    Ok(bytes.into())
}
async fn both(State(state): State<PageState>) -> Html<String> {
    page(Some(state.player), Some(state.display))
}
async fn player(State(state): State<PageState>) -> Html<String> {
    page(Some(state.player), None)
}
async fn display(State(state): State<PageState>) -> Html<String> {
    page(None, Some(state.display))
}
fn page(player: Option<[u8; 32]>, display: Option<[u8; 32]>) -> Html<String> {
    let roots=[("player",player),("display",display)].into_iter().filter_map(|(role,credential)|credential.map(|credential|format!("<section class=\"client {role}\" id=\"{role}\" data-local-binding=\"{}\"><div class=\"connection\" role=\"status\">Connecting to your game…</div><div class=\"view\"></div></section>",hex(&credential)))).collect::<String>();
    let layout = if player.is_none() {
        "clients display-only"
    } else if display.is_none() {
        "clients player-only"
    } else {
        "clients"
    };
    Html(
        HTML.replace("__ROOTS__", &roots)
            .replace("__LAYOUT__", layout),
    )
}
const HTML: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>DungeonFlux — The Broken Seal</title><style>
*{box-sizing:border-box}body{margin:0;background:#080f17;color:#efe6ce;font-family:Georgia,serif}header{padding:22px 5%;display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid #bd96653b;background:#0b1720}header strong{font-size:24px;letter-spacing:3px;color:#eac488}header span,.eyebrow{font:11px system-ui;letter-spacing:2px;text-transform:uppercase;color:#aeb7b4}.clients{display:grid;grid-template-columns:minmax(320px,400px) 1fr;min-height:calc(100vh - 76px);gap:24px;padding:24px 4%}.clients.display-only{grid-template-columns:1fr}.clients.player-only{grid-template-columns:minmax(0,400px);justify-content:center}.client{position:relative;border:1px solid #cba46d4d;border-radius:18px;background:#10202ae8;overflow:hidden;box-shadow:0 15px 50px #0008}.connection{padding:10px 18px;background:#081219;color:#a8c9ba;font:12px system-ui;letter-spacing:.4px}.art{width:100%;height:260px;object-fit:cover;object-position:44% 48%;display:block}.display .art{height:calc(100vh - 200px);min-height:400px;object-position:45% 45%}.scene-copy{padding:24px}.display .scene-copy{position:absolute;bottom:0;right:0;left:0;padding:70px 40px 32px;background:linear-gradient(transparent,#061118f7 45%)}h1,h2{margin:8px 0 14px;font-weight:400;line-height:1.1}h1{font-size:54px}.player h2{font-size:30px}.narration{font-size:17px;line-height:1.5;color:#d1d3c6;margin:12px 0 20px}.display .narration{max-width:640px;font-size:22px}.offered{display:block;background:linear-gradient(135deg,#355447,#20392e);border:1px solid #8cb498;color:#f5eedf;border-radius:10px;width:100%;padding:17px 18px;min-height:56px;text-align:left;font:18px Georgia;cursor:pointer;box-shadow:0 5px 16px #0004}.offered small{display:block;color:#bed0bd;font:12px system-ui;margin-top:8px}.offered:disabled{opacity:.5;cursor:default}.tools{display:flex;flex-wrap:wrap;gap:8px;margin:18px 0 0}.tools button{background:#132530;border:1px solid #7c8a825e;border-radius:7px;padding:11px;color:#d7ddd1;min-height:44px;cursor:pointer}.result{border:1px solid #c59b5f77;border-radius:12px;background:#162630;margin:20px 0;padding:20px}.dice{font-size:40px;color:#f2ce90}.check-label{font:12px system-ui;letter-spacing:1px;color:#9fc1ae}.clue{border-left:2px solid #cbaa67;padding-left:16px;color:#eed69a;line-height:1.6}.receipt{font:11px system-ui;color:#8cb5a0;line-height:1.6}.feedback{font:13px system-ui;color:#d9b681;margin-top:12px}.scope{font:11px system-ui;color:#9ba9a3;line-height:1.5;margin-top:20px}footer{padding:18px 4%;font:11px system-ui;color:#899995;line-height:1.5}footer a{color:#aebfb2}button:focus-visible{outline:3px solid #f1c782;outline-offset:3px}@media(max-width:900px){.clients{grid-template-columns:1fr;max-width:680px;margin:auto}.display .art{height:380px;min-height:0}.display .scene-copy{padding:60px 24px 24px}h1{font-size:36px}.display .narration{font-size:17px}}@media(max-width:420px){header{padding:16px}header strong{font-size:18px}header span{font-size:9px;max-width:110px;text-align:right}.clients{padding:14px 10px;gap:14px}.art{height:185px}.scene-copy{padding:18px}.player h2{font-size:27px}.narration{font-size:15px;margin-bottom:16px}.offered{padding:14px;font-size:17px}.result{padding:16px}.display .art{height:330px}.tools{gap:6px}}@media(prefers-reduced-motion:reduce){*{scroll-behavior:auto}}
</style></head><body><header><strong>DUNGEONFLUX</strong><span>The Broken Seal · a playable investigation</span></header><main class="__LAYOUT__">__ROOTS__</main><footer>Local gameplay slice · preconfigured Mara · one normal Intelligence (Investigation) check · SRD 5.2.1 subset. This work includes material from the System Reference Document 5.2.1 (“SRD 5.2.1”) by Wizards of the Coast LLC, available at <a href="https://www.dndbeyond.com/srd">dndbeyond.com/srd</a>. The SRD 5.2.1 is licensed under the <a href="https://creativecommons.org/licenses/by/4.0/legalcode">Creative Commons Attribution 4.0 International License</a>.</footer><script type="module">import init from '/pkg/df_tools.js'; await init();</script></body></html>"#;

/// Run a 170-second loopback demonstration using an already owned empty PG cluster.
pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let web_root = std::env::args()
        .nth(2)
        .ok_or("generated gameplay web directory required")?;
    let asset_root = std::env::args()
        .nth(3)
        .ok_or("fixed gameplay asset directory required")?;
    let glue_bytes = bounded_asset(
        std::path::Path::new(&web_root).join("df_tools.js"),
        1024 * 1024,
    )?;
    let wasm_bytes = bounded_asset(
        std::path::Path::new(&web_root).join("df_tools_bg.wasm"),
        32 * 1024 * 1024,
    )?;
    let art_bytes = bounded_asset(
        std::path::Path::new(&asset_root).join("scenes/mara-harbor-v4.png"),
        16 * 1024 * 1024,
    )?;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 63309)).await?;
    let initial =
        model::initial().map_err(|_| io::Error::other("gameplay baseline validation failed"))?;
    let codec = NativeCodecLimits {
        maximum_document_bytes: 1024 * 1024,
        maximum_allocated_bytes: 2 * 1024 * 1024,
        maximum_collection_items: 1024,
        maximum_text_bytes: 1024,
    };
    let fence = model::random::<16>().map_err(|_| io::Error::other("native fence unavailable"))?;
    let player_credential =
        model::random::<32>().map_err(|_| io::Error::other("local grant unavailable"))?;
    let display_credential =
        model::random::<32>().map_err(|_| io::Error::other("local grant unavailable"))?;
    let database = database_config()?;
    let (mut grant_client, grant_connection) = database.connect(tokio_postgres::NoTls).await?;
    let grant_driver = tokio::spawn(grant_connection);
    let initialization = tokio::time::timeout(
        Duration::from_secs(5),
        df_persistence::local_demo_scope::initialize(
            &mut grant_client,
            &initial,
            fence,
            player_credential,
            display_credential,
            codec,
        ),
    )
    .await;
    if !matches!(initialization, Ok(Ok(()))) {
        drop(grant_client);
        grant_driver.await??;
        return Err(io::Error::other(format!(
            "owned gameplay database initialization refused: {initialization:?}"
        ))
        .into());
    }
    let runtime = tokio::runtime::Handle::current();
    let issuer = LocalDemoScopeIssuer::new(
        runtime.clone(),
        grant_client,
        initial.basis().session,
        fence,
    )
    .map_err(|_| io::Error::other("local issuer construction refused"))?;
    let (client, connection) = database.connect(tokio_postgres::NoTls).await?;
    let context = OperationContext {
        trace_parent: String::new(),
        build: crate::BUILD_ID.to_owned(),
    };
    let repository = PostgresRepository::<LocalDemoAuthority>::from_connected_no_tls(
        runtime.clone(),
        client,
        connection,
        NativeRepositoryOptions {
            transaction_bounds: NativeTransactionBounds {
                transaction: Duration::from_secs(2),
                rollback: Duration::from_millis(250),
                driver_join: Duration::from_secs(2),
            },
            codec_limits: codec,
            maximum_receipt_bytes: 4096,
            verifier: Some(df_persistence::local_demo_scope::verifier()),
            recovery: Some(
                model::recovery()
                    .map_err(|_| io::Error::other("source recovery inventory invalid"))?,
            ),
        },
        &context,
    )
    .await
    .map_err(|_| io::Error::other("native repository setup failed"))?;
    let (updates, receiver) = watch::channel(initial.clone());
    let owner = DurableOwner::new(
        repository,
        model::HarborEngine,
        actor::Publication(updates.clone()),
        initial,
        4096,
    )
    .map_err(|_| io::Error::other("canonical owner setup refused"))?;
    let (sender, inbox) = bounded_inbox::<actor::Call>();
    let thread = std::thread::Builder::new()
        .name("df-real-gameplay-owner".to_owned())
        .spawn(move || {
            let mut actor = actor::Actor {
                owner,
                issuer,
                codec,
                fenced: false,
                recovery_wakeup: updates,
                calls_remaining: 128,
            };
            let drained = inbox.run(&mut actor);
            let mut repository = actor.owner.into_repository();
            let closed = repository.close();
            drop(actor.issuer);
            (drained, closed)
        })?;
    let service = Service {
        actor: sender.clone(),
        updates: receiver,
        streams: Arc::new(Semaphore::new(4)),
    };
    let (incoming_sender, incoming) =
        NativeIncoming::bounded(NonZeroUsize::new(4).ok_or("invalid native incoming bound")?)?;
    let grpc = tonic::transport::Server::builder()
        .initial_stream_window_size(MESSAGE_BYTES as u32)
        .initial_connection_window_size(df_rpc_bridge::RECEIVE_BYTES as u32)
        .max_frame_size(FRAME_BYTES as u32)
        .http2_max_header_list_size(FRAME_BYTES as u32)
        .max_concurrent_streams(CONCURRENT_STREAMS)
        .add_service(
            rpc::action_service_server::ActionServiceServer::new(service.clone())
                .max_decoding_message_size(8192)
                .max_encoding_message_size(8192),
        )
        .add_service(
            rpc::session_service_server::SessionServiceServer::new(service.clone())
                .max_decoding_message_size(8192)
                .max_encoding_message_size(8192),
        )
        .serve_with_incoming(incoming);
    let state = PageState {
        incoming: incoming_sender,
        connections: Arc::new(Semaphore::new(4)),
        player: player_credential,
        display: display_credential,
        glue: glue_bytes,
        wasm: wasm_bytes,
        art: art_bytes,
    };
    let router = Router::new()
        .route("/gameplay", get(both))
        .route("/gameplay/player", get(player))
        .route("/gameplay/display", get(display))
        .route("/gameplay/tunnel", get(socket))
        .route("/pkg/df_tools.js", get(glue))
        .route("/pkg/df_tools_bg.wasm", get(wasm))
        .route("/assets/ui/scenes/mara-harbor-v4.png", get(art))
        .with_state(state);
    let qualification = if std::env::args().nth(4).as_deref() == Some("--qualification") {
        let (cancel, cancelled) = oneshot::channel();
        Some((
            cancel,
            tokio::spawn(qualification::run(
                service,
                player_credential,
                display_credential,
                cancelled,
                database,
            )),
        ))
    } else {
        None
    };
    let outcome: Result<(), Box<dyn std::error::Error>> = {
        tokio::select! {
            result=grpc=>result.map_err(Into::into),result=axum::serve(listener,router)=>result.map_err(Into::into),
            result=tokio::signal::ctrl_c()=>result.map_err(Into::into),_=tokio::time::sleep(Duration::from_secs(170))=>Ok(()),
        }
    };
    let qualification_outcome = if let Some((cancel, task)) = qualification {
        let _ = cancel.send(());
        match task.await {
            Ok(result) => result,
            Err(_) => Err(io::Error::other("qualification consumer did not finish")),
        }
    } else {
        Ok(())
    };
    sender
        .stop()
        .map_err(|_| io::Error::other("gameplay inbox stop failed"))?;
    let (drained, closed) = tokio::task::spawn_blocking(move || thread.join())
        .await?
        .map_err(|_| io::Error::other("gameplay actor join failed"))?;
    drained.map_err(|_| io::Error::other("gameplay actor drain failed"))?;
    closed.map_err(|_| io::Error::other("gameplay repository close failed"))?;
    grant_driver.await??;
    qualification_outcome?;
    outcome
}

fn database_config() -> Result<tokio_postgres::Config, io::Error> {
    let name = match std::env::var("DF_GAMEPLAY_DEMO_DATABASE") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => "df_gameplay_demo_20261004_r03".to_owned(),
        Err(_) => return Err(io::Error::other("local demonstration database invalid")),
    };
    if name.len() > 63
        || !name.starts_with("df_gameplay_demo_")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(io::Error::other("local demonstration database invalid"));
    }
    let mut configuration = tokio_postgres::Config::new();
    configuration
        .host("127.0.0.1")
        .port(55517)
        .user("df_gameplay_demo_admin_20261004")
        .dbname(&name);
    Ok(configuration)
}
