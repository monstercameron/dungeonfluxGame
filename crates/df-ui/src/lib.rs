//! Shared Rust browser presentation primitives.
//!
//! Labels are already localized plain text. Role layout is presentation only;
//! callers retain server-authorized view, action, focus, and resource ownership.

mod campaign_surface;
#[cfg(target_arch = "wasm32")]
mod campaign_theme;
#[cfg(target_arch = "wasm32")]
mod controls;
mod draft_state;
#[cfg(target_arch = "wasm32")]
mod layout;
mod theme;

#[cfg(target_arch = "wasm32")]
pub use campaign_surface::{CampaignError, CampaignSurface};
pub use campaign_surface::{
    CampaignLimits, CampaignMember, CampaignObjective, CampaignValidationError, CampaignView,
    ConceptScene, ObjectiveState,
};

#[cfg(target_arch = "wasm32")]
pub use controls::{
    ActionView, ControlError, ControlledAction, ControlledTextInput, DraftUpdate, InputFeedback,
    TextInputView,
};
pub use draft_state::{DraftError, MAX_DRAFT_UTF16_UNITS};
#[cfg(target_arch = "wasm32")]
pub use layout::{LayoutRole, LayoutRoot, UiError, action_button, panel, stack, text_input};
pub use theme::ThemeToken;
