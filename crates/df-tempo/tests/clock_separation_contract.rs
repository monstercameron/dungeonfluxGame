//! Executable original Tempo D01 contract at the actual engine/world-to-tempo boundary.
//! Fixture values identify this example; they do not calibrate a live clock or pause policy.
mod tempo_fixture;

const SOURCE_DECISION: &str = include_str!("support/clock_separation_contract.json");

use std::time::Duration;

use df_engine::director_staging::{
    DirectorCandidates, DirectorError, DirectorLimits, DirectorStaging, EnvironmentalRequest,
    ScheduleDirectorLimits, ScheduleDirectorRequest, StagedDirectors, compose_director_candidates,
    compose_schedule_candidates,
};
use df_model::checkpoint::{Checkpoint, ExecutionMode, RecordId, ReferenceInventory, ThreatClock};
use df_tempo::elapsed::{ElapsedDisposition, TempoAdvanceError, advance_elapsed};
use df_world::{
    DueSelectionLimits, DueSelectionRequest, ScheduleAdvancementError, ScheduleAdvancementLimits,
};
use tempo_fixture::*;

fn current(mode: ExecutionMode) -> Checkpoint {
    let base = checkpoint_for(mode, vec![(content(), 7)], 40);
    let mut state = base.state().clone();
    state.threats.push(ThreatClock {
        id: RecordId::from_bytes(&[17; 16]).unwrap(),
        definition: content(),
        progress: 3,
        capacity: 8,
    });
    checkpoint_from(state)
}

fn director_limits() -> DirectorLimits {
    DirectorLimits {
        maximum_checkpoint_bytes: 1024 * 1024,
        maximum_pass_bytes: 5 * 1024 * 1024,
        maximum_relationships: 10,
        world: DueSelectionLimits {
            queue_events: 10,
            selected_events: 10,
            output_bytes: 1024 * 1024,
        },
    }
}

fn schedule_limits() -> ScheduleDirectorLimits {
    let directors = director_limits();
    ScheduleDirectorLimits {
        directors,
        schedule: ScheduleAdvancementLimits {
            selection: directors.world,
            entities: 10,
            destinations: 10,
            movements: 10,
            output_bytes: 1024 * 1024,
        },
        checkpoint: checkpoint_limits(),
    }
}

fn scheduled(current: &Checkpoint, paused: bool) -> Box<StagedDirectors<'_>> {
    let selected = compose_schedule_candidates(
        current,
        current.pins(),
        ScheduleDirectorRequest {
            accepted_time: Some(DueSelectionRequest {
                expected_basis: current.basis(),
                target_time: current.state().logical_time,
                paused,
                deadline_remaining: Duration::from_secs(1),
                policy: &current.state().tempo.policy,
            }),
            destinations: &[],
            environmental: EnvironmentalRequest::NotApplicable,
        },
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        ReferenceInventory {
            rules: &[],
            content: std::slice::from_ref(&current.state().tempo.policy),
            resources: &[],
            assets: &[],
        },
        schedule_limits(),
    )
    .unwrap();
    match selected {
        DirectorStaging::Staged(staged) => staged,
        other => panic!("the admitted empty schedule must stage: {other:?}"),
    }
}

#[test]
fn paused_actual_schedule_and_maximum_wall_elapsed_preserve_canonical_state() {
    assert!(SOURCE_DECISION.contains("pause cannot advance canonical time"));
    let current = current(ExecutionMode::Live);
    let original = current.clone();
    let staged = scheduled(&current, true);
    let selected = staged.candidate();
    assert_eq!(selected.state().logical_time, current.state().logical_time);
    assert_eq!(selected.state().facts, current.state().facts);
    assert_eq!(selected.state().decisions, current.state().decisions);
    assert_eq!(selected.state().threats, current.state().threats);
    let before = selected.clone();

    let proposal = advance_elapsed(
        selected,
        staged.pins(),
        request(selected, Duration::MAX, true),
        Some(&policy(selected)),
        limits(),
    )
    .unwrap();
    assert_eq!(proposal.disposition, ElapsedDisposition::Paused);
    assert_eq!(proposal.elapsed_ticks, 0);
    assert_eq!(proposal.state, selected.state().tempo);
    let mut proposed_state = selected.state().clone();
    proposed_state.tempo = proposal.state;
    assert_eq!(proposed_state, *selected.state());
    assert_eq!(*selected, before);
    assert_eq!(current, original);
}

