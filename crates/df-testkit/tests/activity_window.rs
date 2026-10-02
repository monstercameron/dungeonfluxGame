use std::time::Duration;

use df_experience::window::{
    AcceptedActivity, ActivityWindow, ActivityWindowLimits, Observation, WindowError,
};
use df_testkit::{ClockDomain, ClockError, VirtualClocks};
use df_types::{OperationId, RecoveryEpoch, RunId, SessionId, SessionRevision};

fn activity() -> AcceptedActivity<u8, u8> {
    AcceptedActivity {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        operation: OperationId::from_bytes(&[3; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 0),
        participant: 1,
        event: 1,
        at: Duration::ZERO,
    }
}

fn window(event: AcceptedActivity<u8, u8>) -> ActivityWindow<u8, u8> {
    ActivityWindow::new(
        event.session,
        event.run,
        ActivityWindowLimits {
            max_events: 1,
            horizon: Duration::from_secs(10),
        },
    )
    .unwrap()
}

#[test]
fn presentation_during_campaign_pause_does_not_expire_logical_activity() {
    let mut clocks = VirtualClocks::new(Duration::ZERO, Duration::ZERO);
    let event = activity();
    let mut window = window(event);
    assert_eq!(
        window.observe(event, event.revision, clocks.snapshot().logical),
        Ok(Observation::Inserted { removed_expired: 0 })
    );

    // A paused fixture advances presentation alone, without sleeping or a game rule.
    clocks
        .advance_presentation_by(Duration::from_secs(86_400))
        .unwrap();
    assert_eq!(window.advance(clocks.snapshot().logical), Ok(0));
    assert_eq!(window.activity_count(&1), 1);

    clocks
        .advance_logical_to(Duration::from_secs(10) - Duration::from_nanos(1))
        .unwrap();
    assert_eq!(window.advance(clocks.snapshot().logical), Ok(0));
    clocks.advance_logical_by(Duration::from_nanos(1)).unwrap();
    assert_eq!(window.advance(clocks.snapshot().logical), Ok(1));
    assert!(window.is_empty());
    assert_eq!(clocks.snapshot().presentation, Duration::from_secs(86_400));
    assert_eq!(
        window.observe(event, event.revision, clocks.snapshot().logical),
        Err(WindowError::ExpiredActivity)
    );
}

#[test]
fn rejected_clock_rewind_preserves_consumer_time_and_later_expiry() {
    let mut clocks = VirtualClocks::new(Duration::ZERO, Duration::from_secs(30));
    let event = activity();
    let mut window = window(event);
    window
        .observe(event, event.revision, clocks.snapshot().logical)
        .unwrap();
    clocks.advance_logical_to(Duration::from_secs(9)).unwrap();
    assert_eq!(window.advance(clocks.snapshot().logical), Ok(0));
    let before = clocks.snapshot();

    assert_eq!(
        clocks.advance_logical_to(Duration::from_secs(8)),
        Err(ClockError::TimeRegression {
            domain: ClockDomain::Logical,
            current: Duration::from_secs(9),
            requested: Duration::from_secs(8),
        })
    );
    assert_eq!(clocks.snapshot(), before);
    assert_eq!(window.advance(clocks.snapshot().logical), Ok(0));
    assert_eq!(window.activity_count(&1), 1);

    clocks.advance_logical_by(Duration::from_secs(1)).unwrap();
    assert_eq!(window.advance(clocks.snapshot().logical), Ok(1));
    assert!(window.is_empty());
    assert_eq!(clocks.snapshot().presentation, Duration::from_secs(30));
}
