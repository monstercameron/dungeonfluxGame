//! Shared Rust browser presentation primitives.
//!
//! Labels are already localized plain text. Role layout is presentation only;
//! callers retain server-authorized view, action, focus, and resource ownership.

#[cfg(target_arch = "wasm32")]
mod layout;
mod theme;

#[cfg(target_arch = "wasm32")]
pub use layout::{LayoutRole, LayoutRoot, UiError, action_button, panel, stack, text_input};
pub use theme::ThemeToken;