#[test]
fn actual_director_cosmetic_advance_cannot_advance_threat_or_rule_history() {
    let current = current(ExecutionMode::Live);
    let original = current.clone();
    let directors = compose_director_candidates(
        &current,
        current.pins(),
        DueSelectionRequest {
            expected_basis: current.basis(),
            target_time: current.state().logical_time,
            paused: false,
            deadline_remaining: Duration::from_secs(1),
            policy: &current.state().tempo.policy,
        },
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        director_limits(),
    )
    .unwrap();
    let DirectorStaging::Staged(staged) = directors else {
        panic!("the bounded empty world must reach tempo")
    };
    let selected = staged.candidate();
    let advance = advance_elapsed(
        selected,
        staged.pins(),
        request(selected, Duration::from_secs(2), false),
        Some(&policy(selected)),
        limits(),
    )
    .unwrap();
    assert_eq!(advance.disposition, ElapsedDisposition::Advanced);
    assert_eq!(advance.elapsed_ticks, 20);
    assert_eq!(advance.state.presentation_ticks, 60);
    let mut state = selected.state().clone();
    state.tempo = advance.state;
    let candidate = checkpoint_from(state);
    assert_eq!(candidate.basis(), selected.basis());
    assert_eq!(candidate.pins(), selected.pins());
    let mut restored = candidate.state().clone();
    restored.tempo.presentation_ticks = selected.state().tempo.presentation_ticks;
    assert_eq!(restored, *selected.state());
    assert_eq!(candidate.state().logical_time, current.state().logical_time);
    assert_eq!(candidate.state().threats, current.state().threats);
    assert_eq!(candidate.state().decisions, current.state().decisions);
    assert_eq!(candidate.state().facts, current.state().facts);
    assert_eq!(candidate.state().draws, current.state().draws);
    assert_eq!(candidate.state().intents, current.state().intents);
    assert_eq!(current, original);
}

#[test]
fn cosmetic_elapsed_cannot_supply_missing_accepted_game_time() {
    let current = current(ExecutionMode::Live);
    let original = current.clone();
    let result = compose_schedule_candidates(
        &current,
        current.pins(),
        ScheduleDirectorRequest {
            accepted_time: None,
            destinations: &[],
            environmental: EnvironmentalRequest::NotApplicable,
        },
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        ReferenceInventory {
            rules: &[],
            content: std::slice::from_ref(&current.state().tempo.policy),
            resources: &[],
            assets: &[],
        },
        schedule_limits(),
    );
    assert_eq!(
        result,
        Err(DirectorError::Schedule(
            ScheduleAdvancementError::TimeNotAccepted
        ))
    );
    let cosmetic = advance_elapsed(
        &current,
        current.pins(),
        request(&current, Duration::from_secs(1), false),
        Some(&policy(&current)),
        limits(),
    )
    .unwrap();
    assert_eq!(cosmetic.elapsed_ticks, 10);
    assert_eq!(current, original);
}

#[test]
fn paused_request_still_requires_exact_current_binding_and_policy() {
    let current = current(ExecutionMode::Live);
    let original = current.clone();
    let mut stale = request(&current, Duration::MAX, true);
    stale.expected_basis.revision = revision(7);
    assert!(matches!(
        advance_elapsed(
            &current,
            current.pins(),
            stale,
            Some(&policy(&current)),
            limits()
        ),
        Err(TempoAdvanceError::Binding(_))
    ));
    let mut wrong_pins = current.pins().clone();
    wrong_pins.content.package_digest.0[0] ^= 1;
    assert!(matches!(
        advance_elapsed(
            &current,
            &wrong_pins,
            request(&current, Duration::MAX, true),
            Some(&policy(&current)),
            limits()
        ),
        Err(TempoAdvanceError::Binding(_))
    ));
    assert_eq!(
        advance_elapsed(
            &current,
            current.pins(),
            request(&current, Duration::MAX, true),
            None,
            limits()
        ),
        Err(TempoAdvanceError::PolicyUnavailable)
    );
    let mut wrong_policy = policy(&current);
    wrong_policy.definition.entry = label("unknown-clock-policy");
    assert_eq!(
        advance_elapsed(
            &current,
            current.pins(),
            request(&current, Duration::MAX, true),
            Some(&wrong_policy),
            limits()
        ),
        Err(TempoAdvanceError::PolicyMismatch)
    );
    assert_eq!(current, original);
}

#[test]
fn logical_time_and_presentation_anchor_are_separate_refusal_boundaries() {
    let current = current(ExecutionMode::Live);
    let original = current.clone();
    let mut wrong_logical = request(&current, Duration::from_secs(1), false);
    wrong_logical.observed_logical_time.ticks += 1;
    assert_eq!(
        advance_elapsed(
            &current,
            current.pins(),
            wrong_logical,
            Some(&policy(&current)),
            limits()
        ),
        Err(TempoAdvanceError::LogicalTimeMismatch)
    );
    let mut old_anchor = request(&current, Duration::MAX, true);
    old_anchor.from_presentation_ticks -= 1;
    assert_eq!(
        advance_elapsed(
            &current,
            current.pins(),
            old_anchor,
            Some(&policy(&current)),
            limits()
        ),
        Err(TempoAdvanceError::StalePresentationAnchor)
    );
    assert_eq!(current, original);
}

