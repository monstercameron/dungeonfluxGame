pub mod inbox;

#[cfg(not(target_arch = "wasm32"))]
pub mod submission;
