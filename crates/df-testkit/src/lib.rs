//! Test-only deterministic controls. Use as a development dependency in test suites.
//! Supplied elapsed time, recorded draws and semantics grant no gameplay authority.

mod clocks;
mod provider_fake;
mod replay;

pub use clocks::{ClockDomain, ClockError, ClockSnapshot, VirtualClocks};
pub use provider_fake::{
    FakeFailure, FakeLimits, FakeOutcome, FakeProvider, FakeProviderError, FakeStep, FakeSuccess,
};
pub use replay::{
    DrawRequest, PreparedJobKey, ReplayContext, ReplayError, ReplayLimits, SemanticReplay,
    SuppliedDice,
};