#[test]
fn replay_and_prepared_only_preserve_their_distinct_owner_contracts() {
    let replay = current(ExecutionMode::Replay);
    let replay_before = replay.clone();
    let replayed = advance_elapsed(
        &replay,
        replay.pins(),
        request(&replay, Duration::MAX, false),
        Some(&policy(&replay)),
        limits(),
    )
    .unwrap();
    assert_eq!(replayed.disposition, ElapsedDisposition::Replay);
    assert_eq!(replayed.state, replay.state().tempo);
    assert_eq!(replay, replay_before);

    let prepared = current(ExecutionMode::PreparedOnly);
    let before = prepared.clone();
    let proposal = advance_elapsed(
        &prepared,
        prepared.pins(),
        request(&prepared, Duration::from_secs(1), false),
        Some(&policy(&prepared)),
        limits(),
    )
    .unwrap();
    assert_eq!(proposal.elapsed_ticks, 10);
    let mut detached = prepared.state().clone();
    detached.tempo = proposal.state;
    assert_eq!(detached.mode, ExecutionMode::PreparedOnly);
    let mut restored = detached;
    restored.tempo.presentation_ticks = prepared.state().tempo.presentation_ticks;
    assert_eq!(restored, *prepared.state());
    assert_eq!(prepared, before);
}

#[test]
fn checked_wall_conversion_refuses_fractional_excess_and_overflow_without_partial_state() {
    let current = current(ExecutionMode::Live);
    let original = current.clone();
    for (elapsed, expected) in [
        (Duration::from_nanos(1), TempoAdvanceError::FractionalTicks),
        (Duration::from_secs(11), TempoAdvanceError::ElapsedLimit),
        (Duration::MAX, TempoAdvanceError::FractionalTicks),
        (
            Duration::from_secs(u64::MAX),
            TempoAdvanceError::TimeOverflow,
        ),
    ] {
        assert_eq!(
            advance_elapsed(
                &current,
                current.pins(),
                request(&current, elapsed, false),
                Some(&policy(&current)),
                limits()
            ),
            Err(expected)
        );
        assert_eq!(current, original);
    }
    let overflowing = checkpoint_for(ExecutionMode::Live, vec![], u64::MAX);
    let before = overflowing.clone();
    assert_eq!(
        advance_elapsed(
            &overflowing,
            overflowing.pins(),
            request(&overflowing, Duration::from_secs(1), false),
            Some(&policy(&overflowing)),
            limits()
        ),
        Err(TempoAdvanceError::TimeOverflow)
    );
    assert_eq!(overflowing, before);
}

#[test]
fn repeated_pure_input_is_deterministic_but_an_advanced_anchor_refuses_old_elapsed() {
    let current = current(ExecutionMode::Live);
    let admitted = request(&current, Duration::from_secs(1), false);
    let elapsed_policy = policy(&current);
    let first = advance_elapsed(
        &current,
        current.pins(),
        admitted,
        Some(&elapsed_policy),
        limits(),
    )
    .unwrap();
    let duplicate = advance_elapsed(
        &current,
        current.pins(),
        admitted,
        Some(&elapsed_policy),
        limits(),
    )
    .unwrap();
    assert_eq!(first, duplicate);
    let mut state = current.state().clone();
    state.tempo = first.state;
    let next = checkpoint_from(state);
    assert_eq!(
        advance_elapsed(
            &next,
            next.pins(),
            admitted,
            Some(&elapsed_policy),
            limits()
        ),
        Err(TempoAdvanceError::StalePresentationAnchor)
    );
    assert_eq!(next.state().logical_time, current.state().logical_time);
    assert_eq!(next.state().decisions, current.state().decisions);
}

#[test]
fn capacity_and_invalid_policy_are_not_bypassed_by_pause_or_replay() {
    for mode in [ExecutionMode::Live, ExecutionMode::Replay] {
        let current = current(mode);
        let original = current.clone();
        let mut invalid = policy(&current);
        invalid.ticks_per_second = 0;
        assert_eq!(
            advance_elapsed(
                &current,
                current.pins(),
                request(&current, Duration::MAX, true),
                Some(&invalid),
                limits()
            ),
            Err(TempoAdvanceError::InvalidPolicy)
        );
        let mut bounded = limits();
        bounded.maximum_fatigue_entries = 0;
        assert_eq!(
            advance_elapsed(
                &current,
                current.pins(),
                request(&current, Duration::MAX, true),
                Some(&policy(&current)),
                bounded
            ),
            Err(TempoAdvanceError::Capacity)
        );
        assert_eq!(current, original);
    }
}
