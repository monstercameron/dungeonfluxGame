mod tempo_fixture;

use df_model::checkpoint::{CheckpointError, ExecutionMode};
use df_tempo::elapsed::*;
use df_types::{BuildIdentity, RunId, SessionId};
use std::time::Duration;
use tempo_fixture::*;

#[test]
fn explicit_elapsed_changes_only_presentation_anchor() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![(content(), 7)], 40);
    let original = checkpoint.clone();
    let advance = advance_elapsed(
        &checkpoint,
        checkpoint.pins(),
        request(&checkpoint, Duration::from_millis(2500), false),
        Some(&policy(&checkpoint)),
        limits(),
    )
    .unwrap();
    let mut expected = checkpoint.state().tempo.clone();
    expected.presentation_ticks = 65;
    assert_eq!(advance.state, expected);
    assert_eq!(advance.elapsed_ticks, 25);
    assert_eq!(advance.disposition, ElapsedDisposition::Advanced);
    assert_eq!(checkpoint, original);
    assert!(!checkpoint.state().activity[0].spotlight_opt_in);
}

#[test]
fn zero_elapsed_has_no_new_work() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let result = advance_elapsed(
        &checkpoint,
        checkpoint.pins(),
        request(&checkpoint, Duration::ZERO, false),
        Some(&policy(&checkpoint)),
        limits(),
    )
    .unwrap();
    assert_eq!(result.state, checkpoint.state().tempo);
    assert_eq!(result.elapsed_ticks, 0);
    assert_eq!(result.disposition, ElapsedDisposition::Unchanged);
}

#[test]
fn pause_does_not_catch_up_huge_elapsed_or_change_logical_time() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![(content(), 7)], 40);
    let original = checkpoint.clone();
    let result = advance_elapsed(
        &checkpoint,
        checkpoint.pins(),
        request(&checkpoint, Duration::MAX, true),
        Some(&policy(&checkpoint)),
        limits(),
    )
    .unwrap();
    assert_eq!(result.state, checkpoint.state().tempo);
    assert_eq!(result.elapsed_ticks, 0);
    assert_eq!(result.disposition, ElapsedDisposition::Paused);
    assert_eq!(checkpoint, original);
}

#[test]
fn replay_never_repeats_elapsed_presentation_work() {
    let checkpoint = checkpoint_for(ExecutionMode::Replay, vec![(content(), 7)], 40);
    let result = advance_elapsed(
        &checkpoint,
        checkpoint.pins(),
        request(&checkpoint, Duration::MAX, false),
        Some(&policy(&checkpoint)),
        limits(),
    )
    .unwrap();
    assert_eq!(result.state, checkpoint.state().tempo);
    assert_eq!(result.disposition, ElapsedDisposition::Replay);
    assert_eq!(result.elapsed_ticks, 0);
}

#[test]
fn prepared_only_returns_data_without_mutating_state() {
    let checkpoint = checkpoint_for(ExecutionMode::PreparedOnly, vec![], 40);
    let result = advance_elapsed(
        &checkpoint,
        checkpoint.pins(),
        request(&checkpoint, Duration::from_secs(1), false),
        Some(&policy(&checkpoint)),
        limits(),
    )
    .unwrap();
    assert_eq!(result.state.presentation_ticks, 50);
    assert_eq!(checkpoint.state().tempo.presentation_ticks, 40);
    assert!(checkpoint.state().intents.is_empty());
}

#[test]
fn fractional_tick_is_refused_without_rounding_or_hidden_remainder() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![], 40);
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            request(&checkpoint, Duration::from_nanos(1), false),
            Some(&policy(&checkpoint)),
            limits()
        ),
        Err(TempoAdvanceError::FractionalTicks)
    );
    let exact = advance_elapsed(
        &checkpoint,
        checkpoint.pins(),
        request(&checkpoint, Duration::from_millis(100), false),
        Some(&policy(&checkpoint)),
        limits(),
    )
    .unwrap();
    assert_eq!(exact.elapsed_ticks, 1);
}

#[test]
fn elapsed_limit_is_exact_and_refuses_excess_without_partial_advance() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![], 40);
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            request(&checkpoint, Duration::from_secs(11), false),
            Some(&policy(&checkpoint)),
            limits()
        ),
        Err(TempoAdvanceError::ElapsedLimit)
    );
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            request(&checkpoint, Duration::from_secs(10), false),
            Some(&policy(&checkpoint)),
            limits()
        )
        .unwrap()
        .state
        .presentation_ticks,
        140
    );
}

