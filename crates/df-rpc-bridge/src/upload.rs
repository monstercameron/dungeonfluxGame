use futures::channel::oneshot;
#[cfg(any(target_arch = "wasm32", test))]
use futures::{FutureExt, future::Shared};
#[cfg(any(target_arch = "wasm32", test))]
use std::cell::RefCell;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tonic::Status;

#[cfg(any(target_arch = "wasm32", test))]
pub(super) type ConnectionClosed = Shared<oneshot::Receiver<()>>;

/// One connection-owned signal reaches uploads even while application input is pending.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) struct ConnectionCancellation {
    sender: RefCell<Option<oneshot::Sender<()>>>,
    closed: ConnectionClosed,
}
#[cfg(any(target_arch = "wasm32", test))]
impl ConnectionCancellation {
    pub(super) fn new() -> Self {
        let (sender, receiver) = oneshot::channel();
        Self {
            sender: RefCell::new(Some(sender)),
            closed: receiver.shared(),
        }
    }
    pub(super) fn subscribe(&self) -> ConnectionClosed {
        self.closed.clone()
    }
    pub(super) fn cancel(&self) {
        let sender = self.sender.borrow_mut().take();
        if let Some(sender) = sender {
            let _ = sender.send(());
        }
    }
}

pub(super) enum UploadResult {
    Completed,
    Failed(Status),
    Cancelled,
}

/// One bounded outcome remains owned by the response until upload termination.
pub(super) struct UploadCompletion {
    receiver: Option<oneshot::Receiver<UploadResult>>,
}

impl UploadCompletion {
    pub(super) fn new(receiver: oneshot::Receiver<UploadResult>) -> Self {
        Self {
            receiver: Some(receiver),
        }
    }

