//! Pure source-pinned handler selection. Mechanics and source admission are separate owners.
mod command_handler;
mod dispatch;

pub use command_handler::{InvocationError, RulesCommandHandler, stage_handler};
pub use dispatch::{DispatchError, DispatchRegistry, HandlerRegistration, RegistryError};
