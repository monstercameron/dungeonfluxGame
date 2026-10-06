use df_commerce::{
    Access, ObservationAuthority, PaidInvoice, PaymentOutcome, SubscriptionObservation,
    SubscriptionRefusal, SubscriptionState, effective_access, observe_subscription,
};
use df_types::{Currency, Money, MoneyError};

const GRACE_SECONDS: u64 = 72 * 60 * 60;

fn pending() -> SubscriptionState {
    SubscriptionState {
        currency: Currency::parse("USD").unwrap(),
        revision: 7,
        object_revision: 4,
        access: Access::PendingInitial,
        paid_through: 0,
        grace_until: None,
    }
}

fn invoice() -> PaidInvoice {
    PaidInvoice {
        period_start: 90,
        paid_through: 200,
        selected_allowance: Money::new(pending().currency, 123),
        allowance_already_recorded: false,
    }
}

fn settled() -> SubscriptionObservation {
    SubscriptionObservation {
        authority: ObservationAuthority::VerifiedLatest,
        expected_revision: 7,
        object_revision: 5,
        outcome: PaymentOutcome::Settled(invoice()),
    }
}

fn active() -> SubscriptionState {
    observe_subscription(pending(), settled(), 100)
        .unwrap()
        .next
}

fn renewal_failure(state: SubscriptionState) -> SubscriptionObservation {
    SubscriptionObservation {
        expected_revision: state.revision,
        object_revision: state.object_revision + 1,
        outcome: PaymentOutcome::RenewalFailed,
        ..settled()
    }
}

#[test]
fn verified_current_invoice_proposes_exact_supplied_allowance_without_mutation() {
    let state = pending();
    let before = state;
    let transition = observe_subscription(state, settled(), 100).unwrap();
    assert_eq!(state, before);
    assert_eq!(transition.before_revision, 7);
    assert_eq!(transition.next.revision, 8);
    assert_eq!(transition.next.object_revision, 5);
    assert_eq!(transition.next.access, Access::Active);
    assert_eq!(transition.next.paid_through, 200);
    assert_eq!(transition.next.grace_until, None);
    assert_eq!(transition.allowance, Some(invoice().selected_allowance));
    assert_eq!(effective_access(transition.next, 199), Access::Active);
    assert_eq!(effective_access(transition.next, 200), Access::ReadOnly);
    assert_eq!(
        effective_access(transition.next, u64::MAX),
        Access::ReadOnly
    );
}

#[test]
fn unverified_webhook_unknown_and_stale_inputs_cannot_propose_state_or_credit() {
    let state = pending();
    let before = state;
    for (observation, refusal) in [
        (
            SubscriptionObservation {
                authority: ObservationAuthority::Unverified,
                ..settled()
            },
            SubscriptionRefusal::Unverified,
        ),
        (
            SubscriptionObservation {
                authority: ObservationAuthority::WebhookOnly,
                ..settled()
            },
            SubscriptionRefusal::NeedsAuthoritativeRefresh,
        ),
        (
            SubscriptionObservation {
                outcome: PaymentOutcome::Unknown,
                ..settled()
            },
            SubscriptionRefusal::UnknownPayment,
        ),
        (
            SubscriptionObservation {
                expected_revision: 6,
                ..settled()
            },
            SubscriptionRefusal::StaleRevision,
        ),
        (
            SubscriptionObservation {
                object_revision: 4,
                ..settled()
            },
            SubscriptionRefusal::StaleObject,
        ),
        (
            SubscriptionObservation {
                object_revision: 3,
                ..settled()
            },
            SubscriptionRefusal::StaleObject,
        ),
    ] {
        assert_eq!(observe_subscription(state, observation, 100), Err(refusal));
        assert_eq!(state, before);
    }
}

#[test]
fn renewal_failure_uses_paid_boundary_and_never_extends_grace_or_allowance() {
    let state = active();
    let grace = observe_subscription(state, renewal_failure(state), 200).unwrap();
    assert_eq!(grace.allowance, None);
    assert_eq!(grace.next.access, Access::Grace);
    assert_eq!(grace.next.grace_until, Some(200 + GRACE_SECONDS));
    assert_eq!(effective_access(grace.next, 200), Access::Grace);
    assert_eq!(
        effective_access(grace.next, 200 + GRACE_SECONDS - 1),
        Access::Grace
    );
    assert_eq!(
        effective_access(grace.next, 200 + GRACE_SECONDS),
        Access::ReadOnly
    );
    let repeated = observe_subscription(grace.next, renewal_failure(grace.next), 250).unwrap();
    assert_eq!(repeated.next.grace_until, grace.next.grace_until);
    assert_eq!(repeated.allowance, None);
    let late = observe_subscription(
        repeated.next,
        renewal_failure(repeated.next),
        200 + GRACE_SECONDS,
    )
    .unwrap();
    assert_eq!(late.next.access, Access::ReadOnly);
    assert_eq!(late.next.grace_until, grace.next.grace_until);
    assert_eq!(late.allowance, None);
}

