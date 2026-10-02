#![cfg(not(target_arch = "wasm32"))]

use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

use df_protocol::transport_fixture::{
    Sample,
    sample::Behavior,
    transport_fixture_client::TransportFixtureClient,
    transport_fixture_server::{TransportFixture, TransportFixtureServer},
};
use futures::{Stream, stream};
use prost::Message;
use prost_types::FileDescriptorSet;
use tonic::{
    Code, Request, Response, Status, Streaming,
    codegen::{Service, http},
    metadata::MetadataMap,
};

// This finite test consumer checks generated bindings, not a production game service.
struct NativeBindingFixture;
type SampleStream = Pin<Box<dyn Stream<Item = Result<Sample, Status>> + Send>>;

fn terminal(code: Code) -> Status {
    let mut metadata = MetadataMap::new();
    metadata.insert(
        "binding-terminal",
        tonic::metadata::MetadataValue::from_static("observed"),
    );
    Status::with_metadata(code, "finite native binding fixture", metadata)
}

fn check_request<T>(request: &Request<T>) -> Result<(), Status> {
    if !request
        .metadata()
        .get("binding-request")
        .is_some_and(|value| value == "observed")
    {
        return Err(terminal(Code::InvalidArgument));
    }
    Ok(())
}

fn check_sample(sample: &Sample) -> Result<(), Status> {
    if sample.behavior == Behavior::Reject as i32 {
        return Err(terminal(Code::InvalidArgument));
    }
    Ok(())
}

fn response<T>(value: T) -> Response<T> {
    let mut response = Response::new(value);
    response.metadata_mut().insert(
        "binding-response",
        tonic::metadata::MetadataValue::from_static("observed"),
    );
    response
}

#[tonic::async_trait]
impl TransportFixture for NativeBindingFixture {
    async fn unary(&self, request: Request<Sample>) -> Result<Response<Sample>, Status> {
        check_request(&request)?;
        let sample = request.into_inner();
        check_sample(&sample)?;
        Ok(response(sample))
    }

    type ServerStreamStream = SampleStream;

    async fn server_stream(
        &self,
        request: Request<Sample>,
    ) -> Result<Response<Self::ServerStreamStream>, Status> {
        check_request(&request)?;
        let sample = request.into_inner();
        let status = if sample.behavior == Behavior::Reject as i32 {
            Code::InvalidArgument
        } else {
            Code::Ok
        };
        Ok(response(Box::pin(stream::iter([
            Ok(sample),
            Err(terminal(status)),
        ]))))
    }

    async fn client_stream(
        &self,
        request: Request<Streaming<Sample>>,
    ) -> Result<Response<Sample>, Status> {
        check_request(&request)?;
        let mut incoming = request.into_inner();
        let mut count = 0;
        while let Some(sample) = incoming.message().await? {
            check_sample(&sample)?;
            count += 1;
            if count > 2 {
                return Err(terminal(Code::ResourceExhausted));
            }
        }
        Ok(response(Sample {
            sequence: count,
            payload: b"half-close observed".to_vec(),
            ..Sample::default()
        }))
    }

    type BidiStream = SampleStream;

    async fn bidi(
        &self,
        request: Request<Streaming<Sample>>,
    ) -> Result<Response<Self::BidiStream>, Status> {
        check_request(&request)?;
        let output = stream::unfold(
            (request.into_inner(), false),
            |(mut incoming, closed)| async move {
                if closed {
                    return None;
                }
                match incoming.message().await {
                    Ok(Some(sample)) => {
                        let output = check_sample(&sample).map(|()| sample);
                        let closed = output.is_err();
                        Some((output, (incoming, closed)))
                    }
                    Ok(None) => Some((Err(terminal(Code::Ok)), (incoming, true))),
                    Err(status) => Some((Err(status), (incoming, true))),
                }
            },
        );
        Ok(response(Box::pin(output)))
    }
}

fn sample(sequence: u32) -> Sample {
    Sample {
        sequence,
        payload: vec![0, 255, 128, sequence as u8],
        behavior: Behavior::Echo as i32,
        active_calls: 7,
        deadline_calls: 11,
    }
}

