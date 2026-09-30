use crate::{CONCURRENT_STREAMS, FRAME_BYTES, MESSAGE_BYTES, RECEIVE_BYTES};
use bytes::Bytes;
use futures::{
    FutureExt,
    future::{AbortHandle, Abortable, poll_fn},
};
use gloo_timers::callback::Timeout;
use http_body::{Body, Frame};
use http_body_util::BodyExt;
use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tower_service::Service;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, MessageEvent, WebSocket};

struct ReceiveState {
    messages: VecDeque<Bytes>,
    bytes: usize,
    open: bool,
    closed: bool,
    failed: bool,
    waker: Option<Waker>,
}
impl ReceiveState {
    fn wake(&mut self) {
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}
struct WebSocketIo {
    socket: WebSocket,
    state: Rc<RefCell<ReceiveState>>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _open: Closure<dyn FnMut(Event)>,
    _close: Closure<dyn FnMut(Event)>,
    _error: Closure<dyn FnMut(Event)>,
    write_wakeup: Option<Timeout>,
}
impl WebSocketIo {
    async fn connect(url: &str) -> io::Result<Self> {
        let socket =
            WebSocket::new(url).map_err(|_| io::Error::other("WebSocket creation failed"))?;
        socket.set_binary_type(web_sys::BinaryType::Arraybuffer);
        let state = Rc::new(RefCell::new(ReceiveState {
            messages: VecDeque::new(),
            bytes: 0,
            open: false,
            closed: false,
            failed: false,
            waker: None,
        }));
        let message_state = state.clone();
        let message_socket = socket.clone();
        let message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let mut state = message_state.borrow_mut();
            let Ok(array) = event.data().dyn_into::<js_sys::ArrayBuffer>() else {
                state.failed = true;
                state.wake();
                let _ = message_socket.close();
                return;
            };
            let size = array.byte_length() as usize;
            if size > MESSAGE_BYTES
                || state.bytes.saturating_add(size) > RECEIVE_BYTES
                || state.messages.len() >= 256
            {
                state.failed = true;
                state.messages.clear();
                state.bytes = 0;
                state.wake();
                let _ = message_socket.close();
                return;
            }
            let bytes = js_sys::Uint8Array::new(&array).to_vec();
            state.bytes += bytes.len();
            state.messages.push_back(bytes.into());
            state.wake();
        }) as Box<dyn FnMut(MessageEvent)>);
        socket.set_onmessage(Some(message.as_ref().unchecked_ref()));
        let open_state = state.clone();
        let open = Closure::wrap(Box::new(move |_: Event| {
            let mut state = open_state.borrow_mut();
            state.open = true;
            state.wake();
        }) as Box<dyn FnMut(Event)>);
        socket.set_onopen(Some(open.as_ref().unchecked_ref()));
        let close_state = state.clone();
        let close = Closure::wrap(Box::new(move |_: Event| {
            let mut state = close_state.borrow_mut();
            state.closed = true;
            state.wake();
        }) as Box<dyn FnMut(Event)>);
        socket.set_onclose(Some(close.as_ref().unchecked_ref()));
        let error_state = state.clone();
        let error = Closure::wrap(Box::new(move |_: Event| {
            let mut state = error_state.borrow_mut();
            state.failed = true;
            state.wake();
        }) as Box<dyn FnMut(Event)>);
        socket.set_onerror(Some(error.as_ref().unchecked_ref()));
        let io = Self {
            socket,
            state,
            _message: message,
            _open: open,
            _close: close,
            _error: error,
            write_wakeup: None,
        };
        poll_fn(|context| {
            let mut state = io.state.borrow_mut();
            if state.failed || state.closed {
                return Poll::Ready(Err(io::Error::other("WebSocket admission failed")));
            }
            if state.open {
                return Poll::Ready(Ok(()));
            }
            state.waker = Some(context.waker().clone());
            Poll::Pending
        })
        .await?;
        Ok(io)
    }
    fn wait_for_write(&mut self, context: &Context<'_>) {
        let waker = context.waker().clone();
        self.write_wakeup = Some(Timeout::new(5, move || waker.wake()));
    }
}
impl Drop for WebSocketIo {
    fn drop(&mut self) {
        self.socket.set_onmessage(None);
        self.socket.set_onopen(None);
        self.socket.set_onclose(None);
        self.socket.set_onerror(None);
        let _ = self.socket.close();
    }
}
impl AsyncRead for WebSocketIo {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut state = self.state.borrow_mut();
        if state.failed {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "WebSocket receive bound or transport failure",
            )));
        }
        if let Some(mut bytes) = state.messages.pop_front() {
            let count = buffer.remaining().min(bytes.len());
            buffer.put_slice(&bytes.split_to(count));
            state.bytes -= count;
            if !bytes.is_empty() {
                state.messages.push_front(bytes);
            }
            return Poll::Ready(Ok(()));
        }
        if state.closed {
            return Poll::Ready(Ok(()));
        }
        state.waker = Some(context.waker().clone());
        Poll::Pending
    }
}
impl AsyncWrite for WebSocketIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.socket.ready_state() != WebSocket::OPEN {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "WebSocket closed",
            )));
        }
        if self.socket.buffered_amount() as usize >= MESSAGE_BYTES {
            self.wait_for_write(context);
            return Poll::Pending;
        }
        let count = bytes.len().min(FRAME_BYTES);
        Poll::Ready(
            self.socket
                .send_with_u8_array(&bytes[..count])
                .map(|()| count)
                .map_err(|_| io::Error::other("WebSocket send failed")),
        )
    }
    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.socket.buffered_amount() == 0 {
            Poll::Ready(Ok(()))
        } else {
            self.wait_for_write(context);
            Poll::Pending
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(
            self.socket
                .close()
                .map_err(|_| io::Error::other("WebSocket close failed")),
        )
    }
}

