use crate::{ConnectionMetrics, FRAME_BYTES, MESSAGE_BYTES, RECEIVE_BYTES};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf},
    sync::mpsc,
};

/// Connection owned by tonic. Dropping it releases the associated WebSocket pump.
pub struct TunnelStream(PipeIo);
struct PipeIo {
    stream: DuplexStream,
    metrics: Arc<ConnectionMetrics>,
    grpc_end: bool,
}
impl Drop for PipeIo {
    fn drop(&mut self) {
        self.metrics.pipe_drop();
    }
}
impl AsyncRead for PipeIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let metrics = self.metrics.clone();
        let to_grpc = self.grpc_end;
        let result = metrics.pipe(to_grpc, |counter| {
            let before = buffer.filled().len();
            match Pin::new(&mut self.stream).poll_read(context, buffer) {
                Poll::Ready(Ok(())) => {
                    Poll::Ready(counter.transferred(buffer.filled().len() - before, false))
                }
                other => other,
            }
        });
        result.unwrap_or_else(|error| Poll::Ready(Err(error)))
    }
}
impl AsyncWrite for PipeIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let metrics = self.metrics.clone();
        let to_grpc = !self.grpc_end;
        let result = metrics.pipe(to_grpc, |counter| {
            match Pin::new(&mut self.stream).poll_write(context, bytes) {
                Poll::Ready(Ok(count)) => {
                    Poll::Ready(counter.transferred(count, true).map(|()| count))
                }
                other => other,
            }
        });
        result.unwrap_or_else(|error| Poll::Ready(Err(error)))
    }
    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(context)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(context)
    }
}
struct MessageScope {
    metrics: Arc<ConnectionMetrics>,
    bytes: usize,
}
impl Drop for MessageScope {
    fn drop(&mut self) {
        if self.metrics.socket_receive(self.bytes, false).is_err() {
            self.metrics.reject();
        }
    }
}
struct ConnectionScope(Arc<ConnectionMetrics>);
impl Drop for ConnectionScope {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl tonic::transport::server::Connected for TunnelStream {
    type ConnectInfo = ();
    fn connect_info(&self) {}
}
impl AsyncRead for TunnelStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(context, buffer)
    }
}
impl AsyncWrite for TunnelStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(context, bytes)
    }
    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(context)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(context)
    }
}