fn request<T>(value: T) -> Request<T> {
    let mut request = Request::new(value);
    request.metadata_mut().insert(
        "binding-request",
        tonic::metadata::MetadataValue::from_static("observed"),
    );
    request
}

fn client() -> TransportFixtureClient<TransportFixtureServer<NativeBindingFixture>> {
    TransportFixtureClient::new(TransportFixtureServer::new(NativeBindingFixture))
}

fn assert_terminal(status: Status, code: Code) {
    assert_eq!(status.code(), code);
    assert_eq!(
        status.metadata().get("binding-terminal").unwrap(),
        "observed"
    );
}

#[test]
fn generated_descriptor_preserves_exact_experimental_service_and_four_mode_signatures() {
    let descriptor = FileDescriptorSet::decode(df_protocol::FILE_DESCRIPTOR_SET).unwrap();
    let file = descriptor
        .file
        .iter()
        .find(|file| file.name.as_deref() == Some("transport_fixture.proto"))
        .unwrap();
    assert_eq!(
        file.package.as_deref(),
        Some("dungeonflux.experimental.transport.v1")
    );
    assert_eq!(file.service.len(), 1);
    let service = &file.service[0];
    assert_eq!(service.name.as_deref(), Some("TransportFixture"));
    assert_eq!(
        <TransportFixtureServer<NativeBindingFixture> as tonic::server::NamedService>::NAME,
        "dungeonflux.experimental.transport.v1.TransportFixture"
    );
    let signatures: Vec<_> = service
        .method
        .iter()
        .map(|method| {
            (
                method.name.as_deref().unwrap(),
                method.input_type.as_deref().unwrap(),
                method.output_type.as_deref().unwrap(),
                method.client_streaming.unwrap_or(false),
                method.server_streaming.unwrap_or(false),
            )
        })
        .collect();
    let sample_type = ".dungeonflux.experimental.transport.v1.Sample";
    assert_eq!(
        signatures,
        [
            ("Unary", sample_type, sample_type, false, false),
            ("ServerStream", sample_type, sample_type, false, true),
            ("ClientStream", sample_type, sample_type, true, false),
            ("Bidi", sample_type, sample_type, true, true),
        ]
    );
}

#[tokio::test]
async fn generated_unary_and_server_stream_preserve_messages_headers_and_ok_trailers() {
    let mut client = client();
    let unary = client.unary(request(sample(9))).await.unwrap();
    assert_eq!(
        unary.metadata().get("binding-response").unwrap(),
        "observed"
    );
    assert_eq!(unary.into_inner(), sample(9));
    let response = client.server_stream(request(sample(4))).await.unwrap();
    assert_eq!(
        response.metadata().get("binding-response").unwrap(),
        "observed"
    );
    let mut output = response.into_inner();
    assert_eq!(output.message().await.unwrap(), Some(sample(4)));
    assert_eq!(output.message().await.unwrap(), None);
    assert_eq!(
        output
            .trailers()
            .await
            .unwrap()
            .unwrap()
            .get("binding-terminal")
            .unwrap(),
        "observed"
    );
}

#[tokio::test]
async fn generated_client_stream_waits_for_half_close_and_bidi_replies_before_half_close() {
    let mut client = client();
    {
        let (sender, receiver) = tokio::sync::mpsc::channel(2);
        sender.send(sample(1)).await.unwrap();
        let response = client.client_stream(request(tokio_stream::wrappers::ReceiverStream::new(
            receiver,
        )));
        tokio::pin!(response);
        assert!(futures::poll!(&mut response).is_pending());
        drop(sender);
        let response = response.await.unwrap();
        assert_eq!(
            response.metadata().get("binding-response").unwrap(),
            "observed"
        );
        assert_eq!(response.get_ref().sequence, 1);
        assert_eq!(response.get_ref().payload, b"half-close observed");
    }

    let (sender, receiver) = tokio::sync::mpsc::channel(2);
    sender.send(sample(1)).await.unwrap();
    let response = client
        .bidi(request(tokio_stream::wrappers::ReceiverStream::new(
            receiver,
        )))
        .await
        .unwrap();
    assert_eq!(
        response.metadata().get("binding-response").unwrap(),
        "observed"
    );
    let mut output = response.into_inner();
    assert_eq!(output.message().await.unwrap(), Some(sample(1)));
    sender.send(sample(2)).await.unwrap();
    assert_eq!(output.message().await.unwrap(), Some(sample(2)));
    drop(sender);
    assert_eq!(output.message().await.unwrap(), None);
    assert_eq!(
        output
            .trailers()
            .await
            .unwrap()
            .unwrap()
            .get("binding-terminal")
            .unwrap(),
        "observed"
    );
}

