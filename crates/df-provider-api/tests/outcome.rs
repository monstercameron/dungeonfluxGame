//! Bounded synthetic consumers of the public classifier, CheckedRequest and
//! BudgetStore. Admission uses actual df-commerce policy; finite callback output
//! is not provider egress. The fixture models exact full-maximum billing only,
//! never a production ledger, invoice verifier or general settlement policy.

use std::time::Duration;

use df_commerce::{
    SpendConsent, SpendCounter, SpendOperationObservation, SpendOperationStatus, SpendRefusal,
    SpendRequest, SpendScope, SpendSnapshot, propose_spend,
};
use df_model::checkpoint::{Basis, ExecutionMode, JobId};
use df_provider_api::{
    BudgetAdmission, BudgetMutation, BudgetStore, CheckedRequest, ProviderAttemptObservation,
    ProviderBillingClass, ProviderFailureClass, ProviderLiabilityDisposition, ProviderNextAction,
    ProviderOutcomeDecision, ProviderOutcomeError, ProviderResultClass, ProviderRetryPolicy,
    RequestBinding, RequestError, RequestIdentity, RequestIdentityField, RequestLimits,
    RequestOwnerState, RequestUsage, classify_provider_outcome,
};
use df_types::{
    Currency, Money, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
    Usage, UsageUnit,
};

fn operation(byte: u8) -> OperationId {
    OperationId::from_bytes(&[byte; 16]).unwrap()
}

fn money(amount: u128) -> Money {
    Money::new(Currency::parse("USD").unwrap(), amount)
}

fn binding(mode: ExecutionMode) -> RequestBinding<RevisionLabel> {
    RequestBinding {
        identity: RequestIdentity {
            basis: Basis {
                session: SessionId::from_bytes(&[1; 16]).unwrap(),
                run: RunId::from_bytes(&[2; 16]).unwrap(),
                revision: SessionRevision::new(RecoveryEpoch::new(3).unwrap(), 4),
            },
            job: JobId::from_bytes(&[5; 16]).unwrap(),
            operation: operation(6),
            generation: 7,
        },
        semantic_basis: RevisionLabel::new(Some("source-rights-quote-current")).unwrap(),
        mode,
        deadline: Duration::from_secs(2),
    }
}

fn owner(current: &RequestBinding<RevisionLabel>) -> RequestOwnerState<'_, RevisionLabel> {
    RequestOwnerState {
        current: Some(current),
        elapsed: Duration::ZERO,
        cancelled: false,
    }
}

fn request(current: &RequestBinding<RevisionLabel>) -> CheckedRequest<RevisionLabel> {
    let candidate = RequestBinding {
        identity: current.identity,
        semantic_basis: current.semantic_basis.clone(),
        mode: current.mode,
        deadline: current.deadline,
    };
    CheckedRequest::new(
        candidate,
        b"data",
        RequestUsage::new(3, Duration::from_nanos(5), Usage::new(2, UsageUnit::Token)),
        RequestLimits::new(
            4,
            3,
            Duration::from_nanos(5),
            Usage::new(2, UsageUnit::Token),
        )
        .unwrap(),
        owner(current),
    )
    .unwrap()
}

fn policy() -> ProviderRetryPolicy {
    ProviderRetryPolicy {
        attempts_remaining: 2,
        retry: true,
        fallback: true,
    }
}

