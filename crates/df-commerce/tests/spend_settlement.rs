use df_commerce::{
    ObservationAuthority, SpendCounter, SpendReservation, SpendReservationStatus, SpendScope,
    SpendSettlementObservation, SpendSettlementOutcome, SpendSettlementRefusal, SpendSnapshot,
    propose_spend_settlement,
};
use df_types::{Currency, LiabilityRate, Money, MoneyError, OperationId, Usage, UsageUnit};

fn money(micros: u128) -> Money {
    Money::new(Currency::parse("USD").unwrap(), micros)
}

fn operation() -> OperationId {
    OperationId::from_bytes(&[71; 16]).unwrap()
}

fn snapshot() -> SpendSnapshot {
    SpendSnapshot {
        currency: money(0).currency(),
        revision: 4,
        counters: [SpendCounter {
            used: money(80),
            limit: money(100),
        }; 6],
    }
}

fn reservation(status: SpendReservationStatus) -> SpendReservation {
    SpendReservation {
        operation: operation(),
        maximum_supplier_liability: money(60),
        status,
    }
}

fn known(actual_units: u128, billed_waste: u128) -> SpendSettlementObservation {
    SpendSettlementObservation {
        authority: ObservationAuthority::VerifiedLatest,
        operation: operation(),
        expected_revision: 4,
        outcome: SpendSettlementOutcome::Known {
            actual_usage: Usage::new(actual_units, UsageUnit::Token),
            billed_waste: money(billed_waste),
            liability_rate: LiabilityRate::new(money(0).currency(), UsageUnit::Token, 1, 1)
                .unwrap(),
        },
    }
}

#[test]
fn verified_usage_and_billed_waste_settle_once_and_release_known_unused_from_all_scopes() {
    let state = snapshot();
    let held = reservation(SpendReservationStatus::UnknownLiability);
    let observation = known(35, 5);
    let proposal = propose_spend_settlement(&state, held, observation).unwrap();
    assert_eq!(proposal.operation, operation());
    assert_eq!(proposal.before_revision, 4);
    assert_eq!(proposal.next.revision, 5);
    assert_eq!(proposal.maximum_supplier_liability, money(60));
    assert_eq!(proposal.actual_supplier_liability, money(40));
    assert_eq!(proposal.known_unused_release, money(20));
    assert_eq!(proposal.platform_loss, money(0));
    assert!(!proposal.supplier_admissions_blocked);
    assert!(
        proposal
            .next
            .counters
            .iter()
            .all(|counter| counter.used == money(60))
    );
    assert_eq!(state, snapshot());
    assert_eq!(held, reservation(SpendReservationStatus::UnknownLiability));
    assert_eq!(observation, known(35, 5));
    assert_eq!(
        propose_spend_settlement(
            &proposal.next,
            reservation(SpendReservationStatus::SettledKnown),
            SpendSettlementObservation {
                expected_revision: 5,
                ..observation
            },
        ),
        Err(SpendSettlementRefusal::AlreadySettled)
    );
}

#[test]
fn exact_maximum_settles_without_release_or_loss() {
    let proposal = propose_spend_settlement(
        &snapshot(),
        reservation(SpendReservationStatus::Dispatching),
        known(50, 10),
    )
    .unwrap();
    assert_eq!(proposal.known_unused_release, money(0));
    assert_eq!(proposal.platform_loss, money(0));
    assert_eq!(proposal.next.counters, snapshot().counters);
}

#[test]
fn unknown_or_untrusted_outcome_keeps_full_liability() {
    let state = snapshot();
    let held = reservation(SpendReservationStatus::UnknownLiability);
    let unknown = SpendSettlementObservation {
        outcome: SpendSettlementOutcome::Unknown,
        ..known(35, 5)
    };
    assert_eq!(
        propose_spend_settlement(&state, held, unknown),
        Err(SpendSettlementRefusal::UnknownLiability)
    );
    assert_eq!(
        propose_spend_settlement(
            &state,
            held,
            SpendSettlementObservation {
                authority: ObservationAuthority::WebhookOnly,
                ..known(35, 5)
            },
        ),
        Err(SpendSettlementRefusal::NeedsAuthoritativeRefresh)
    );
    assert_eq!(
        propose_spend_settlement(
            &state,
            held,
            SpendSettlementObservation {
                authority: ObservationAuthority::Unverified,
                ..known(35, 5)
            },
        ),
        Err(SpendSettlementRefusal::Unverified)
    );
    assert_eq!(state, snapshot());
}

