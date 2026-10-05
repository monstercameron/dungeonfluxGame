use std::time::Duration;

use df_experience::window::{
    AcceptedActivity, ActivityWindow, ActivityWindowLimits, Observation, WindowError,
};
use df_types::{OperationId, RecoveryEpoch, RunId, SessionId, SessionRevision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Participant(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Event(u16);

fn session(value: u8) -> SessionId {
    SessionId::from_bytes(&[value; 16]).unwrap()
}

fn run(value: u8) -> RunId {
    RunId::from_bytes(&[value; 16]).unwrap()
}

fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
}

fn time(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

fn window(max_events: usize) -> ActivityWindow<Participant, Event> {
    ActivityWindow::new(
        session(1),
        run(2),
        ActivityWindowLimits {
            max_events,
            horizon: time(10),
        },
    )
    .unwrap()
}

fn activity(participant: u16, event: u16, at: u64) -> AcceptedActivity<Participant, Event> {
    AcceptedActivity {
        session: session(1),
        run: run(2),
        operation: OperationId::from_bytes(&[3; 16]).unwrap(),
        revision: revision(1, 7),
        participant: Participant(participant),
        event: Event(event),
        at: time(at),
    }
}

#[test]
fn accepted_events_dedupe_without_coalescing_distinct_events_in_one_operation() {
    let mut window = window(4);
    let first = activity(1, 10, 0);
    assert!(window.is_empty());
    assert_eq!(
        window.observe(first, revision(1, 9), time(5)),
        Ok(Observation::Inserted { removed_expired: 0 })
    );
    assert_eq!(
        window.observe(first, revision(1, 9), time(5)),
        Ok(Observation::Duplicate { removed_expired: 0 })
    );
    window
        .observe(activity(1, 11, 3), revision(1, 9), time(5))
        .unwrap();
    window
        .observe(activity(2, 12, 4), revision(1, 9), time(5))
        .unwrap();
    assert_eq!(window.activity_count(&Participant(1)), 2);
    assert_eq!(window.activity_count(&Participant(2)), 1);
    assert_eq!(window.activity_count(&Participant(99)), 0);
    assert_eq!(window.len(), 3);
}

#[test]
fn conflicting_retained_identity_refuses_without_changing_counts() {
    let mut window = window(4);
    let event = activity(1, 10, 3);
    window.observe(event, revision(1, 9), time(5)).unwrap();
    let mut conflicts = [event; 4];
    conflicts[0].participant = Participant(2);
    conflicts[1].operation = OperationId::from_bytes(&[4; 16]).unwrap();
    conflicts[2].revision = revision(1, 8);
    conflicts[3].at = time(4);
    for conflict in conflicts {
        assert_eq!(
            window.observe(conflict, revision(1, 9), time(6)),
            Err(WindowError::EventIdConflict)
        );
        assert_eq!(window.len(), 1);
        assert_eq!(window.activity_count(&Participant(1)), 1);
        assert_eq!(window.activity_count(&Participant(2)), 0);
    }
    // Failed observations did not advance the owned clock.
    assert_eq!(
        window.observe(event, revision(1, 9), time(5)),
        Ok(Observation::Duplicate { removed_expired: 0 })
    );
}

#[test]
fn out_of_order_committed_events_within_the_current_window_are_counted() {
    let mut window = window(4);
    window
        .observe(activity(1, 10, 8), revision(1, 9), time(9))
        .unwrap();
    let mut earlier = activity(1, 11, 2);
    earlier.revision = revision(1, 3);
    window.observe(earlier, revision(1, 9), time(9)).unwrap();
    assert_eq!(window.activity_count(&Participant(1)), 2);
    assert_eq!(window.advance(time(12)), Ok(1));
    assert_eq!(window.activity_count(&Participant(1)), 1);
}

#[test]
fn expired_replay_is_not_reinserted_after_dedupe_records_are_released() {
    let mut window = window(2);
    let old = activity(1, 10, 0);
    window.observe(old, revision(1, 9), time(0)).unwrap();
    assert_eq!(window.advance(time(9)), Ok(0));
    assert_eq!(window.advance(time(10)), Ok(1));
    assert!(window.is_empty());
    assert_eq!(
        window.observe(old, revision(1, 9), time(10)),
        Err(WindowError::ExpiredActivity)
    );
    assert_eq!(
        window.observe(old, revision(1, 9), time(100)),
        Err(WindowError::ExpiredActivity)
    );
    window
        .observe(activity(1, 11, 11), revision(1, 9), time(11))
        .unwrap();
    assert_eq!(window.len(), 1);
}

#[test]
fn overflow_refuses_without_evicting_or_advancing_time_then_expiry_frees_capacity() {
    let mut window = window(2);
    let first = activity(1, 10, 1);
    window.observe(first, revision(1, 9), time(5)).unwrap();
    window
        .observe(activity(2, 11, 2), revision(1, 9), time(5))
        .unwrap();
    let refused = activity(3, 12, 5);
    assert_eq!(
        window.observe(refused, revision(1, 9), time(6)),
        Err(WindowError::Capacity)
    );
    assert_eq!(window.len(), 2);
    assert_eq!(window.activity_count(&Participant(3)), 0);
    assert_eq!(
        window.observe(first, revision(1, 9), time(5)),
        Ok(Observation::Duplicate { removed_expired: 0 })
    );
    assert_eq!(window.advance(time(11)), Ok(1));
    assert_eq!(
        window.observe(refused, revision(1, 9), time(11)),
        Ok(Observation::Inserted { removed_expired: 0 })
    );
    assert_eq!(window.activity_count(&Participant(1)), 0);
    assert_eq!(window.activity_count(&Participant(2)), 1);
    assert_eq!(window.activity_count(&Participant(3)), 1);
    // An ordinary admission also expires old records before checking live capacity.
    assert_eq!(
        window.observe(activity(4, 13, 12), revision(1, 9), time(12)),
        Ok(Observation::Inserted { removed_expired: 1 })
    );
    assert_eq!(window.len(), 2);
    assert_eq!(window.activity_count(&Participant(2)), 0);
    assert_eq!(window.activity_count(&Participant(3)), 1);
    assert_eq!(window.activity_count(&Participant(4)), 1);
}

#[test]
fn provenance_and_time_refusals_preserve_the_whole_window() {
    let mut window = window(2);
    window
        .observe(activity(1, 1, 1), revision(1, 9), time(5))
        .unwrap();
    let candidate = activity(2, 2, 4);
    let mut variants = [candidate; 5];
    variants[0].session = session(9);
    variants[1].run = run(9);
    variants[2].revision = revision(2, 1);
    variants[3].revision = revision(1, 10);
    variants[4].at = time(7);
    let expected = [
        WindowError::WrongSession,
        WindowError::WrongRun,
        WindowError::WrongEpoch,
        WindowError::FutureRevision,
        WindowError::FutureActivity,
    ];
    for (variant, expected) in variants.into_iter().zip(expected) {
        assert_eq!(
            window.observe(variant, revision(1, 9), time(6)),
            Err(expected)
        );
        assert_eq!(window.len(), 1);
        assert_eq!(window.activity_count(&Participant(1)), 1);
        assert_eq!(window.activity_count(&Participant(2)), 0);
    }
    assert_eq!(
        window.observe(candidate, revision(1, 9), time(4)),
        Err(WindowError::TimeRegression)
    );
    assert_eq!(window.advance(time(4)), Err(WindowError::TimeRegression));
    window.observe(candidate, revision(1, 9), time(5)).unwrap();
    assert_eq!(window.len(), 2);
}

#[test]
fn a_duplicate_can_expire_other_records_and_reports_the_removed_count() {
    let mut window = window(2);
    window
        .observe(activity(1, 1, 0), revision(1, 9), time(9))
        .unwrap();
    let retained = activity(2, 2, 8);
    window.observe(retained, revision(1, 9), time(9)).unwrap();
    assert_eq!(
        window.observe(retained, revision(1, 9), time(10)),
        Ok(Observation::Duplicate { removed_expired: 1 })
    );
    assert_eq!(window.len(), 1);
    assert_eq!(window.activity_count(&Participant(1)), 0);
    assert_eq!(window.activity_count(&Participant(2)), 1);
}

#[test]
fn a_window_cannot_mix_successful_observation_epochs_even_after_expiry() {
    let mut window = window(2);
    let first = activity(1, 1, 1);
    window.observe(first, revision(1, 9), time(5)).unwrap();
    let mut next_epoch = activity(2, 2, 4);
    next_epoch.revision = revision(2, 1);

    assert_eq!(
        window.observe(next_epoch, revision(2, 1), time(6)),
        Err(WindowError::WrongEpoch)
    );
    assert_eq!(window.activity_count(&Participant(1)), 1);
    assert_eq!(window.activity_count(&Participant(2)), 0);
    assert_eq!(
        window.observe(first, revision(1, 9), time(5)),
        Ok(Observation::Duplicate { removed_expired: 0 })
    );

    assert_eq!(window.advance(time(11)), Ok(1));
    next_epoch.at = time(11);
    assert_eq!(
        window.observe(next_epoch, revision(2, 1), time(11)),
        Err(WindowError::WrongEpoch)
    );
    assert!(window.is_empty());
}

#[test]
fn accepted_owner_basis_cannot_regress_but_historical_activity_is_valid() {
    let mut window = window(4);
    let first = activity(1, 1, 1);
    window.observe(first, revision(1, 9), time(5)).unwrap();
    let mut historical = activity(2, 2, 4);
    historical.revision = revision(1, 3);
    assert_eq!(
        window.observe(historical, revision(1, 6), time(6)),
        Err(WindowError::StaleBasis)
    );
    assert_eq!(window.len(), 1);
    assert_eq!(window.activity_count(&Participant(2)), 0);
    assert_eq!(
        window.observe(historical, revision(1, 10), time(5)),
        Ok(Observation::Inserted { removed_expired: 0 })
    );
    assert_eq!(
        window.observe(historical, revision(1, 11), time(6)),
        Ok(Observation::Duplicate { removed_expired: 0 })
    );
    assert_eq!(window.activity_count(&Participant(1)), 1);
    assert_eq!(window.activity_count(&Participant(2)), 1);
    assert_eq!(
        window.observe(historical, revision(1, 10), time(6)),
        Err(WindowError::StaleBasis)
    );

    assert_eq!(window.advance(time(20)), Ok(2));
    historical.at = time(20);
    assert_eq!(
        window.observe(historical, revision(1, 10), time(20)),
        Err(WindowError::StaleBasis)
    );
    assert!(window.is_empty());
}

#[test]
fn first_refused_observation_does_not_bind_recovery_epoch_or_clock() {
    let mut window = window(1);
    let mut future = activity(1, 1, 7);
    future.revision = revision(2, 1);
    assert_eq!(
        window.observe(future, revision(2, 1), time(6)),
        Err(WindowError::FutureActivity)
    );
    assert!(window.is_empty());
    assert_eq!(
        window.observe(activity(1, 1, 4), revision(1, 9), time(5)),
        Ok(Observation::Inserted { removed_expired: 0 })
    );
}

#[test]
fn refused_capacity_does_not_advance_the_accepted_owner_basis() {
    let mut window = window(1);
    let first = activity(1, 1, 1);
    window.observe(first, revision(1, 9), time(5)).unwrap();
    assert_eq!(
        window.observe(activity(2, 2, 4), revision(1, 10), time(6)),
        Err(WindowError::Capacity)
    );
    assert_eq!(window.len(), 1);
    assert_eq!(window.activity_count(&Participant(2)), 0);
    assert_eq!(
        window.observe(first, revision(1, 9), time(5)),
        Ok(Observation::Duplicate { removed_expired: 0 })
    );
}

#[test]
fn an_expired_refusal_does_not_silently_prune_unrelated_records() {
    let mut window = window(2);
    let old = activity(1, 1, 0);
    window.observe(old, revision(1, 9), time(9)).unwrap();
    assert_eq!(
        window.observe(old, revision(1, 9), time(10)),
        Err(WindowError::ExpiredActivity)
    );
    assert_eq!(window.len(), 1);
    assert_eq!(window.advance(time(10)), Ok(1));
}

#[test]
fn explicit_count_time_and_inline_byte_bounds_are_required_before_allocation() {
    for limits in [
        ActivityWindowLimits {
            max_events: 0,
            horizon: time(1),
        },
        ActivityWindowLimits {
            max_events: 4097,
            horizon: time(1),
        },
        ActivityWindowLimits {
            max_events: 1,
            horizon: Duration::ZERO,
        },
    ] {
        assert!(matches!(
            ActivityWindow::<Participant, Event>::new(session(1), run(2), limits),
            Err(WindowError::InvalidLimits)
        ));
    }
    assert!(matches!(
        ActivityWindow::<[u8; 4096], Event>::new(
            session(1),
            run(2),
            ActivityWindowLimits {
                max_events: 4096,
                horizon: time(1)
            }
        ),
        Err(WindowError::RecordBudget)
    ));
    let mut maximum = ActivityWindow::<Participant, Event>::new(
        session(1),
        run(2),
        ActivityWindowLimits {
            max_events: 4096,
            horizon: Duration::MAX,
        },
    )
    .unwrap();
    assert_eq!(
        maximum.observe(activity(1, 1, 0), revision(1, 9), Duration::MAX),
        Err(WindowError::ExpiredActivity)
    );
    assert!(maximum.is_empty());
}
