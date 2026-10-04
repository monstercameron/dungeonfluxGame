//! Pure source-pinned handler selection. Mechanics and source admission are separate owners.
mod command_handler;
mod dispatch;

pub use command_handler::{InvocationError, RulesCommandHandler, RulesCommandInput, stage_handler};
pub use dispatch::{DispatchError, DispatchRegistry, HandlerRegistration, RegistryError};

pub mod ability_check;
pub mod current_responses;
pub mod preconditions;