#[test]
fn stale_wrong_operation_and_unsent_reservations_cannot_settle() {
    let state = snapshot();
    let held = reservation(SpendReservationStatus::Dispatching);
    assert_eq!(
        propose_spend_settlement(
            &state,
            held,
            SpendSettlementObservation {
                expected_revision: 3,
                ..known(35, 5)
            },
        ),
        Err(SpendSettlementRefusal::StaleRevision)
    );
    assert_eq!(
        propose_spend_settlement(
            &state,
            held,
            SpendSettlementObservation {
                operation: OperationId::from_bytes(&[72; 16]).unwrap(),
                ..known(35, 5)
            },
        ),
        Err(SpendSettlementRefusal::WrongOperation)
    );
    assert_eq!(
        propose_spend_settlement(
            &state,
            reservation(SpendReservationStatus::UnsentCanceled),
            known(35, 5),
        ),
        Err(SpendSettlementRefusal::NotDispatched)
    );
    assert_eq!(state, snapshot());
}

#[test]
fn supplier_overrun_is_platform_loss_and_blocks_supplier_without_raising_customer_exposure() {
    let proposal = propose_spend_settlement(
        &snapshot(),
        reservation(SpendReservationStatus::Dispatching),
        known(65, 5),
    )
    .unwrap();
    assert_eq!(proposal.actual_supplier_liability, money(70));
    assert_eq!(proposal.known_unused_release, money(0));
    assert_eq!(proposal.platform_loss, money(10));
    assert!(proposal.supplier_admissions_blocked);
    for (index, counter) in proposal.next.counters.into_iter().enumerate() {
        assert_eq!(counter.used, if index < 3 { money(90) } else { money(80) });
    }
}

#[test]
fn invalid_money_or_exposure_never_mints_a_settlement() {
    let mut low_exposure = snapshot();
    low_exposure.counters[3].used = money(59);
    assert_eq!(
        propose_spend_settlement(
            &low_exposure,
            reservation(SpendReservationStatus::Dispatching),
            known(35, 5),
        ),
        Err(SpendSettlementRefusal::ReservationExceedsExposure(
            SpendScope::TenantPayer
        ))
    );
    let mismatched_usage = SpendSettlementObservation {
        outcome: SpendSettlementOutcome::Known {
            actual_usage: Usage::new(35, UsageUnit::Image),
            billed_waste: money(5),
            liability_rate: LiabilityRate::new(money(0).currency(), UsageUnit::Token, 1, 1)
                .unwrap(),
        },
        ..known(35, 5)
    };
    assert_eq!(
        propose_spend_settlement(
            &snapshot(),
            reservation(SpendReservationStatus::Dispatching),
            mismatched_usage,
        ),
        Err(SpendSettlementRefusal::Money(MoneyError::UnitMismatch))
    );
    let wrong_currency = SpendSettlementObservation {
        outcome: SpendSettlementOutcome::Known {
            actual_usage: Usage::new(35, UsageUnit::Token),
            billed_waste: Money::new(Currency::parse("EUR").unwrap(), 5),
            liability_rate: LiabilityRate::new(money(0).currency(), UsageUnit::Token, 1, 1)
                .unwrap(),
        },
        ..known(35, 5)
    };
    assert_eq!(
        propose_spend_settlement(
            &snapshot(),
            reservation(SpendReservationStatus::Dispatching),
            wrong_currency,
        ),
        Err(SpendSettlementRefusal::Money(MoneyError::CurrencyMismatch))
    );
    let overflowing_rate = SpendSettlementObservation {
        outcome: SpendSettlementOutcome::Known {
            actual_usage: Usage::new(2, UsageUnit::Token),
            billed_waste: money(0),
            liability_rate: LiabilityRate::new(money(0).currency(), UsageUnit::Token, u128::MAX, 1)
                .unwrap(),
        },
        ..known(35, 5)
    };
    assert_eq!(
        propose_spend_settlement(
            &snapshot(),
            reservation(SpendReservationStatus::Dispatching),
            overflowing_rate,
        ),
        Err(SpendSettlementRefusal::Money(MoneyError::Overflow))
    );
}