struct AbortOnDrop(AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Owns the local driver lifetime. Drop closes the socket and releases callbacks.
pub struct BrowserConnection {
    driver: AbortOnDrop,
}
impl BrowserConnection {
    /// Connect one HTTP/2 session with bounded frame, header, DATA credit and concurrency.
    pub async fn connect(url: &str) -> io::Result<(Self, BrowserChannel)> {
        let socket = WebSocketIo::connect(url).await?;
        let (sender, connection) = h2::client::Builder::new()
            .initial_window_size(MESSAGE_BYTES as u32)
            .initial_connection_window_size(RECEIVE_BYTES as u32)
            .max_frame_size(FRAME_BYTES as u32)
            .max_header_list_size(FRAME_BYTES as u32)
            .max_send_buffer_size(MESSAGE_BYTES)
            .initial_max_send_streams(CONCURRENT_STREAMS as usize)
            .enable_push(false)
            .handshake(socket)
            .await
            .map_err(io::Error::other)?;
        let (abort, registration) = AbortHandle::new_pair();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(result) = Abortable::new(connection, registration).await {
                let context = df_observe::OperationContext {
                    trace_parent: String::new(),
                    build: "S00-experimental".to_owned(),
                };
                df_observe::record(
                    &context,
                    "bridge.driver",
                    if result.is_ok() { "closed" } else { "failed" },
                    0,
                );
            }
        });
        Ok((
            Self {
                driver: AbortOnDrop(abort),
            },
            BrowserChannel { sender },
        ))
    }
    /// Explicit cancellation is idempotent and owned by the fixture run.
    pub fn close(&self) {
        self.driver.0.abort();
    }
}

