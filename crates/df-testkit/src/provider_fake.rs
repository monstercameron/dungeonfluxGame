use std::{collections::VecDeque, fmt};

use df_provider_api::{CheckedRequest, RequestError, RequestOwnerState, RequestUsage};
use df_types::Usage;

/// Explicit bounds for a single test script. No provider capacity is implied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeLimits {
    pub maximum_steps: usize,
    pub maximum_request_bytes: usize,
    pub maximum_response_bytes: usize,
}

/// Test-only failure classes; production adapters own their own error mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakeFailure {
    Rejected,
    RateLimited,
    Transport,
    InvalidResponse,
}

/// One terminal scripted result. Usage is the supplied actual quantity and can
/// be nonzero after failure/cancellation or greater than the declared request.
#[derive(Eq, PartialEq)]
pub enum FakeOutcome {
    Success { bytes: Vec<u8>, usage: Usage },
    Failure { kind: FakeFailure, usage: Usage },
    Cancelled { usage: Usage },
}

impl fmt::Debug for FakeOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success { usage, .. } => formatter
                .debug_struct("Success")
                .field("usage", usage)
                .finish_non_exhaustive(),
            Self::Failure { kind, usage } => formatter
                .debug_struct("Failure")
                .field("kind", kind)
                .field("usage", usage)
                .finish(),
            Self::Cancelled { usage } => formatter
                .debug_struct("Cancelled")
                .field("usage", usage)
                .finish(),
        }
    }
}

/// Complete successful response from one consumed fixture step.
#[derive(Eq, PartialEq)]
pub struct FakeSuccess {
    pub bytes: Vec<u8>,
    pub usage: Usage,
}

impl fmt::Debug for FakeSuccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FakeSuccess")
            .field("usage", &self.usage)
            .finish_non_exhaustive()
    }
}

impl FakeOutcome {
    fn usage(&self) -> Usage {
        match self {
            Self::Success { usage, .. }
            | Self::Failure { usage, .. }
            | Self::Cancelled { usage } => *usage,
        }
    }
}

/// One exact request/terminal result pair, supplied by the test owner.
/// Matching bytes and counters does not qualify source, audience or provider policy.
pub struct FakeStep {
    pub request_bytes: Vec<u8>,
    pub declared_usage: RequestUsage,
    pub outcome: FakeOutcome,
}

/// Structural or current-owner refusals never consume a scripted step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakeProviderError {
    InvalidLimits,
    Capacity,
    UnitMismatch,
    Request(RequestError),
    PayloadMismatch,
    DeclaredUsageMismatch,
    ProviderFailure { kind: FakeFailure, usage: Usage },
    Cancelled { usage: Usage },
    Exhausted,
    Unused { remaining: usize },
}

/// Bounded, ordered, one-shot provider fixture over the canonical checked request.
/// It never dispatches or changes the request's pinned execution mode.
pub struct FakeProvider {
    steps: VecDeque<FakeStep>,
    consumed: usize,
}

impl FakeProvider {
    pub fn new(steps: Vec<FakeStep>, limits: FakeLimits) -> Result<Self, FakeProviderError> {
        if limits.maximum_steps == 0
            || limits.maximum_request_bytes == 0
            || limits.maximum_response_bytes == 0
        {
            return Err(FakeProviderError::InvalidLimits);
        }
        if steps.len() > limits.maximum_steps {
            return Err(FakeProviderError::Capacity);
        }
        for step in &steps {
            if step.request_bytes.len() > limits.maximum_request_bytes
                || matches!(&step.outcome, FakeOutcome::Success { bytes, .. } if bytes.len() > limits.maximum_response_bytes)
            {
                return Err(FakeProviderError::Capacity);
            }
            if step.declared_usage.usage().unit() != step.outcome.usage().unit() {
                return Err(FakeProviderError::UnitMismatch);
            }
        }
        Ok(Self {
            steps: steps.into(),
            consumed: 0,
        })
    }

    pub fn consumed(&self) -> usize {
        self.consumed
    }

    pub fn remaining(&self) -> usize {
        self.steps.len()
    }

    /// Recheck current ownership before comparing payload/counters. A stale,
    /// cancelled or expired request cannot consume the next terminal result.
    /// A matched provider failure or terminal cancellation consumes its step and
    /// returns a typed error carrying the supplied partial usage.
    pub fn take<Semantic: Eq>(
        &mut self,
        request: &CheckedRequest<Semantic>,
        owner: RequestOwnerState<'_, Semantic>,
    ) -> Result<FakeSuccess, FakeProviderError> {
        request
            .validate_current(owner)
            .map_err(FakeProviderError::Request)?;
        let step = self.steps.front().ok_or(FakeProviderError::Exhausted)?;
        if request.payload() != step.request_bytes {
            return Err(FakeProviderError::PayloadMismatch);
        }
        if request.usage() != step.declared_usage {
            return Err(FakeProviderError::DeclaredUsageMismatch);
        }
        let step = self.steps.pop_front().ok_or(FakeProviderError::Exhausted)?;
        self.consumed += 1;
        match step.outcome {
            FakeOutcome::Success { bytes, usage } => Ok(FakeSuccess { bytes, usage }),
            FakeOutcome::Failure { kind, usage } => {
                Err(FakeProviderError::ProviderFailure { kind, usage })
            }
            FakeOutcome::Cancelled { usage } => Err(FakeProviderError::Cancelled { usage }),
        }
    }

    pub fn finish(&self) -> Result<(), FakeProviderError> {
        match self.remaining() {
            0 => Ok(()),
            remaining => Err(FakeProviderError::Unused { remaining }),
        }
    }
}
