use std::time::Duration;

use df_testkit::{ClockDomain, ClockError, ClockSnapshot, VirtualClocks};

#[test]
fn caller_supplied_anchors_advance_independently_with_nanosecond_precision() {
    let logical = Duration::from_secs(200);
    let presentation = Duration::from_secs(7);
    let mut clocks = VirtualClocks::new(logical, presentation);
    let initial = clocks.snapshot();

    assert_eq!(
        clocks.advance_presentation_by(Duration::from_nanos(1)),
        Ok(presentation + Duration::from_nanos(1))
    );
    assert_eq!(clocks.snapshot().logical, logical);
    assert_eq!(initial.presentation, presentation);
    assert_eq!(
        clocks.advance_logical_by(Duration::from_secs(3600)),
        Ok(Duration::from_secs(3800))
    );
    assert_eq!(
        clocks.snapshot(),
        ClockSnapshot {
            logical: Duration::from_secs(3800),
            presentation: presentation + Duration::from_nanos(1),
        }
    );
}

#[test]
fn equal_and_zero_advances_preserve_both_anchors_even_at_maximum_duration() {
    let mut clocks = VirtualClocks::new(Duration::MAX, Duration::MAX);
    let before = clocks.snapshot();

    assert_eq!(clocks.advance_logical_by(Duration::ZERO), Ok(Duration::MAX));
    assert_eq!(
        clocks.advance_presentation_by(Duration::ZERO),
        Ok(Duration::MAX)
    );
    assert_eq!(clocks.advance_logical_to(Duration::MAX), Ok(Duration::MAX));
    assert_eq!(
        clocks.advance_presentation_to(Duration::MAX),
        Ok(Duration::MAX)
    );
    assert_eq!(clocks.snapshot(), before);
}

#[test]
fn regression_refuses_the_named_domain_without_changing_either_clock() {
    let current = Duration::from_secs(10);
    let requested = current - Duration::from_nanos(1);
    let mut clocks = VirtualClocks::new(current, current);
    let before = clocks.snapshot();

    assert_eq!(
        clocks.advance_logical_to(requested),
        Err(ClockError::TimeRegression {
            domain: ClockDomain::Logical,
            current,
            requested,
        })
    );
    assert_eq!(clocks.snapshot(), before);
    assert_eq!(
        clocks.advance_presentation_to(requested),
        Err(ClockError::TimeRegression {
            domain: ClockDomain::Presentation,
            current,
            requested,
        })
    );
    assert_eq!(clocks.snapshot(), before);
    assert_eq!(
        clocks.advance_presentation_to(current + Duration::from_nanos(1)),
        Ok(current + Duration::from_nanos(1))
    );
    assert_eq!(clocks.snapshot().logical, current);
}

#[test]
fn overflow_refuses_without_wrap_saturation_or_partial_mutation() {
    let current = Duration::MAX - Duration::from_nanos(1);
    let delta = Duration::from_nanos(2);
    let mut clocks = VirtualClocks::new(current, current);
    let before = clocks.snapshot();

    assert_eq!(
        clocks.advance_logical_by(delta),
        Err(ClockError::Overflow {
            domain: ClockDomain::Logical,
            current,
            delta,
        })
    );
    assert_eq!(clocks.snapshot(), before);
    assert_eq!(
        clocks.advance_presentation_by(delta),
        Err(ClockError::Overflow {
            domain: ClockDomain::Presentation,
            current,
            delta,
        })
    );
    assert_eq!(clocks.snapshot(), before);
    assert_eq!(
        clocks.advance_logical_by(Duration::from_nanos(1)),
        Ok(Duration::MAX)
    );
    assert_eq!(clocks.snapshot().presentation, current);
}
