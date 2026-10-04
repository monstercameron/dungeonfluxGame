#![cfg(not(target_arch = "wasm32"))]

use std::{
    cell::RefCell,
    future::Future,
    io,
    pin::Pin,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};

use bytes::Bytes;
use df_client::connection::{CallOutcome, RpcConnection, RpcError};
use df_protocol::transport_fixture::{Sample, transport_fixture_client::TransportFixtureClient};
use df_rpc_bridge::{CONCURRENT_STREAMS, RPC_MESSAGE_BYTES};
use futures::{Stream, StreamExt, executor::block_on, stream, task::ArcWake};
use http_body::Frame;
use http_body_util::StreamBody;
use prost::Message;
use tonic::{Code, Request, Streaming, body::Body, codec::Codec};
use tower_service::Service;

#[derive(Clone, Copy)]
enum Mode {
    Normal,
    Status(Code),
    PendingCall,
    PendingStream,
}

#[derive(Default)]
struct Observed {
    calls: usize,
    paths: Vec<String>,
    samples: Vec<Vec<Sample>>,
    timeouts: Vec<bool>,
}

#[derive(Clone)]
struct FixtureChannel {
    mode: Mode,
    observed: Rc<RefCell<Observed>>,
}

impl Service<http::Request<Body>> for FixtureChannel {
    type Response = http::Response<Body>;
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let mode = self.mode;
        let observed = self.observed.clone();
        Box::pin(async move {
            observed.borrow_mut().calls += 1;
            if matches!(mode, Mode::PendingCall) {
                return futures::future::pending().await;
            }
            let path = request.uri().path().to_owned();
            let timeout = request.headers().contains_key("grpc-timeout");
            let codec = &mut tonic_prost::ProstCodec::<Sample, Sample>::default();
            let mut request_stream = Streaming::new_request(
                codec.decoder(),
                request.into_body(),
                None,
                Some(RPC_MESSAGE_BYTES),
            );
            let mut samples = Vec::new();
            while let Some(sample) = request_stream.message().await.map_err(io::Error::other)? {
                samples.push(sample);
            }
            {
                let mut observed = observed.borrow_mut();
                observed.paths.push(path.clone());
                observed.timeouts.push(timeout);
                observed.samples.push(samples.clone());
            }
            let replies = if path.ends_with("ClientStream") {
                vec![Sample {
                    sequence: samples.len() as u32,
                    ..Sample::default()
                }]
            } else {
                samples
            };
            let mut frames = Vec::new();
            for sample in replies {
                let protobuf = sample.encode_to_vec();
                let mut bytes = vec![0];
                bytes.extend_from_slice(&(protobuf.len() as u32).to_be_bytes());
                bytes.extend_from_slice(&protobuf);
                frames.push(Ok(Frame::data(Bytes::from(bytes))));
            }
            let mut trailers = http::HeaderMap::new();
            let code = match mode {
                Mode::Status(code) => code,
                _ => Code::Ok,
            };
            trailers.insert(
                "grpc-status",
                http::HeaderValue::from_str(&(code as i32).to_string()).unwrap(),
            );
            trailers.insert(
                "fixture-terminal",
                http::HeaderValue::from_static("observed"),
            );
            if code != Code::Ok {
                trailers.insert(
                    "grpc-status-details-bin",
                    http::HeaderValue::from_static("AQID"),
                );
            }
            let frames: Pin<Box<dyn Stream<Item = Result<Frame<Bytes>, io::Error>> + Send>> =
                if matches!(mode, Mode::PendingStream) {
                    Box::pin(stream::iter(frames).chain(stream::pending()))
                } else {
                    frames.push(Ok(Frame::trailers(trailers)));
                    Box::pin(stream::iter(frames))
                };
            Ok(http::Response::builder()
                .header("content-type", "application/grpc")
                .header("fixture-initial", "observed")
                .body(Body::new(StreamBody::new(frames)))
                .unwrap())
        })
    }
}

type Client = TransportFixtureClient<FixtureChannel>;

