//! Native verdict qualification only. Actual rAF/socket evidence is a separate required run.
#[path = "../src/qualification/callback_progress.rs"]
mod callback_progress;
use callback_progress::{CallbackProgress, FramePoint, MAX_FRAME_SAMPLES, ObservationError};

fn point(time_ms: f64, bytes: usize, items: usize, yields: usize, classes: u8) -> FramePoint {
    FramePoint {
        time_ms,
        callback_bytes: bytes,
        callback_items: items,
        decode_yields: yields,
        completed_classes: classes,
    }
}
fn observed() -> CallbackProgress {
    let mut observation = CallbackProgress::default();
    observation.record(point(10.0, 100, 1, 0, 0)).unwrap();
    observation.record(point(30.0, 900, 3, 2, 1)).unwrap();
    observation
}

#[test]
fn callback_and_decode_progress_between_live_frames_is_observed() {
    let mut observation = observed();
    observation.record(point(50.0, 1500, 5, 3, 0b1111)).unwrap();
    assert_eq!(observation.qualify(), Ok(()));
    assert_eq!(observation.gaps(), vec![20.0]);
    let report = observation.report();
    assert!(report.contains("overlap_intervals=1"));
    assert!(report.contains("delivered=3 retained=2"));
    assert!(report.contains("time_ms: 30.0, callback_bytes: 900"));
    assert!(report.contains("Native verdict tests do not prove browser behavior"));
}

#[test]
fn empty_or_single_frame_does_not_prove_overlap() {
    let mut observation = CallbackProgress::default();
    assert_eq!(observation.qualify(), Err(ObservationError::NoOverlap));
    assert!(observation.gaps().is_empty());
    observation.record(point(10.0, 5000, 500, 70, 0)).unwrap();
    assert_eq!(observation.qualify(), Err(ObservationError::NoOverlap));
}

#[test]
fn old_callback_and_decode_totals_cannot_qualify_idle_frames() {
    let mut observation = CallbackProgress::default();
    for time in [10.0, 30.0, 50.0] {
        observation.record(point(time, 5000, 500, 70, 0)).unwrap();
    }
    assert_eq!(observation.qualify(), Err(ObservationError::NoOverlap));
}

#[test]
fn callback_and_decode_must_advance_in_the_same_active_interval() {
    let mut observation = CallbackProgress::default();
    for value in [
        point(10.0, 0, 0, 0, 0),
        point(30.0, 800, 2, 0, 0),
        point(50.0, 800, 2, 2, 0),
    ] {
        observation.record(value).unwrap();
    }
    assert_eq!(observation.qualify(), Err(ObservationError::NoOverlap));
}

#[test]
fn callback_byte_and_item_progress_are_both_required() {
    for value in [point(30.0, 800, 1, 2, 0), point(30.0, 100, 2, 2, 0)] {
        let mut observation = CallbackProgress::default();
        observation.record(point(10.0, 100, 1, 0, 0)).unwrap();
        observation.record(value).unwrap();
        assert_eq!(observation.qualify(), Err(ObservationError::NoOverlap));
    }
}

#[test]
fn first_frame_after_work_completion_cannot_qualify_prior_traffic() {
    for classes in [0, 0b1111] {
        let mut observation = CallbackProgress::default();
        observation.record(point(10.0, 100, 1, 0, classes)).unwrap();
        observation.record(point(30.0, 900, 3, 2, 0b1111)).unwrap();
        assert_eq!(observation.qualify(), Err(ObservationError::NoOverlap));
    }
}

#[test]
fn nonfinite_negative_equal_and_backwards_timestamps_fail_closed() {
    for time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 10.0, 9.0] {
        let mut observation = CallbackProgress::default();
        observation.record(point(10.0, 100, 1, 0, 0)).unwrap();
        assert_eq!(
            observation.record(point(time, 900, 3, 2, 0)),
            Err(ObservationError::InvalidTimestamp)
        );
        assert_eq!(
            observation.qualify(),
            Err(ObservationError::InvalidTimestamp)
        );
        assert!(observation.report().contains("Rejected observation"));
    }
    let mut observation = CallbackProgress::default();
    assert_eq!(
        observation.record(point(f64::NAN, 0, 0, 0, 0)),
        Err(ObservationError::InvalidTimestamp)
    );
}

#[test]
fn any_connection_counter_regression_invalidates_earlier_overlap() {
    for value in [
        point(50.0, 899, 3, 2, 1),
        point(50.0, 900, 2, 2, 1),
        point(50.0, 900, 3, 1, 1),
    ] {
        let mut observation = observed();
        assert_eq!(
            observation.record(value),
            Err(ObservationError::CounterRegression)
        );
        assert_eq!(
            observation.qualify(),
            Err(ObservationError::CounterRegression)
        );
    }
}

