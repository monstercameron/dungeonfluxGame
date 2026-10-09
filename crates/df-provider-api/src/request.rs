use std::{fmt, time::Duration};

use df_model::checkpoint::{AssetRequestKey, Basis, ExecutionMode, JobId};
use df_types::{LocaleTag, OperationId, Usage};

/// Caller-selected work bounds, without default capacity or spend admission.
/// The byte bound is positive. Zero other bounds disable that dimension rather
/// than admitting unlimited work. Duration is work duration, not the job deadline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestLimits {
    max_bytes: usize,
    max_tokens: u128,
    max_duration: Duration,
    max_usage: Usage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidRequestLimits;

impl RequestLimits {
    pub fn new(
        max_bytes: usize,
        max_tokens: u128,
        max_duration: Duration,
        max_usage: Usage,
    ) -> Result<Self, InvalidRequestLimits> {
        if max_bytes == 0 {
            return Err(InvalidRequestLimits);
        }
        Ok(Self {
            max_bytes,
            max_tokens,
            max_duration,
            max_usage,
        })
    }

    pub fn max_bytes(self) -> usize {
        self.max_bytes
    }

    pub fn max_tokens(self) -> u128 {
        self.max_tokens
    }

    pub fn max_duration(self) -> Duration {
        self.max_duration
    }

    pub fn max_usage(self) -> Usage {
        self.max_usage
    }
}

/// Declared input/output work counters; not a tokenizer, invoice or usage receipt.
/// The modality owner derives these counters from its actual contracted request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestUsage {
    tokens: u128,
    duration: Duration,
    usage: Usage,
}

impl RequestUsage {
    pub fn new(tokens: u128, duration: Duration, usage: Usage) -> Self {
        Self {
            tokens,
            duration,
            usage,
        }
    }

    pub fn tokens(self) -> u128 {
        self.tokens
    }

    pub fn duration(self) -> Duration {
        self.duration
    }

    pub fn usage(self) -> Usage {
        self.usage
    }
}

/// Existing canonical identities for one owner-admitted job, never credentials.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestIdentity {
    pub basis: Basis,
    pub job: JobId,
    pub operation: OperationId,
    pub generation: u64,
}

/// Pinned owner-supplied job binding, kept independent of an RPC receipt wait.
///
/// `Semantic` is the existing caller's complete, bounded semantic key, including
/// the applicable source/access/rights, audience, contract/output profile, locale
/// and quote revisions. Equality checks cannot establish those rights or approve
/// a quote. This boundary does not derive, hash, shorten or replace that schema.
/// `deadline` is a positive elapsed duration from this job's owner admission.
pub struct RequestBinding<Semantic> {
    pub identity: RequestIdentity,
    pub semantic_basis: Semantic,
    pub mode: ExecutionMode,
    pub deadline: Duration,
}

impl RequestBinding<AssetRequestKey> {
    /// Forms a recording/cache identity from the complete asset key and its locale.
    ///
    /// The caller supplies the checked locale of the admitted output. The canonical
    /// asset key is preserved in full; locale is mandatory and has no default.
    /// This binds identity, not rights, publication authority or execution readiness.
    /// Checked admission and current-owner revalidation still apply to the result.
    pub fn with_recording_locale(
        self,
        locale: LocaleTag,
    ) -> RequestBinding<(AssetRequestKey, LocaleTag)> {
        RequestBinding {
            identity: self.identity,
            semantic_basis: (self.semantic_basis, locale),
            mode: self.mode,
            deadline: self.deadline,
        }
    }
}

impl<Semantic> fmt::Debug for RequestBinding<Semantic> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestBinding")
            .finish_non_exhaustive()
    }
}

/// Current owner observation at construction or consumption, not trusted auth.
///
/// Only explicit owner cancellation/run termination sets `cancelled`; losing a
/// receipt wait does not. The caller supplies monotonic elapsed time measured
/// from the same admission as `deadline`, and rechecks current authority at egress.
/// An unavailable or not yet qualified semantic policy supplies `current: None`;
/// missing policy cannot become an empty/default binding or a usable request.
pub struct RequestOwnerState<'a, Semantic> {
    pub current: Option<&'a RequestBinding<Semantic>>,
    pub elapsed: Duration,
    pub cancelled: bool,
}

impl<Semantic> fmt::Debug for RequestOwnerState<'_, Semantic> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestOwnerState")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestIdentityField {
    Session,
    Run,
    Revision,
    Job,
    Operation,
    Generation,
    Mode,
    Deadline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestLimit {
    Bytes,
    Tokens,
    Duration,
    Units,
}

/// Payload-free diagnostic facts for the caller's shared observation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestError {
    InvalidGeneration,
    InvalidDeadline,
    Cancelled,
    DeadlineExceeded,
    CurrentBasisUnavailable,
    IdentityMismatch(RequestIdentityField),
    SemanticBasisMismatch,
    UnitMismatch,
    LimitExceeded(RequestLimit),
    AllocationUnavailable,
}