fn connection(mode: Mode) -> (RpcConnection<Client>, Rc<RefCell<Observed>>) {
    let observed = Rc::new(RefCell::new(Observed::default()));
    let channel = FixtureChannel {
        mode,
        observed: observed.clone(),
    };
    let client = TransportFixtureClient::new(channel)
        .max_encoding_message_size(RPC_MESSAGE_BYTES)
        .max_decoding_message_size(RPC_MESSAGE_BYTES);
    (RpcConnection::new(client), observed)
}

fn sample(sequence: u32) -> Sample {
    Sample {
        sequence,
        payload: vec![7, 8],
        ..Sample::default()
    }
}

#[test]
fn actual_generated_client_preserves_all_four_modes_metadata_and_half_close() {
    block_on(async {
        let (connection, observed) = connection(Mode::Normal);
        let (_, unary) = connection
            .unary(async |client: &mut Client| {
                let mut request = Request::new(sample(9));
                request.set_timeout(std::time::Duration::from_secs(1));
                client.unary(request).await
            })
            .unwrap();
        let reply = unary.await.unwrap();
        assert_eq!(reply.metadata().get("fixture-initial").unwrap(), "observed");
        assert_eq!(reply.into_inner(), sample(9));

        let (_, upload) = connection
            .unary(async |client: &mut Client| {
                client
                    .client_stream(stream::iter([sample(1), sample(2)]))
                    .await
            })
            .unwrap();
        assert_eq!(upload.await.unwrap().into_inner().sequence, 2);

        let (_, response) = connection
            .streaming(async |client: &mut Client| client.server_stream(sample(3)).await)
            .unwrap();
        let response = response.await.unwrap();
        assert_eq!(
            response.metadata().get("fixture-initial").unwrap(),
            "observed"
        );
        let mut received = response.into_inner();
        assert!(matches!(
            received.trailers().await,
            Err(RpcError::StreamNotDrained)
        ));
        assert_eq!(received.message().await.unwrap(), Some(sample(3)));
        assert_eq!(received.message().await.unwrap(), None);
        assert_eq!(
            received
                .trailers()
                .await
                .unwrap()
                .unwrap()
                .get("fixture-terminal")
                .unwrap(),
            "observed"
        );
        assert!(matches!(
            received.message().await,
            Err(RpcError::StreamClosed)
        ));

        let (_, response) = connection
            .streaming(async |client: &mut Client| {
                client.bidi(stream::iter([sample(4), sample(5)])).await
            })
            .unwrap();
        let mut received = response.await.unwrap().into_inner();
        assert_eq!(received.message().await.unwrap(), Some(sample(4)));
        assert_eq!(received.message().await.unwrap(), Some(sample(5)));
        assert_eq!(received.message().await.unwrap(), None);
        assert!(received.trailers().await.unwrap().is_some());
        let observed = observed.borrow();
        assert_eq!(observed.calls, 4);
        assert!(observed.timeouts[0]);
        assert_eq!(observed.samples[1], [sample(1), sample(2)]);
        assert_eq!(observed.samples[3], [sample(4), sample(5)]);
        assert_eq!(
            observed
                .paths
                .iter()
                .map(|path| path.rsplit('/').next().unwrap())
                .collect::<Vec<_>>(),
            ["Unary", "ClientStream", "ServerStream", "Bidi"]
        );
    });
}

#[test]
fn grpc_status_codes_details_and_metadata_are_preserved_without_string_parsing() {
    block_on(async {
        for code in [
            Code::InvalidArgument,
            Code::PermissionDenied,
            Code::Unavailable,
            Code::DeadlineExceeded,
            Code::Cancelled,
        ] {
            let (connection, _) = connection(Mode::Status(code));
            let (_, response) = connection
                .unary(async |client: &mut Client| client.unary(sample(1)).await)
                .unwrap();
            let error = response.await.err().unwrap();
            let RpcError::Status(status) = error else {
                panic!("unary server status lost its typed category")
            };
            assert_eq!(status.code(), code);
            assert_eq!(status.details(), [1, 2, 3]);
            assert_eq!(
                status.metadata().get("fixture-terminal").unwrap(),
                "observed"
            );
            let (_, response) = connection
                .streaming(async |client: &mut Client| client.server_stream(sample(1)).await)
                .unwrap();
            let mut received = response.await.unwrap().into_inner();
            assert_eq!(received.message().await.unwrap(), Some(sample(1)));
            let error = received.message().await.err().unwrap();
            let RpcError::Status(status) = error else {
                panic!("server status lost its typed category")
            };
            assert_eq!(status.code(), code);
            assert_eq!(status.details(), [1, 2, 3]);
            assert_eq!(
                status.metadata().get("fixture-terminal").unwrap(),
                "observed"
            );
            assert!(matches!(
                received.message().await,
                Err(RpcError::StreamClosed)
            ));
        }
    });
}

