#![cfg(not(target_arch = "wasm32"))]

use std::{
    cell::RefCell,
    convert::Infallible,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use df_protocol::transport_fixture::{
    Sample,
    transport_fixture_client::TransportFixtureClient,
    transport_fixture_server::{TransportFixture, TransportFixtureServer},
};
use futures::{
    FutureExt, Stream, StreamExt, TryStreamExt,
    executor::block_on,
    future::LocalBoxFuture,
    stream::{self, FusedStream},
};
use tonic::{
    Code, Request, Response, Status, Streaming,
    codegen::{Service, http},
    metadata::{MetadataMap, MetadataValue},
};

// A refusal-only service with an explicitly local, !Send future. This tests the
// actual generated client bound and dispatch; it does not simulate browser I/O.
struct LocalService {
    calls: Rc<RefCell<Vec<(String, bool)>>>,
}

impl Service<http::Request<tonic::body::Body>> for LocalService {
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
        let calls = self.calls.clone();
        async move {
            let has_metadata = request
                .headers()
                .get("binding-request")
                .is_some_and(|value| value == "observed");
            calls
                .borrow_mut()
                .push((request.uri().path().to_owned(), has_metadata));
            let mut metadata = MetadataMap::new();
            metadata.insert("local-terminal", MetadataValue::from_static("observed"));
            Ok(
                Status::with_metadata(Code::Unimplemented, "local refusal fixture", metadata)
                    .into_http(),
            )
        }
        .boxed_local()
    }
}

fn assert_refusal(status: Status) {
    assert_eq!(status.code(), Code::Unimplemented);
    assert_eq!(status.metadata().get("local-terminal").unwrap(), "observed");
}

fn request<T>(body: T) -> Request<T> {
    let mut request = Request::new(body);
    request
        .metadata_mut()
        .insert("binding-request", MetadataValue::from_static("observed"));
    request
}

#[test]
fn actual_generated_four_modes_accept_local_service_future_and_preserve_typed_refusal() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut client = TransportFixtureClient::new(LocalService {
        calls: calls.clone(),
    });
    block_on(async {
        assert_refusal(client.unary(request(Sample::default())).await.unwrap_err());
        assert_refusal(
            client
                .server_stream(request(Sample::default()))
                .await
                .unwrap_err(),
        );
        assert_refusal(
            client
                .client_stream(request(stream::iter([Sample::default()])))
                .await
                .unwrap_err(),
        );
        assert_refusal(
            client
                .bidi(request(stream::iter([Sample::default()])))
                .await
                .unwrap_err(),
        );
    });
    let calls = calls.borrow();
    assert_eq!(
        calls
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        [
            "/dungeonflux.experimental.transport.v1.TransportFixture/Unary",
            "/dungeonflux.experimental.transport.v1.TransportFixture/ServerStream",
            "/dungeonflux.experimental.transport.v1.TransportFixture/ClientStream",
            "/dungeonflux.experimental.transport.v1.TransportFixture/Bidi",
        ]
    );
    assert!(calls.iter().all(|(_, has_metadata)| *has_metadata));
}

// The actual generated server emits one exact protobuf message and genuine
// grpc-status/metadata trailers. This is a local terminal-lifecycle regression,
// not browser transport evidence.
struct TerminalFixture;
type TerminalStream = Pin<Box<dyn Stream<Item = Result<Sample, Status>> + Send>>;

#[tonic::async_trait]
impl TransportFixture for TerminalFixture {
    async fn unary(&self, _: Request<Sample>) -> Result<Response<Sample>, Status> {
        Err(Status::unimplemented("terminal stream fixture only"))
    }

    type ServerStreamStream = TerminalStream;

    async fn server_stream(
        &self,
        request: Request<Sample>,
    ) -> Result<Response<Self::ServerStreamStream>, Status> {
        let mut metadata = MetadataMap::new();
        metadata.insert("fixture-terminal", MetadataValue::from_static("observed"));
        Ok(Response::new(Box::pin(stream::iter([
            Ok(request.into_inner()),
            Err(Status::with_metadata(
                Code::Ok,
                "terminal fixture",
                metadata,
            )),
        ]))))
    }

    async fn client_stream(
        &self,
        _: Request<Streaming<Sample>>,
    ) -> Result<Response<Sample>, Status> {
        Err(Status::unimplemented("terminal stream fixture only"))
    }

    type BidiStream = TerminalStream;

    async fn bidi(
        &self,
        _: Request<Streaming<Sample>>,
    ) -> Result<Response<Self::BidiStream>, Status> {
        Err(Status::unimplemented("terminal stream fixture only"))
    }
}

struct LocalTerminalService {
    server: TransportFixtureServer<TerminalFixture>,
    local: Rc<RefCell<usize>>,
}

impl Service<http::Request<tonic::body::Body>> for LocalTerminalService {
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TransportFixtureServer<TerminalFixture> as Service<
            http::Request<tonic::body::Body>,
        >>::poll_ready(&mut self.server, context)
    }

    fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
        let response = self.server.call(request);
        let local = self.local.clone();
        async move {
            let response = response.await;
            // Retaining Rc across await makes this actual generated server
            // adapter future explicitly local, just like the browser service.
            *local.borrow_mut() += 1;
            response
        }
        .boxed_local()
    }
}

fn terminal_client() -> TransportFixtureClient<LocalTerminalService> {
    TransportFixtureClient::new(LocalTerminalService {
        server: TransportFixtureServer::new(TerminalFixture),
        local: Rc::new(RefCell::new(0)),
    })
}

#[test]
fn generated_terminal_stream_retains_real_trailers_and_fused_eof_after_metadata_extraction() {
    let expected = Sample {
        sequence: u32::MAX,
        payload: vec![0, 255, 128, 0, 1, 127, 254],
        behavior: df_protocol::transport_fixture::sample::Behavior::Echo as i32,
        active_calls: 7,
        deadline_calls: 11,
    };
    block_on(async {
        // Reproduce the pinned Tonic lifecycle that caused the browser qualifier
        // to report truncation after it had already consumed valid OK trailers.
        let mut raw = terminal_client()
            .server_stream(request(expected.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(raw.message().await.unwrap(), Some(expected.clone()));
        assert!(raw.message().await.unwrap().is_none());
        let trailers = raw.trailers().await.unwrap().unwrap();
        assert_eq!(trailers.get("fixture-terminal").unwrap(), "observed");
        let status = raw.message().await.unwrap_err();
        assert_eq!(status.code(), Code::Unknown);
        assert!(status.message().contains("missing grpc-status trailer"));

        // Use the same generated messages and genuine native trailers with the
        // standard Rust fused boundary used by the repaired browser consumer.
        let mut fused = terminal_client()
            .server_stream(request(expected.clone()))
            .await
            .unwrap()
            .into_inner()
            .fuse();
        assert_eq!(fused.try_next().await.unwrap(), Some(expected));
        assert!(fused.try_next().await.unwrap().is_none());
        assert!(fused.is_terminated());
        let trailers = fused.get_mut().trailers().await.unwrap().unwrap();
        assert_eq!(trailers.get("fixture-terminal").unwrap(), "observed");
        assert!(fused.try_next().await.unwrap().is_none());
        assert!(fused.try_next().await.unwrap().is_none());
    });
}
