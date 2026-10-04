//! Player-role presentation mounts for already permitted views.
mod character_creation;
pub use character_creation::PlayerCharacterConnection;
#[cfg(target_arch = "wasm32")]
pub use character_creation::PlayerCharacterScreen;