/// Run both byte directions in the upgrade's scope. Admission never queues without a bound.
pub async fn accept_websocket(
    socket: WebSocket,
    incoming: mpsc::Sender<Result<TunnelStream, io::Error>>,
) -> io::Result<()> {
    accept_websocket_measured(socket, incoming, Arc::new(ConnectionMetrics::new(false))).await
}
/// Own the real pipe counters and observe both HTTP/2 wire directions.
pub async fn accept_websocket_measured(
    socket: WebSocket,
    incoming: mpsc::Sender<Result<TunnelStream, io::Error>>,
    metrics: Arc<ConnectionMetrics>,
) -> io::Result<()> {
    let _scope = ConnectionScope(metrics.clone());
    let (grpc, tunnel) = tokio::io::duplex(RECEIVE_BYTES);
    let grpc = PipeIo {
        stream: grpc,
        metrics: metrics.clone(),
        grpc_end: true,
    };
    let tunnel = PipeIo {
        stream: tunnel,
        metrics: metrics.clone(),
        grpc_end: false,
    };
    incoming
        .try_send(Ok(TunnelStream(grpc)))
        .map_err(|_| io::Error::other("connection capacity exhausted"))?;
    let (mut outgoing, mut incoming_socket) = socket.split();
    let (mut reader, mut writer) = tokio::io::split(tunnel);
    let receive = async {
        while let Some(message) = incoming_socket.next().await {
            match message.map_err(io::Error::other)? {
                Message::Binary(bytes) if bytes.len() <= MESSAGE_BYTES => {
                    metrics.socket_receive(bytes.len(), true)?;
                    let _message_owner = MessageScope {
                        metrics: metrics.clone(),
                        bytes: bytes.len(),
                    };
                    metrics.observe(true, &bytes)?;
                    for chunk in bytes.chunks(FRAME_BYTES) {
                        writer.write_all(chunk).await?;
                        metrics.yield_decode()?;
                        tokio::task::yield_now().await;
                    }
                }
                Message::Close(_) => return Ok(()),
                Message::Ping(_) | Message::Pong(_) => {}
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "binary tunnel message rejected",
                    ));
                }
            }
            metrics.yield_decode()?;
            tokio::task::yield_now().await;
        }
        Ok(())
    };
    let send = async {
        let mut buffer = vec![0; FRAME_BYTES];
        loop {
            let count = reader.read(&mut buffer).await?;
            if count == 0 {
                return Ok(());
            }
            metrics.observe(false, &buffer[..count])?;
            metrics.backlog(count)?;
            outgoing
                .send(Message::Binary(buffer[..count].to_vec().into()))
                .await
                .map_err(io::Error::other)?;
            metrics.backlog(0)?;
        }
    };
    let result = tokio::select! { result = receive => result, result = send => result };
    if result.is_err() {
        metrics.reject();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, extract::WebSocketUpgrade, routing::get};
    use bytes::Bytes;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn oversized_binary_websocket_is_rejected_at_gateway() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (incoming, mut receiver) = mpsc::channel(1);
        let (result_sender, result_receiver) = oneshot::channel();
        let result_sender = std::sync::Arc::new(std::sync::Mutex::new(Some(result_sender)));
        let app = Router::new().route(
            "/",
            get(move |upgrade: WebSocketUpgrade| {
                let incoming = incoming.clone();
                let result_sender = result_sender.clone();
                async move {
                    upgrade
                        .read_buffer_size(FRAME_BYTES)
                        .write_buffer_size(FRAME_BYTES)
                        .max_write_buffer_size(MESSAGE_BYTES)
                        .max_message_size(MESSAGE_BYTES)
                        .max_frame_size(MESSAGE_BYTES)
                        .on_upgrade(move |socket| async move {
                            let result = accept_websocket(socket, incoming).await;
                            if let Some(sender) = result_sender.lock().unwrap().take() {
                                sender.send(result).unwrap();
                            }
                        })
                }
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/"))
            .await
            .unwrap();
        let connection = receiver.recv().await.unwrap().unwrap();
        socket
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                vec![0; MESSAGE_BYTES + 1].into(),
            ))
            .await
            .unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), result_receiver)
            .await
            .unwrap()
            .unwrap();
        assert!(
            result.is_err(),
            "gateway accepted an oversized tunnel message"
        );
        drop(connection);
        drop(socket);
        server.abort();
        assert!(server.await.unwrap_err().is_cancelled());
    }
    enum Rejection {
        Adapter,
        Connection,
        Stream,
    }
    async fn reject_wire(mut wire: Vec<u8>, rejection: Rejection) -> crate::ConnectionSnapshot {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (incoming, mut receiver) = mpsc::channel(1);
        let metrics = Arc::new(ConnectionMetrics::new(false));
        let (result_sender, result_receiver) = oneshot::channel();
        let result_sender = Arc::new(std::sync::Mutex::new(Some(result_sender)));
        let observed = metrics.clone();
        let app = Router::new().route(
            "/",
            get(move |upgrade: WebSocketUpgrade| {
                let incoming = incoming.clone();
                let metrics = observed.clone();
                let sender = result_sender.clone();
                async move {
                    upgrade
                        .read_buffer_size(FRAME_BYTES)
                        .write_buffer_size(FRAME_BYTES)
                        .max_write_buffer_size(MESSAGE_BYTES)
                        .max_message_size(MESSAGE_BYTES)
                        .max_frame_size(MESSAGE_BYTES)
                        .on_upgrade(move |socket| async move {
                            let result = accept_websocket_measured(socket, incoming, metrics).await;
                            if let Some(sender) = sender.lock().unwrap().take() {
                                let _ = sender.send(result);
                            }
                        })
                }
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/"))
            .await
            .unwrap();
        let connection = receiver.recv().await.unwrap().unwrap();
        let codec = tokio::spawn(async move {
            let mut connection = h2::server::Builder::new()
                .max_header_list_size(FRAME_BYTES as u32)
                .max_frame_size(FRAME_BYTES as u32)
                .handshake::<_, Bytes>(connection)
                .await?;
            while let Some(request) = connection.accept().await {
                request?;
            }
            Ok::<_, h2::Error>(())
        });
        let mut preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".to_vec();
        preface.extend([0, 0, 0, 4, 0, 0, 0, 0, 0]);
        preface.append(&mut wire);
        socket
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                preface.into(),
            ))
            .await
            .unwrap();
        if matches!(rejection, Rejection::Stream) {
            let reset = async {
                let mut bytes = Vec::new();
                let mut header_431 = false;
                while let Some(message) = socket.next().await {
                    if let tokio_tungstenite::tungstenite::Message::Binary(chunk) = message.unwrap()
                    {
                        bytes.extend(chunk);
                        assert!(bytes.len() <= 64 * 1024, "unbounded rejection response");
                        while bytes.len() >= 9 {
                            let length = ((bytes[0] as usize) << 16)
                                | ((bytes[1] as usize) << 8)
                                | bytes[2] as usize;
                            if bytes.len() < 9 + length {
                                break;
                            }
                            if bytes[3] == 1 {
                                header_431 = true;
                            }
                            if bytes[3] == 3 && bytes[5..9] == [0, 0, 0, 1] {
                                assert_eq!(
                                    &bytes[9..13],
                                    &[0, 0, 0, 1],
                                    "h2 oversized headers use PROTOCOL_ERROR reset"
                                );
                                assert!(
                                    header_431,
                                    "native h2 did not emit header-list-specific response before reset"
                                );
                                return;
                            }
                            bytes.drain(..9 + length);
                        }
                    }
                }
                panic!("no native header-list stream reset");
            };
            tokio::time::timeout(std::time::Duration::from_secs(2), reset)
                .await
                .unwrap();
            socket.close(None).await.unwrap();
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), result_receiver)
                .await
                .unwrap()
                .unwrap();
            codec.abort();
            let _ = codec.await;
        } else if matches!(rejection, Rejection::Connection) {
            assert!(
                tokio::time::timeout(std::time::Duration::from_secs(2), codec)
                    .await
                    .unwrap()
                    .unwrap()
                    .is_err(),
                "h2 did not reject continuation/header pressure"
            );
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), result_receiver)
                .await
                .unwrap()
                .unwrap();
        } else {
            assert!(
                tokio::time::timeout(std::time::Duration::from_secs(2), result_receiver)
                    .await
                    .unwrap()
                    .unwrap()
                    .is_err()
            );
            assert!(metrics.snapshot().unwrap().rejected);
            codec.abort();
            let _ = codec.await;
        }
        drop(socket);
        server.abort();
        let _ = server.await;
        let snapshot = metrics.snapshot().unwrap();
        assert!(snapshot.envelope_within_limit());
        assert_eq!(snapshot.pipe_owners, 0);
        snapshot
    }
    #[tokio::test]
    async fn oversized_http2_frame_rejected_before_pipe_admission() {
        let mut wire = vec![0, 0x40, 1, 1, 4, 0, 0, 0, 1];
        wire.resize(wire.len() + FRAME_BYTES + 1, 0);
        reject_wire(wire, Rejection::Adapter).await;
    }
    #[tokio::test]
    async fn tiny_http2_control_flood_rejected_at_native_boundary() {
        let mut wire = vec![];
        for _ in 0..2049 {
            wire.extend([0, 0, 0, 4, 0, 0, 0, 0, 0]);
        }
        reject_wire(wire, Rejection::Adapter).await;
    }
    #[tokio::test]
    async fn native_incoming_control_rate_rejects_exact_total_101_and_releases_owners() {
        // reject_wire prepends the client's initial SETTINGS, so these 100
        // additional frames make precisely 101 controls on the physical tunnel.
        let mut wire = vec![];
        for _ in 0..100 {
            wire.extend([0, 0, 0, 4, 1, 0, 0, 0, 0]);
        }
        let snapshot = reject_wire(wire, Rejection::Adapter).await;
        assert_eq!(snapshot.received_control_frames, 100);
        assert_eq!(snapshot.received_frames, 101);
        assert_eq!(snapshot.incoming_control_window.len(), 100);
        assert_eq!(snapshot.rejected_control.unwrap().kind, 4);
        assert!(snapshot.closed);
    }
    #[tokio::test]
    async fn continuation_chain_rejected_by_established_h2_limit() {
        let mut wire = vec![0, 0, 1, 1, 0, 0, 0, 0, 1, 0x82];
        for _ in 0..128 {
            wire.extend([0, 0, 1, 9, 0, 0, 0, 0, 1, 0x82]);
        }
        reject_wire(wire, Rejection::Connection).await;
    }
    #[tokio::test]
    async fn oversized_hpack_header_list_rejected_at_native_boundary() {
        // Small encoded block, oversized decoded list (300 literal x headers).
        let mut block = vec![0x82, 0x86, 0x84, 0x01, 9];
        block.extend(b"localhost");
        for _ in 0..300 {
            block.extend([0, 1, b'x', 32]);
            block.extend([b'a'; 32]);
        }
        let length = block.len();
        let mut wire = vec![0, (length >> 8) as u8, length as u8, 1, 4, 0, 0, 0, 1];
        wire.extend(block);
        reject_wire(wire, Rejection::Stream).await;
    }
}