    pub(super) fn poll_completion(
        &mut self,
        context: &mut Context<'_>,
        owner_cancelled: bool,
    ) -> Poll<Result<(), Status>> {
        let Some(receiver) = self.receiver.as_mut() else {
            return Poll::Ready(Ok(()));
        };
        let result = match Pin::new(receiver).poll(context) {
            Poll::Ready(result) => result,
            Poll::Pending => return Poll::Pending,
        };
        self.receiver = None;
        Poll::Ready(match result {
            Ok(UploadResult::Completed) => Ok(()),
            Ok(UploadResult::Failed(status)) => Err(status),
            Ok(UploadResult::Cancelled) if owner_cancelled => Ok(()),
            Ok(UploadResult::Cancelled) => Err(Status::cancelled("request upload cancelled")),
            Err(_) => Err(Status::cancelled("request upload outcome unavailable")),
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{ConnectionCancellation, UploadCompletion, UploadResult};
    use bytes::{Buf, BufMut, Bytes};
    use futures::{
        FutureExt,
        channel::oneshot,
        future::{LocalBoxFuture, poll_fn},
        task::{ArcWake, waker},
    };
    use http_body::{Body, Frame};
    use http_body_util::BodyExt;
    use std::{
        error::Error,
        fmt,
        future::Future,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll},
    };
    use tonic::{Code, Status};
    use tower_service::Service;

    #[derive(Debug)]
    struct EncodeFailure;
    impl fmt::Display for EncodeFailure {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("owned encoder failure")
        }
    }
    impl Error for EncodeFailure {}

    struct WakeCounter(AtomicUsize);
    impl ArcWake for WakeCounter {
        fn wake_by_ref(counter: &Arc<Self>) {
            counter.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn completion_result(result: UploadResult, owner_cancelled: bool) -> Result<(), Status> {
        let (sender, receiver) = oneshot::channel();
        assert!(sender.send(result).is_ok());
        let mut completion = UploadCompletion::new(receiver);
        futures::executor::block_on(poll_fn(|context| {
            completion.poll_completion(context, owner_cancelled)
        }))
    }

    #[test]
    fn connection_close_wakes_all_pending_upload_listeners_once() {
        let cancellation = ConnectionCancellation::new();
        let mut first = cancellation.subscribe();
        let mut second = cancellation.subscribe();
        let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let wake = waker(counter.clone());
        let mut context = Context::from_waker(&wake);
        assert!(Pin::new(&mut first).poll(&mut context).is_pending());
        assert!(Pin::new(&mut second).poll(&mut context).is_pending());
        cancellation.cancel();
        assert!(counter.0.load(Ordering::SeqCst) > 0);
        assert!(matches!(
            Pin::new(&mut first).poll(&mut context),
            Poll::Ready(Ok(()))
        ));
        assert!(matches!(
            Pin::new(&mut second).poll(&mut context),
            Poll::Ready(Ok(()))
        ));
        let wakes = counter.0.load(Ordering::SeqCst);
        cancellation.cancel();
        assert_eq!(counter.0.load(Ordering::SeqCst), wakes);
        let mut late = cancellation.subscribe();
        assert!(matches!(
            Pin::new(&mut late).poll(&mut context),
            Poll::Ready(Ok(()))
        ));
    }

    #[test]
    fn old_connection_close_cannot_cancel_fresh_upload_listener() {
        let old = ConnectionCancellation::new();
        let fresh = ConnectionCancellation::new();
        old.cancel();
        let mut listener = fresh.subscribe();
        let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let wake = waker(counter.clone());
        let mut context = Context::from_waker(&wake);
        assert!(Pin::new(&mut listener).poll(&mut context).is_pending());
        assert_eq!(counter.0.load(Ordering::SeqCst), 0);
        fresh.cancel();
        assert!(matches!(
            Pin::new(&mut listener).poll(&mut context),
            Poll::Ready(Ok(()))
        ));
    }

    #[test]
    fn lost_connection_signal_owner_wakes_pending_listener() {
        let cancellation = ConnectionCancellation::new();
        let mut listener = cancellation.subscribe();
        let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let wake = waker(counter.clone());
        let mut context = Context::from_waker(&wake);
        assert!(Pin::new(&mut listener).poll(&mut context).is_pending());
        drop(cancellation);
        assert!(counter.0.load(Ordering::SeqCst) > 0);
        assert!(matches!(
            Pin::new(&mut listener).poll(&mut context),
            Poll::Ready(Err(_))
        ));
    }

    #[test]
    fn upload_failure_preserves_original_status_through_tonic_conversion() {
        let mut status = Status::with_details(
            Code::OutOfRange,
            "encoder limit",
            Bytes::from_static(b"details"),
        );
        status
            .metadata_mut()
            .insert("x-upload", "original".parse().unwrap());
        status.set_source(Arc::new(EncodeFailure));
        let observed = completion_result(UploadResult::Failed(status), false).unwrap_err();
        let observed = Status::from_error(Box::new(observed));
        assert_eq!(observed.code(), Code::OutOfRange);
        assert_eq!(observed.message(), "encoder limit");
        assert_eq!(observed.details(), b"details");
        assert_eq!(observed.metadata().get("x-upload").unwrap(), "original");
        assert!(observed.source().unwrap().is::<EncodeFailure>());
    }

    #[test]
    fn published_failure_wins_even_when_response_owner_cancels_upload() {
        let observed = completion_result(
            UploadResult::Failed(Status::out_of_range("published before reset")),
            true,
        )
        .unwrap_err();
        assert_eq!(observed.code(), Code::OutOfRange);
    }

    #[test]
    fn pending_completion_wakes_and_success_is_not_polled_twice() {
        fn assert_send<T: Send>() {}
        assert_send::<UploadCompletion>();
        let (sender, receiver) = oneshot::channel();
        let mut completion = UploadCompletion::new(receiver);
        let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let wake = waker(counter.clone());
        let mut context = Context::from_waker(&wake);
        assert!(completion.poll_completion(&mut context, false).is_pending());
        assert!(sender.send(UploadResult::Completed).is_ok());
        assert_eq!(counter.0.load(Ordering::SeqCst), 1);
        assert!(matches!(
            completion.poll_completion(&mut context, false),
            Poll::Ready(Ok(()))
        ));
        assert!(matches!(
            completion.poll_completion(&mut context, false),
            Poll::Ready(Ok(()))
        ));
    }

    #[test]
    fn only_explicit_owner_cancellation_accepts_cancelled_upload() {
        assert!(completion_result(UploadResult::Cancelled, true).is_ok());
        assert_eq!(
            completion_result(UploadResult::Cancelled, false)
                .unwrap_err()
                .code(),
            Code::Cancelled
        );
        let (sender, receiver) = oneshot::channel::<UploadResult>();
        drop(sender);
        let mut completion = UploadCompletion::new(receiver);
        let observed = futures::executor::block_on(poll_fn(|context| {
            completion.poll_completion(context, true)
        }))
        .unwrap_err();
        assert_eq!(observed.code(), Code::Cancelled);
        assert_eq!(observed.message(), "request upload outcome unavailable");
    }

    #[test]
    fn transport_error_keeps_tonic_error_source_and_mapping() {
        let error = h2::Error::from(h2::Reason::CANCEL);
        let status = Status::from_error(Box::new(error));
        let expected = status.code();
        let observed = completion_result(UploadResult::Failed(status), false).unwrap_err();
        let observed = Status::from_error(Box::new(observed));
        assert_eq!(observed.code(), expected);
        assert!(observed.source().unwrap().is::<h2::Error>());
    }

    // This consumer uses Tonic's real EncodeBody and message limit, rather than
    // manufacturing a status to represent a hypothetical encoding failure.
    #[derive(Clone)]
    struct UploadConsumer {
        after_headers: bool,
    }
    impl Service<http::Request<tonic::body::Body>> for UploadConsumer {
        type Response = http::Response<tonic::body::Body>;
        type Error = Status;
        type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
            let after_headers = self.after_headers;
            async move {
                let mut body = request.into_body();
                while let Some(frame) = body.frame().await {
                    if let Err(status) = frame {
                        let (sender, receiver) = oneshot::channel();
                        assert!(sender.send(UploadResult::Failed(status)).is_ok());
                        let mut completion = UploadCompletion::new(receiver);
                        if after_headers {
                            let mut response =
                                http::Response::new(tonic::body::Body::new(UploadFailureBody {
                                    completion,
                                    finished: false,
                                }));
                            response.headers_mut().insert(
                                http::header::CONTENT_TYPE,
                                http::HeaderValue::from_static("application/grpc"),
                            );
                            return Ok(response);
                        }
                        poll_fn(|context| completion.poll_completion(context, false)).await?;
                    }
                }
                Err(Status::internal(
                    "encoding limit did not reject the message",
                ))
            }
            .boxed_local()
        }
    }

    struct UploadFailureBody {
        completion: UploadCompletion,
        finished: bool,
    }
    impl Body for UploadFailureBody {
        type Data = Bytes;
        type Error = Status;
        fn poll_frame(
            mut self: Pin<&mut Self>,
            context: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
            if self.finished {
                return Poll::Ready(None);
            }
            match self.completion.poll_completion(context, false) {
                Poll::Ready(Err(status)) => {
                    self.finished = true;
                    Poll::Ready(Some(Err(status)))
                }
                Poll::Ready(Ok(())) => {
                    self.finished = true;
                    Poll::Ready(None)
                }
                Poll::Pending => Poll::Pending,
            }
        }
    }

    struct BytesCodec;
    struct BytesEncoder;
    struct BytesDecoder;
    impl tonic::codec::Codec for BytesCodec {
        type Encode = Bytes;
        type Decode = Bytes;
        type Encoder = BytesEncoder;
        type Decoder = BytesDecoder;
        fn encoder(&mut self) -> Self::Encoder {
            BytesEncoder
        }
        fn decoder(&mut self) -> Self::Decoder {
            BytesDecoder
        }
    }
    impl tonic::codec::Encoder for BytesEncoder {
        type Item = Bytes;
        type Error = Status;
        fn encode(
            &mut self,
            item: Self::Item,
            buffer: &mut tonic::codec::EncodeBuf<'_>,
        ) -> Result<(), Self::Error> {
            buffer.put_slice(&item);
            Ok(())
        }
    }
    impl tonic::codec::Decoder for BytesDecoder {
        type Item = Bytes;
        type Error = Status;
        fn decode(
            &mut self,
            buffer: &mut tonic::codec::DecodeBuf<'_>,
        ) -> Result<Option<Self::Item>, Self::Error> {
            Ok(Some(buffer.copy_to_bytes(buffer.remaining())))
        }
    }

    #[test]
    fn tonic_actual_encoding_limit_survives_owned_upload_completion() {
        let observed = futures::executor::block_on(async {
            let mut client = tonic::client::Grpc::new(UploadConsumer {
                after_headers: false,
            })
            .max_encoding_message_size(1);
            client
                .unary(
                    tonic::Request::new(Bytes::from_static(b"larger than one byte")),
                    http::uri::PathAndQuery::from_static("/fixture/Unary"),
                    BytesCodec,
                )
                .await
                .unwrap_err()
        });
        assert_eq!(observed.code(), Code::OutOfRange);
        assert!(observed.message().contains("limit"));
    }

    #[test]
    fn tonic_actual_encoding_failure_survives_response_headers_and_stream_decode() {
        let observed = futures::executor::block_on(async {
            let mut client = tonic::client::Grpc::new(UploadConsumer {
                after_headers: true,
            })
            .max_encoding_message_size(1);
            let mut response = client
                .streaming(
                    tonic::Request::new(futures::stream::iter([Bytes::from_static(
                        b"larger than one byte",
                    )])),
                    http::uri::PathAndQuery::from_static("/fixture/Bidi"),
                    BytesCodec,
                )
                .await
                .unwrap()
                .into_inner();
            response.message().await.unwrap_err()
        });
        assert_eq!(observed.code(), Code::OutOfRange);
        assert!(observed.message().contains("limit"));
    }
}
