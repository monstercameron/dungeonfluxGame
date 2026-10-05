#![cfg(not(target_arch = "wasm32"))]

use axum::{
    Router,
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
};
use df_protocol::common as rpc;
use df_rpc_bridge::{FRAME_BYTES, MESSAGE_BYTES, NativeAdmission, NativeIncoming};
use futures::Stream;
use std::{
    num::NonZeroUsize,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Semaphore, broadcast};
use tonic::{Request, Response, Status};

const PLAYER: &str = "5151515151515151515151515151515151515151515151515151515151515151";
const DISPLAY: &str = "5252525252525252525252525252525252525252525252525252525252525252";

#[derive(Clone)]
struct Service {
    events: broadcast::Sender<()>,
    watches: Arc<AtomicUsize>,
    stale_sent: Arc<AtomicUsize>,
    input: Arc<Mutex<Option<(u64, u64)>>>,
    streams: Arc<Semaphore>,
}

fn revision(epoch: u64, sequence: u64) -> rpc::SessionRevision {
    rpc::SessionRevision {
        epoch: Some(rpc::RecoveryEpoch { value: Some(epoch) }),
        sequence: Some(sequence),
    }
}

fn role<T>(request: &Request<T>) -> Result<bool, Status> {
    match request
        .metadata()
        .get("x-df-local-binding")
        .and_then(|value| value.to_str().ok())
    {
        Some(PLAYER) => Ok(true),
        Some(DISPLAY) => Ok(false),
        _ => Err(Status::unauthenticated("fixture binding required")),
    }
}

fn view(player: bool, stale: bool) -> rpc::ViewMessage {
    let scene = rpc::GameplayScene {
        destination: rpc::HarborDestination::LanternWharf as i32,
        title: if stale {
            "STALE retired epoch must never appear"
        } else {
            "Recovered epoch 2 — Lantern Wharf"
        }
        .to_owned(),
        scene_asset: "assets/ui/scenes/mara-harbor-v4.png".to_owned(),
    };
    let narration = if stale { "This delayed snapshot belongs to epoch 1, sequence 99." } else { "Current permitted view: epoch 2, sequence 1. The recovered scene must remain after delayed old traffic." }.to_owned();
    let journey = rpc::JourneyView {
        phase: rpc::JourneyPhase::Opening as i32,
        room_code: "LANTERN".to_owned(),
        ..Default::default()
    };
    let audience = if player {
        rpc::view_message::Audience::Player(rpc::PlayerGameplayView {
            narration,
            scene: Some(scene),
            journey: Some(journey),
            offers: vec![rpc::GameplayActionOffer {
                offer_id: "recovery-basis-check".to_owned(),
                action_kind: rpc::GameplayActionKind::ExamineHarborSeal as i32,
                label: "Check recovered view basis".to_owned(),
                ..Default::default()
            }],
            ..Default::default()
        })
    } else {
        rpc::view_message::Audience::Display(rpc::DisplayGameplayView {
            narration,
            scene: Some(scene),
            journey: Some(journey),
            ..Default::default()
        })
    };
    rpc::ViewMessage {
        revision: Some(if stale {
            revision(1, 99)
        } else {
            revision(2, 1)
        }),
        audience: Some(audience),
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
        let player = role(&request)?;
        let request = request.into_inner();
        if request
            .session_id
            .as_ref()
            .and_then(|id| id.value.as_deref())
            != Some([0x41; 16].as_slice())
            || request.run_id.as_ref().and_then(|id| id.value.as_deref())
                != Some([0x42; 16].as_slice())
        {
            return Err(Status::invalid_argument("fixture session/run required"));
        }
        let permit = self
            .streams
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::resource_exhausted("fixture stream bound"))?;
        self.watches.fetch_add(1, Ordering::SeqCst);
        let events = self.events.subscribe();
        let sent = self.stale_sent.clone();
        let stream = futures::stream::unfold(
            (true, events, permit, false),
            move |(first, mut events, permit, ended)| {
                let sent = sent.clone();
                async move {
                    if ended {
                        return None;
                    }
                    let result = if first {
                        Ok(view(player, false))
                    } else {
                        match events.recv().await {
                            Ok(()) => {
                                sent.fetch_add(1, Ordering::SeqCst);
                                Ok(view(player, true))
                            }
                            Err(error) => Err(Status::unavailable(format!(
                                "fixture stream ended: {error}"
                            ))),
                        }
                    };
                    let ended = result.is_err();
                    Some((result, (false, events, permit, ended)))
                }
            },
        );
        Ok(Response::new(Box::pin(stream)))
    }
}

