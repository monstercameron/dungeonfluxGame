//! Synthetic consumer fixture. The only capacity calculation is df-commerce's
//! production pure proposal; this fixture has no durable ledger or send authority.

use df_commerce::{
    Access, AllowanceGrantId, AllowancePaymentObservation, AllowanceRenewal, AllowanceState,
    AllowanceTerms, ObservationAuthority, PaidInvoice, PaidPeriod, PaymentOutcome, SpendConsent,
    SpendCounter, SpendOperationObservation, SpendOperationStatus, SpendRefusal, SpendRequest,
    SpendScope, SpendSnapshot, SubscriptionObservation, SubscriptionState,
    effective_allowance_terms, propose_allowance_downgrade, propose_allowance_renewal,
    propose_spend,
};
use df_model::checkpoint::{ExecutionMode, JobId};
use df_provider_api::{BudgetAdmission, BudgetMutation, BudgetStore};
use df_types::{
    Currency, LiabilityRate, Money, MoneyError, OperationId, PaidInvoiceId, PriceVersion,
    SubscriptionId, Usage, UsageUnit,
};

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

fn pending_allowance() -> AllowanceState {
    AllowanceState {
        subscription: SubscriptionId::from_bytes(&[8; 16]).unwrap(),
        revision: 7,
        subscription_state: SubscriptionState {
            currency: currency(),
            revision: 3,
            object_revision: 2,
            access: Access::PendingInitial,
            paid_through: 0,
            grace_until: None,
        },
        current_terms: AllowanceTerms {
            price_version: PriceVersion::from_bytes(&[9; 16]).unwrap(),
            supplier_allowance: money(80),
        },
        current_grant: None,
        scheduled_downgrade: None,
    }
}

fn allowance_payment(
    state: AllowanceState,
    invoice: u8,
    period: PaidPeriod,
    terms: AllowanceTerms,
) -> (AllowanceRenewal, AllowancePaymentObservation) {
    let id = AllowanceGrantId {
        subscription: state.subscription,
        invoice: PaidInvoiceId::from_bytes(&[invoice; 16]).unwrap(),
        period,
        price_version: terms.price_version,
    };
    (
        AllowanceRenewal {
            expected_revision: state.revision,
            grant: id,
        },
        AllowancePaymentObservation {
            grant: id,
            subscription: SubscriptionObservation {
                authority: ObservationAuthority::VerifiedLatest,
                expected_revision: state.subscription_state.revision,
                object_revision: state.subscription_state.object_revision + 1,
                outcome: PaymentOutcome::Settled(PaidInvoice {
                    period_start: period.start,
                    paid_through: period.end,
                    selected_allowance: terms.supplier_allowance,
                    allowance_already_recorded: false,
                }),
            },
        },
    )
}

#[test]
fn paid_period_and_downgrade_candidates_feed_the_existing_budget_admission() {
    let pending = pending_allowance();
    let (initial, payment) = allowance_payment(
        pending,
        10,
        PaidPeriod {
            start: 90,
            end: 200,
        },
        pending.current_terms,
    );
    let paid = propose_allowance_renewal(&pending, initial, payment, 100).unwrap();
    let mut store = CommerceFixture::new();
    // Synthetic native owner binds the pure candidate to this existing consumer's
    // supplied consent. This does not qualify a durable grant transaction.
    store.consent = SpendConsent {
        revision: paid.next.revision,
        maximum: paid.grant.unwrap().supplier_allowance,
    };
    let previous_consent = store.consent;
    let mut first_admission = admission(&1, &previous_consent, &11);
    first_admission.maximum_supplier_liability = money(40);
    assert!(matches!(
        store.reserve(first_admission),
        BudgetMutation::Committed {
            replayed: false,
            ..
        }
    ));
    let held = store.snapshot;
    assert!(
        held.counters
            .iter()
            .all(|counter| counter.used == money(60))
    );

    let lower = AllowanceTerms {
        price_version: PriceVersion::from_bytes(&[12; 16]).unwrap(),
        supplier_allowance: money(30),
    };
    let scheduled =
        propose_allowance_downgrade(&paid.next, paid.next.revision, lower, 150).unwrap();
    assert_eq!(scheduled.grant, None);
    assert_eq!(
        effective_allowance_terms(&scheduled.next, 199),
        Ok(paid.next.current_terms)
    );
    assert_eq!(effective_allowance_terms(&scheduled.next, 200), Ok(lower));
    let (request, payment) = allowance_payment(
        scheduled.next,
        13,
        PaidPeriod {
            start: 200,
            end: 400,
        },
        lower,
    );
    let renewed = propose_allowance_renewal(&scheduled.next, request, payment, 200).unwrap();
    let mut next_store = CommerceFixture::new();
    next_store.snapshot = held;
    next_store.requested_revision = held.revision;
    next_store.consent = SpendConsent {
        revision: renewed.next.revision,
        maximum: renewed.grant.unwrap().supplier_allowance,
    };
    assert_eq!(next_store.snapshot, held);
    assert!(matches!(
        next_store.reserve(admission(&1, &previous_consent, &11)),
        BudgetMutation::Refused(Refusal::Grant)
    ));
    let grant = next_store.consent;
    let next_operation = OperationId::from_bytes(&[14; 16]).unwrap();
    let mut above_new_terms = admission(&1, &grant, &11);
    above_new_terms.operation = next_operation;
    above_new_terms.maximum_supplier_liability = money(40);
    assert!(matches!(
        next_store.reserve(above_new_terms),
        BudgetMutation::Refused(Refusal::Commerce(SpendRefusal::CustomerConsentExceeded))
    ));
    assert_eq!(next_store.snapshot, held);
    let mut within_new_terms = admission(&1, &grant, &11);
    within_new_terms.operation = next_operation;
    within_new_terms.maximum_supplier_liability = money(30);
    assert!(matches!(
        next_store.reserve(within_new_terms),
        BudgetMutation::Committed {
            replayed: false,
            ..
        }
    ));
    assert!(
        next_store
            .snapshot
            .counters
            .iter()
            .all(|counter| counter.used == money(90))
    );
    let after = next_store.snapshot;
    assert!(propose_allowance_renewal(&renewed.next, request, payment, 200).is_err());
    assert_eq!(next_store.snapshot, after);
    assert_eq!(store.snapshot, held);
}