/// Owned bounded bytes, declared work and the unchanged caller's admitted binding.
///
/// This proves bounds and binding equality at an explicit observation, never
/// semantic qualification, authorization or provider readiness. `Semantic` has
/// its own caller-owned shape/retention bounds; `max_bytes` covers payload only.
/// No raw prompt, audio, private basis or quote enters the default Debug output.
pub struct CheckedRequest<Semantic> {
    binding: RequestBinding<Semantic>,
    payload: Vec<u8>,
    usage: RequestUsage,
    limits: RequestLimits,
}

impl<Semantic> fmt::Debug for CheckedRequest<Semantic> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedRequest")
            .finish_non_exhaustive()
    }
}

impl<Semantic: Eq> CheckedRequest<Semantic> {
    /// Checks lifetime/binding, then unit equality, bytes, tokens, duration and
    /// units, before allocating. Empty bytes/zero counters remain structurally
    /// valid; modality-specific semantic requirements are enforced by their owner.
    /// This accepts pinned live/prepared/replay inputs equally, but starts no call,
    /// reservation, timer or retry and never changes an admitted execution mode.
    pub fn new(
        binding: RequestBinding<Semantic>,
        payload: &[u8],
        usage: RequestUsage,
        limits: RequestLimits,
        owner: RequestOwnerState<'_, Semantic>,
    ) -> Result<Self, RequestError> {
        validate_binding(&binding, owner)?;
        if usage.usage.unit() != limits.max_usage.unit() {
            return Err(RequestError::UnitMismatch);
        }
        if payload.len() > limits.max_bytes {
            return Err(RequestError::LimitExceeded(RequestLimit::Bytes));
        }
        if usage.tokens > limits.max_tokens {
            return Err(RequestError::LimitExceeded(RequestLimit::Tokens));
        }
        if usage.duration > limits.max_duration {
            return Err(RequestError::LimitExceeded(RequestLimit::Duration));
        }
        if usage.usage.quantity() > limits.max_usage.quantity() {
            return Err(RequestError::LimitExceeded(RequestLimit::Units));
        }
        let mut owned_payload = Vec::new();
        owned_payload
            .try_reserve_exact(payload.len())
            .map_err(|_| RequestError::AllocationUnavailable)?;
        owned_payload.extend_from_slice(payload);
        Ok(Self {
            binding,
            payload: owned_payload,
            usage,
            limits,
        })
    }

    /// Rechecks queued work without mutation. Cancellation/deadline do not undo
    /// committed decisions or release sent/unknown supplier liability. There is
    /// no provider-send permit here; native egress still owns durable admission.
    pub fn validate_current(
        &self,
        owner: RequestOwnerState<'_, Semantic>,
    ) -> Result<(), RequestError> {
        validate_binding(&self.binding, owner)
    }
}

impl<Semantic> CheckedRequest<Semantic> {
    pub fn binding(&self) -> &RequestBinding<Semantic> {
        &self.binding
    }

    /// Raw bytes for the authorized consumer; never log them by default.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn usage(&self) -> RequestUsage {
        self.usage
    }

    pub fn limits(&self) -> RequestLimits {
        self.limits
    }

    pub fn into_parts(self) -> (RequestBinding<Semantic>, Vec<u8>, RequestUsage) {
        (self.binding, self.payload, self.usage)
    }
}

fn validate_binding<Semantic: Eq>(
    binding: &RequestBinding<Semantic>,
    owner: RequestOwnerState<'_, Semantic>,
) -> Result<(), RequestError> {
    if binding.identity.generation == 0 {
        return Err(RequestError::InvalidGeneration);
    }
    if binding.deadline.is_zero() {
        return Err(RequestError::InvalidDeadline);
    }
    if owner.cancelled {
        return Err(RequestError::Cancelled);
    }
    if owner.elapsed >= binding.deadline {
        return Err(RequestError::DeadlineExceeded);
    }
    let current = owner.current.ok_or(RequestError::CurrentBasisUnavailable)?;
    let expected = &current.identity;
    let candidate = &binding.identity;
    let mismatch = if candidate.basis.session != expected.basis.session {
        Some(RequestIdentityField::Session)
    } else if candidate.basis.run != expected.basis.run {
        Some(RequestIdentityField::Run)
    } else if candidate.basis.revision != expected.basis.revision {
        Some(RequestIdentityField::Revision)
    } else if candidate.job != expected.job {
        Some(RequestIdentityField::Job)
    } else if candidate.operation != expected.operation {
        Some(RequestIdentityField::Operation)
    } else if candidate.generation != expected.generation {
        Some(RequestIdentityField::Generation)
    } else if binding.mode != current.mode {
        Some(RequestIdentityField::Mode)
    } else if binding.deadline != current.deadline {
        Some(RequestIdentityField::Deadline)
    } else {
        None
    };
    if let Some(field) = mismatch {
        return Err(RequestError::IdentityMismatch(field));
    }
    if binding.semantic_basis != current.semantic_basis {
        return Err(RequestError::SemanticBasisMismatch);
    }
    Ok(())
}
