use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, MemberId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};
use df_world::{
    AdmittedScheduleDestination, DueSelectionError, DueSelectionLimits, DueSelectionRequest,
    ScheduleAdvancement, ScheduleAdvancementError, ScheduleAdvancementLimits,
    stage_schedule_advancement,
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
fn selection_limits() -> DueSelectionLimits {
    DueSelectionLimits {
        queue_events: 4,
        selected_events: 2,
        output_bytes: 1024 * 1024,
    }
}

fn limits() -> ScheduleAdvancementLimits {
    ScheduleAdvancementLimits {
        selection: selection_limits(),
        entities: 4,
        destinations: 4,
        movements: 2,
        output_bytes: 1024 * 1024,
    }
}
fn scheduled() -> GameState {
    let mut supplied = state();
    supplied.entities[0].location = Some(entity(5));
    for value in 5..=7 {
        supplied.entities.push(WorldEntity {
            id: entity(value),
            definition: content(),
            location: None,
            position: None,
            identity_revision: label("fixture-location-1"),
        });
    }
    supplied.schedules = vec![
        event(10, 1, "fixture-entry-1"),
        event(11, 2, "fixture-entry-1"),
        event(12, 3, "fixture-entry-1"),
    ];
    supplied
}
fn destinations(definition: &ContentReference) -> Vec<AdmittedScheduleDestination<'_>> {
    vec![
        AdmittedScheduleDestination {
            event: id(1),
            definition,
            entity: entity(4),
            expected_location: Some(entity(5)),
            destination: entity(6),
        },
        AdmittedScheduleDestination {
            event: id(2),
            definition,
            entity: entity(4),
            expected_location: Some(entity(6)),
            destination: entity(7),
        },
        AdmittedScheduleDestination {
            event: id(3),
            definition,
            entity: entity(4),
            expected_location: Some(entity(7)),
            destination: entity(5),
        },
    ]
}
fn stage<'a>(
    checkpoint: &'a Checkpoint,
    target: u64,
    definition: &ContentReference,
    destinations: &[AdmittedScheduleDestination<'_>],
) -> Result<ScheduleAdvancement<'a>, ScheduleAdvancementError> {
    stage_schedule_advancement(
        checkpoint,
        &pins(),
        Some(request(target, definition)),
        destinations,
        limits(),
    )
}

#[test]
fn only_admitted_game_time_stages_due_locations_and_never_mutates_basis() {
    let checkpoint = checkpoint(scheduled());
    let original = checkpoint.state().clone();
    let definition = content();
    let destinations = destinations(&definition);
    assert_eq!(
        stage_schedule_advancement(&checkpoint, &pins(), None, &destinations, limits()),
        Err(ScheduleAdvancementError::TimeNotAccepted),
    );
    assert!(
        stage(&checkpoint, 9, &definition, &destinations)
            .unwrap()
            .movements
            .is_empty()
    );
    let proposal = stage(&checkpoint, 10, &definition, &destinations).unwrap();
    assert_eq!(proposal.movements.len(), 1);
    assert_eq!(proposal.movements[0].before, Some(entity(5)));
    assert_eq!(proposal.movements[0].after, entity(6));
    assert_eq!(proposal.due.proposed_time, time(10));
    assert_eq!(checkpoint.state(), &original);
    assert_eq!(
        stage(&checkpoint, 10, &definition, &destinations).unwrap(),
        proposal
    );
}

#[test]
fn paused_campaign_retains_location_cursor_and_backlog_without_wall_time() {
    let mut supplied = scheduled();
    supplied.logical_time = time(12);
    let checkpoint = checkpoint(supplied);
    let definition = content();
    let destinations = destinations(&definition);
    let mut paused = request(12, &definition);
    paused.paused = true;
    let proposal =
        stage_schedule_advancement(&checkpoint, &pins(), Some(paused), &destinations, limits())
            .unwrap();
    assert!(proposal.movements.is_empty());
    assert_eq!(proposal.due.cursor.last_processed, None);
    assert_eq!(
        proposal.due.cursor.pending_events,
        vec![id(1), id(2), id(3)]
    );
    assert_eq!(checkpoint.state().entities[0].location, Some(entity(5)));
    let mut paused = request(13, &definition);
    paused.paused = true;
    assert_eq!(
        stage_schedule_advancement(&checkpoint, &pins(), Some(paused), &destinations, limits()),
        Err(ScheduleAdvancementError::DueSelection(
            DueSelectionError::InvalidTime
        ))
    );
}

