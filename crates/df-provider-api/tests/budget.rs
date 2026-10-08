//! Synthetic consumer fixture. The only capacity calculation is df-commerce's
//! production pure proposal; this fixture has no durable ledger or send authority.

use df_commerce::{
    SpendConsent, SpendCounter, SpendOperationObservation, SpendOperationStatus, SpendRefusal,
    SpendRequest, SpendScope, SpendSnapshot, propose_spend,
};
use df_model::checkpoint::{ExecutionMode, JobId};
use df_provider_api::{BudgetAdmission, BudgetMutation, BudgetStore};
use df_types::{Currency, Money, OperationId, Usage, UsageUnit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Reservation {
    scope: u64,
    grant_revision: u64,
    quote: u64,
    job: JobId,
    operation: OperationId,
    mode: ExecutionMode,
    maximum_usage: Usage,
    maximum_supplier_liability: Money,
    revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Commerce(SpendRefusal),
    Scope,
    Quote,
    Grant,
    NonLive,
    Reservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Storage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Acknowledgement {
    Current,
    Pending,
    Unknown,
    FailedBeforeCommit,
}

struct CommerceFixture {
    snapshot: SpendSnapshot,
    requested_revision: u64,
    consent: SpendConsent,
    observation: SpendOperationStatus,
    receipt: Option<Reservation>,
    acknowledgement: Acknowledgement,
}

impl CommerceFixture {
    fn new() -> Self {
        Self {
            snapshot: SpendSnapshot {
                currency: currency(),
                revision: 9,
                counters: [SpendCounter {
                    used: money(20),
                    limit: money(100),
                }; 6],
            },
            requested_revision: 9,
            consent: SpendConsent {
                revision: 7,
                maximum: money(80),
            },
            observation: SpendOperationStatus::Unseen,
            receipt: None,
            acknowledgement: Acknowledgement::Current,
        }
    }
}

impl BudgetStore for CommerceFixture {
    type Scope = u64;
    type Grant = SpendConsent;
    type Quote = u64;
    type Reservation = Reservation;
    type DispatchClaim = OperationId;
    type DispatchPermit = ();
    type Settlement = Money;
    type SettlementReceipt = Money;
    type LedgerView = SpendSnapshot;
    type Refusal = Refusal;
    type Failure = Failure;

    fn inspect(&mut self, scope: &Self::Scope) -> Result<Self::LedgerView, Self::Failure> {
        if *scope != 1 {
            return Err(Failure::Storage);
        }
        Ok(self.snapshot)
    }

    fn reserve(
        &mut self,
        admission: BudgetAdmission<'_, Self::Scope, Self::Grant, Self::Quote>,
    ) -> BudgetMutation<Self::Reservation, Self::Refusal, Self::Failure> {
        if *admission.scope != 1 {
            return BudgetMutation::Refused(Refusal::Scope);
        }
        if *admission.quote != 11 {
            return BudgetMutation::Refused(Refusal::Quote);
        }
        if *admission.grant != self.consent {
            return BudgetMutation::Refused(Refusal::Grant);
        }
        if admission.mode != ExecutionMode::Live {
            return BudgetMutation::Refused(Refusal::NonLive);
        }
        if self.acknowledgement == Acknowledgement::FailedBeforeCommit {
            return BudgetMutation::Failed(Failure::Storage);
        }

        let request = SpendRequest {
            operation: admission.operation,
            expected_revision: if self.observation == SpendOperationStatus::Unseen {
                self.requested_revision
            } else {
                // Native deduplication resolves a retained operation before comparing
                // it with the current ledger revision.
                self.snapshot.revision
            },
            expected_consent_revision: admission.grant.revision,
            maximum_supplier_liability: admission.maximum_supplier_liability,
        };
        let observation = SpendOperationObservation {
            operation: admission.operation,
            status: self.observation,
        };
        match propose_spend(&self.snapshot, *admission.grant, request, observation) {
            Ok(proposal) => {
                let reservation = Reservation {
                    scope: *admission.scope,
                    grant_revision: admission.grant.revision,
                    quote: *admission.quote,
                    job: admission.job,
                    operation: admission.operation,
                    mode: admission.mode,
                    maximum_usage: admission.maximum_usage,
                    maximum_supplier_liability: admission.maximum_supplier_liability,
                    revision: proposal.next.revision,
                };
                self.snapshot = proposal.next;
                self.receipt = Some(reservation);
                self.observation = SpendOperationStatus::Recorded;
                match self.acknowledgement {
                    Acknowledgement::Current => BudgetMutation::Committed {
                        result: reservation,
                        replayed: false,
                    },
                    Acknowledgement::Pending => BudgetMutation::Pending,
                    Acknowledgement::Unknown => {
                        self.observation = SpendOperationStatus::Unknown;
                        BudgetMutation::Unknown
                    }
                    Acknowledgement::FailedBeforeCommit => unreachable!(),
                }
            }
            Err(SpendRefusal::DuplicateOperation) => {
                let Some(receipt) = self.receipt else {
                    return BudgetMutation::Unknown;
                };
                if receipt.scope != *admission.scope
                    || receipt.grant_revision != admission.grant.revision
                    || receipt.quote != *admission.quote
                    || receipt.job != admission.job
                    || receipt.operation != admission.operation
                    || receipt.mode != admission.mode
                    || receipt.maximum_usage != admission.maximum_usage
                    || receipt.maximum_supplier_liability != admission.maximum_supplier_liability
                {
                    return BudgetMutation::Refused(Refusal::Reservation);
                }
                if self.acknowledgement == Acknowledgement::Pending {
                    return BudgetMutation::Pending;
                }
                BudgetMutation::Committed {
                    result: receipt,
                    replayed: true,
                }
            }
            Err(SpendRefusal::UnknownOperation) => BudgetMutation::Unknown,
            Err(error) => BudgetMutation::Refused(Refusal::Commerce(error)),
        }
    }

    fn claim_dispatch(
        &mut self,
        reservation: &Self::Reservation,
        _claim: &Self::DispatchClaim,
    ) -> BudgetMutation<Self::DispatchPermit, Self::Refusal, Self::Failure> {
        if self.receipt != Some(*reservation) {
            return BudgetMutation::Refused(Refusal::Reservation);
        }
        if self.observation == SpendOperationStatus::Unknown {
            return BudgetMutation::Unknown;
        }
        BudgetMutation::Pending
    }

    fn settle(
        &mut self,
        _reservation: &Self::Reservation,
        _settlement: &Self::Settlement,
    ) -> BudgetMutation<Self::SettlementReceipt, Self::Refusal, Self::Failure> {
        BudgetMutation::Pending
    }

    fn reconcile(
        &mut self,
        _reservation: &Self::Reservation,
    ) -> BudgetMutation<Self::SettlementReceipt, Self::Refusal, Self::Failure> {
        BudgetMutation::Unknown
    }
}

fn currency() -> Currency {
    Currency::parse("USD").unwrap()
}

fn money(micros: u128) -> Money {
    Money::new(currency(), micros)
}

fn operation() -> OperationId {
    OperationId::from_bytes(&[1; 16]).unwrap()
}

fn job() -> JobId {
    JobId::from_bytes(&[2; 16]).unwrap()
}

fn admission<'a>(
    scope: &'a u64,
    grant: &'a SpendConsent,
    quote: &'a u64,
) -> BudgetAdmission<'a, u64, SpendConsent, u64> {
    BudgetAdmission {
        scope,
        grant,
        quote,
        job: job(),
        operation: operation(),
        mode: ExecutionMode::Live,
        maximum_usage: Usage::new(4, UsageUnit::Token),
        maximum_supplier_liability: money(80),
    }
}

