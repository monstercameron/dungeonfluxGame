#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    df_tools::fixture::serve().await
}
#[cfg(target_arch = "wasm32")]
fn main() {}
