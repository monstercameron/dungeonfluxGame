#![cfg(not(target_arch = "wasm32"))]

use axum::{Router, extract::WebSocketUpgrade, response::IntoResponse, routing::get};
use df_rpc_bridge::{ConnectionMetrics, NativeIncoming, TunnelStream};
use futures::{SinkExt, StreamExt};
use std::{io, num::NonZeroUsize, sync::Arc, time::Duration};
use tokio::{io::AsyncReadExt, sync::mpsc, task::JoinHandle};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

type ClientSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

struct NativeCase {
    socket: ClientSocket,
    stream: TunnelStream,
    result: mpsc::Receiver<io::Result<()>>,
    metrics: Arc<ConnectionMetrics>,
    listener: JoinHandle<()>,
}

async fn accepted_case() -> NativeCase {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("owned listener");
    let address = listener.local_addr().expect("owned address");
    let (admission, mut incoming) =
        NativeIncoming::bounded(NonZeroUsize::new(1).expect("nonzero capacity"))
            .expect("bounded admission");
    let metrics = Arc::new(ConnectionMetrics::new(false));
    let (result_sender, result) = mpsc::channel(1);
    let app = Router::new().route(
        "/",
        get({
            let metrics = metrics.clone();
            move |upgrade: WebSocketUpgrade| {
                let metrics = metrics.clone();
                let result_sender = result_sender.clone();
                let permit = admission.try_reserve().expect("one admitted connection");
                async move {
                    upgrade
                        .on_upgrade(move |socket| async move {
                            let result = permit.accept_websocket_measured(socket, metrics).await;
                            let _ = result_sender.send(result).await;
                        })
                        .into_response()
                }
            }
        }),
    );
    let listener = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("owned Axum server");
    });
    let (socket, _) = connect_async(format!("ws://{address}/"))
        .await
        .expect("real WebSocket connect");
    let stream = tokio::time::timeout(Duration::from_secs(2), incoming.next())
        .await
        .expect("admitted stream wait")
        .expect("admitted stream")
        .expect("admitted stream error");
    NativeCase {
        socket,
        stream,
        result,
        metrics,
        listener,
    }
}

async fn finish(mut case: NativeCase, expected_error: Option<io::ErrorKind>) {
    let result = tokio::time::timeout(Duration::from_secs(2), case.result.recv())
        .await
        .expect("native pump close bound")
        .expect("native pump result missing");
    match expected_error {
        Some(kind) => assert_eq!(result.expect_err("expected transport failure").kind(), kind),
        None => result.expect("clean native transport close"),
    }
    drop(case.stream);
    case.listener.abort();
    let _ = case.listener.await;
    let snapshot = case.metrics.snapshot().expect("native resource snapshot");
    assert!(snapshot.closed, "connection owner must close");
    assert_eq!(snapshot.pipe_owners, 0, "both byte pipes must release");
    assert_eq!(snapshot.websocket_receive.current, 0);
    assert_eq!(snapshot.pipe_to_grpc.current, 0);
    assert_eq!(snapshot.pipe_to_websocket.current, 0);
    assert_eq!(snapshot.rejected, expected_error.is_some());
}

#[tokio::test]
async fn idle_websocket_close_releases_native_tunnel_and_blocked_read() {
    let mut case = accepted_case().await;
    case.socket.close(None).await.expect("owned client close");
    let mut byte = [0; 1];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), case.stream.read(&mut byte))
            .await
            .expect("idle reader wake")
            .expect("idle reader result"),
        0
    );
    finish(case, None).await;
}

#[tokio::test]
async fn partial_http2_preface_close_is_transport_loss_with_released_owners() {
    let mut case = accepted_case().await;
    case.socket
        .send(Message::Binary(b"PRI * HT".to_vec().into()))
        .await
        .expect("partial real WebSocket message");
    let mut read = [0; 8];
    tokio::time::timeout(Duration::from_secs(2), case.stream.read_exact(&mut read))
        .await
        .expect("partial preface read bound")
        .expect("partial preface read");
    assert_eq!(&read, b"PRI * HT");
    case.socket.close(None).await.expect("owned client close");
    let mut tail = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), case.stream.read_to_end(&mut tail))
        .await
        .expect("blocked reader wake")
        .expect("blocked reader result");
    assert!(tail.is_empty());
    finish(case, Some(io::ErrorKind::UnexpectedEof)).await;
}