#[test]
fn cancellation_before_poll_does_not_submit_and_capacity_releases_on_disposal() {
    let (connection, observed) = connection(Mode::Normal);
    let mut pending = Vec::new();
    for _ in 0..CONCURRENT_STREAMS {
        let call = connection
            .unary(async |client: &mut Client| client.unary(sample(1)).await)
            .unwrap();
        pending.push(call);
    }
    assert!(matches!(
        connection.unary(async |client: &mut Client| client.unary(sample(2)).await),
        Err(RpcError::Capacity)
    ));
    let (cancel, response) = pending.pop().unwrap();
    cancel.cancel();
    assert!(matches!(block_on(response), Err(RpcError::Cancelled)));
    assert_eq!(observed.borrow().calls, 0);
    assert!(
        connection
            .unary(async |client: &mut Client| client.unary(sample(3)).await)
            .is_ok()
    );
    drop(pending);
}

struct WakeCounter(AtomicUsize);

impl ArcWake for WakeCounter {
    fn wake_by_ref(owner: &Arc<Self>) {
        owner.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn drop_wakes_old_pending_calls_without_affecting_a_replacement_generation() {
    let (old, _) = connection(Mode::PendingCall);
    let (cancel, response) = old
        .unary(async |client: &mut Client| client.unary(sample(1)).await)
        .unwrap();
    let mut response = Box::pin(response);
    let wake_count = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let waker = futures::task::waker(wake_count.clone());
    let mut context = Context::from_waker(&waker);
    assert!(response.as_mut().poll(&mut context).is_pending());
    drop(old);
    assert_eq!(wake_count.0.load(Ordering::SeqCst), 1);
    assert!(matches!(
        block_on(response),
        Err(RpcError::ConnectionClosed)
    ));
    cancel.cancel();
    let (replacement, _) = connection(Mode::Normal);
    let (_, response) = replacement
        .unary(async |client: &mut Client| client.unary(sample(2)).await)
        .unwrap();
    assert_eq!(block_on(response).unwrap().into_inner(), sample(2));
}

#[test]
fn cancel_interrupts_a_pending_response_stream_and_releases_only_its_slot() {
    block_on(async {
        let (connection, _) = connection(Mode::PendingStream);
        let (cancel, response) = connection
            .streaming(async |client: &mut Client| client.server_stream(sample(1)).await)
            .unwrap();
        let mut received = response.await.unwrap().into_inner();
        assert_eq!(received.message().await.unwrap(), Some(sample(1)));
        let mut next = Box::pin(received.message());
        assert!(futures::poll!(next.as_mut()).is_pending());
        cancel.cancel();
        assert!(matches!(next.await, Err(RpcError::Cancelled)));
        assert!(matches!(
            received.message().await,
            Err(RpcError::StreamClosed)
        ));
        assert!(
            connection
                .unary(async |client: &mut Client| client.unary(sample(2)).await)
                .is_ok()
        );
    });
}

#[test]
fn retained_call_outcomes_do_not_retain_capacity_or_change_on_late_cancellation() {
    let (connection, _) = connection(Mode::Normal);
    let (observation, response) = connection
        .unary(async |client: &mut Client| client.unary(sample(1)).await)
        .unwrap();
    assert_eq!(observation.outcome(), CallOutcome::Waiting);
    block_on(response).unwrap();
    assert_eq!(observation.outcome(), CallOutcome::ResponseReceived);
    observation.cancel();
    connection.close();
    assert_eq!(observation.outcome(), CallOutcome::ResponseReceived);

    let (connection, _) = self::connection(Mode::PendingCall);
    let (disposed, response) = connection
        .unary(async |client: &mut Client| client.unary(sample(1)).await)
        .unwrap();
    drop(response);
    assert_eq!(disposed.outcome(), CallOutcome::WaitDisposed);
    let (cancelled, response) = connection
        .unary(async |client: &mut Client| client.unary(sample(2)).await)
        .unwrap();
    cancelled.cancel();
    assert!(matches!(block_on(response), Err(RpcError::Cancelled)));
    assert_eq!(cancelled.outcome(), CallOutcome::Cancelled);
    drop(connection);
    assert_eq!(cancelled.outcome(), CallOutcome::Cancelled);
}

#[test]
fn queued_newer_old_generation_views_cannot_replace_reconnected_current_view() {
    use df_client::{
        connection_views::{ConnectionViewAcceptance, ConnectionViews},
        revisions::ViewAcceptance,
    };
    use df_types::{ClientBindingId, RecoveryEpoch, SessionRevision};
    let binding = ClientBindingId::from_bytes(&[1; 16]).unwrap();
    let epoch = RecoveryEpoch::new(1).unwrap();
    let (old, _) = connection(Mode::Normal);
    let old_generation = old.generation();
    let mut views = ConnectionViews::new(binding, old_generation.clone());
    assert_eq!(
        views.accept(
            &old_generation,
            binding,
            SessionRevision::new(epoch, 1),
            sample(1)
        ),
        ConnectionViewAcceptance::View(ViewAcceptance::Applied)
    );
    old.close();
    assert!(!old_generation.is_active());
    assert_eq!(
        views.accept(
            &old_generation,
            binding,
            SessionRevision::new(epoch, 999),
            sample(99)
        ),
        ConnectionViewAcceptance::ObsoleteGeneration
    );
    let (replacement, _) = connection(Mode::Normal);
    let current = replacement.generation();
    assert!(views.reconnect(current.clone()));
    assert!(!views.reconnect(old_generation.clone()));
    assert_eq!(
        views.accept(&current, binding, SessionRevision::new(epoch, 2), sample(2)),
        ConnectionViewAcceptance::View(ViewAcceptance::Applied)
    );
    assert_eq!(
        views.accept(
            &old_generation,
            binding,
            SessionRevision::new(epoch, 1000),
            sample(99)
        ),
        ConnectionViewAcceptance::ObsoleteGeneration
    );
    assert_eq!(
        views.current(),
        Some((SessionRevision::new(epoch, 2), &sample(2)))
    );
}

#[test]
fn original_pending_lookup_survives_connection_closure_and_replacement() {
    use df_client::receipts::{
        LookupOutcome, OperationKey, OperationResolution, ReceiptLookup, UncertainOperation,
    };
    use df_types::{OperationId, RecoveryEpoch, SessionId};
    struct InProgressLookup;
    impl ReceiptLookup for InProgressLookup {
        type Receipt = ();
        type Error = std::convert::Infallible;
        async fn lookup(&mut self, _: OperationKey) -> Result<LookupOutcome<()>, Self::Error> {
            Ok(LookupOutcome::InProgress)
        }
    }
    let key = OperationKey::new(
        SessionId::from_bytes(&[1; 16]).unwrap(),
        OperationId::from_bytes(&[2; 16]).unwrap(),
        RecoveryEpoch::new(1).unwrap(),
    );
    let mut operation = UncertainOperation::new(key, "retained original draft");
    block_on(operation.resolve(&mut InProgressLookup)).unwrap();
    let (old, _) = connection(Mode::Normal);
    old.close();
    let (replacement, _) = connection(Mode::Normal);
    let (_, response) = replacement
        .unary(async |client: &mut Client| client.unary(sample(2)).await)
        .unwrap();
    block_on(response).unwrap();
    assert_eq!(operation.key(), key);
    assert_eq!(operation.intent(), &"retained original draft");
    assert_eq!(operation.resolution(), &OperationResolution::InProgress);
}