fn observation(
    result: ProviderResultClass,
    billing: ProviderBillingClass,
) -> ProviderAttemptObservation {
    ProviderAttemptObservation {
        operation: operation(6),
        result,
        billing,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ack {
    Current,
    Replayed,
    Pending,
    Unknown,
    Refused,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Commerce(SpendRefusal),
    Binding,
    AlreadyClaimed,
    StorageBound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Storage,
}

type LedgerOutcome = BudgetMutation<OperationId, Refusal, Failure>;

fn ledger_outcome(ack: Ack, operation: OperationId) -> LedgerOutcome {
    match ack {
        Ack::Current | Ack::Replayed => BudgetMutation::Committed {
            result: operation,
            replayed: ack == Ack::Replayed,
        },
        Ack::Pending => BudgetMutation::Pending,
        Ack::Unknown => BudgetMutation::Unknown,
        Ack::Refused => BudgetMutation::Refused(Refusal::Binding),
        Ack::Failed => BudgetMutation::Failed(Failure::Storage),
    }
}

fn classify(
    request: &CheckedRequest<RevisionLabel>,
    current: &RequestBinding<RevisionLabel>,
    result: ProviderResultClass,
    billing: ProviderBillingClass,
    ack: Ack,
) -> ProviderOutcomeDecision {
    classify_provider_outcome(
        request,
        owner(current),
        observation(result, billing),
        &ledger_outcome(ack, operation(6)),
        policy(),
    )
    .unwrap()
}

#[test]
fn complete_output_and_known_billing_are_separate_from_recovery_permission() {
    let current = binding(ExecutionMode::Live);
    let request = request(&current);
    for ack in [Ack::Current, Ack::Replayed] {
        for (billing, liability) in [
            (
                ProviderBillingClass::VerifiedUnused,
                ProviderLiabilityDisposition::KnownUnused,
            ),
            (
                ProviderBillingClass::VerifiedLiability,
                ProviderLiabilityDisposition::KnownLiability,
            ),
        ] {
            let actual = classify(
                &request,
                &current,
                ProviderResultClass::Complete,
                billing,
                ack,
            );
            assert_eq!(actual.result, ProviderResultClass::Complete);
            assert_eq!(actual.liability, liability);
            assert_eq!(actual.next, ProviderNextAction::Complete);
            assert_eq!(actual.current_request, Ok(()));
            assert_eq!(actual.operation, current.identity.operation);

            let incomplete = classify(
                &request,
                &current,
                ProviderResultClass::Incomplete,
                billing,
                ack,
            );
            assert_eq!(incomplete.result, ProviderResultClass::Incomplete);
            assert_eq!(incomplete.liability, liability);
            assert_eq!(incomplete.next, ProviderNextAction::Stop);
        }
    }
    assert_eq!(request.payload(), b"data");
    assert_eq!(request.binding().identity, current.identity);
}

#[test]
fn missing_invoice_never_fabricates_free_spend_or_erases_complete_output() {
    let current = binding(ExecutionMode::Live);
    let request = request(&current);
    for result in [
        ProviderResultClass::Complete,
        ProviderResultClass::Failed(ProviderFailureClass::Unavailable),
        ProviderResultClass::Failed(ProviderFailureClass::Unknown),
        ProviderResultClass::Incomplete,
    ] {
        for ack in [
            Ack::Current,
            Ack::Replayed,
            Ack::Pending,
            Ack::Unknown,
            Ack::Refused,
            Ack::Failed,
        ] {
            let actual = classify(
                &request,
                &current,
                result,
                ProviderBillingClass::MissingOrAmbiguous,
                ack,
            );
            assert_eq!(actual.result, result);
            assert_eq!(
                actual.liability,
                ProviderLiabilityDisposition::RetainWorstCase
            );
            assert_eq!(actual.next, ProviderNextAction::ReconcileSameOperation);
        }
    }
}

#[test]
fn a_failed_or_refused_settlement_does_not_prove_the_provider_was_free_or_unsent() {
    let current = binding(ExecutionMode::Live);
    let request = request(&current);
    for ack in [Ack::Pending, Ack::Unknown, Ack::Refused, Ack::Failed] {
        for billing in [
            ProviderBillingClass::VerifiedUnused,
            ProviderBillingClass::VerifiedLiability,
        ] {
            for result in [
                ProviderResultClass::Complete,
                ProviderResultClass::Failed(ProviderFailureClass::Unavailable),
            ] {
                let actual = classify(&request, &current, result, billing, ack);
                assert_eq!(actual.result, result);
                assert_eq!(
                    actual.liability,
                    ProviderLiabilityDisposition::RetainWorstCase
                );
                assert_eq!(actual.next, ProviderNextAction::ReconcileSameOperation);
            }
        }
    }
}

#[test]
fn closed_failure_classes_and_explicit_policy_bound_every_new_work_candidate() {
    let current = binding(ExecutionMode::Live);
    let request = request(&current);
    for class in [
        ProviderFailureClass::InvalidRequest,
        ProviderFailureClass::Denied,
        ProviderFailureClass::Capacity,
        ProviderFailureClass::Unavailable,
        ProviderFailureClass::Contract,
        ProviderFailureClass::Cancelled,
        ProviderFailureClass::Deadline,
        ProviderFailureClass::Unknown,
    ] {
        let eligible = matches!(
            class,
            ProviderFailureClass::Capacity | ProviderFailureClass::Unavailable
        );
        for attempts_remaining in [0, 1, u32::MAX] {
            for retry in [false, true] {
                for fallback in [false, true] {
                    let selected = ProviderRetryPolicy {
                        attempts_remaining,
                        retry,
                        fallback,
                    };
                    let actual = classify_provider_outcome(
                        &request,
                        owner(&current),
                        observation(
                            ProviderResultClass::Failed(class),
                            ProviderBillingClass::VerifiedLiability,
                        ),
                        &ledger_outcome(Ack::Current, operation(6)),
                        selected,
                    )
                    .unwrap();
                    assert_eq!(
                        actual.next,
                        if eligible && attempts_remaining > 0 && (retry || fallback) {
                            ProviderNextAction::FreshAdmissionCandidates { retry, fallback }
                        } else {
                            ProviderNextAction::Stop
                        }
                    );
                    assert_eq!(
                        actual.liability,
                        ProviderLiabilityDisposition::KnownLiability
                    );
                }
            }
        }
    }
}

#[test]
fn admitted_prepared_and_replay_modes_never_become_paid_retry_or_fallback() {
    for mode in [ExecutionMode::PreparedOnly, ExecutionMode::Replay] {
        let current = binding(mode);
        let request = request(&current);
        for class in [
            ProviderFailureClass::Capacity,
            ProviderFailureClass::Unavailable,
        ] {
            let actual = classify(
                &request,
                &current,
                ProviderResultClass::Failed(class),
                ProviderBillingClass::VerifiedUnused,
                Ack::Current,
            );
            assert_eq!(actual.next, ProviderNextAction::Stop);
            assert_eq!(request.binding().mode, mode);
        }
        let missing = classify(
            &request,
            &current,
            ProviderResultClass::Incomplete,
            ProviderBillingClass::MissingOrAmbiguous,
            Ack::Current,
        );
        assert_eq!(missing.result, ProviderResultClass::Incomplete);
        assert_eq!(missing.next, ProviderNextAction::ReconcileSameOperation);
    }
}

#[test]
fn positive_eligibility_reuses_actual_checked_request_current_owner_validation() {
    let current = binding(ExecutionMode::Live);
    let request = request(&current);
    let transient = observation(
        ProviderResultClass::Failed(ProviderFailureClass::Unavailable),
        ProviderBillingClass::VerifiedUnused,
    );
    let settlement = ledger_outcome(Ack::Current, operation(6));
    let cancelled = classify_provider_outcome(
        &request,
        RequestOwnerState {
            cancelled: true,
            ..owner(&current)
        },
        transient,
        &settlement,
        policy(),
    )
    .unwrap();
    assert_eq!(cancelled.current_request, Err(RequestError::Cancelled));
    assert_eq!(cancelled.next, ProviderNextAction::Stop);
    let expired = classify_provider_outcome(
        &request,
        RequestOwnerState {
            elapsed: current.deadline,
            ..owner(&current)
        },
        transient,
        &settlement,
        policy(),
    )
    .unwrap();
    assert_eq!(expired.current_request, Err(RequestError::DeadlineExceeded));
    assert_eq!(expired.next, ProviderNextAction::Stop);
    let missing = classify_provider_outcome(
        &request,
        RequestOwnerState {
            current: None,
            ..owner(&current)
        },
        transient,
        &settlement,
        policy(),
    )
    .unwrap();
    assert_eq!(
        missing.current_request,
        Err(RequestError::CurrentBasisUnavailable)
    );
    assert_eq!(missing.next, ProviderNextAction::Stop);
    for field in [
        RequestIdentityField::Session,
        RequestIdentityField::Run,
        RequestIdentityField::Revision,
        RequestIdentityField::Job,
        RequestIdentityField::Operation,
        RequestIdentityField::Generation,
        RequestIdentityField::Mode,
        RequestIdentityField::Deadline,
    ] {
        let mut replaced = binding(current.mode);
        match field {
            RequestIdentityField::Session => {
                replaced.identity.basis.session = SessionId::from_bytes(&[21; 16]).unwrap()
            }
            RequestIdentityField::Run => {
                replaced.identity.basis.run = RunId::from_bytes(&[22; 16]).unwrap()
            }
            RequestIdentityField::Revision => {
                replaced.identity.basis.revision =
                    SessionRevision::new(RecoveryEpoch::new(3).unwrap(), 5)
            }
            RequestIdentityField::Job => {
                replaced.identity.job = JobId::from_bytes(&[23; 16]).unwrap()
            }
            RequestIdentityField::Operation => replaced.identity.operation = operation(24),
            RequestIdentityField::Generation => replaced.identity.generation += 1,
            RequestIdentityField::Mode => replaced.mode = ExecutionMode::Replay,
            RequestIdentityField::Deadline => replaced.deadline += Duration::from_secs(1),
        }
        let actual =
            classify_provider_outcome(&request, owner(&replaced), transient, &settlement, policy())
                .unwrap();
        assert_eq!(
            actual.current_request,
            Err(RequestError::IdentityMismatch(field))
        );
        assert_eq!(actual.next, ProviderNextAction::Stop);
    }
    let mut replaced = binding(current.mode);
    replaced.semantic_basis = RevisionLabel::new(Some("revoked-source-rights-quote")).unwrap();
    let actual =
        classify_provider_outcome(&request, owner(&replaced), transient, &settlement, policy())
            .unwrap();
    assert_eq!(
        actual.current_request,
        Err(RequestError::SemanticBasisMismatch)
    );
    assert_eq!(actual.next, ProviderNextAction::Stop);
    assert_eq!(request.binding().identity, current.identity);
    assert_eq!(request.payload(), b"data");
}

#[test]
fn cleanup_stops_new_work_and_preserves_complete_output_and_unknown_exposure() {
    let current = binding(ExecutionMode::Live);
    let request = request(&current);
    for state in [
        RequestOwnerState {
            cancelled: true,
            ..owner(&current)
        },
        RequestOwnerState {
            elapsed: Duration::MAX,
            ..owner(&current)
        },
        RequestOwnerState {
            current: None,
            ..owner(&current)
        },
    ] {
        let actual = classify_provider_outcome(
            &request,
            state,
            observation(
                ProviderResultClass::Complete,
                ProviderBillingClass::MissingOrAmbiguous,
            ),
            &ledger_outcome(Ack::Unknown, operation(6)),
            policy(),
        )
        .unwrap();
        assert!(actual.current_request.is_err());
        assert_eq!(actual.result, ProviderResultClass::Complete);
        assert_eq!(
            actual.liability,
            ProviderLiabilityDisposition::RetainWorstCase
        );
        assert_eq!(actual.next, ProviderNextAction::ReconcileSameOperation);
    }
}

#[test]
fn foreign_attempt_facts_cannot_describe_or_retry_the_admitted_operation() {
    let current = binding(ExecutionMode::Live);
    let request = request(&current);
    for billing in [
        ProviderBillingClass::VerifiedUnused,
        ProviderBillingClass::MissingOrAmbiguous,
    ] {
        let facts = ProviderAttemptObservation {
            operation: operation(9),
            ..observation(ProviderResultClass::Complete, billing)
        };
        assert_eq!(
            classify_provider_outcome(
                &request,
                owner(&current),
                facts,
                &ledger_outcome(Ack::Current, operation(6)),
                policy()
            ),
            Err(ProviderOutcomeError::OperationMismatch)
        );
    }
    assert_eq!(request.binding().identity.operation, operation(6));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Reservation {
    operation: OperationId,
    job: JobId,
    mode: ExecutionMode,
    usage: Usage,
    maximum: Money,
    quote: u64,
    grant: SpendConsent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Entry {
    reservation: Reservation,
    status: SpendOperationStatus,
    claimed: bool,
}

struct CommerceConsumer {
    snapshot: SpendSnapshot,
    grant: SpendConsent,
    quote: u64,
    entries: [Option<Entry>; 2],
    invoice: Option<Money>,
    settle_ack: Ack,
    reconcile_ack: Ack,
    settle_calls: usize,
    reconcile_calls: usize,
    finite_dispatch_calls: usize,
}

impl CommerceConsumer {
    fn new(limit: u128) -> Self {
        Self {
            snapshot: SpendSnapshot {
                currency: money(0).currency(),
                revision: 9,
                counters: [SpendCounter {
                    used: money(20),
                    limit: money(limit),
                }; 6],
            },
            grant: SpendConsent {
                revision: 7,
                maximum: money(80),
            },
            quote: 11,
            entries: [None; 2],
            invoice: None,
            settle_ack: Ack::Unknown,
            reconcile_ack: Ack::Unknown,
            settle_calls: 0,
            reconcile_calls: 0,
            finite_dispatch_calls: 0,
        }
    }

    fn locate(&self, reservation: &Reservation) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.is_some_and(|entry| entry.reservation == *reservation))
    }

    fn finish(&mut self, reservation: &Reservation, ack: Ack) -> LedgerOutcome {
        let Some(index) = self.locate(reservation) else {
            return BudgetMutation::Refused(Refusal::Binding);
        };
        let entry = self.entries[index].as_mut().unwrap();
        if !entry.claimed {
            return BudgetMutation::Refused(Refusal::Binding);
        }
        // Only a verified invoice equal to the reserved maximum is modeled.
        // No guessed/free invoice or copied general settlement arithmetic.
        let known = matches!(ack, Ack::Current | Ack::Replayed)
            && self.invoice == Some(reservation.maximum);
        entry.status = if known {
            SpendOperationStatus::Recorded
        } else {
            SpendOperationStatus::Unknown
        };
        if matches!(ack, Ack::Current | Ack::Replayed) && !known {
            BudgetMutation::Unknown
        } else {
            ledger_outcome(ack, reservation.operation)
        }
    }

    fn start_finite(&mut self, reservation: &Reservation) {
        if let BudgetMutation::Committed {
            replayed: false, ..
        } = self.claim_dispatch(reservation, &reservation.operation)
        {
            self.finite_dispatch_calls += 1;
        }
    }
}

impl BudgetStore for CommerceConsumer {
    type Scope = u64;
    type Grant = SpendConsent;
    type Quote = u64;
    type Reservation = Reservation;
    type DispatchClaim = OperationId;
    type DispatchPermit = ();
    type Settlement = Option<Money>;
    type SettlementReceipt = OperationId;
    type LedgerView = SpendSnapshot;
    type Refusal = Refusal;
    type Failure = Failure;

    fn inspect(&mut self, scope: &u64) -> Result<SpendSnapshot, Failure> {
        if *scope != 1 {
            return Err(Failure::Storage);
        }
        Ok(self.snapshot)
    }

    fn reserve(
        &mut self,
        admission: BudgetAdmission<'_, u64, SpendConsent, u64>,
    ) -> BudgetMutation<Reservation, Refusal, Failure> {
        if *admission.scope != 1
            || *admission.grant != self.grant
            || *admission.quote != self.quote
            || admission.mode != ExecutionMode::Live
        {
            return BudgetMutation::Refused(Refusal::Binding);
        }
        let candidate = Reservation {
            operation: admission.operation,
            job: admission.job,
            mode: admission.mode,
            usage: admission.maximum_usage,
            maximum: admission.maximum_supplier_liability,
            quote: *admission.quote,
            grant: *admission.grant,
        };
        let prior = self
            .entries
            .iter()
            .flatten()
            .find(|entry| entry.reservation.operation == admission.operation);
        if let Some(prior) = prior {
            if prior.reservation != candidate {
                return BudgetMutation::Refused(Refusal::Binding);
            }
            if prior.status == SpendOperationStatus::Recorded {
                return BudgetMutation::Committed {
                    result: prior.reservation,
                    replayed: true,
                };
            }
        }
        let status = prior.map_or(SpendOperationStatus::Unseen, |entry| entry.status);
        let proposed = propose_spend(
            &self.snapshot,
            *admission.grant,
            SpendRequest {
                operation: admission.operation,
                expected_revision: self.snapshot.revision,
                expected_consent_revision: admission.grant.revision,
                maximum_supplier_liability: admission.maximum_supplier_liability,
            },
            SpendOperationObservation {
                operation: admission.operation,
                status,
            },
        );
        match proposed {
            Err(refusal) => BudgetMutation::Refused(Refusal::Commerce(refusal)),
            Ok(proposal) => {
                let Some(index) = self.entries.iter().position(Option::is_none) else {
                    return BudgetMutation::Refused(Refusal::StorageBound);
                };
                self.snapshot = proposal.next;
                self.entries[index] = Some(Entry {
                    reservation: candidate,
                    status: SpendOperationStatus::Recorded,
                    claimed: false,
                });
                BudgetMutation::Committed {
                    result: candidate,
                    replayed: false,
                }
            }
        }
    }

    fn claim_dispatch(
        &mut self,
        reservation: &Reservation,
        claim: &OperationId,
    ) -> BudgetMutation<(), Refusal, Failure> {
        let Some(index) = self.locate(reservation) else {
            return BudgetMutation::Refused(Refusal::Binding);
        };
        let entry = self.entries[index].as_mut().unwrap();
        if *claim != reservation.operation {
            return BudgetMutation::Refused(Refusal::Binding);
        }
        if entry.status == SpendOperationStatus::Unknown {
            return BudgetMutation::Refused(Refusal::Commerce(SpendRefusal::UnknownOperation));
        }
        if entry.claimed {
            return BudgetMutation::Refused(Refusal::AlreadyClaimed);
        }
        entry.claimed = true;
        BudgetMutation::Committed {
            result: (),
            replayed: false,
        }
    }

    fn settle(&mut self, reservation: &Reservation, settlement: &Option<Money>) -> LedgerOutcome {
        self.settle_calls += 1;
        self.invoice = *settlement;
        self.finish(reservation, self.settle_ack)
    }

    fn reconcile(&mut self, reservation: &Reservation) -> LedgerOutcome {
        self.reconcile_calls += 1;
        self.finish(reservation, self.reconcile_ack)
    }
}

fn admission<'a>(
    request: &CheckedRequest<RevisionLabel>,
    grant: &'a SpendConsent,
    quote: &'a u64,
) -> BudgetAdmission<'a, u64, SpendConsent, u64> {
    BudgetAdmission {
        scope: &1,
        grant,
        quote,
        job: request.binding().identity.job,
        operation: request.binding().identity.operation,
        mode: request.binding().mode,
        maximum_usage: request.usage().usage(),
        maximum_supplier_liability: money(80),
    }
}