#[tokio::test]
async fn generated_four_modes_preserve_typed_failures_and_terminal_metadata() {
    let rejected = Sample {
        behavior: Behavior::Reject as i32,
        ..sample(1)
    };
    let mut client = client();
    assert_terminal(
        client.unary(request(rejected.clone())).await.unwrap_err(),
        Code::InvalidArgument,
    );
    assert_terminal(
        client.unary(Request::new(sample(1))).await.unwrap_err(),
        Code::InvalidArgument,
    );
    let mut output = client
        .server_stream(request(rejected.clone()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(output.message().await.unwrap(), Some(rejected.clone()));
    assert_terminal(output.message().await.unwrap_err(), Code::InvalidArgument);
    assert_eq!(output.message().await.unwrap(), None);
    assert_terminal(
        client
            .client_stream(request(stream::iter([rejected.clone()])))
            .await
            .unwrap_err(),
        Code::InvalidArgument,
    );
    assert_terminal(
        client
            .client_stream(request(stream::iter([sample(1), sample(2), sample(3)])))
            .await
            .unwrap_err(),
        Code::ResourceExhausted,
    );
    let mut output = client
        .bidi(request(stream::iter([sample(1), rejected])))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(output.message().await.unwrap(), Some(sample(1)));
    assert_terminal(output.message().await.unwrap_err(), Code::InvalidArgument);
    assert_eq!(output.message().await.unwrap(), None);
}

struct UnknownRoute {
    server: TransportFixtureServer<NativeBindingFixture>,
    path: &'static str,
}

impl Service<http::Request<tonic::body::Body>> for UnknownRoute {
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = tonic::codegen::BoxFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::<http::Request<tonic::body::Body>>::poll_ready(&mut self.server, context)
    }

    fn call(&mut self, mut request: http::Request<tonic::body::Body>) -> Self::Future {
        *request.uri_mut() = http::Uri::from_static(self.path);
        self.server.call(request)
    }
}

#[tokio::test]
async fn generated_dispatch_refuses_unknown_service_and_method_with_unimplemented() {
    for path in [
        "/dungeonflux.experimental.transport.v1.TransportFixture/Unknown",
        "/dungeonflux.public.v1.SessionService/GetOperation",
    ] {
        let mut client = TransportFixtureClient::new(UnknownRoute {
            server: TransportFixtureServer::new(NativeBindingFixture),
            path,
        });
        assert_eq!(
            client.unary(request(sample(1))).await.unwrap_err().code(),
            Code::Unimplemented
        );
    }
}

#[tokio::test]
async fn generated_server_and_client_enforce_exact_encoded_message_bounds() {
    let sample = sample(1);
    let limit = sample.encoded_len();
    let server = TransportFixtureServer::new(NativeBindingFixture)
        .max_decoding_message_size(limit)
        .max_encoding_message_size(limit);
    let mut client = TransportFixtureClient::new(server.clone())
        .max_decoding_message_size(limit)
        .max_encoding_message_size(limit);
    assert_eq!(
        client
            .unary(request(sample.clone()))
            .await
            .unwrap()
            .into_inner(),
        sample
    );
    for (server, client_decode, client_encode) in [
        (
            server.clone().max_decoding_message_size(limit - 1),
            limit,
            limit,
        ),
        (
            server.clone().max_encoding_message_size(limit - 1),
            limit,
            limit,
        ),
        (server.clone(), limit - 1, limit),
        (server, limit, limit - 1),
    ] {
        let mut client = TransportFixtureClient::new(server)
            .max_decoding_message_size(client_decode)
            .max_encoding_message_size(client_encode);
        assert_eq!(
            client
                .unary(request(sample.clone()))
                .await
                .unwrap_err()
                .code(),
            Code::OutOfRange
        );
    }
}