/// Generated tonic-client compatible service, with a local future and local h2 driver.
#[derive(Clone)]
pub struct BrowserChannel {
    sender: h2::client::SendRequest<Bytes>,
}
impl Service<http::Request<tonic::body::Body>> for BrowserChannel {
    type Response = http::Response<ReceiveBody>;
    type Error = h2::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.sender.poll_ready(context)
    }
    fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
        let (mut parts, body) = request.into_parts();
        let path = parts
            .uri
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/");
        let uri = format!("http://fixture{path}").parse();
        let Ok(uri) = uri else {
            return futures::future::ready(Err(h2::Reason::PROTOCOL_ERROR.into())).boxed_local();
        };
        parts.uri = uri;
        parts.version = http::Version::HTTP_2;
        let result = self
            .sender
            .send_request(http::Request::from_parts(parts, ()), false);
        async move {
            let (response, mut send) = result?;
            let (abort, registration) = AbortHandle::new_pair();
            let guard = AbortOnDrop(abort);
            wasm_bindgen_futures::spawn_local(async move {
                let upload = async {
                    let mut body = body;
                    while let Some(frame) = body.frame().await {
                        let frame =
                            frame.map_err(|_| h2::Error::from(h2::Reason::INTERNAL_ERROR))?;
                        match frame.into_data() {
                            Ok(mut bytes) => {
                                while !bytes.is_empty() {
                                    send.reserve_capacity(bytes.len().min(FRAME_BYTES));
                                    let capacity = poll_fn(|context| send.poll_capacity(context))
                                        .await
                                        .ok_or_else(|| h2::Error::from(h2::Reason::CANCEL))??;
                                    let count = capacity.min(bytes.len()).min(FRAME_BYTES);
                                    if count != 0 {
                                        send.send_data(bytes.split_to(count), false)?;
                                    }
                                }
                            }
                            Err(frame) => {
                                let trailers = frame
                                    .into_trailers()
                                    .map_err(|_| h2::Error::from(h2::Reason::PROTOCOL_ERROR))?;
                                send.send_trailers(trailers)?;
                                return Ok(());
                            }
                        }
                    }
                    send.send_data(Bytes::new(), true)?;
                    Ok::<(), h2::Error>(())
                };
                if let Ok(Err(_)) = Abortable::new(upload, registration).await {
                    send.send_reset(h2::Reason::INTERNAL_ERROR);
                }
            });
            let response = response.await?;
            Ok(response.map(|receive| ReceiveBody {
                receive,
                _upload: guard,
                data_finished: false,
                finished: false,
                credit: 0,
            }))
        }
        .boxed_local()
    }
}

/// DATA and trailers remain distinct HTTP/2 frames. Credit follows body consumption.
pub struct ReceiveBody {
    receive: h2::RecvStream,
    _upload: AbortOnDrop,
    data_finished: bool,
    finished: bool,
    credit: usize,
}
impl Body for ReceiveBody {
    type Data = Bytes;
    type Error = h2::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        if self.finished {
            return Poll::Ready(None);
        }
        let credit = std::mem::take(&mut self.credit);
        if let Err(error) = self.receive.flow_control().release_capacity(credit) {
            return Poll::Ready(Some(Err(error)));
        }
        if !self.data_finished {
            match self.receive.poll_data(context) {
                Poll::Ready(Some(Ok(bytes))) => {
                    self.credit = bytes.len();
                    return Poll::Ready(Some(Ok(Frame::data(bytes))));
                }
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Some(Err(error))),
                Poll::Ready(None) => self.data_finished = true,
                Poll::Pending => return Poll::Pending,
            }
        }
        match self.receive.poll_trailers(context) {
            Poll::Ready(Ok(trailers)) => {
                self.finished = true;
                Poll::Ready(trailers.map(|trailers| Ok(Frame::trailers(trailers))))
            }
            Poll::Ready(Err(error)) => {
                self.finished = true;
                Poll::Ready(Some(Err(error)))
            }
            Poll::Pending => Poll::Pending,
        }
    }
    fn is_end_stream(&self) -> bool {
        self.finished
    }
}
