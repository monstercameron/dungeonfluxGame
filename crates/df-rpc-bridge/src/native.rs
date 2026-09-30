use crate::{FRAME_BYTES, MESSAGE_BYTES, RECEIVE_BYTES};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf},
    sync::mpsc,
};

/// Connection owned by tonic. Dropping it releases the associated WebSocket pump.
pub struct TunnelStream(DuplexStream);
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
    let (grpc, tunnel) = tokio::io::duplex(RECEIVE_BYTES);
    incoming
        .try_send(Ok(TunnelStream(grpc)))
        .map_err(|_| io::Error::other("connection capacity exhausted"))?;
    let (mut outgoing, mut incoming_socket) = socket.split();
    let (mut reader, mut writer) = tokio::io::split(tunnel);
    let receive = async {
        while let Some(message) = incoming_socket.next().await {
            match message.map_err(io::Error::other)? {
                Message::Binary(bytes) if bytes.len() <= MESSAGE_BYTES => {
                    writer.write_all(&bytes).await?
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
            outgoing
                .send(Message::Binary(buffer[..count].to_vec().into()))
                .await
                .map_err(io::Error::other)?;
        }
    };
    tokio::select! { result = receive => result, result = send => result }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, extract::WebSocketUpgrade, routing::get};
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
}
