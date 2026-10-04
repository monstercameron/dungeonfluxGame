#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("--gameplay-demo") {
        df_tools::gameplay::serve().await
    } else {
        df_tools::fixture::serve().await
    }
}
#[cfg(target_arch = "wasm32")]
fn main() {}
