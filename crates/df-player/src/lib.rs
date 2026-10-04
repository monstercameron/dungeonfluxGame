//! Player-role presentation mounts for already permitted views.
mod character_creation;
pub use character_creation::PlayerCharacterConnection;
#[cfg(target_arch = "wasm32")]
pub use character_creation::PlayerCharacterScreen;

mod character_sheet;
pub use character_sheet::PlayerSheetConnection;
#[cfg(target_arch = "wasm32")]
pub use character_sheet::PlayerSheetScreen;

mod exploration;
#[cfg(target_arch = "wasm32")]
pub use exploration::{ExplorationMountError, PlayerExploration};