fn reserve_original(
    store: &mut CommerceConsumer,
    request: &CheckedRequest<RevisionLabel>,
) -> Reservation {
    let grant = store.grant;
    let quote = store.quote;
    let BudgetMutation::Committed {
        result,
        replayed: false,
    } = store.reserve(admission(request, &grant, &quote))
    else {
        panic!("positive admission fixture");
    };
    store.start_finite(&result);
    assert_eq!(store.finite_dispatch_calls, 1);
    result
}

fn fresh_request() -> (RequestBinding<RevisionLabel>, CheckedRequest<RevisionLabel>) {
    let mut current = binding(ExecutionMode::Live);
    current.identity.operation = operation(7);
    current.semantic_basis = RevisionLabel::new(Some("fresh-native-admitted-branch")).unwrap();
    let request = request(&current);
    (current, request)
}

#[test]
fn actual_budget_consumer_retains_all_six_maxima_on_missing_invoice_and_reconciles_same_operation()
{
    let current = binding(ExecutionMode::Live);
    let request = request(&current);
    let mut store = CommerceConsumer::new(100);
    let reserved = reserve_original(&mut store, &request);
    let exposure = store.inspect(&1).unwrap();
    assert!(exposure.counters.iter().all(|row| row.used == money(100)));
    let settlement = store.settle(&reserved, &None);
    let decision = classify_provider_outcome(
        &request,
        owner(&current),
        observation(
            ProviderResultClass::Complete,
            ProviderBillingClass::MissingOrAmbiguous,
        ),
        &settlement,
        policy(),
    )
    .unwrap();
    assert_eq!(decision.result, ProviderResultClass::Complete);
    assert_eq!(decision.next, ProviderNextAction::ReconcileSameOperation);
    let reconciled = store.reconcile(&reserved);
    let after = classify_provider_outcome(
        &request,
        owner(&current),
        observation(decision.result, ProviderBillingClass::MissingOrAmbiguous),
        &reconciled,
        policy(),
    )
    .unwrap();
    assert_eq!(after, decision);
    let grant = store.grant;
    let quote = store.quote;
    assert!(matches!(
        store.reserve(admission(&request, &grant, &quote)),
        BudgetMutation::Refused(Refusal::Commerce(SpendRefusal::UnknownOperation))
    ));
    let (_, fresh) = fresh_request();
    assert!(matches!(
        store.reserve(admission(&fresh, &grant, &quote)),
        BudgetMutation::Refused(Refusal::Commerce(SpendRefusal::BudgetExceeded(
            SpendScope::PlatformDay
        )))
    ));
    store.start_finite(&reserved);
    assert_eq!(store.finite_dispatch_calls, 1);
    assert_eq!(store.inspect(&1).unwrap(), exposure);
    assert_eq!(store.settle_calls, 1);
    assert_eq!(store.reconcile_calls, 1);
}

