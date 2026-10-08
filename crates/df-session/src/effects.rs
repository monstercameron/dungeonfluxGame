//! Consumer-owned native port for an already committed effect's dispatch claim.
//! A claim is durable only when its repository implementation confirms it. This
//! crate has no PostgreSQL, protected-journal, commerce, or provider-send adapter.

use df_model::checkpoint::{EffectId, JobId};
use df_observe::OperationContext;
use df_types::{OperationId, RunId};

/// Knowledge retained for one recorded attempt. Dispatching is possibly sent even
/// if its actor or dispatcher loses ownership before observing a socket send.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchKnowledge {
    Dispatching,
    Unknown,
    Completed,
    VerifiedUnsentCanceled,
}

/// The stable attempt and provider key survive owner loss and recovery. The key
/// supports same-attempt supplier lookup, never automatic paid retransmission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchRecord<AttemptId, ReservationId, ProviderKey> {
    pub attempt_id: AttemptId,
    pub reservation_id: ReservationId,
    pub provider_key: ProviderKey,
    pub knowledge: DispatchKnowledge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchLookup<AttemptId, ReservationId, ProviderKey> {
    Missing,
    Pending,
    Recorded(DispatchRecord<AttemptId, ReservationId, ProviderKey>),
}

/// Only Acquired supplies the one newly recorded claim token. AlreadyRecorded
/// provides lookup evidence but never another token capable of deliberate egress.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchClaim<ClaimToken, AttemptId, ReservationId, ProviderKey> {
    Acquired(ClaimToken),
    AlreadyRecorded(DispatchRecord<AttemptId, ReservationId, ProviderKey>),
}

pub type ClaimResult<ClaimToken, AttemptId, ReservationId, ProviderKey> =
    Result<DispatchClaim<ClaimToken, AttemptId, ReservationId, ProviderKey>, EffectRepositoryError>;
pub type LookupResult<AttemptId, ReservationId, ProviderKey> =
    Result<DispatchLookup<AttemptId, ReservationId, ProviderKey>, EffectRepositoryError>;

/// The current actor's exact accepted job identity. A recovered worker may bind
/// a new generation explicitly while keeping the original durable job identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectCompletion {
    pub run: RunId,
    pub operation: OperationId,
    pub job: JobId,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionDisposition {
    Recorded,
    Stale,
}

/// UnresolvedClaim means the write may have committed: inspect the same effect
/// and attempt before any other action. Unavailable does not imply an unsent effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectRepositoryError {
    Unauthorized,
    StaleOwner,
    ExpiredOwner,
    NotPending,
    SpendPermitUnavailable,
    JournalUnconfirmed,
    UnresolvedClaim,
    Unavailable,
}

/// Native persistence port for one effect authority, separate from request waits.
/// Implementations authenticate Scope independently and atomically check a current
/// unexpired owner, Pending committed intent, reservation/spend permit, and confirmed
/// protected-journal permit before recording one Dispatching attempt plus a unique
/// provider key. A lost claim response is UnresolvedClaim, never an unsent verdict.
/// ClaimToken proves only that claim recording was acknowledged. Native egress alone
/// owns sockets and must recheck lease/revocation and journal confirmation directly
/// before sending; an in-flight packet may outlive that check and retain liability.
/// No transaction spans a provider call. A second attempt cannot gain a token for
/// a recorded effect, including after owner loss, timeout, or Unknown outcome.
pub trait EffectRepository {
    type Scope;
    type AttemptId: Eq;
    type OwnerFence: Eq;
    type ReservationId: Eq;
    type ProviderKey: Eq;
    type ClaimToken;

    fn claim_dispatch(
        &mut self,
        scope: &Self::Scope,
        effect_id: EffectId,
        attempt_id: &Self::AttemptId,
        owner_fence: &Self::OwnerFence,
        reservation_id: &Self::ReservationId,
        context: &OperationContext,
    ) -> ClaimResult<Self::ClaimToken, Self::AttemptId, Self::ReservationId, Self::ProviderKey>;

    /// Lookup reads the same authenticated effect identity after uncertainty or
    /// takeover. Pending is valid only when no attempt was ever recorded; neither
    /// Pending nor Missing by itself authorizes replay of an uncertain claim.
    fn inspect_dispatch(
        &mut self,
        scope: &Self::Scope,
        effect_id: EffectId,
        context: &OperationContext,
    ) -> LookupResult<Self::AttemptId, Self::ReservationId, Self::ProviderKey>;