#[test]
fn settlement_after_failure_grants_once_and_old_invoice_cannot_reactivate() {
    let state = active();
    let grace = observe_subscription(state, renewal_failure(state), 200)
        .unwrap()
        .next;
    let old_delivery = SubscriptionObservation {
        expected_revision: grace.revision,
        object_revision: grace.object_revision + 1,
        ..settled()
    };
    assert_eq!(
        observe_subscription(grace, old_delivery, 201),
        Err(SubscriptionRefusal::InvalidPeriod)
    );
    let new_invoice = PaidInvoice {
        period_start: 200,
        paid_through: 400,
        selected_allowance: Money::new(state.currency, u128::MAX),
        allowance_already_recorded: false,
    };
    let renewal = SubscriptionObservation {
        outcome: PaymentOutcome::Settled(new_invoice),
        ..old_delivery
    };
    let renewed = observe_subscription(grace, renewal, 250).unwrap();
    assert_eq!(renewed.next.access, Access::Active);
    assert_eq!(renewed.next.grace_until, None);
    assert_eq!(renewed.allowance, Some(new_invoice.selected_allowance));
    let duplicate = SubscriptionObservation {
        outcome: PaymentOutcome::Settled(PaidInvoice {
            allowance_already_recorded: true,
            ..new_invoice
        }),
        ..renewal
    };
    assert_eq!(
        observe_subscription(grace, duplicate, 250),
        Err(SubscriptionRefusal::DuplicateAllowance)
    );
    assert_eq!(grace.access, Access::Grace);
    assert_eq!(
        observe_subscription(renewed.next, renewal, 250),
        Err(SubscriptionRefusal::StaleRevision)
    );
}

#[test]
fn invalid_period_currency_and_arithmetic_refuse_without_partial_transition() {
    let state = pending();
    let before = state;
    for invalid in [
        PaidInvoice {
            period_start: 200,
            ..invoice()
        },
        PaidInvoice {
            period_start: 101,
            ..invoice()
        },
        PaidInvoice {
            paid_through: 100,
            ..invoice()
        },
    ] {
        assert_eq!(
            observe_subscription(
                state,
                SubscriptionObservation {
                    outcome: PaymentOutcome::Settled(invalid),
                    ..settled()
                },
                100,
            ),
            Err(SubscriptionRefusal::InvalidPeriod)
        );
        assert_eq!(state, before);
    }
    assert_eq!(
        observe_subscription(
            state,
            SubscriptionObservation {
                outcome: PaymentOutcome::Settled(PaidInvoice {
                    selected_allowance: Money::new(Currency::parse("EUR").unwrap(), 123),
                    ..invoice()
                }),
                ..settled()
            },
            100,
        ),
        Err(SubscriptionRefusal::Money(MoneyError::CurrencyMismatch))
    );
    let exhausted = SubscriptionState {
        revision: u64::MAX,
        ..state
    };
    assert_eq!(
        observe_subscription(
            exhausted,
            SubscriptionObservation {
                expected_revision: u64::MAX,
                ..settled()
            },
            100,
        ),
        Err(SubscriptionRefusal::RevisionOverflow)
    );
    assert_eq!(exhausted.revision, u64::MAX);
    let late = SubscriptionState {
        paid_through: u64::MAX,
        ..active()
    };
    assert_eq!(
        observe_subscription(late, renewal_failure(late), u64::MAX),
        Err(SubscriptionRefusal::TimeOverflow)
    );
    assert_eq!(late.paid_through, u64::MAX);
    assert_eq!(state, before);
}

#[test]
fn initial_failure_and_restricted_state_never_create_allowance() {
    let failure = SubscriptionObservation {
        outcome: PaymentOutcome::InitialFailed,
        ..settled()
    };
    let denied = observe_subscription(pending(), failure, 100).unwrap();
    assert_eq!(denied.next.access, Access::NoAccess);
    assert_eq!(denied.allowance, None);
    for access in [Access::NoAccess, Access::Restricted] {
        let state = SubscriptionState {
            access,
            ..pending()
        };
        assert_eq!(effective_access(state, 100), access);
        assert_eq!(
            observe_subscription(state, settled(), 100),
            Err(SubscriptionRefusal::WrongState)
        );
        assert_eq!(
            observe_subscription(state, renewal_failure(state), 100),
            Err(SubscriptionRefusal::WrongState)
        );
    }
    let state = active();
    assert_eq!(
        observe_subscription(
            state,
            SubscriptionObservation {
                expected_revision: state.revision,
                object_revision: state.object_revision + 1,
                ..failure
            },
            100,
        ),
        Err(SubscriptionRefusal::WrongState)
    );
    assert_eq!(
        observe_subscription(state, renewal_failure(state), 199),
        Err(SubscriptionRefusal::InvalidPeriod)
    );
}

#[test]
fn clock_progression_never_grants_and_malformed_grace_cannot_extend_access() {
    let state = SubscriptionState {
        access: Access::Grace,
        paid_through: 200,
        grace_until: Some(200 + GRACE_SECONDS),
        ..pending()
    };
    assert_eq!(effective_access(state, 199), Access::ReadOnly);
    for deadline in [None, Some(200), Some(201 + GRACE_SECONDS), Some(u64::MAX)] {
        assert_eq!(
            effective_access(
                SubscriptionState {
                    grace_until: deadline,
                    ..state
                },
                250,
            ),
            Access::ReadOnly
        );
    }
    assert_eq!(
        effective_access(pending(), u64::MAX),
        Access::PendingInitial
    );
    assert_eq!(state.grace_until, Some(200 + GRACE_SECONDS));
}
