use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, MemberId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};
use df_world::{
    DueSelection, DueSelectionError, DueSelectionLimits, DueSelectionRequest, select_due_events,
};
use std::time::Duration;

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}
fn entity(value: u8) -> EntityId {
    EntityId::from_bytes(&[value; 16]).unwrap()
}
fn member(value: u8) -> MemberId {
    MemberId::from_bytes(&[value; 16]).unwrap()
}
fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
}
fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: revision(2, 8),
    }
}
fn content() -> ContentReference {
    ContentReference {
        package: label("fixture-package-1"),
        entry: label("fixture-entry-1"),
    }
}
fn rule() -> RuleReference {
    RuleReference {
        catalog: label("fixture-catalog-1"),
        source: label("fixture-source-1"),
        entry: label("fixture-entry-1"),
        clause: label("fixture-clause-1"),
    }
}
fn pins() -> CheckpointPins {
    CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: label("fixture-rules-1"),
            catalog: label("fixture-catalog-1"),
            catalog_digest: ContentDigest([1; 32]),
            source_manifest: label("fixture-sources-1"),
            source_manifest_digest: ContentDigest([2; 32]),
            handler: label("fixture-handler-1"),
            handler_digest: ContentDigest([3; 32]),
        },
        content: ContentPins {
            content: label("fixture-content-1"),
            content_digest: ContentDigest([4; 32]),
            package: label("fixture-package-1"),
            package_digest: ContentDigest([5; 32]),
        },
        build: BuildIdentity::new(
            Some("fixture-source-1"),
            Some("fixture-native-1"),
            Some("fixture-wasm-1"),
            Some("fixture-config-1"),
            Some("fixture-content-1"),
        )
        .unwrap(),
    }
}
fn resource_constraints() -> Vec<ResourceConstraint> {
    vec![ResourceConstraint {
        owner: entity(4),
        resource: label("fixture-resource-1"),
        minimum: 0,
        maximum: 8,
        source: rule(),
    }]
}
fn checkpoint_limits() -> CheckpointLimits {
    CheckpointLimits {
        maximum_records: 100,
        maximum_text_bytes: 256,
        maximum_total_text_bytes: 1024,
        maximum_retained_bytes: 1024 * 1024,
    }
}
fn state() -> GameState {
    GameState {
        mode: ExecutionMode::Replay,
        logical_time: LogicalTime {
            ticks: 9,
            ticks_per_second: 10,
        },
        members: vec![MembershipLink {
            member: member(3),
            character: Some(entity(4)),
        }],
        entities: vec![WorldEntity {
            id: entity(4),
            definition: content(),
            location: None,
            position: Some(Position { x: 0, y: 0, z: 0 }),
            identity_revision: label("fixture-entity-1"),
        }],
        characters: vec![CharacterState {
            entity: entity(4),
            build: content(),
            owner: member(3),
            choices: vec![],
        }],
        resources: vec![ResourceState {
            owner: entity(4),
            resource: label("fixture-resource-1"),
            value: 4,
            minimum: 0,
            maximum: 8,
            source: rule(),
        }],
        inventory: vec![],
        facts: vec![],
        draws: vec![],
        decisions: vec![],
        pending: vec![],
        intents: vec![],
        timers: vec![],
        active_effects: vec![],
        knowledge: vec![],
        beliefs: vec![],
        memories: vec![],
        schedules: vec![],
        threats: vec![],
        relationships: vec![],
        conversations: vec![],
        obligations: vec![],
        narrative: NarrativeState {
            definition: content(),
            active_beats: vec![],
            completed_beats: vec![],
            open_threads: vec![],
            accepted_facts: vec![],
            remaining_budget: 0,
        },
        encounters: vec![],
        activity: vec![],
        tempo: TempoState {
            policy: content(),
            presentation_ticks: 0,
            intensity: 0,
            inertia: 0,
            fatigue: vec![],
        },
        presentation: vec![],
        continuity: continuity(),
    }
}
fn continuity() -> ContinuityState {
    ContinuityState {
        creation: vec![],
        simulation: vec![],
        catch_up: None,
        environment: vec![],
        travel: vec![],
        witnesses: vec![],
        rumors: vec![],
        journal: vec![],
        summaries: vec![],
        retrieval: vec![],
        retrieved: vec![],
        consolidation: vec![],
        npcs: vec![],
        hooks: vec![],
        arcs: vec![],
        remote: None,
        presence: vec![],
        audio: None,
        private_offers: vec![],
        knowledge_cues: vec![],
        moments: vec![],
        demands: vec![],
        asset_jobs: vec![],
        asset_dependencies: vec![],
        canonical_packs: vec![],
        shots: vec![],
        prefetch: None,
        scenes: vec![],
        item_origins: vec![],
        bookends: vec![],
        exports: vec![],
        critical_cues: vec![],
        content_candidates: vec![],
        content_admissions: vec![],
        recovery: RecoveryState {
            origin: None,
            retired_epochs: vec![],
            lost_ranges: vec![],
            suppression_generation: 0,
            redacted_records: vec![],
            unavailable_sources: vec![],
        },
    }
}