#[test]
fn completed_class_masks_must_be_known_and_never_regress() {
    let mut unknown = observed();
    assert_eq!(
        unknown.record(point(50.0, 900, 3, 2, 0b10000)),
        Err(ObservationError::InvalidClasses)
    );
    let mut regressed = observed();
    assert_eq!(
        regressed.record(point(50.0, 900, 3, 2, 0)),
        Err(ObservationError::CompletionRegression)
    );
    assert_eq!(
        regressed.qualify(),
        Err(ObservationError::CompletionRegression)
    );
}

#[test]
fn sample_limit_is_bounded_explicit_and_cannot_preserve_a_success_verdict() {
    let mut observation = CallbackProgress::default();
    for index in 0..MAX_FRAME_SAMPLES {
        observation
            .record(point(index as f64 * 250.0, index * 100, index, index, 0))
            .unwrap();
    }
    assert_eq!(observation.qualify(), Ok(()));
    assert_eq!(observation.gaps().len(), MAX_FRAME_SAMPLES - 1);
    assert_eq!(
        observation.record(point(
            MAX_FRAME_SAMPLES as f64 * 250.0,
            MAX_FRAME_SAMPLES * 100,
            MAX_FRAME_SAMPLES,
            MAX_FRAME_SAMPLES,
            0
        )),
        Err(ObservationError::SampleLimit)
    );
    assert_eq!(observation.qualify(), Err(ObservationError::SampleLimit));
    assert_eq!(observation.gaps().len(), MAX_FRAME_SAMPLES - 1);
    let report = observation.report();
    assert!(
        report
            .contains("delivered=129 retained=128 limit=128 timeline_unretained=1 truncated=true")
    );
    assert!(report.len() < 24 * 1024);
}

#[test]
fn invalid_observation_is_sticky_and_failed_points_do_not_supply_gaps() {
    let mut observation = observed();
    assert_eq!(
        observation.record(point(5.0, 2000, 6, 4, 1)),
        Err(ObservationError::InvalidTimestamp)
    );
    assert_eq!(
        observation.record(point(60.0, 2000, 6, 4, 1)),
        Err(ObservationError::InvalidTimestamp)
    );
    assert!(observation.gaps().is_empty());
    assert!(observation.report().contains("delivered=3 retained=1"));
}

#[test]
fn partial_observations_remain_available_to_deadline_and_transport_error_reports() {
    let observation = observed();
    let before = observation.report();
    assert!(before.contains("10.0,100,1,0,0000,None"));
    assert!(before.contains("time_ms: 30.0, callback_bytes: 900"));
    let terminal =
        format!("INCONCLUSIVE · owned deadline; source-bound browser run incomplete\n{before}");
    assert!(terminal.contains("overlap_intervals=1"));
    assert!(terminal.contains("no production latency budget"));
}

#[test]
fn late_page_uptime_retains_lossless_adjacent_witness_and_gap() {
    let mut observation = CallbackProgress::default();
    observation
        .record(point(1_000_000.125, 100, 1, 0, 0))
        .unwrap();
    observation
        .record(point(1_000_016.375, 900, 3, 2, 1))
        .unwrap();
    observation
        .record(point(1_000_032.625, 900, 3, 2, 15))
        .unwrap();
    assert_eq!(observation.qualify(), Ok(()));
    assert_eq!(observation.gaps(), vec![16.25]);
    let report = observation.report();
    assert!(report.contains("time_ms: 1000000.125"));
    assert!(report.contains("time_ms: 1000016.375"));
    assert!(report.contains("gap_ms=16.25"));
    assert!(report.contains("Maximum adjacent rAF gap across validated frames: Some(16.25)ms"));
}

#[test]
fn high_refresh_frames_fit_owned_thirty_second_observation_without_truncation() {
    for refresh in [60, 120, 144, 240] {
        let mut observation = CallbackProgress::default();
        for index in 0..=refresh * 30 {
            observation
                .record(point(
                    index as f64 * 1000.0 / refresh as f64,
                    index * 100,
                    index,
                    index,
                    0,
                ))
                .unwrap();
        }
        let final_index = refresh * 30 + 1;
        observation
            .record(point(
                final_index as f64 * 1000.0 / refresh as f64,
                final_index * 100,
                final_index,
                final_index,
                15,
            ))
            .unwrap();
        assert_eq!(observation.qualify(), Ok(()));
        assert!(observation.gaps().len() <= 121);
        assert!(observation.report().contains("truncated=false"));
        assert!(observation.report().len() < 24 * 1024);
    }
}

