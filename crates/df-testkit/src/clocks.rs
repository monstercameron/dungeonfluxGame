use std::time::Duration;

/// Identifies the independently controlled elapsed-time domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockDomain {
    Logical,
    Presentation,
}

/// A rejected clock change. Neither clock changes on failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockError {
    TimeRegression {
        domain: ClockDomain,
        current: Duration,
        requested: Duration,
    },
    Overflow {
        domain: ClockDomain,
        current: Duration,
        delta: Duration,
    },
}

/// Elapsed durations from two caller-selected fixed origins, never wall timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockSnapshot {
    pub logical: Duration,
    pub presentation: Duration,
}

/// Test-only, owned logical and presentation clocks with explicit advancement.
///
/// No operation reads wall time, waits, spawns work, or advances the other domain.
/// Callers model a paused campaign by withholding logical advancement while
/// driving presentation independently. This fixture chooses no game pause policy,
/// issues no session epoch, and owns no authoritative game state. Recovery with
/// different origins requires a new fixture rather than rewinding an existing one.
#[derive(Debug)]
pub struct VirtualClocks {
    times: ClockSnapshot,
}

impl VirtualClocks {
    /// Initializes independent caller-supplied elapsed-time anchors.
    pub fn new(logical: Duration, presentation: Duration) -> Self {
        Self {
            times: ClockSnapshot {
                logical,
                presentation,
            },
        }
    }

    pub fn snapshot(&self) -> ClockSnapshot {
        self.times
    }

    /// Advances logical elapsed time only, rejecting overflow without mutation.
    pub fn advance_logical_by(&mut self, delta: Duration) -> Result<Duration, ClockError> {
        advance_by(&mut self.times.logical, delta, ClockDomain::Logical)
    }

    /// Advances presentation elapsed time only, rejecting overflow without mutation.
    pub fn advance_presentation_by(&mut self, delta: Duration) -> Result<Duration, ClockError> {
        advance_by(
            &mut self.times.presentation,
            delta,
            ClockDomain::Presentation,
        )
    }

    /// Sets logical elapsed time to a monotonic value; an equal value is a no-op.
    pub fn advance_logical_to(&mut self, requested: Duration) -> Result<Duration, ClockError> {
        advance_to(&mut self.times.logical, requested, ClockDomain::Logical)
    }

    /// Sets presentation elapsed time to a monotonic value; an equal value is a no-op.
    pub fn advance_presentation_to(&mut self, requested: Duration) -> Result<Duration, ClockError> {
        advance_to(
            &mut self.times.presentation,
            requested,
            ClockDomain::Presentation,
        )
    }
}

fn advance_by(
    current: &mut Duration,
    delta: Duration,
    domain: ClockDomain,
) -> Result<Duration, ClockError> {
    let requested = current.checked_add(delta).ok_or(ClockError::Overflow {
        domain,
        current: *current,
        delta,
    })?;
    *current = requested;
    Ok(requested)
}

fn advance_to(
    current: &mut Duration,
    requested: Duration,
    domain: ClockDomain,
) -> Result<Duration, ClockError> {
    if requested < *current {
        return Err(ClockError::TimeRegression {
            domain,
            current: *current,
            requested,
        });
    }
    *current = requested;
    Ok(requested)
}
