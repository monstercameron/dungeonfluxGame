//! Persistent browser composition over authorized role presentation views.

#[cfg(target_arch = "wasm32")]
mod role_shell;

#[cfg(target_arch = "wasm32")]
pub use role_shell::{DisplayPhase, PlayerPhase, RoleInput, RolePhase, RoleShell, RoleShellError};
