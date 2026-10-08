pub mod inbox;

#[cfg(not(target_arch = "wasm32"))]
pub mod effects;
#[cfg(not(target_arch = "wasm32"))]
pub mod submission;