#[tonic::async_trait]
impl rpc::room_service_server::RoomService for Service {
    async fn join(
        &self,
        request: Request<rpc::JoinRoomRequest>,
    ) -> Result<Response<rpc::JoinRoomResponse>, Status> {
        let body = request.into_inner();
        if body.room_code != "LANTERN"
            || body.join_secret.len() != 32
            || body
                .operation_id
                .as_ref()
                .and_then(|id| id.value.as_ref())
                .is_none_or(|id| id.len() != 16)
        {
            return Err(Status::invalid_argument("bounded fixture join required"));
        }
        let session_id = Some(rpc::SessionId {
            value: Some(vec![0x41; 16]),
        });
        let run_id = Some(rpc::RunId {
            value: Some(vec![0x42; 16]),
        });
        Ok(Response::new(rpc::JoinRoomResponse {
            outcome: Some(rpc::join_room_response::Outcome::Joined(Box::new(
                rpc::JoinedRoom {
                    receipt: Some(rpc::DecisionReceipt {
                        operation_id: body.operation_id,
                        session_id: session_id.clone(),
                        run_id: run_id.clone(),
                        revision: Some(revision(2, 1)),
                        outcome: Some(rpc::decision_receipt::Outcome::Accepted(Default::default())),
                        replayed: false,
                    }),
                    local_binding: PLAYER.to_owned(),
                    session_id,
                    run_id,
                    client_binding_id: Some(rpc::ClientBindingId {
                        value: Some(vec![0x53; 16]),
                    }),
                },
            ))),
        }))
    }
}

#[tonic::async_trait]
impl rpc::action_service_server::ActionService for Service {
    async fn submit(
        &self,
        request: Request<rpc::SubmitActionRequest>,
    ) -> Result<Response<rpc::SubmitActionResponse>, Status> {
        if !role(&request)? {
            return Err(Status::permission_denied("display cannot submit"));
        }
        let body = request.into_inner();
        let observed = body
            .observed_revision
            .ok_or_else(|| Status::invalid_argument("revision required"))?;
        let epoch = observed
            .epoch
            .and_then(|epoch| epoch.value)
            .ok_or_else(|| Status::invalid_argument("epoch required"))?;
        let sequence = observed
            .sequence
            .ok_or_else(|| Status::invalid_argument("sequence required"))?;
        *self
            .input
            .lock()
            .map_err(|_| Status::internal("fixture input record unavailable"))? =
            Some((epoch, sequence));
        Ok(Response::new(rpc::SubmitActionResponse {
            outcome: Some(rpc::submit_action_response::Outcome::CommittedDecision(
                Box::new(rpc::DecisionReceipt {
                    operation_id: body.operation_id,
                    session_id: body.session_id,
                    run_id: body.run_id,
                    revision: Some(revision(2, 1)),
                    replayed: false,
                    outcome: Some(rpc::decision_receipt::Outcome::Rejected(rpc::Rejection {
                        code: rpc::RejectionCode::CapabilityUnavailable as i32,
                    })),
                }),
            )),
        }))
    }
}

#[derive(Clone)]
struct PageState {
    service: Service,
    admission: NativeAdmission,
    origin: String,
    permits: Arc<Semaphore>,
    html: String,
}

