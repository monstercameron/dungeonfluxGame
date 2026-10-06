mod tempo_fixture;

use df_engine::director_staging::*;
use df_model::checkpoint::{ExecutionMode, ReferenceInventory};
use df_tempo::elapsed::*;
use df_world::{DueSelectionLimits, DueSelectionRequest, ScheduleAdvancementLimits};
use std::time::Duration;
use tempo_fixture::*;

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

#[test]
fn real_director_staging_then_tempo_changes_only_cosmetic_anchor() {
    let current = checkpoint_for(ExecutionMode::Live, vec![(content(), 7)], 40);
    let content = content();
    let staged = compose_director_candidates(
        &current,
        current.pins(),
        DueSelectionRequest {
            expected_basis: current.basis(),
            target_time: current.state().logical_time,
            paused: false,
            deadline_remaining: Duration::from_secs(1),
            policy: &content,
        },
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        director_limits(),
    )
    .unwrap();
    let DirectorStaging::Staged(staged) = staged else {
        panic!("empty world must reach downstream tempo")
    };
    assert_eq!(staged.basis(), current.basis());
    assert_eq!(staged.pins(), current.pins());
    assert!(staged.world().events.is_empty());
    let selected = staged.candidate();
    let elapsed = advance_elapsed(
        selected,
        selected.pins(),
        request(selected, Duration::from_secs(1), false),
        Some(&policy(selected)),
        limits(),
    )
    .unwrap();
    let mut state = selected.state().clone();
    state.tempo = elapsed.state;
    let candidate = checkpoint_from(state);
    let mut restored = candidate.state().clone();
    restored.tempo = current.state().tempo.clone();
    assert_eq!(restored, *current.state());
    assert_eq!(candidate.state().tempo.presentation_ticks, 50);
    assert_eq!(candidate.state().logical_time, current.state().logical_time);
    assert_eq!(candidate.state().activity, current.state().activity);
}

#[test]
fn real_schedule_staging_requires_accepted_game_time_and_paused_tempo_stays_fixed() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let content = content();
    let contents = vec![content.clone()];
    let directors = director_limits();
    let staged = compose_schedule_candidates(
        &current,
        current.pins(),
        ScheduleDirectorRequest {
            accepted_time: Some(DueSelectionRequest {
                expected_basis: current.basis(),
                target_time: current.state().logical_time,
                paused: true,
                deadline_remaining: Duration::from_secs(1),
                policy: &content,
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
            content: &contents,
            resources: &[],
            assets: &[],
        },
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
        },
    );
    let DirectorStaging::Staged(staged) = staged.unwrap() else {
        panic!("empty paused world is staged")
    };
    let elapsed = advance_elapsed(
        staged.candidate(),
        staged.pins(),
        request(staged.candidate(), Duration::from_secs(100), true),
        Some(&policy(staged.candidate())),
        limits(),
    )
    .unwrap();
    assert_eq!(elapsed.disposition, ElapsedDisposition::Paused);
    assert_eq!(elapsed.state, current.state().tempo);
    assert_eq!(
        staged.candidate().state().logical_time,
        current.state().logical_time
    );
}
