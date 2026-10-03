//! Native HTTP/2 bytes carried unchanged over bounded binary WebSocket messages.
//! Browser I/O and executor stay local; generated tonic clients use `BrowserChannel`.
pub const FRAME_BYTES: usize = 16 * 1024;
pub const MESSAGE_BYTES: usize = 256 * 1024;
pub const RECEIVE_BYTES: usize = 1024 * 1024;
pub const RPC_MESSAGE_BYTES: usize = 64 * 1024;
pub const CONCURRENT_STREAMS: u32 = 8;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::{BrowserChannel, BrowserConnection};
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{
    AdmissionError, NativeAdmission, NativeConnectionPermit, NativeIncoming, TunnelStream,
    accept_websocket, accept_websocket_measured,
};

mod resources;
pub use resources::{BufferSnapshot, ConnectionMetrics, ConnectionSnapshot, CreditSnapshot};

#[cfg(any(target_arch = "wasm32", test))]
mod upload;