#[test]
fn one_commerce_proposal_controls_all_scopes_and_original_identity() {
    let mut store = CommerceFixture::new();
    let scope = 1;
    let grant = store.consent;
    let quote = 11;
    let accepted = match store.reserve(admission(&scope, &grant, &quote)) {
        BudgetMutation::Committed {
            result,
            replayed: false,
        } => result,
        _ => panic!("expected canonical reservation"),
    };
    assert_eq!(accepted.job, job());
    assert_eq!(accepted.operation, operation());
    assert_eq!(accepted.mode, ExecutionMode::Live);
    assert_eq!(accepted.maximum_usage, Usage::new(4, UsageUnit::Token));
    assert_eq!(accepted.maximum_supplier_liability, money(80));
    assert_eq!(accepted.grant_revision, 7);
    assert_eq!(accepted.quote, 11);
    assert_eq!(accepted.revision, 10);
    assert!(
        store
            .inspect(&scope)
            .unwrap()
            .counters
            .iter()
            .all(|counter| counter.used == money(100))
    );
    assert!(matches!(
        store.reserve(admission(&scope, &grant, &quote)),
        BudgetMutation::Committed {
            result,
            replayed: true
        } if result == accepted
    ));
    assert_eq!(store.snapshot.revision, 10);
    assert!(matches!(
        store.claim_dispatch(&accepted, &operation()),
        BudgetMutation::Pending
    ));
}