#[test]
fn presentation_anchor_overflow_and_duration_conversion_overflow_refuse() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![], u64::MAX);
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            request(&checkpoint, Duration::from_millis(100), false),
            Some(&policy(&checkpoint)),
            limits()
        ),
        Err(TempoAdvanceError::TimeOverflow)
    );
    let mut admitted = policy(&checkpoint);
    admitted.ticks_per_second = u32::MAX;
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            request(&checkpoint, Duration::from_secs(u64::MAX), false),
            Some(&admitted),
            limits()
        ),
        Err(TempoAdvanceError::TimeOverflow)
    );
}

#[test]
fn stale_basis_pins_game_time_and_presentation_anchor_are_distinct() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let original = request(&checkpoint, Duration::from_secs(1), false);
    let mut supplied = original;
    supplied.expected_basis.revision = revision(7);
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            supplied,
            Some(&policy(&checkpoint)),
            limits()
        ),
        Err(TempoAdvanceError::Binding(CheckpointError::StaleBasis))
    );
    let mut stale = pins();
    stale.content.content = label("old-content");
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            &stale,
            original,
            Some(&policy(&checkpoint)),
            limits()
        ),
        Err(TempoAdvanceError::Binding(CheckpointError::ContentMismatch))
    );
    supplied = original;
    supplied.observed_logical_time.ticks += 1;
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            supplied,
            Some(&policy(&checkpoint)),
            limits()
        ),
        Err(TempoAdvanceError::LogicalTimeMismatch)
    );
    supplied = original;
    supplied.from_presentation_ticks -= 1;
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            supplied,
            Some(&policy(&checkpoint)),
            limits()
        ),
        Err(TempoAdvanceError::StalePresentationAnchor)
    );
}

#[test]
fn session_run_and_full_source_pins_refuse_alias_substitutions() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let original = checkpoint.clone();
    let input = request(&checkpoint, Duration::from_secs(1), false);
    let admitted_policy = policy(&checkpoint);

    let mut wrong_session = input;
    wrong_session.expected_basis.session = SessionId::from_bytes(&[6; 16]).unwrap();
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            wrong_session,
            Some(&admitted_policy),
            limits()
        ),
        Err(TempoAdvanceError::Binding(CheckpointError::WrongSession))
    );
    let mut wrong_run = input;
    wrong_run.expected_basis.run = RunId::from_bytes(&[7; 16]).unwrap();
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            wrong_run,
            Some(&admitted_policy),
            limits()
        ),
        Err(TempoAdvanceError::Binding(CheckpointError::WrongRun))
    );

    let mut source_manifest = checkpoint.pins().clone();
    source_manifest.rules.source_manifest_digest.0[0] ^= 1;
    let mut handler = checkpoint.pins().clone();
    handler.rules.handler_digest.0[0] ^= 1;
    let mut catalog = checkpoint.pins().clone();
    catalog.rules.catalog_digest.0[0] ^= 1;
    for stale in [&source_manifest, &handler, &catalog] {
        assert_eq!(
            advance_elapsed(&checkpoint, stale, input, Some(&admitted_policy), limits()),
            Err(TempoAdvanceError::Binding(CheckpointError::RulesMismatch))
        );
    }

    let mut content_digest = checkpoint.pins().clone();
    content_digest.content.content_digest.0[0] ^= 1;
    let mut package_digest = checkpoint.pins().clone();
    package_digest.content.package_digest.0[0] ^= 1;
    for stale in [&content_digest, &package_digest] {
        assert_eq!(
            advance_elapsed(&checkpoint, stale, input, Some(&admitted_policy), limits()),
            Err(TempoAdvanceError::Binding(CheckpointError::ContentMismatch))
        );
    }

    let mut build = checkpoint.pins().clone();
    build.build = BuildIdentity::new(
        Some("other-source-1"),
        Some("fixture-native-1"),
        Some("fixture-wasm-1"),
        Some("fixture-config-1"),
        Some("fixture-content-1"),
    )
    .unwrap();
    assert_eq!(
        advance_elapsed(&checkpoint, &build, input, Some(&admitted_policy), limits()),
        Err(TempoAdvanceError::Binding(CheckpointError::BuildMismatch))
    );
    assert_eq!(checkpoint, original);
}