#[test]
fn renewal_and_clock_expiry_cannot_release_unknown_supplier_exposure() {
    let pending = pending_allowance();
    let (request, payment) = allowance_payment(
        pending,
        10,
        PaidPeriod {
            start: 90,
            end: 200,
        },
        pending.current_terms,
    );
    let paid = propose_allowance_renewal(&pending, request, payment, 100).unwrap();
    let mut store = CommerceFixture::new();
    store.consent = SpendConsent {
        revision: paid.next.revision,
        maximum: paid.grant.unwrap().supplier_allowance,
    };
    store.acknowledgement = Acknowledgement::Unknown;
    let grant = store.consent;
    assert!(matches!(
        store.reserve(admission(&1, &grant, &11)),
        BudgetMutation::Unknown
    ));
    let held = store.snapshot;
    let original = store.receipt.unwrap();
    let (renewal, payment) = allowance_payment(
        paid.next,
        13,
        PaidPeriod {
            start: 200,
            end: 400,
        },
        paid.next.current_terms,
    );
    let next = propose_allowance_renewal(&paid.next, renewal, payment, 250).unwrap();
    store.consent = SpendConsent {
        revision: next.next.revision,
        maximum: next.grant.unwrap().supplier_allowance,
    };
    let current = store.consent;
    assert!(matches!(
        store.reserve(admission(&1, &current, &11)),
        BudgetMutation::Unknown
    ));
    for now in [400, u64::MAX] {
        assert_eq!(
            effective_allowance_terms(&next.next, now),
            Ok(next.next.current_terms)
        );
        assert!(matches!(
            store.claim_dispatch(&original, &operation()),
            BudgetMutation::Unknown
        ));
        assert!(matches!(
            store.reconcile(&original),
            BudgetMutation::Unknown
        ));
        assert_eq!(store.snapshot, held);
        assert_eq!(store.receipt, Some(original));
        assert_eq!(store.observation, SpendOperationStatus::Unknown);
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

fn quoted_amounts() -> (Usage, Money, Money) {
    // Synthetic declared units and rate; neither amount is a verified invoice.
    let rate = LiabilityRate::new(currency(), UsageUnit::Token, 159, 8).unwrap();
    let expected_usage = Usage::new(2, UsageUnit::Token);
    let maximum_usage = Usage::new(4, UsageUnit::Token);
    let expected_invoice = rate.liability(expected_usage).unwrap();
    let buffered_maximum = rate.liability(maximum_usage).unwrap();
    assert_eq!(expected_invoice, money(40));
    assert_eq!(buffered_maximum, money(80));
    assert_ne!(expected_invoice, buffered_maximum);
    (maximum_usage, expected_invoice, buffered_maximum)
}

#[test]
fn quoted_buffered_maximum_is_reserved_instead_of_expected_invoice() {
    let (maximum_usage, expected_invoice, buffered_maximum) = quoted_amounts();
    let mut store = CommerceFixture::new();
    let before = store.snapshot;
    let scope = 1;
    let quote = 11;
    let grant = store.consent;
    let mut candidate = admission(&scope, &grant, &quote);
    candidate.maximum_usage = maximum_usage;
    candidate.maximum_supplier_liability = buffered_maximum;
    let reserved = match store.reserve(candidate) {
        BudgetMutation::Committed {
            result,
            replayed: false,
        } => result,
        _ => panic!("funded buffered maximum must be admitted"),
    };

    assert_eq!(reserved.maximum_supplier_liability, buffered_maximum);
    assert_ne!(reserved.maximum_supplier_liability, expected_invoice);
    assert_eq!(reserved.maximum_usage, maximum_usage);
    assert_eq!(reserved.quote, quote);
    assert_eq!(reserved.job, job());
    assert_eq!(reserved.operation, operation());
    assert_eq!(reserved.mode, ExecutionMode::Live);
    for (prior, current) in before.counters.iter().zip(store.snapshot.counters) {
        assert_eq!(
            current.used,
            prior.used.checked_add(buffered_maximum).unwrap()
        );
        assert_eq!(current.used, money(100));
        assert_ne!(
            current.used,
            prior.used.checked_add(expected_invoice).unwrap()
        );
        assert_eq!(current.limit, prior.limit);
    }
    let held = store.snapshot;
    let mut retry = admission(&scope, &grant, &quote);
    retry.maximum_usage = maximum_usage;
    retry.maximum_supplier_liability = buffered_maximum;
    assert!(matches!(
        store.reserve(retry),
        BudgetMutation::Committed { result, replayed: true } if result == reserved
    ));
    assert_eq!(store.snapshot, held);
    assert_eq!(store.receipt, Some(reserved));
    assert_eq!(store.observation, SpendOperationStatus::Recorded);
}

#[test]
fn forecast_fit_does_not_admit_an_unfunded_buffered_maximum() {
    let (maximum_usage, expected_invoice, buffered_maximum) = quoted_amounts();
    let scopes = [
        SpendScope::PlatformDay,
        SpendScope::PlatformMonth,
        SpendScope::SupplierAccount,
        SpendScope::TenantPayer,
        SpendScope::Campaign,
        SpendScope::Job,
    ];
    for blocked in 0..=scopes.len() {
        let mut store = CommerceFixture::new();
        let expected_refusal = if blocked == 0 {
            store.consent.maximum = money(60);
            SpendRefusal::CustomerConsentExceeded
        } else {
            store.snapshot.counters[blocked - 1].limit = money(99);
            SpendRefusal::BudgetExceeded(scopes[blocked - 1])
        };
        let before = store.snapshot;
        let before_receipt = store.receipt;
        let before_observation = store.observation;
        let grant = store.consent;
        let forecast_candidate = propose_spend(
            &before,
            grant,
            SpendRequest {
                operation: operation(),
                expected_revision: store.requested_revision,
                expected_consent_revision: grant.revision,
                maximum_supplier_liability: expected_invoice,
            },
            SpendOperationObservation {
                operation: operation(),
                status: before_observation,
            },
        )
        .expect("the forecast alone fits, so this control detects estimate substitution");
        assert_eq!(
            forecast_candidate.maximum_supplier_liability,
            expected_invoice
        );
        assert!(
            forecast_candidate
                .next
                .counters
                .iter()
                .all(|counter| counter.used == money(60))
        );

        let mut candidate = admission(&1, &grant, &11);
        candidate.maximum_usage = maximum_usage;
        candidate.maximum_supplier_liability = buffered_maximum;
        assert!(matches!(
            store.reserve(candidate),
            BudgetMutation::Refused(Refusal::Commerce(reason)) if reason == expected_refusal
        ));
        assert_eq!(store.snapshot, before);
        assert_eq!(store.receipt, before_receipt);
        assert_eq!(store.observation, before_observation);
    }
}

#[test]
fn incompatible_maximum_currency_cannot_fall_back_to_the_expected_invoice() {
    let (maximum_usage, expected_invoice, _) = quoted_amounts();
    let mut store = CommerceFixture::new();
    let before = store.snapshot;
    let before_receipt = store.receipt;
    let before_observation = store.observation;
    let grant = store.consent;
    let foreign_rate =
        LiabilityRate::new(Currency::parse("EUR").unwrap(), UsageUnit::Token, 159, 8).unwrap();
    let foreign_maximum = foreign_rate.liability(maximum_usage).unwrap();
    assert_eq!(foreign_maximum.micros(), 80);
    assert_eq!(expected_invoice, money(40));
    let mut candidate = admission(&1, &grant, &11);
    candidate.maximum_usage = maximum_usage;
    candidate.maximum_supplier_liability = foreign_maximum;
    assert!(matches!(
        store.reserve(candidate),
        BudgetMutation::Refused(Refusal::Commerce(SpendRefusal::Money(
            MoneyError::CurrencyMismatch
        )))
    ));
    assert_eq!(store.snapshot, before);
    assert_eq!(store.receipt, before_receipt);
    assert_eq!(store.observation, before_observation);
}
