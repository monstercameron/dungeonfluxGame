//! Test-only deterministic controls. Use as a development dependency in test suites.
//! Supplied elapsed time grants no gameplay authority and never reads a wall clock.

mod clocks;

pub use clocks::{ClockDomain, ClockError, ClockSnapshot, VirtualClocks};