    /// Records effect-job disposition only when the current actor fence, attempt,
    /// originating operation, run, job and generation still match. Stale is inert.
    /// Provider accounting may reconcile an old attempt separately; gameplay still
    /// requires the current actor's SessionRepository decision CAS before publication.
    fn record_completion(
        &mut self,
        scope: &Self::Scope,
        effect_id: EffectId,
        attempt_id: &Self::AttemptId,
        owner_fence: &Self::OwnerFence,
        completion: EffectCompletion,
        context: &OperationContext,
    ) -> Result<CompletionDisposition, EffectRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Finite DESIGN fixture. The booleans stand for external checks, not an
    // authentication issuer, ledger, journal, PostgreSQL adapter or native send.
    struct Fixture {
        effect: EffectId,
        expected: EffectCompletion,
        owner: u8,
        owner_live: bool,
        spend_permit: bool,
        journal_confirmed: bool,
        ambiguous_claim: bool,
        recorded: Option<DispatchRecord<u8, u8, u8>>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                effect: EffectId::from_bytes(&[1; 16]).unwrap(),
                expected: EffectCompletion {
                    run: RunId::from_bytes(&[2; 16]).unwrap(),
                    operation: OperationId::from_bytes(&[3; 16]).unwrap(),
                    job: JobId::from_bytes(&[4; 16]).unwrap(),
                    generation: 1,
                },
                owner: 1,
                owner_live: true,
                spend_permit: true,
                journal_confirmed: true,
                ambiguous_claim: false,
                recorded: None,
            }
        }

        fn context() -> OperationContext {
            OperationContext {
                trace_parent: String::new(),
                build: String::new(),
            }
        }

        fn lose_provider_receipt(&mut self) {
            if let Some(record) = &mut self.recorded {
                record.knowledge = DispatchKnowledge::Unknown;
            }
        }
    }

    impl EffectRepository for Fixture {
        type Scope = u8;
        type AttemptId = u8;
        type OwnerFence = u8;
        type ReservationId = u8;
        type ProviderKey = u8;
        type ClaimToken = u8;

        fn claim_dispatch(
            &mut self,
            scope: &u8,
            effect_id: EffectId,
            attempt_id: &u8,
            owner_fence: &u8,
            reservation_id: &u8,
            _context: &OperationContext,
        ) -> Result<DispatchClaim<u8, u8, u8, u8>, EffectRepositoryError> {
            if *scope != 7 {
                return Err(EffectRepositoryError::Unauthorized);
            }
            if *owner_fence != self.owner {
                return Err(EffectRepositoryError::StaleOwner);
            }
            if !self.owner_live {
                return Err(EffectRepositoryError::ExpiredOwner);
            }
            if effect_id != self.effect {
                return Err(EffectRepositoryError::NotPending);
            }
            if let Some(record) = &self.recorded {
                return Ok(DispatchClaim::AlreadyRecorded(record.clone()));
            }
            if !self.spend_permit {
                return Err(EffectRepositoryError::SpendPermitUnavailable);
            }
            if !self.journal_confirmed {
                return Err(EffectRepositoryError::JournalUnconfirmed);
            }
            self.recorded = Some(DispatchRecord {
                attempt_id: *attempt_id,
                reservation_id: *reservation_id,
                provider_key: 9,
                knowledge: DispatchKnowledge::Dispatching,
            });
            if self.ambiguous_claim {
                return Err(EffectRepositoryError::UnresolvedClaim);
            }
            Ok(DispatchClaim::Acquired(9))
        }

        fn inspect_dispatch(
            &mut self,
            scope: &u8,
            effect_id: EffectId,
            _context: &OperationContext,
        ) -> Result<DispatchLookup<u8, u8, u8>, EffectRepositoryError> {
            if *scope != 7 {
                return Err(EffectRepositoryError::Unauthorized);
            }
            if effect_id != self.effect {
                return Ok(DispatchLookup::Missing);
            }
            Ok(self
                .recorded
                .clone()
                .map_or(DispatchLookup::Pending, DispatchLookup::Recorded))
        }

        fn record_completion(
            &mut self,
            scope: &u8,
            effect_id: EffectId,
            attempt_id: &u8,
            owner_fence: &u8,
            completion: EffectCompletion,
            _context: &OperationContext,
        ) -> Result<CompletionDisposition, EffectRepositoryError> {
            if *scope != 7 {
                return Err(EffectRepositoryError::Unauthorized);
            }
            let Some(record) = &mut self.recorded else {
                return Ok(CompletionDisposition::Stale);
            };
            if effect_id != self.effect
                || *owner_fence != self.owner
                || !self.owner_live
                || *attempt_id != record.attempt_id
                || completion != self.expected
                || !matches!(
                    record.knowledge,
                    DispatchKnowledge::Dispatching | DispatchKnowledge::Unknown
                )
            {
                return Ok(CompletionDisposition::Stale);
            }
            record.knowledge = DispatchKnowledge::Completed;
            Ok(CompletionDisposition::Recorded)
        }
    }

    #[test]
    fn claim_cuts_preserve_possible_send_across_owner_loss_and_unknown() {
        let mut fixture = Fixture::new();
        let context = Fixture::context();
        assert_eq!(
            fixture.inspect_dispatch(&7, fixture.effect, &context),
            Ok(DispatchLookup::Pending)
        );
        assert_eq!(
            fixture.claim_dispatch(&7, fixture.effect, &5, &1, &6, &context),
            Ok(DispatchClaim::Acquired(9))
        );
        fixture.owner_live = false;
        assert_eq!(
            fixture.claim_dispatch(&7, fixture.effect, &8, &1, &6, &context),
            Err(EffectRepositoryError::ExpiredOwner)
        );
        assert_eq!(
            fixture.inspect_dispatch(&7, fixture.effect, &context),
            Ok(DispatchLookup::Recorded(DispatchRecord {
                attempt_id: 5,
                reservation_id: 6,
                provider_key: 9,
                knowledge: DispatchKnowledge::Dispatching
            }))
        );
        fixture.owner = 2;
        fixture.owner_live = true;
        assert_eq!(
            fixture.claim_dispatch(&7, fixture.effect, &8, &1, &6, &context),
            Err(EffectRepositoryError::StaleOwner)
        );
        assert_eq!(
            fixture.claim_dispatch(&7, fixture.effect, &8, &2, &6, &context),
            Ok(DispatchClaim::AlreadyRecorded(DispatchRecord {
                attempt_id: 5,
                reservation_id: 6,
                provider_key: 9,
                knowledge: DispatchKnowledge::Dispatching
            }))
        );
        assert!(matches!(
            fixture.claim_dispatch(&7, fixture.effect, &5, &2, &6, &context),
            Ok(DispatchClaim::AlreadyRecorded(_))
        ));
        fixture.lose_provider_receipt();
        assert_eq!(
            fixture.inspect_dispatch(&7, fixture.effect, &context),
            Ok(DispatchLookup::Recorded(DispatchRecord {
                attempt_id: 5,
                reservation_id: 6,
                provider_key: 9,
                knowledge: DispatchKnowledge::Unknown
            }))
        );
    }

    #[test]
    fn absent_permits_and_ambiguous_claim_never_issue_fresh_token() {
        let context = Fixture::context();
        for (spend, journal, expected) in [
            (false, true, EffectRepositoryError::SpendPermitUnavailable),
            (true, false, EffectRepositoryError::JournalUnconfirmed),
        ] {
            let mut fixture = Fixture::new();
            fixture.spend_permit = spend;
            fixture.journal_confirmed = journal;
            assert_eq!(
                fixture.claim_dispatch(&7, fixture.effect, &5, &1, &6, &context),
                Err(expected)
            );
            assert_eq!(
                fixture.inspect_dispatch(&7, fixture.effect, &context),
                Ok(DispatchLookup::Pending)
            );
        }
        let mut fixture = Fixture::new();
        fixture.ambiguous_claim = true;
        assert_eq!(
            fixture.claim_dispatch(&7, fixture.effect, &5, &1, &6, &context),
            Err(EffectRepositoryError::UnresolvedClaim)
        );
        assert_eq!(
            fixture.inspect_dispatch(&7, fixture.effect, &context),
            Ok(DispatchLookup::Recorded(DispatchRecord {
                attempt_id: 5,
                reservation_id: 6,
                provider_key: 9,
                knowledge: DispatchKnowledge::Dispatching
            }))
        );
        assert!(matches!(
            fixture.claim_dispatch(&7, fixture.effect, &8, &1, &6, &context),
            Ok(DispatchClaim::AlreadyRecorded(_))
        ));
    }

    #[test]
    fn completion_requires_current_fence_and_exact_attempt_job_identity() {
        let mut fixture = Fixture::new();
        let context = Fixture::context();
        assert_eq!(
            fixture.claim_dispatch(&7, fixture.effect, &5, &1, &6, &context),
            Ok(DispatchClaim::Acquired(9))
        );
        let expected = fixture.expected;
        fixture.owner = 2;
        fixture.expected.generation = 2;
        for (attempt, owner, completion) in [
            (5, 1, expected),
            (8, 2, fixture.expected),
            (5, 2, expected),
            (
                5,
                2,
                EffectCompletion {
                    run: RunId::from_bytes(&[8; 16]).unwrap(),
                    ..fixture.expected
                },
            ),
            (
                5,
                2,
                EffectCompletion {
                    operation: OperationId::from_bytes(&[8; 16]).unwrap(),
                    ..fixture.expected
                },
            ),
            (
                5,
                2,
                EffectCompletion {
                    job: JobId::from_bytes(&[8; 16]).unwrap(),
                    ..fixture.expected
                },
            ),
        ] {
            assert_eq!(
                fixture.record_completion(
                    &7,
                    fixture.effect,
                    &attempt,
                    &owner,
                    completion,
                    &context
                ),
                Ok(CompletionDisposition::Stale)
            );
        }
        assert_eq!(
            fixture.record_completion(&7, fixture.effect, &5, &2, fixture.expected, &context),
            Ok(CompletionDisposition::Recorded)
        );
        assert_eq!(
            fixture.inspect_dispatch(&7, fixture.effect, &context),
            Ok(DispatchLookup::Recorded(DispatchRecord {
                attempt_id: 5,
                reservation_id: 6,
                provider_key: 9,
                knowledge: DispatchKnowledge::Completed
            }))
        );
        assert_eq!(
            fixture.record_completion(&7, fixture.effect, &5, &2, fixture.expected, &context),
            Ok(CompletionDisposition::Stale)
        );
    }
}