#[test]
fn sampled_endpoint_deltas_cannot_substitute_for_adjacent_overlap() {
    let mut observation = CallbackProgress::default();
    for value in [
        point(0.0, 0, 0, 0, 0),
        point(100.0, 800, 2, 0, 0),
        point(200.0, 800, 2, 2, 0),
        point(250.0, 800, 2, 2, 0),
    ] {
        observation.record(value).unwrap();
    }
    assert_eq!(observation.qualify(), Err(ObservationError::NoOverlap));
    assert_eq!(observation.gaps(), vec![50.0]);
    assert!(observation.report().contains("timeline_unretained=2"));
}

#[test]
fn unretained_frames_still_validate_and_measure_adjacent_gap() {
    let mut observation = observed();
    observation.record(point(130.0, 900, 3, 2, 1)).unwrap();
    assert!(
        observation
            .report()
            .contains("Maximum adjacent rAF gap across validated frames: Some(100.0)ms")
    );
    assert_eq!(
        observation.record(point(140.0, 899, 3, 2, 1)),
        Err(ObservationError::CounterRegression)
    );
    assert_eq!(
        observation.qualify(),
        Err(ObservationError::CounterRegression)
    );
}

#[test]
fn maximum_width_bounded_observer_leaves_room_for_full_failure_and_cleanup_report() {
    let mut observation = CallbackProgress::default();
    for index in 0..MAX_FRAME_SAMPLES {
        observation
            .record(point(
                1_000_000.125 + index as f64 * 250.0,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                0,
            ))
            .unwrap();
    }
    let observer = observation.report();
    assert!(observer.len() < 24 * 1024);
    // Existing reports carry the full snapshots/build/first failure outside this observer.
    let prefix = "x".repeat(40 * 1024);
    assert!((prefix + &observer).len() < 64 * 1024);
    assert_eq!(observation.qualify(), Err(ObservationError::NoOverlap));
}

#[test]
fn no_validated_frame_pair_reports_gap_as_unobserved() {
    let mut observation = CallbackProgress::default();
    assert!(
        observation
            .report()
            .contains("Maximum adjacent rAF gap across validated frames: None")
    );
    observation.record(point(10.0, 100, 1, 0, 0)).unwrap();
    assert!(
        observation
            .report()
            .contains("Maximum adjacent rAF gap across validated frames: None")
    );
}

#[test]
fn publication_failure_preserves_measured_build_and_cleanup_before_observer() {
    let observation = observed();
    let measured = callback_progress::PressureReport {
        text: "Build: exact-build\nPost-close: closed=true callback_bytes=0 callback_items=0 retained_capacity=0".into(),
        retained_in_failure: false,
    };
    let report = callback_progress::terminal_report(
        "INCONCLUSIVE · publication failed".into(),
        Some(&measured),
        &observation,
    );
    assert!(report.contains("INCONCLUSIVE · publication failed"));
    assert!(report.contains("Build: exact-build"));
    assert!(report.contains("closed=true callback_bytes=0 callback_items=0 retained_capacity=0"));
    assert!(report.contains("Exact adjacent overlap witness"));
}

#[test]
fn first_failure_owns_measured_report_once_without_diagnostic_string_parsing() {
    let observation = observed();
    let measured = callback_progress::PressureReport {
        text: "first measured failure\nBuild: exact-build\nclosed=true callback_bytes=0".into(),
        retained_in_failure: true,
    };
    let report =
        callback_progress::terminal_report(measured.text.clone(), Some(&measured), &observation);
    assert_eq!(report.matches("first measured failure").count(), 1);
    assert_eq!(
        report
            .matches("Callback/render overlap observation:")
            .count(),
        1
    );
    let misleading = callback_progress::PressureReport {
        text: "upstream diagnostic literally contains Callback/render overlap observation:".into(),
        retained_in_failure: false,
    };
    let report =
        callback_progress::terminal_report("INCONCLUSIVE".into(), Some(&misleading), &observation);
    assert!(report.contains("Exact adjacent overlap witness"));
}

#[test]
fn deadline_with_no_measured_pressure_still_retains_partial_observer() {
    let observation = observed();
    let report = callback_progress::terminal_report(
        "INCONCLUSIVE · owned deadline; post-close counters UNPERFORMED".into(),
        None,
        &observation,
    );
    assert!(report.contains("post-close counters UNPERFORMED"));
    assert!(report.contains("Exact adjacent overlap witness"));
}