#[test]
fn failed_settlement_and_reconciliation_after_possible_send_do_not_clear_exposure() {
    let current = binding(ExecutionMode::Live);
    let request = request(&current);
    for ack in [Ack::Failed, Ack::Refused, Ack::Pending, Ack::Unknown] {
        let mut store = CommerceConsumer::new(100);
        let reserved = reserve_original(&mut store, &request);
        let exposure = store.snapshot;
        store.settle_ack = ack;
        store.reconcile_ack = ack;
        let unsettled = store.settle(&reserved, &Some(money(80)));
        let reconciliation = store.reconcile(&reserved);
        for outcome in [&unsettled, &reconciliation] {
            let classified = classify_provider_outcome(
                &request,
                owner(&current),
                observation(
                    ProviderResultClass::Complete,
                    ProviderBillingClass::VerifiedLiability,
                ),
                outcome,
                policy(),
            )
            .unwrap();
            assert_eq!(classified.result, ProviderResultClass::Complete);
            assert_eq!(
                classified.liability,
                ProviderLiabilityDisposition::RetainWorstCase
            );
            assert_eq!(classified.next, ProviderNextAction::ReconcileSameOperation);
        }
        store.start_finite(&reserved);
        assert_eq!(store.finite_dispatch_calls, 1);
        assert_eq!(store.snapshot, exposure);
    }
}