#[test]
fn ordered_movement_chain_preserves_future_events_and_resumes_committed_cursor() {
    let first_checkpoint = checkpoint(scheduled());
    let definition = content();
    let destinations = destinations(&definition);
    let first = stage(&first_checkpoint, 12, &definition, &destinations).unwrap();
    assert_eq!(
        first
            .movements
            .iter()
            .map(|movement| movement.event)
            .collect::<Vec<_>>(),
        vec![id(1), id(2)]
    );
    assert_eq!(first.movements[1].before, Some(first.movements[0].after));
    assert_eq!(first.due.cursor.pending_events, vec![id(3)]);
    let mut committed = first_checkpoint.state().clone();
    committed.logical_time = first.due.proposed_time;
    committed.continuity.catch_up = Some(first.due.cursor.clone());
    committed.entities[0].location = Some(entity(7));
    let resumed = checkpoint(committed);
    let next = stage(&resumed, 12, &definition, &destinations).unwrap();
    assert_eq!(next.movements.len(), 1);
    assert_eq!(next.movements[0].event, id(3));
    assert_eq!(next.movements[0].before, Some(entity(7)));
    assert_eq!(next.due.remaining_due(), 0);
    let mut completed = resumed.state().clone();
    completed.continuity.catch_up = Some(next.due.cursor.clone());
    completed.entities[0].location = Some(entity(5));
    assert!(
        stage(&checkpoint(completed), 12, &definition, &destinations)
            .unwrap()
            .movements
            .is_empty()
    );
}

#[test]
fn schedule_and_mapping_arrival_order_do_not_change_candidates() {
    let definition = content();
    let mut mappings = destinations(&definition);
    let first_checkpoint = checkpoint(scheduled());
    let first = stage(&first_checkpoint, 12, &definition, &mappings).unwrap();
    let mut shuffled = scheduled();
    shuffled.schedules.reverse();
    mappings.reverse();
    assert_eq!(
        stage(&checkpoint(shuffled), 12, &definition, &mappings).unwrap(),
        first
    );
}

#[test]
fn missing_mapping_never_invents_location_and_keeps_other_selected_consequences() {
    let checkpoint = checkpoint(scheduled());
    let definition = content();
    let proposal = stage(&checkpoint, 10, &definition, &[]).unwrap();
    assert!(proposal.movements.is_empty());
    assert_eq!(proposal.due.events[0].id, id(1));
    assert_eq!(checkpoint.state().entities[0].location, Some(entity(5)));
}

#[test]
fn invalid_destination_mapping_and_stale_location_refuse_the_whole_candidate() {
    let checkpoint = checkpoint(scheduled());
    let original = checkpoint.state().clone();
    let definition = content();
    let mut mappings = destinations(&definition);
    mappings[1].expected_location = Some(entity(5));
    assert_eq!(
        stage(&checkpoint, 11, &definition, &mappings),
        Err(ScheduleAdvancementError::StaleLocation)
    );
    mappings[1].expected_location = Some(entity(6));
    mappings[0].destination = entity(99);
    assert_eq!(
        stage(&checkpoint, 10, &definition, &mappings),
        Err(ScheduleAdvancementError::UnknownEntity)
    );
    mappings[0].destination = entity(6);
    mappings[0].event = id(99);
    assert_eq!(
        stage(&checkpoint, 10, &definition, &mappings),
        Err(ScheduleAdvancementError::UnknownEvent)
    );
    mappings[0].event = id(2);
    assert_eq!(
        stage(&checkpoint, 10, &definition, &mappings),
        Err(ScheduleAdvancementError::DuplicateDestination)
    );
    mappings[0].event = id(1);
    mappings[0].entity = entity(5);
    assert_eq!(
        stage(&checkpoint, 10, &definition, &mappings),
        Err(ScheduleAdvancementError::StaleDestination)
    );
    mappings[0].entity = entity(4);
    let wrong = ContentReference {
        entry: label("unrelated-entry"),
        ..content()
    };
    mappings[0].definition = &wrong;
    assert_eq!(
        stage(&checkpoint, 10, &definition, &mappings),
        Err(ScheduleAdvancementError::StaleDestination)
    );
    assert_eq!(checkpoint.state(), &original);
}

