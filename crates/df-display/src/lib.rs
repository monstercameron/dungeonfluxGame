//! Shared-display presentation mounts for public views only.
mod character_creation;
#[cfg(target_arch = "wasm32")]
pub use character_creation::DisplayCharacterScreen;

mod exploration;
#[cfg(target_arch = "wasm32")]
pub use exploration::{DisplayExploration, ExplorationMountError};

mod combat;
#[cfg(target_arch = "wasm32")]
pub use combat::{CombatMountError, DisplayCombat};

mod session_overlays;
#[cfg(target_arch = "wasm32")]
pub use session_overlays::{DisplaySessionOverlays, SessionOverlayMountError};

mod join;
pub use join::{DisplayJoinConnection, DisplayJoinInput};
#[cfg(target_arch = "wasm32")]
pub use join::{DisplayJoinError, DisplayJoinScreen};