#[test]
fn every_exhausted_hierarchy_row_refuses_without_partial_reservation() {
    let scopes = [
        SpendScope::PlatformDay,
        SpendScope::PlatformMonth,
        SpendScope::SupplierAccount,
        SpendScope::TenantPayer,
        SpendScope::Campaign,
        SpendScope::Job,
    ];
    for (index, expected_scope) in scopes.into_iter().enumerate() {
        let mut store = CommerceFixture::new();
        store.snapshot.counters[index].limit = money(99);
        let before = store.snapshot;
        let grant = store.consent;
        assert!(matches!(
            store.reserve(admission(&1, &grant, &11)),
            BudgetMutation::Refused(Refusal::Commerce(SpendRefusal::BudgetExceeded(scope)))
                if scope == expected_scope
        ));
        assert_eq!(store.snapshot, before);
        assert_eq!(store.receipt, None);
    }
}

#[test]
fn stale_ledger_observation_refuses_before_any_scope_changes() {
    let mut store = CommerceFixture::new();
    store.requested_revision = 8;
    let before = store.snapshot;
    let grant = store.consent;
    assert!(matches!(
        store.reserve(admission(&1, &grant, &11)),
        BudgetMutation::Refused(Refusal::Commerce(SpendRefusal::StaleRevision))
    ));
    assert_eq!(store.snapshot, before);
    assert_eq!(store.receipt, None);
}

#[test]
fn changed_replay_and_unknown_acknowledgement_never_create_a_new_paid_attempt() {
    let mut store = CommerceFixture::new();
    let grant = store.consent;
    let scope = 1;
    let quote = 11;
    let accepted = match store.reserve(admission(&scope, &grant, &quote)) {
        BudgetMutation::Committed { result, .. } => result,
        _ => panic!("expected original reservation"),
    };
    let mut changed = admission(&scope, &grant, &quote);
    changed.maximum_usage = Usage::new(5, UsageUnit::Token);
    assert!(matches!(
        store.reserve(changed),
        BudgetMutation::Refused(Refusal::Reservation)
    ));
    assert_eq!(store.snapshot.revision, 10);

    let mut uncertain = CommerceFixture::new();
    uncertain.acknowledgement = Acknowledgement::Unknown;
    assert!(matches!(
        uncertain.reserve(admission(&scope, &grant, &quote)),
        BudgetMutation::Unknown
    ));
    let held = uncertain.snapshot;
    let original = uncertain.receipt.unwrap();
    assert!(matches!(
        uncertain.reserve(admission(&scope, &grant, &quote)),
        BudgetMutation::Unknown
    ));
    assert!(matches!(
        uncertain.claim_dispatch(&original, &operation()),
        BudgetMutation::Unknown
    ));
    assert!(matches!(
        uncertain.reconcile(&original),
        BudgetMutation::Unknown
    ));
    assert_eq!(uncertain.snapshot, held);
    assert!(
        held.counters
            .iter()
            .all(|counter| counter.used == money(100))
    );
    assert!(matches!(
        uncertain.claim_dispatch(&accepted, &operation()),
        BudgetMutation::Unknown
    ));
}

#[test]
fn stale_grant_nonlive_mode_and_known_failure_have_distinct_no_commit_outcomes() {
    let mut store = CommerceFixture::new();
    let stale = SpendConsent {
        revision: 6,
        maximum: money(80),
    };
    let initial = store.snapshot;
    assert!(matches!(
        store.reserve(admission(&1, &stale, &11)),
        BudgetMutation::Refused(Refusal::Grant)
    ));
    let grant = store.consent;
    let mut prepared = admission(&1, &grant, &11);
    prepared.mode = ExecutionMode::PreparedOnly;
    assert!(matches!(
        store.reserve(prepared),
        BudgetMutation::Refused(Refusal::NonLive)
    ));
    store.acknowledgement = Acknowledgement::FailedBeforeCommit;
    assert!(matches!(
        store.reserve(admission(&1, &grant, &11)),
        BudgetMutation::Failed(Failure::Storage)
    ));
    assert_eq!(store.snapshot, initial);
    assert_eq!(store.receipt, None);
    assert_eq!(store.inspect(&2), Err(Failure::Storage));
    let missing = Reservation {
        scope: 1,
        grant_revision: grant.revision,
        quote: 11,
        job: job(),
        operation: operation(),
        mode: ExecutionMode::Live,
        maximum_usage: Usage::new(4, UsageUnit::Token),
        maximum_supplier_liability: money(80),
        revision: 10,
    };
    assert!(matches!(
        store.claim_dispatch(&missing, &operation()),
        BudgetMutation::Refused(Refusal::Reservation)
    ));
    store.acknowledgement = Acknowledgement::Pending;
    assert!(matches!(
        store.reserve(admission(&1, &grant, &11)),
        BudgetMutation::Pending
    ));
    assert!(matches!(
        store.reserve(admission(&1, &grant, &11)),
        BudgetMutation::Pending
    ));
    assert_eq!(store.snapshot.revision, 10);
    assert!(store.receipt.is_some());
}