async fn page(State(state): State<PageState>) -> Html<String> {
    Html(state.html)
}
async fn stale(State(state): State<PageState>) -> impl IntoResponse {
    match state.service.events.send(()) {
        Ok(receivers) => (
            StatusCode::OK,
            format!("stale snapshot queued for {receivers} watchers"),
        ),
        Err(_) => (StatusCode::CONFLICT, "no active watcher".to_owned()),
    }
}
async fn fixture_status(State(state): State<PageState>) -> Result<String, StatusCode> {
    let input = *state
        .service
        .input
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(format!(
        "watches={} stale_sent={} last_input={input:?}",
        state.service.watches.load(Ordering::SeqCst),
        state.service.stale_sent.load(Ordering::SeqCst)
    ))
}
async fn socket(
    State(state): State<PageState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> axum::response::Response {
    if headers
        .get("origin")
        .and_then(|origin| origin.to_str().ok())
        != Some(state.origin.as_str())
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(permit) = state.permits.try_acquire_owned() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(admission) = state.admission.try_reserve() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    upgrade
        .read_buffer_size(FRAME_BYTES)
        .write_buffer_size(FRAME_BYTES)
        .max_write_buffer_size(MESSAGE_BYTES)
        .max_message_size(MESSAGE_BYTES)
        .max_frame_size(MESSAGE_BYTES)
        .on_upgrade(move |mut socket| async move {
            if socket
                .send(axum::extract::ws::Message::Binary(Vec::new().into()))
                .await
                .is_err()
            {
                drop(permit);
                return;
            }
            let _result = admission
                .accept_websocket_measured(
                    socket,
                    Arc::new(df_rpc_bridge::ConnectionMetrics::new(false)),
                )
                .await;
            drop(permit);
        })
}

/// Temporary independent-evaluation harness only. The browser loads actual candidate
/// df_tools WASM/glue and production gameplay HTML; only the generated RPC peer is fake.
#[tokio::test]
#[ignore = "requires coordinator runtime/browser lease and candidate WASM"]
async fn serve_actual_gameplay_client_with_late_old_epoch() -> Result<(), Box<dyn std::error::Error>>
{
    let web = std::env::var("DF_REVISION_FIXTURE_WEB")?;
    let assets = std::env::var("DF_REVISION_FIXTURE_ASSETS")?;
    let port: u16 = std::env::var("DF_REVISION_FIXTURE_PORT")?.parse()?;
    if port == 0 {
        return Err("nonzero owned port required".into());
    }
    let html = std::fs::read_to_string(std::env::var("DF_REVISION_FIXTURE_HTML")?)?;
    let roots = format!(
        "<section class=\"client player\" id=\"player\" data-room=\"LANTERN\"><div class=\"connection\" role=\"status\"></div><div class=\"view\"></div></section><section class=\"client display\" id=\"display\" data-local-binding=\"{DISPLAY}\"><div class=\"connection\" role=\"status\"></div><div class=\"view\"></div></section>"
    );
    let html = html
        .replace("__ROOTS__", &roots)
        .replace("__LAYOUT__", "clients");
    let (events, _) = broadcast::channel(8);
    let service = Service {
        events,
        watches: Arc::new(AtomicUsize::new(0)),
        stale_sent: Arc::new(AtomicUsize::new(0)),
        input: Arc::new(Mutex::new(None)),
        streams: Arc::new(Semaphore::new(8)),
    };
    let (admission, incoming) =
        NativeIncoming::bounded(NonZeroUsize::new(4).ok_or("connection bound")?)?;
    let grpc = tonic::transport::Server::builder()
        .initial_stream_window_size(MESSAGE_BYTES as u32)
        .initial_connection_window_size(df_rpc_bridge::RECEIVE_BYTES as u32)
        .max_frame_size(FRAME_BYTES as u32)
        .http2_max_header_list_size(FRAME_BYTES as u32)
        .max_concurrent_streams(df_rpc_bridge::CONCURRENT_STREAMS)
        .add_service(
            rpc::session_service_server::SessionServiceServer::new(service.clone())
                .max_decoding_message_size(8192)
                .max_encoding_message_size(8192),
        )
        .add_service(
            rpc::action_service_server::ActionServiceServer::new(service.clone())
                .max_decoding_message_size(8192)
                .max_encoding_message_size(8192),
        )
        .add_service(
            rpc::room_service_server::RoomServiceServer::new(service.clone())
                .max_decoding_message_size(8192)
                .max_encoding_message_size(8192),
        )
        .serve_with_incoming(incoming);
    let state = PageState {
        service,
        admission,
        origin: format!("http://127.0.0.1:{port}"),
        permits: Arc::new(Semaphore::new(4)),
        html,
    };
    let router = Router::new()
        .route("/gameplay", get(page))
        .route("/gameplay/tunnel", get(socket))
        .route("/fixture/stale", get(stale))
        .route("/fixture/status", get(fixture_status))
        .nest_service("/pkg", tower_http::services::ServeDir::new(web))
        .nest_service("/assets", tower_http::services::ServeDir::new(assets))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!("revision-fixture ready http://127.0.0.1:{port}/gameplay; finite 170-second lifetime");
    tokio::select! {
        result = grpc => result?,
        result = axum::serve(listener, router) => result?,
        () = tokio::time::sleep(Duration::from_secs(170)) => {},
    }
    Ok(())
}