fn checkpoint(supplied: GameState) -> Checkpoint {
    let mut entries = vec![content()];
    entries.extend(
        supplied
            .schedules
            .iter()
            .map(|event| event.definition.clone()),
    );
    if let Some(cursor) = &supplied.continuity.catch_up {
        entries.push(cursor.policy.clone());
    }
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis(),
        pins(),
        supplied,
        ReferenceInventory {
            rules: &[rule()],
            content: &entries,
            resources: &resource_constraints(),
            assets: &[],
        },
        checkpoint_limits(),
    )
    .unwrap()
}
fn id(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}
fn time(ticks: u64) -> LogicalTime {
    LogicalTime {
        ticks,
        ticks_per_second: 10,
    }
}
fn event(ticks: u64, value: u8, entry: &str) -> ScheduledEvent {
    ScheduledEvent {
        id: id(value),
        entity: entity(4),
        due: time(ticks),
        definition: ContentReference {
            entry: label(entry),
            ..content()
        },
    }
}
fn request<'a>(target: u64, policy: &'a ContentReference) -> DueSelectionRequest<'a> {
    DueSelectionRequest {
        expected_basis: basis(),
        target_time: time(target),
        paused: false,
        deadline_remaining: Duration::from_secs(1),
        policy,
    }
}
fn limits() -> DueSelectionLimits {
    DueSelectionLimits {
        queue_events: 4,
        selected_events: 2,
        output_bytes: 1024 * 1024,
    }
}
fn scheduled() -> GameState {
    let mut supplied = state();
    supplied.schedules = vec![
        event(10, 3, "long-event"),
        event(10, 1, "event"),
        event(10, 2, "event"),
        event(12, 4, "event"),
    ];
    supplied
}
fn selected_ids(selected: &DueSelection<'_>) -> Vec<RecordId> {
    selected.events.iter().map(|event| event.id).collect()
}

