//! Native composition ownership contract; not a running server or readiness capability.
#![forbid(unsafe_code)]
#![cfg(not(target_arch = "wasm32"))]

pub(crate) mod composition_registry;
pub(crate) mod deployment;
pub(crate) mod shutdown_policy;
