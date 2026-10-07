use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, MemberId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};
use df_world::{
    AdmittedScheduleDestination, DueSelection, DueSelectionError, DueSelectionLimits,
    DueSelectionRequest, ScheduleAdvancementError, ScheduleAdvancementLimits,
    ScheduledLocationChange, select_due_events, stage_schedule_advancement,
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
fn paused_campaign_does_not_advance_for_positive_caller_time_budgets() {
    let mut supplied = scheduled();
    supplied.logical_time = time(10);
    let checkpoint = checkpoint(supplied);
    let before = checkpoint.state().clone();
    let policy = content();

    let mut short_budget = request(10, &policy);
    short_budget.paused = true;
    short_budget.deadline_remaining = Duration::from_nanos(1);
    let short = select_due_events(&checkpoint, &pins(), short_budget, limits()).unwrap();

    let mut long_budget = request(10, &policy);
    long_budget.paused = true;
    long_budget.deadline_remaining = Duration::from_secs(60);
    let long = select_due_events(&checkpoint, &pins(), long_budget, limits()).unwrap();

    assert_eq!(short, long);
    assert!(short.events.is_empty());
    assert_eq!(short.cursor.pending_events, vec![id(1), id(2), id(3)]);
    assert_eq!(checkpoint.state(), &before);
    assert_eq!(checkpoint.state().logical_time, time(10));
    assert_eq!(checkpoint.state().entities[0].location, None);
}

#[test]
fn pause_and_missing_time_admission_refuse_without_mutating_the_checkpoint() {
    let checkpoint = checkpoint(scheduled());
    let before = checkpoint.state().clone();
    let policy = content();

    let mut paused_future = request(11, &policy);
    paused_future.paused = true;
    assert_eq!(
        select_due_events(&checkpoint, &pins(), paused_future, limits()),
        Err(DueSelectionError::InvalidTime)
    );
    assert_eq!(
        stage_schedule_advancement(&checkpoint, &pins(), None, &[], world_limits()),
        Err(ScheduleAdvancementError::TimeNotAccepted)
    );
    assert_eq!(checkpoint.state(), &before);
}

#[test]
fn stale_basis_pins_cursor_and_time_units_refuse_without_mutation() {
    let policy = content();
    let current_checkpoint = checkpoint(scheduled());
    let before = current_checkpoint.state().clone();

    let mut stale_basis = request(10, &policy);
    stale_basis.expected_basis.revision = revision(2, 9);
    assert!(matches!(
        select_due_events(&current_checkpoint, &pins(), stale_basis, limits()),
        Err(DueSelectionError::Checkpoint(_))
    ));

    let mut stale_pins = pins();
    stale_pins.content.content = label("changed-content");
    assert!(matches!(
        select_due_events(
            &current_checkpoint,
            &stale_pins,
            request(10, &policy),
            limits()
        ),
        Err(DueSelectionError::Checkpoint(_))
    ));

    assert_eq!(
        select_due_events(&current_checkpoint, &pins(), request(8, &policy), limits()),
        Err(DueSelectionError::InvalidTime)
    );
    let mut mismatched_units = request(10, &policy);
    mismatched_units.target_time.ticks_per_second = 20;
    assert_eq!(
        select_due_events(&current_checkpoint, &pins(), mismatched_units, limits()),
        Err(DueSelectionError::InvalidTime)
    );

    let mut stale_cursor_state = scheduled();
    stale_cursor_state.logical_time = time(10);
    stale_cursor_state.continuity.catch_up = Some(CatchUpCursor {
        last_processed: Some(id(1)),
        pending_events: vec![id(3), id(2)],
        policy: content(),
        time: time(10),
    });
    let stale_cursor_checkpoint = checkpoint(stale_cursor_state);
    let stale_cursor_before = stale_cursor_checkpoint.state().clone();
    assert_eq!(
        select_due_events(
            &stale_cursor_checkpoint,
            &pins(),
            request(10, &policy),
            limits()
        ),
        Err(DueSelectionError::StaleCursor)
    );
    assert_eq!(stale_cursor_checkpoint.state(), &stale_cursor_before);
    assert_eq!(current_checkpoint.state(), &before);
}

#[test]
fn accepted_unpaused_time_stages_ordered_destination_and_replays_exactly() {
    let mut supplied = scheduled();
    supplied.entities.push(WorldEntity {
        id: entity(5),
        definition: content(),
        location: None,
        position: None,
        identity_revision: label("fixture-destination-1"),
    });
    let checkpoint = checkpoint(supplied);
    let before = checkpoint.state().clone();
    let policy = content();
    let definition = ContentReference {
        entry: label("event"),
        ..content()
    };
    let destinations = [AdmittedScheduleDestination {
        event: id(1),
        definition: &definition,
        entity: entity(4),
        expected_location: None,
        destination: entity(5),
    }];

    let staged = stage_schedule_advancement(
        &checkpoint,
        &pins(),
        Some(request(10, &policy)),
        &destinations,
        world_limits(),
    )
    .unwrap();
    let replay = stage_schedule_advancement(
        &checkpoint,
        &pins(),
        Some(request(10, &policy)),
        &destinations,
        world_limits(),
    )
    .unwrap();

    assert_eq!(selected_ids(&staged.due), vec![id(1), id(2), id(3)]);
    assert_eq!(
        staged.movements,
        vec![ScheduledLocationChange {
            event: id(1),
            entity: entity(4),
            before: None,
            after: entity(5),
        }]
    );
    assert_eq!(staged, replay);
    assert_eq!(checkpoint.state(), &before);
    assert_eq!(checkpoint.state().logical_time, time(9));
    assert_eq!(checkpoint.state().entities[0].location, None);
}

fn world_limits() -> ScheduleAdvancementLimits {
    ScheduleAdvancementLimits {
        selection: DueSelectionLimits {
            selected_events: 4,
            ..limits()
        },
        entities: 8,
        destinations: 8,
        movements: 8,
        output_bytes: 1024 * 1024,
    }
}