#[test]
fn known_settled_candidate_still_needs_actual_fresh_admission_and_capacity() {
    for limit in [100, 200] {
        let current = binding(ExecutionMode::Live);
        let request = request(&current);
        let mut store = CommerceConsumer::new(limit);
        let reserved = reserve_original(&mut store, &request);
        let prior_exposure = store.snapshot;
        store.settle_ack = Ack::Current;
        let settled = store.settle(&reserved, &Some(money(80)));
        let decision = classify_provider_outcome(
            &request,
            owner(&current),
            observation(
                ProviderResultClass::Failed(ProviderFailureClass::Unavailable),
                ProviderBillingClass::VerifiedLiability,
            ),
            &settled,
            ProviderRetryPolicy {
                attempts_remaining: 1,
                retry: false,
                fallback: true,
            },
        )
        .unwrap();
        assert_eq!(
            decision.next,
            ProviderNextAction::FreshAdmissionCandidates {
                retry: false,
                fallback: true
            }
        );
        assert_eq!(
            decision.liability,
            ProviderLiabilityDisposition::KnownLiability
        );
        assert_eq!(store.snapshot, prior_exposure);
        let (fresh_current, fresh) = fresh_request();
        fresh.validate_current(owner(&fresh_current)).unwrap();
        store.quote = 12;
        let grant = store.grant;
        let quote = store.quote;
        match store.reserve(admission(&fresh, &grant, &quote)) {
            BudgetMutation::Committed {
                result,
                replayed: false,
            } if limit == 200 => {
                assert_eq!(result.operation, operation(7));
                store.start_finite(&result);
                assert_eq!(store.finite_dispatch_calls, 2);
                assert!(
                    store
                        .snapshot
                        .counters
                        .iter()
                        .all(|row| row.used == money(180))
                );
                // Both original and branch identities remain retained.
                assert_eq!(
                    store.entries[0].unwrap().reservation.operation,
                    operation(6)
                );
                assert_eq!(
                    store.entries[1].unwrap().reservation.operation,
                    operation(7)
                );
            }
            BudgetMutation::Refused(Refusal::Commerce(SpendRefusal::BudgetExceeded(
                SpendScope::PlatformDay,
            ))) if limit == 100 => {
                assert_eq!(store.finite_dispatch_calls, 1);
                assert_eq!(store.snapshot, prior_exposure);
            }
            _ => panic!("fresh admission must follow actual canonical capacity"),
        }
    }
}