#[test]
fn stale_basis_and_changed_content_are_not_reinterpreted_as_accepted_time() {
    let checkpoint = checkpoint(scheduled());
    let definition = content();
    let mappings = destinations(&definition);
    let mut stale = request(10, &definition);
    stale.expected_basis.revision = revision(2, 7);
    assert_eq!(
        stage_schedule_advancement(&checkpoint, &pins(), Some(stale), &mappings, limits()),
        Err(ScheduleAdvancementError::DueSelection(
            DueSelectionError::Checkpoint(CheckpointError::StaleBasis)
        ))
    );
    let mut changed = pins();
    changed.content.package_digest = ContentDigest([99; 32]);
    assert_eq!(
        stage_schedule_advancement(
            &checkpoint,
            &changed,
            Some(request(10, &definition)),
            &mappings,
            limits()
        ),
        Err(ScheduleAdvancementError::DueSelection(
            DueSelectionError::Checkpoint(CheckpointError::ContentMismatch)
        ))
    );
}

#[test]
fn cursor_with_missing_remaining_event_is_refused_before_movement() {
    let mut supplied = scheduled();
    supplied.logical_time = time(11);
    supplied.continuity.catch_up = Some(CatchUpCursor {
        last_processed: Some(id(1)),
        pending_events: vec![],
        policy: content(),
        time: time(11),
    });
    let checkpoint = checkpoint(supplied);
    let definition = content();
    assert_eq!(
        stage(&checkpoint, 12, &definition, &destinations(&definition)),
        Err(ScheduleAdvancementError::DueSelection(
            DueSelectionError::StaleCursor
        ))
    );
}

#[test]
fn output_and_movement_capacity_fail_atomically_instead_of_dropping_consequences() {
    let checkpoint = checkpoint(scheduled());
    let definition = content();
    let mappings = destinations(&definition);
    let proposal = stage(&checkpoint, 11, &definition, &mappings).unwrap();
    let mut bounded = limits();
    bounded.output_bytes = proposal.accounted_output_bytes - 1;
    assert_eq!(
        stage_schedule_advancement(
            &checkpoint,
            &pins(),
            Some(request(11, &definition)),
            &mappings,
            bounded
        ),
        Err(ScheduleAdvancementError::Capacity)
    );
    bounded = limits();
    bounded.movements = 1;
    assert_eq!(
        stage_schedule_advancement(
            &checkpoint,
            &pins(),
            Some(request(11, &definition)),
            &mappings,
            bounded
        ),
        Err(ScheduleAdvancementError::Capacity)
    );
    bounded = limits();
    bounded.entities = 3;
    assert_eq!(
        stage_schedule_advancement(
            &checkpoint,
            &pins(),
            Some(request(11, &definition)),
            &mappings,
            bounded
        ),
        Err(ScheduleAdvancementError::Capacity)
    );
}

#[test]
fn maximum_logical_tick_is_supplied_exactly_and_oversize_counts_are_refused_before_allocation() {
    let mut supplied = scheduled();
    supplied.schedules.truncate(1);
    supplied.schedules[0].due = time(u64::MAX);
    let checkpoint = checkpoint(supplied);
    let definition = content();
    let mappings = destinations(&definition);
    let one = &mappings[..1];
    let proposal = stage(&checkpoint, u64::MAX, &definition, one).unwrap();
    assert_eq!(proposal.due.proposed_time.ticks, u64::MAX);
    assert_eq!(proposal.movements.len(), 1);
    for bound in [0, usize::MAX] {
        let mut invalid = limits();
        invalid.movements = bound;
        assert_eq!(
            stage_schedule_advancement(
                &checkpoint,
                &pins(),
                Some(request(u64::MAX, &definition)),
                one,
                invalid
            ),
            Err(ScheduleAdvancementError::InvalidLimits)
        );
    }
}
