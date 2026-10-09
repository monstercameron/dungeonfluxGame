#![cfg(not(target_arch = "wasm32"))]

use axum::{Router, extract::WebSocketUpgrade, response::IntoResponse, routing::get};
use df_rpc_bridge::{ConnectionMetrics, ConnectionSnapshot, NativeIncoming, TunnelStream};
use futures::StreamExt;
use std::{
    future::Future,
    io,
    num::NonZeroUsize,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

struct Listener(JoinHandle<()>);
impl Drop for Listener {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Listener {
    async fn stop(&mut self) {
        self.0.abort();
        assert!((&mut self.0).await.unwrap_err().is_cancelled());
    }
}
struct NativeOwner {
    socket: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    stream: Option<TunnelStream>,
    result: mpsc::Receiver<io::Result<()>>,
    metrics: Arc<ConnectionMetrics>,
    listener: Listener,
}
async fn accepted_owner() -> NativeOwner {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (admission, mut incoming) = NativeIncoming::bounded(NonZeroUsize::new(1).unwrap()).unwrap();
    let metrics = Arc::new(ConnectionMetrics::new(false));
    let (sender, result) = mpsc::channel(1);
    let app = Router::new().route(
        "/",
        get({
            let metrics = metrics.clone();
            move |upgrade: WebSocketUpgrade| {
                let metrics = metrics.clone();
                let sender = sender.clone();
                let permit = admission.try_reserve().unwrap();
                async move {
                    upgrade
                        .on_upgrade(move |socket| async move {
                            let outcome = permit.accept_websocket_measured(socket, metrics).await;
                            let _ = sender.send(outcome).await;
                        })
                        .into_response()
                }
            }
        }),
    );
    let listener = Listener(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let (socket, _) = connect_async(format!("ws://{address}/")).await.unwrap();
    let stream = tokio::time::timeout(Duration::from_secs(2), incoming.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    NativeOwner {
        socket,
        stream: Some(stream),
        result,
        metrics,
        listener,
    }
}
#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn pending<F: Future>(future: Pin<&mut F>, wakes: &Arc<WakeCount>) -> bool {
    let waker = Waker::from(wakes.clone());
    matches!(future.poll(&mut Context::from_waker(&waker)), Poll::Pending)
}
fn witness(case: &str, blocked: bool, woke: bool, snapshot: &ConnectionSnapshot) -> String {
    format!(
        r#"    {{"case":"{case}","reader_pending":{blocked},"reader_woken":{woke},"pump":"Ok","closed":{},"rejected":{},"pipe_owners":{},"pipe_to_grpc_current":{},"pipe_to_websocket_current":{},"websocket_receive_current":{}}}"#,
        snapshot.closed,
        snapshot.rejected,
        snapshot.pipe_owners,
        snapshot.pipe_to_grpc.current,
        snapshot.pipe_to_websocket.current,
        snapshot.websocket_receive.current,
    )
}
#[tokio::test]
async fn actual_native_owner_drop_and_shutdown_have_observed_terminal_cleanup() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut witnesses = Vec::new();
        for shutdown in [false, true] {
            let mut owner = accepted_owner().await;
            let stream = owner.stream.take().unwrap();
            let mut blocked = false;
            let mut woke = false;
            if shutdown {
                let (mut reader, mut writer) = tokio::io::split(stream);
                let wakes = Arc::new(WakeCount::default());
                let mut bytes = [0u8; 1];
                let mut read = Box::pin(reader.read(&mut bytes));
                blocked = pending(read.as_mut(), &wakes);
                assert!(blocked, "actual native read must block before shutdown");
                writer.shutdown().await.unwrap();
                tokio::time::timeout(Duration::from_secs(2), owner.result.recv())
                    .await.unwrap().unwrap().unwrap();
                woke = wakes.0.load(Ordering::SeqCst) > 0;
                assert!(woke, "shutdown must wake retained read half");
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(2), read.as_mut())
                        .await.unwrap().unwrap(),
                    0,
                );
                drop(read);
                drop(reader);
                drop(writer);
            } else {
                drop(stream);
                tokio::time::timeout(Duration::from_secs(2), owner.result.recv())
                    .await.unwrap().unwrap().unwrap();
            }
            drop(owner.socket);
            owner.listener.stop().await;
            let snapshot = owner.metrics.snapshot().unwrap();
            assert!(snapshot.closed);
            assert!(!snapshot.rejected);
            assert_eq!(snapshot.pipe_owners, 0);
            assert_eq!(snapshot.pipe_to_grpc.current, 0);
            assert_eq!(snapshot.pipe_to_websocket.current, 0);
            assert_eq!(snapshot.websocket_receive.current, 0);
            assert!(snapshot.envelope_within_limit());
            witnesses.push(witness(
                if shutdown { "local_shutdown" } else { "local_drop" },
                blocked, woke, &snapshot,
            ));
        }
        let actual = format!(
            "{{\n  \"contract_version\": 1,\n  \"browser_qualification\": \"UNPERFORMED_BY_NATIVE_FIXTURE\",\n  \"native_cases\": [\n{}\n  ]\n}}\n",
            witnesses.join(",\n"),
        );
        println!("RESULT_DERIVED_OWNED_CLOSE_JSON={actual}");
        assert_eq!(actual, include_str!("owned_close_contract.json"));
    }).await.unwrap();
}