#[test]
fn same_operation_reconciliation_and_cleanup_never_create_another_branch_or_dispatch() {
    let current = binding(ExecutionMode::Live);
    let request = request(&current);
    let mut store = CommerceConsumer::new(200);
    let reserved = reserve_original(&mut store, &request);
    let exposure = store.snapshot;
    store.invoice = Some(money(80));
    store.reconcile_ack = Ack::Current;
    let first = store.reconcile(&reserved);
    store.reconcile_ack = Ack::Replayed;
    let replayed = store.reconcile(&reserved);
    for outcome in [&first, &replayed] {
        let actual = classify_provider_outcome(
            &request,
            RequestOwnerState {
                cancelled: true,
                ..owner(&current)
            },
            observation(
                ProviderResultClass::Failed(ProviderFailureClass::Unavailable),
                ProviderBillingClass::VerifiedLiability,
            ),
            outcome,
            policy(),
        )
        .unwrap();
        assert_eq!(actual.current_request, Err(RequestError::Cancelled));
        assert_eq!(actual.next, ProviderNextAction::Stop);
        store.start_finite(&reserved);
    }
    assert_eq!(store.finite_dispatch_calls, 1);
    assert_eq!(store.snapshot, exposure);
    assert_eq!(store.entries.iter().flatten().count(), 1);
}
