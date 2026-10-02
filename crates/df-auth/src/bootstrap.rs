/// Maximum number of synchronous atomic allocation calls in one bootstrap invocation.
pub const MAX_BOOTSTRAP_ATTEMPTS: u8 = 16;

/// The caller requested an attempt bound outside `1..=MAX_BOOTSTRAP_ATTEMPTS`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidBootstrapLimit;

/// The durable allocation owner reports the result of one atomic bootstrap operation.
///
/// Only `Retryable` permits an automatic retry. The owner must know that retrying
/// the same scoped key and fingerprint cannot create a second allocation. A lost
/// commit acknowledgement is `Indeterminate`, never a fabricated refusal.
pub enum BootstrapStoreOutcome<Allocation, Error> {
    Committed {
        allocation: Allocation,
        replayed: bool,
    },
    Retryable(Error),
    Indeterminate(Error),
    Conflict,
    Expired,
    Denied,
}

/// Caller-owned atomic guest allocation and deduplication boundary.
///
/// `Scope`, `Key` and `Fingerprint` are validated by their actual owner. Trace
/// context and a client-selected role cannot mint any of them. The key exists
/// before a principal does; it is not a session or member identifier.
///
/// An implementation must atomically compare the scoped key and canonical
/// fingerprint, allocate the principal and credential grant at most once, and
/// retain their committed result. A matching retry returns that same allocation;
/// another fingerprint returns `Conflict`. Retired or expired keys return
/// `Expired`, even after their full result is no longer retained. Missing receipt
/// data alone must never authorize another allocation. Calls must have their own
/// bounded synchronous lifetime; this policy does not implement persistence,
/// credential issuance, key entropy or remote I/O deadlines.
pub trait GuestBootstrapStore {
    type Scope;
    type Key;
    type Fingerprint;
    type Allocation;
    type Error;

    fn allocate_or_replay(
        &mut self,
        scope: &Self::Scope,
        key: &Self::Key,
        fingerprint: &Self::Fingerprint,
    ) -> BootstrapStoreOutcome<Self::Allocation, Self::Error>;
}

/// A result acknowledged as committed by the allocation owner.
///
/// The allocation may contain credential material and deliberately has no
/// default diagnostic representation. `attempts` counts actual port calls.
pub struct BootstrapReceipt<Allocation> {
    pub allocation: Allocation,
    pub replayed: bool,
    pub attempts: u8,
}

/// A terminal bootstrap outcome; none of these outcomes acknowledges allocation.
#[derive(Debug, Eq, PartialEq)]
pub enum BootstrapError<Error> {
    Unavailable(Error),
    Indeterminate(Error),
    Conflict,
    Expired,
    Denied,
}

/// Nonsecret outcome of one actual allocation-owner call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapAttemptOutcome {
    Committed,
    Replayed,
    Unavailable,
    Indeterminate,
    Conflict,
    Expired,
    Denied,
}

/// Caller-owned observation adapter for the existing shared instrumentation path.
///
/// The adapter captures the caller's actual operation context and forwards these
/// bounded facts through that path. Neither authentication nor allocation uses
/// this diagnostic port. The policy supplies no keys, fingerprints, principals,
/// credential material or error payloads. The adapter owns bounded emission and
/// visible telemetry-loss handling; this crate installs no logger or SDK.
pub trait BootstrapObserver {
    fn attempt_finished(&mut self, attempt: u8, outcome: BootstrapAttemptOutcome);
}

/// Bounded invocation policy over the single actual atomic allocation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootstrapPolicy {
    maximum_attempts: u8,
}

impl BootstrapPolicy {
    pub fn new(maximum_attempts: u8) -> Result<Self, InvalidBootstrapLimit> {
        if !(1..=MAX_BOOTSTRAP_ATTEMPTS).contains(&maximum_attempts) {
            return Err(InvalidBootstrapLimit);
        }
        Ok(Self { maximum_attempts })
    }

    /// Reuse the exact borrowed scoped key and fingerprint on every attempt.
    ///
    /// No key, principal, credential or fingerprint value is exported in
    /// the observation port. Indeterminate commits stop immediately; an explicit later
    /// caller retry must preserve the original key, not invent a new intent.
    pub fn begin_guest<Store: GuestBootstrapStore>(
        &self,
        store: &mut Store,
        scope: &Store::Scope,
        key: &Store::Key,
        fingerprint: &Store::Fingerprint,
        observer: &mut impl BootstrapObserver,
    ) -> Result<BootstrapReceipt<Store::Allocation>, BootstrapError<Store::Error>> {
        let mut attempts = 0;
        loop {
            // The constructor caps this counter at sixteen before any call.
            attempts += 1;
            match store.allocate_or_replay(scope, key, fingerprint) {
                BootstrapStoreOutcome::Committed {
                    allocation,
                    replayed,
                } => {
                    observer.attempt_finished(
                        attempts,
                        if replayed {
                            BootstrapAttemptOutcome::Replayed
                        } else {
                            BootstrapAttemptOutcome::Committed
                        },
                    );
                    return Ok(BootstrapReceipt {
                        allocation,
                        replayed,
                        attempts,
                    });
                }
                BootstrapStoreOutcome::Retryable(error) => {
                    observer.attempt_finished(attempts, BootstrapAttemptOutcome::Unavailable);
                    if attempts == self.maximum_attempts {
                        return Err(BootstrapError::Unavailable(error));
                    }
                }
                BootstrapStoreOutcome::Indeterminate(error) => {
                    observer.attempt_finished(attempts, BootstrapAttemptOutcome::Indeterminate);
                    return Err(BootstrapError::Indeterminate(error));
                }
                BootstrapStoreOutcome::Conflict => {
                    observer.attempt_finished(attempts, BootstrapAttemptOutcome::Conflict);
                    return Err(BootstrapError::Conflict);
                }
                BootstrapStoreOutcome::Expired => {
                    observer.attempt_finished(attempts, BootstrapAttemptOutcome::Expired);
                    return Err(BootstrapError::Expired);
                }
                BootstrapStoreOutcome::Denied => {
                    observer.attempt_finished(attempts, BootstrapAttemptOutcome::Denied);
                    return Err(BootstrapError::Denied);
                }
            }
        }
    }
}
