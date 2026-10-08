#![cfg(target_arch = "wasm32")]

use df_rpc_bridge::{BrowserChannel, BrowserConnection};
use tower_service::Service;

type BrowserServiceFuture = <BrowserChannel as Service<http::Request<tonic::body::Body>>>::Future;

/// Compile-time witness for the real browser connection and local service future.
///
/// Root's browser fixture supplies the generated-client and runtime execution
/// against this same public channel. This probe only establishes that the
/// channel's concrete associated future can be owned by the single-threaded
/// browser executor.
pub fn spawn_local_browser_service(url: String, request: http::Request<tonic::body::Body>) {
    wasm_bindgen_futures::spawn_local(async move {
        let Ok((connection, mut channel)) = BrowserConnection::connect(&url).await else {
            return;
        };
        let response: BrowserServiceFuture = channel.call(request);
        let _ = response.await;
        connection.close();
    });
}

#[cfg(rpc_local_executor_require_send)]
fn require_send<T: Send>() {}

#[cfg(rpc_local_executor_require_send)]
pub fn require_browser_service_future_to_be_send() {
    require_send::<BrowserServiceFuture>();
}