#[test]
fn due_prefix_resumes_at_same_tick_and_excludes_future_backlog() {
    let policy = content();
    let first_checkpoint = checkpoint(scheduled());
    let first =
        select_due_events(&first_checkpoint, &pins(), request(10, &policy), limits()).unwrap();
    assert_eq!(selected_ids(&first), vec![id(1), id(2)]);
    assert_eq!(first.cursor.pending_events, vec![id(3)]);
    assert_eq!(first.cursor.last_processed, Some(id(2)));
    let mut next_state = first_checkpoint.state().clone();
    next_state.logical_time = first.proposed_time;
    next_state.continuity.catch_up = Some(first.cursor.clone());
    let next_checkpoint = checkpoint(next_state);
    let next =
        select_due_events(&next_checkpoint, &pins(), request(10, &policy), limits()).unwrap();
    assert_eq!(selected_ids(&next), vec![id(3)]);
    assert_eq!(next.remaining_due(), 0);
    let mut later_state = next_checkpoint.state().clone();
    later_state.continuity.catch_up = Some(next.cursor);
    let later_checkpoint = checkpoint(later_state);
    let later =
        select_due_events(&later_checkpoint, &pins(), request(12, &policy), limits()).unwrap();
    assert_eq!(selected_ids(&later), vec![id(4)]);
}
#[test]
fn arrival_permutations_and_exact_retries_are_deterministic() {
    let policy = content();
    let mut observed = 0;
    for first in 1..=3 {
        for second in 1..=3 {
            for third in 1..=3 {
                if first == second || first == third || second == third {
                    continue;
                }
                let mut supplied = state();
                supplied.logical_time = time(8);
                supplied.schedules = [first, second, third]
                    .into_iter()
                    .map(|id| event(if id == 3 { 9 } else { 10 }, id, "event"))
                    .collect();
                let checkpoint = checkpoint(supplied);
                let selected =
                    select_due_events(&checkpoint, &pins(), request(10, &policy), limits())
                        .unwrap();
                assert_eq!(selected_ids(&selected), vec![id(3), id(1)]);
                assert_eq!(
                    selected,
                    select_due_events(&checkpoint, &pins(), request(10, &policy), limits())
                        .unwrap()
                );
                observed += 1;
            }
        }
    }
    assert_eq!(observed, 6);
}
#[test]
fn byte_bound_stops_at_the_first_event_without_skipping() {
    let policy = content();
    let mut supplied = state();
    supplied.schedules = vec![
        event(10, 1, &"x".repeat(100)),
        event(10, 2, &"y".repeat(100)),
        event(10, 3, "x"),
    ];
    let checkpoint = checkpoint(supplied);
    let mut one = limits();
    one.selected_events = 1;
    let first = select_due_events(&checkpoint, &pins(), request(10, &policy), one).unwrap();
    let mut bounded = limits();
    bounded.output_bytes = first.accounted_output_bytes + std::mem::size_of::<ScheduledEvent>();
    let selected = select_due_events(&checkpoint, &pins(), request(10, &policy), bounded).unwrap();
    assert_eq!(selected_ids(&selected), vec![id(1)]);
    assert_eq!(selected.cursor.pending_events, vec![id(2), id(3)]);
}
#[test]
fn count_bound_stops_even_when_bytes_remain() {
    let checkpoint = checkpoint(scheduled());
    let policy = content();
    let mut bounded = limits();
    bounded.selected_events = 1;
    let selected = select_due_events(&checkpoint, &pins(), request(10, &policy), bounded).unwrap();
    assert_eq!(selected_ids(&selected), vec![id(1)]);
    assert_eq!(selected.cursor.pending_events, vec![id(2), id(3)]);
}
#[test]
fn paused_selection_preserves_cursor_and_exposes_backlog() {
    let mut supplied = scheduled();
    supplied.logical_time = time(10);
    let cursor = CatchUpCursor {
        last_processed: Some(id(1)),
        pending_events: vec![id(2), id(3)],
        policy: content(),
        time: time(10),
    };
    supplied.continuity.catch_up = Some(cursor.clone());
    let checkpoint = checkpoint(supplied);
    let policy = content();
    let mut paused = request(10, &policy);
    paused.paused = true;
    let selected = select_due_events(&checkpoint, &pins(), paused, limits()).unwrap();
    assert!(selected.events.is_empty());
    assert_eq!(selected.cursor, cursor);
    let mut paused = request(11, &policy);
    paused.paused = true;
    assert_eq!(
        select_due_events(&checkpoint, &pins(), paused, limits()),
        Err(DueSelectionError::InvalidTime)
    );
}
#[test]
fn expected_session_run_and_revision_are_rechecked() {
    let checkpoint = checkpoint(scheduled());
    let policy = content();
    for changed in [
        Basis {
            session: SessionId::from_bytes(&[9; 16]).unwrap(),
            ..basis()
        },
        Basis {
            run: RunId::from_bytes(&[9; 16]).unwrap(),
            ..basis()
        },
        Basis {
            revision: revision(2, 9),
            ..basis()
        },
    ] {
        let mut admission = request(10, &policy);
        admission.expected_basis = changed;
        assert!(matches!(
            select_due_events(&checkpoint, &pins(), admission, limits()),
            Err(DueSelectionError::Checkpoint(_))
        ));
    }
}
#[test]
fn source_content_and_build_pins_are_rechecked() {
    let checkpoint = checkpoint(scheduled());
    let policy = content();
    for component in 0..3 {
        let mut admitted = pins();
        match component {
            0 => admitted.rules.handler = label("changed"),
            1 => admitted.content.content = label("changed"),
            _ => {
                admitted.build =
                    BuildIdentity::new(Some("changed"), Some("n"), Some("w"), Some("c"), Some("p"))
                        .unwrap()
            }
        }
        assert!(matches!(
            select_due_events(&checkpoint, &admitted, request(10, &policy), limits()),
            Err(DueSelectionError::Checkpoint(_))
        ));
    }
}
#[test]
fn missing_or_future_last_processed_cursor_is_refused() {
    let policy = content();
    for last in [id(99), id(4)] {
        let mut supplied = scheduled();
        supplied.logical_time = time(10);
        supplied.continuity.catch_up = Some(CatchUpCursor {
            last_processed: Some(last),
            pending_events: vec![],
            policy: content(),
            time: time(10),
        });
        let checkpoint = checkpoint(supplied);
        assert_eq!(
            select_due_events(&checkpoint, &pins(), request(10, &policy), limits()),
            Err(DueSelectionError::StaleCursor)
        );
    }
}
#[test]
fn cursor_requires_exact_ordered_pending_ids_and_current_time() {
    let policy = content();
    for (pending, time) in [
        (vec![id(3), id(2)], time(10)),
        (vec![id(2)], time(10)),
        (vec![id(2), id(3)], time(9)),
    ] {
        let mut supplied = scheduled();
        supplied.logical_time = crate_time(10);
        supplied.continuity.catch_up = Some(CatchUpCursor {
            last_processed: Some(id(1)),
            pending_events: pending,
            policy: content(),
            time,
        });
        let checkpoint = checkpoint(supplied);
        assert_eq!(
            select_due_events(&checkpoint, &pins(), request(10, &policy), limits()),
            Err(DueSelectionError::StaleCursor)
        );
    }
}
fn crate_time(ticks: u64) -> LogicalTime {
    time(ticks)
}
#[test]
fn duplicate_schedule_identity_is_refused_by_canonical_checkpoint_admission() {
    let mut supplied = scheduled();
    supplied.schedules[3].id = id(1);
    let entries: Vec<_> = supplied
        .schedules
        .iter()
        .map(|event| event.definition.clone())
        .chain([content()])
        .collect();
    assert_eq!(
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis(),
            pins(),
            supplied,
            ReferenceInventory {
                rules: &[rule()],
                content: &entries,
                resources: &resource_constraints(),
                assets: &[]
            },
            checkpoint_limits()
        ),
        Err(CheckpointError::DuplicateIdentity)
    );
}
#[test]
fn expired_admission_refuses_and_positive_supplied_deadline_admits() {
    let checkpoint = checkpoint(scheduled());
    let policy = content();
    let mut admission = request(10, &policy);
    admission.deadline_remaining = Duration::ZERO;
    assert_eq!(
        select_due_events(&checkpoint, &pins(), admission, limits()),
        Err(DueSelectionError::Deadline)
    );
    let mut admission = request(10, &policy);
    admission.deadline_remaining = Duration::from_nanos(1);
    assert!(select_due_events(&checkpoint, &pins(), admission, limits()).is_ok());
}
#[test]
fn zero_infeasible_queue_and_output_limits_are_refused() {
    let checkpoint = checkpoint(scheduled());
    let policy = content();
    for bounded in [
        DueSelectionLimits {
            queue_events: 0,
            ..limits()
        },
        DueSelectionLimits {
            queue_events: 4097,
            ..limits()
        },
        DueSelectionLimits {
            selected_events: 0,
            ..limits()
        },
        DueSelectionLimits {
            selected_events: 5,
            ..limits()
        },
        DueSelectionLimits {
            output_bytes: 0,
            ..limits()
        },
    ] {
        assert_eq!(
            select_due_events(&checkpoint, &pins(), request(10, &policy), bounded),
            Err(DueSelectionError::InvalidLimits)
        );
    }
    assert_eq!(
        select_due_events(
            &checkpoint,
            &pins(),
            request(10, &policy),
            DueSelectionLimits {
                queue_events: 3,
                ..limits()
            }
        ),
        Err(DueSelectionError::QueueCapacity)
    );
    assert_eq!(
        select_due_events(
            &checkpoint,
            &pins(),
            request(10, &policy),
            DueSelectionLimits {
                output_bytes: 1,
                ..limits()
            }
        ),
        Err(DueSelectionError::OutputCapacity)
    );
}
#[test]
fn time_units_regression_and_paused_advance_are_refused() {
    let policy = content();
    let checkpoint = checkpoint(scheduled());
    assert_eq!(
        select_due_events(&checkpoint, &pins(), request(8, &policy), limits()),
        Err(DueSelectionError::InvalidTime)
    );
    let mut changed = request(10, &policy);
    changed.target_time.ticks_per_second = 20;
    assert_eq!(
        select_due_events(&checkpoint, &pins(), changed, limits()),
        Err(DueSelectionError::InvalidTime)
    );
    let mut supplied = scheduled();
    supplied.schedules[0].due.ticks_per_second = 20;
    let checkpoint = crate_checkpoint(supplied);
    assert_eq!(
        select_due_events(&checkpoint, &pins(), request(10, &policy), limits()),
        Err(DueSelectionError::InvalidTime)
    );
}
fn crate_checkpoint(state: GameState) -> Checkpoint {
    checkpoint(state)
}
#[test]
fn empty_and_maximum_tick_queues_do_not_mutate_the_checkpoint() {
    let policy = content();
    let empty = checkpoint(state());
    let selected = select_due_events(&empty, &pins(), request(9, &policy), limits()).unwrap();
    assert!(selected.events.is_empty());
    assert_eq!(selected.remaining_due(), 0);
    let mut supplied = state();
    supplied.logical_time = time(u64::MAX);
    supplied.schedules = vec![event(u64::MAX, 1, "event")];
    let checkpoint = checkpoint(supplied);
    let before = checkpoint.state().clone();
    let selected =
        select_due_events(&checkpoint, &pins(), request(u64::MAX, &policy), limits()).unwrap();
    assert_eq!(selected_ids(&selected), vec![id(1)]);
    assert_eq!(checkpoint.state(), &before);
}