#[test]
fn absent_mismatched_or_invalid_policy_never_selects_defaults() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let input = request(&checkpoint, Duration::from_secs(1), false);
    assert_eq!(
        advance_elapsed(&checkpoint, checkpoint.pins(), input, None, limits()),
        Err(TempoAdvanceError::PolicyUnavailable)
    );
    let mut admitted = policy(&checkpoint);
    admitted.definition.entry = label("other-policy");
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            input,
            Some(&admitted),
            limits()
        ),
        Err(TempoAdvanceError::PolicyMismatch)
    );
    admitted = policy(&checkpoint);
    admitted.definition.package = label("other-package");
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            input,
            Some(&admitted),
            limits()
        ),
        Err(TempoAdvanceError::PolicyMismatch)
    );
    admitted = policy(&checkpoint);
    admitted.ticks_per_second = 0;
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            input,
            Some(&admitted),
            limits()
        ),
        Err(TempoAdvanceError::InvalidPolicy)
    );
    admitted = policy(&checkpoint);
    admitted.maximum_elapsed_ticks = 0;
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            input,
            Some(&admitted),
            limits()
        ),
        Err(TempoAdvanceError::InvalidPolicy)
    );
}

#[test]
fn input_capacity_is_checked_before_clone_and_fatigue_zero_limit_allows_empty() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![(content(), 7)], 40);
    let input = request(&checkpoint, Duration::from_secs(1), false);
    let mut bounds = limits();
    bounds.maximum_checkpoint_bytes = 0;
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            input,
            Some(&policy(&checkpoint)),
            bounds
        ),
        Err(TempoAdvanceError::InvalidLimits)
    );
    bounds = limits();
    bounds.maximum_checkpoint_bytes = 1;
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            input,
            Some(&policy(&checkpoint)),
            bounds
        ),
        Err(TempoAdvanceError::Capacity)
    );
    bounds = limits();
    bounds.maximum_fatigue_entries = 0;
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            input,
            Some(&policy(&checkpoint)),
            bounds
        ),
        Err(TempoAdvanceError::Capacity)
    );
    let retained = checkpoint.retained_bytes().unwrap();
    bounds.maximum_checkpoint_bytes = retained;
    bounds.maximum_fatigue_entries = checkpoint.state().tempo.fatigue.len();
    assert!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            input,
            Some(&policy(&checkpoint)),
            bounds
        )
        .is_ok()
    );
    bounds.maximum_checkpoint_bytes = retained - 1;
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            input,
            Some(&policy(&checkpoint)),
            bounds
        ),
        Err(TempoAdvanceError::Capacity)
    );
    bounds.maximum_checkpoint_bytes = retained;
    bounds.maximum_fatigue_entries = checkpoint.state().tempo.fatigue.len() - 1;
    assert_eq!(
        advance_elapsed(
            &checkpoint,
            checkpoint.pins(),
            input,
            Some(&policy(&checkpoint)),
            bounds
        ),
        Err(TempoAdvanceError::Capacity)
    );
    bounds = limits();
    bounds.maximum_fatigue_entries = 0;
    let empty = checkpoint_for(ExecutionMode::Live, vec![], 40);
    assert!(
        advance_elapsed(
            &empty,
            empty.pins(),
            request(&empty, Duration::from_secs(1), false),
            Some(&policy(&empty)),
            bounds
        )
        .is_ok()
    );
}

#[test]
fn repeated_request_after_staging_is_rejected_but_pure_same_input_is_deterministic() {
    let checkpoint = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let input = request(&checkpoint, Duration::from_secs(1), false);
    let first = advance_elapsed(
        &checkpoint,
        checkpoint.pins(),
        input,
        Some(&policy(&checkpoint)),
        limits(),
    )
    .unwrap();
    let repeated = advance_elapsed(
        &checkpoint,
        checkpoint.pins(),
        input,
        Some(&policy(&checkpoint)),
        limits(),
    )
    .unwrap();
    assert_eq!(first, repeated);
    let mut state = checkpoint.state().clone();
    state.tempo = first.state;
    let staged = checkpoint_from(state);
    assert_eq!(
        advance_elapsed(
            &staged,
            staged.pins(),
            input,
            Some(&policy(&staged)),
            limits()
        ),
        Err(TempoAdvanceError::StalePresentationAnchor)
    );
}
