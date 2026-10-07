use df_engine::director_staging::*;
use df_engine::effect_emission;
use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, MemberId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};
use df_world::{
    AdmittedEnvironmentalChange, AdmittedScheduleDestination, DueSelectionLimits,
    DueSelectionRequest, EnvironmentalDeltaError, EnvironmentalDeltaLimits,
    ScheduleAdvancementError, ScheduleAdvancementLimits,
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

fn checkpoint_at(
    supplied: GameState,
    supplied_basis: Basis,
    supplied_pins: CheckpointPins,
) -> Checkpoint {
    try_checkpoint_at(supplied, supplied_basis, supplied_pins).unwrap()
}

fn try_checkpoint_at(
    supplied: GameState,
    supplied_basis: Basis,
    supplied_pins: CheckpointPins,
) -> Result<Checkpoint, CheckpointError> {
    let mut entries = admitted_content();
    entries.extend(
        supplied
            .schedules
            .iter()
            .map(|event| event.definition.clone()),
    );
    if let Some(cursor) = &supplied.continuity.catch_up {
        entries.push(cursor.policy.clone());
    }
    let assets: Vec<_> = supplied
        .continuity
        .scenes
        .iter()
        .map(|scene| scene.geometry.clone())
        .collect();
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        supplied_basis,
        supplied_pins,
        supplied,
        ReferenceInventory {
            rules: &[rule()],
            content: &entries,
            resources: &resource_constraints(),
            assets: &assets,
        },
        checkpoint_limits(),
    )
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

fn checkpoint(supplied: GameState) -> Checkpoint {
    checkpoint_at(supplied, basis(), pins())
}

fn limits() -> DirectorLimits {
    DirectorLimits {
        maximum_checkpoint_bytes: 1024 * 1024,
        maximum_pass_bytes: 8 * 1024 * 1024,
        maximum_relationships: 32,
        world: DueSelectionLimits {
            queue_events: 32,
            selected_events: 4,
            output_bytes: 64 * 1024,
        },
    }
}

fn compose<'a>(
    current: &'a Checkpoint,
    interaction: Option<Checkpoint>,
    narrative: Option<Checkpoint>,
    bounds: DirectorLimits,
) -> Result<DirectorStaging<'a>, DirectorError> {
    compose_director_candidates(
        current,
        current.pins(),
        DueSelectionRequest {
            expected_basis: current.basis(),
            target_time: current.state().logical_time,
            paused: false,
            deadline_remaining: Duration::from_secs(1),
            policy: &content(),
        },
        DirectorCandidates {
            interaction,
            narrative,
        },
        bounds,
    )
}

fn interaction_state() -> GameState {
    let mut supplied = state();
    supplied.relationships.push(Relationship {
        subject: entity(4),
        object: entity(4),
        policy: content(),
        state: label("fixture-authored-reaction"),
        trust: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        affection: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        respect: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        fear: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        suspicion: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        debt: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        familiarity: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
    });
    supplied
}

#[test]
fn bounded_policy_candidates_are_detached_and_cannot_change_authoritative_records() {
    let current = checkpoint(state());
    let unchanged = current.clone();
    let interaction = checkpoint(interaction_state());
    let mut story = interaction_state();
    story.narrative.open_threads.push(content());
    let narrative = checkpoint(story.clone());
    let staged = compose(&current, Some(interaction), Some(narrative), limits()).unwrap();
    let DirectorStaging::Staged(candidate) = staged else {
        panic!("unexpected world work")
    };
    assert_eq!(candidate.basis(), current.basis());
    assert_eq!(candidate.pins(), current.pins());
    assert_eq!(candidate.candidate().state(), &story);
    assert!(candidate.world().events.is_empty());
    assert_eq!(candidate.candidate().state().facts, current.state().facts);
    assert_eq!(
        candidate.candidate().state().intents,
        current.state().intents
    );
    assert_eq!(candidate.candidate().state().draws, current.state().draws);
    assert_eq!(
        candidate.candidate().state().resources,
        current.state().resources
    );
    assert_eq!(current, unchanged);
}

#[test]
fn unresolved_world_events_preserve_cursor_and_stop_policy_staging() {
    let mut supplied = state();
    supplied.schedules = vec![
        ScheduledEvent {
            id: id(9),
            entity: entity(4),
            due: time(9),
            definition: content(),
        },
        ScheduledEvent {
            id: id(7),
            entity: entity(4),
            due: time(9),
            definition: content(),
        },
        ScheduledEvent {
            id: id(8),
            entity: entity(4),
            due: time(7),
            definition: content(),
        },
    ];
    let current = checkpoint(supplied);
    let unchanged = current.clone();
    // The contradictory downstream snapshot is never selected while world consequences wait.
    let staged = compose(
        &current,
        Some(checkpoint(interaction_state())),
        None,
        limits(),
    )
    .unwrap();
    let DirectorStaging::WorldPending(world) = staged else {
        panic!("missing pending boundary")
    };
    assert_eq!(
        world
            .events
            .iter()
            .map(|event| event.id)
            .collect::<Vec<_>>(),
        vec![id(8), id(7), id(9)]
    );
    assert_eq!(world.cursor.last_processed, Some(id(9)));
    assert_eq!(current.state().continuity.catch_up, None);
    assert_eq!(current, unchanged);
}

#[test]
fn paused_due_backlog_remains_pending_even_without_selected_events() {
    let mut supplied = state();
    supplied.schedules.push(ScheduledEvent {
        id: id(8),
        entity: entity(4),
        due: time(7),
        definition: content(),
    });
    let current = checkpoint(supplied);
    let output = compose_director_candidates(
        &current,
        current.pins(),
        DueSelectionRequest {
            expected_basis: current.basis(),
            target_time: current.state().logical_time,
            paused: true,
            deadline_remaining: Duration::from_secs(1),
            policy: &content(),
        },
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        limits(),
    )
    .unwrap();
    let DirectorStaging::WorldPending(world) = output else {
        panic!("lost paused backlog")
    };
    assert!(world.events.is_empty());
    assert_eq!(world.remaining_due(), 1);
}

#[test]
fn sibling_and_downstream_authority_conflicts_discard_the_entire_policy_candidate() {
    let current = checkpoint(state());
    let unchanged = current.clone();
    let mut illicit = interaction_state();
    illicit.resources.first_mut().unwrap().value = 3;
    assert_eq!(
        compose(&current, Some(checkpoint(illicit)), None, limits()),
        Err(DirectorError::AuthorityConflict(DirectorStage::Interaction)),
    );
    let interaction = checkpoint(interaction_state());
    let mut stale_story = state();
    stale_story.narrative.open_threads.push(content());
    assert_eq!(
        compose(
            &current,
            Some(interaction),
            Some(checkpoint(stale_story)),
            limits()
        ),
        Err(DirectorError::AuthorityConflict(DirectorStage::Narrative)),
    );
    assert_eq!(current, unchanged);
}

#[test]
fn directors_cannot_fabricate_canonical_facts_or_relabel_source_bindings() {
    let current = checkpoint(state());
    let mut fabricated = state();
    fabricated.facts.push(GameFact {
        id: FactId::from_bytes(&[9; 16]).unwrap(),
        revision: basis().revision,
        operation: df_types::OperationId::from_bytes(&[5; 16]).unwrap(),
        ordinal: 0,
        cause: None,
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: content(),
            subjects: vec![entity(4)],
        },
    });
    assert_eq!(
        compose(&current, Some(checkpoint(fabricated)), None, limits()),
        Err(DirectorError::AuthorityConflict(DirectorStage::Interaction)),
    );
    let mut wrong_pins = pins();
    wrong_pins.rules.handler = label("unadmitted-handler");
    assert_eq!(
        compose(
            &current,
            Some(checkpoint_at(state(), basis(), wrong_pins)),
            None,
            limits()
        ),
        Err(DirectorError::ProposalBinding {
            stage: DirectorStage::Interaction,
            error: CheckpointError::RulesMismatch,
        }),
    );
    let mut wrong_basis = basis();
    wrong_basis.run = RunId::from_bytes(&[8; 16]).unwrap();
    assert_eq!(
        compose(
            &current,
            None,
            Some(checkpoint_at(state(), wrong_basis, pins())),
            limits()
        ),
        Err(DirectorError::ProposalBinding {
            stage: DirectorStage::Narrative,
            error: CheckpointError::WrongRun,
        }),
    );
}

#[test]
fn duplicate_directional_relationships_are_not_resolved_by_vector_order() {
    let current = checkpoint(state());
    let unchanged = current.clone();
    let mut ambiguous = interaction_state();
    let mut conflicting = ambiguous.relationships.first().unwrap().clone();
    conflicting.state = label("fixture-conflicting-reaction");
    ambiguous.relationships.push(conflicting);
    let original = ambiguous.clone();
    for reverse in [false, true] {
        let mut supplied = ambiguous.clone();
        if reverse {
            supplied.relationships.reverse();
        }
        let before = supplied.clone();
        assert_eq!(
            try_checkpoint_at(supplied.clone(), basis(), pins()),
            Err(CheckpointError::DuplicateIdentity)
        );
        assert_eq!(supplied, before);
    }
    assert_eq!(ambiguous, original);
    assert_eq!(current, unchanged);
}

#[test]
fn retained_capacities_and_checked_pass_budget_refuse_before_staging() {
    let current = checkpoint(state());
    let mut bounds = limits();
    bounds.maximum_checkpoint_bytes = current.retained_bytes().unwrap() - 1;
    assert_eq!(
        compose(&current, None, None, bounds),
        Err(DirectorError::Capacity)
    );
    bounds = limits();
    bounds.maximum_pass_bytes = 1;
    assert_eq!(
        compose(&current, None, None, bounds),
        Err(DirectorError::Capacity)
    );
    bounds = limits();
    bounds.maximum_checkpoint_bytes = usize::MAX;
    assert_eq!(
        compose(&current, None, None, bounds),
        Err(DirectorError::Capacity)
    );
    bounds = limits();
    bounds.maximum_relationships = 0;
    assert_eq!(
        compose(&current, None, None, bounds),
        Err(DirectorError::InvalidLimits)
    );
}

#[test]
fn world_deadline_refusal_exposes_no_policy_candidate() {
    let current = checkpoint(state());
    let request = DueSelectionRequest {
        expected_basis: current.basis(),
        target_time: time(9),
        paused: false,
        deadline_remaining: Duration::ZERO,
        policy: &content(),
    };
    assert_eq!(
        compose_director_candidates(
            &current,
            current.pins(),
            request,
            DirectorCandidates {
                interaction: None,
                narrative: None
            },
            limits(),
        ),
        Err(DirectorError::World(df_world::DueSelectionError::Deadline)),
    );
}

#[test]
fn stale_input_and_content_bindings_are_refused_before_director_selection() {
    let current = checkpoint(state());
    let mut expected = current.basis();
    expected.revision = revision(2, 7);
    let request = DueSelectionRequest {
        expected_basis: expected,
        target_time: time(9),
        paused: false,
        deadline_remaining: Duration::from_secs(1),
        policy: &content(),
    };
    assert_eq!(
        compose_director_candidates(
            &current,
            current.pins(),
            request,
            DirectorCandidates {
                interaction: None,
                narrative: None
            },
            limits(),
        ),
        Err(DirectorError::Binding(CheckpointError::StaleBasis)),
    );
    let mut wrong_pins = pins();
    wrong_pins.content.content_digest = ContentDigest([9; 32]);
    assert_eq!(
        compose(
            &current,
            None,
            Some(checkpoint_at(state(), basis(), wrong_pins)),
            limits()
        ),
        Err(DirectorError::ProposalBinding {
            stage: DirectorStage::Narrative,
            error: CheckpointError::ContentMismatch,
        }),
    );
}

#[test]
fn stale_world_cursor_does_not_publish_a_policy_snapshot() {
    let mut supplied = state();
    supplied.continuity.catch_up = Some(CatchUpCursor {
        last_processed: Some(id(7)),
        pending_events: vec![],
        policy: content(),
        time: time(9),
    });
    let current = checkpoint(supplied);
    let unchanged = current.clone();
    assert_eq!(
        compose(&current, None, None, limits()),
        Err(DirectorError::World(
            df_world::DueSelectionError::StaleCursor
        )),
    );
    assert_eq!(current, unchanged);
}

#[test]
fn each_canonical_pending_kind_stops_world_and_preserves_existing_resolution() {
    let offered = vec![OfferedResponse {
        participant: member(3),
        offer: label("fixture-offer"),
        options: vec![label("fixture-option")],
        source: rule(),
    }];
    for next in [
        PendingInput::Choice {
            remaining: offered.clone(),
        },
        PendingInput::Reaction {
            remaining: offered.clone(),
        },
        PendingInput::Roll {
            participant: member(3),
            sides: vec![6],
            source: rule(),
        },
        PendingInput::Ruling {
            permitted: offered,
            source: rule(),
        },
    ] {
        let mut supplied = state();
        let fact = FactId::from_bytes(&[7; 16]).unwrap();
        supplied.facts.push(GameFact {
            id: fact,
            revision: basis().revision,
            operation: df_types::OperationId::from_bytes(&[5; 16]).unwrap(),
            ordinal: 0,
            cause: None,
            audience: AudienceScope::Shared,
            value: FactValue::ContentEvent {
                definition: content(),
                subjects: vec![entity(4)],
            },
        });
        supplied.pending.push(PendingResolution {
            id: ResolutionId::from_bytes(&[9; 16]).unwrap(),
            basis: basis(),
            continuation: label("fixture-continuation"),
            window: ResolutionWindow {
                id: WindowId::from_bytes(&[10; 16]).unwrap(),
                phase: TriggerPhase::BeforeDraw,
                causal_fact: fact,
                source: rule(),
                timer: None,
            },
            next,
            choices: vec![],
            draw_ordinals: vec![],
            spent: vec![],
            rulings: vec![],
        });
        let current = checkpoint(supplied);
        let unchanged = current.clone();
        let output = compose(
            &current,
            Some(checkpoint(interaction_state())),
            None,
            limits(),
        )
        .unwrap();
        let DirectorStaging::RulesPending(pending) = output else {
            panic!("skipped pending rules")
        };
        assert_eq!(pending, current.state().pending);
        assert_eq!(current, unchanged);
    }
}

fn schedule_state() -> GameState {
    let mut supplied = state();
    for id in [entity(5), entity(6)] {
        supplied.entities.push(WorldEntity {
            id,
            definition: content(),
            location: None,
            position: None,
            identity_revision: label("fixture-destination"),
        });
    }
    supplied.schedules = vec![
        ScheduledEvent {
            id: id(8),
            entity: entity(4),
            due: time(10),
            definition: content(),
        },
        ScheduledEvent {
            id: id(9),
            entity: entity(4),
            due: time(11),
            definition: content(),
        },
    ];
    supplied
}

fn schedule_limits() -> ScheduleDirectorLimits {
    let directors = limits();
    ScheduleDirectorLimits {
        directors,
        schedule: ScheduleAdvancementLimits {
            selection: directors.world,
            entities: 32,
            destinations: 32,
            movements: 16,
            output_bytes: 128 * 1024,
        },
        checkpoint: checkpoint_limits(),
    }
}

fn schedule_compose<'a>(
    current: &'a Checkpoint,
    destinations: &[AdmittedScheduleDestination<'_>],
    candidates: DirectorCandidates,
    limits: ScheduleDirectorLimits,
) -> Result<DirectorStaging<'a>, DirectorError> {
    compose_schedule_candidates(
        current,
        current.pins(),
        ScheduleDirectorRequest {
            accepted_time: Some(DueSelectionRequest {
                expected_basis: current.basis(),
                target_time: time(12),
                paused: false,
                deadline_remaining: Duration::from_secs(1),
                policy: &content(),
            }),
            destinations,
            environmental: EnvironmentalRequest::NotApplicable,
        },
        candidates,
        ReferenceInventory {
            rules: &[rule()],
            content: &admitted_content(),
            resources: &resource_constraints(),
            assets: &[],
        },
        limits,
    )
}

#[test]
fn admitted_schedule_chain_merges_sibling_policy_and_stages_real_cursor_atomically() {
    let current = checkpoint(schedule_state());
    let unchanged = current.clone();
    let definition = content();
    let destinations = [
        AdmittedScheduleDestination {
            event: id(8),
            definition: &definition,
            entity: entity(4),
            expected_location: None,
            destination: entity(5),
        },
        AdmittedScheduleDestination {
            event: id(9),
            definition: &definition,
            entity: entity(4),
            expected_location: Some(entity(5)),
            destination: entity(6),
        },
    ];
    let mut interaction = schedule_state();
    interaction.relationships = interaction_state().relationships;
    let outcome = schedule_compose(
        &current,
        &destinations,
        DirectorCandidates {
            interaction: Some(checkpoint(interaction)),
            narrative: None,
        },
        schedule_limits(),
    )
    .unwrap();
    let DirectorStaging::Staged(staged) = outcome else {
        panic!("admitted mapped schedule did not stage")
    };
    assert_eq!(staged.candidate().state().logical_time, time(12));
    assert_eq!(
        staged
            .candidate()
            .state()
            .entities
            .first()
            .unwrap()
            .location,
        Some(entity(6))
    );
    assert_eq!(
        staged.candidate().state().continuity.catch_up.as_ref(),
        Some(&staged.world().cursor)
    );
    assert_eq!(staged.world().cursor.last_processed, Some(id(9)));
    assert_eq!(
        staged.candidate().state().relationships,
        interaction_state().relationships
    );
    assert_eq!(staged.candidate().state().facts, current.state().facts);
    assert_eq!(staged.candidate().state().intents, current.state().intents);
    assert_eq!(current, unchanged);
}

#[test]
fn unmapped_selected_schedule_cannot_advance_time_location_or_cursor() {
    let current = checkpoint(schedule_state());
    let unchanged = current.clone();
    let definition = content();
    let partial = [AdmittedScheduleDestination {
        event: id(8),
        definition: &definition,
        entity: entity(4),
        expected_location: None,
        destination: entity(5),
    }];
    let outcome = schedule_compose(
        &current,
        &partial,
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        schedule_limits(),
    )
    .unwrap();
    let DirectorStaging::WorldPending(due) = outcome else {
        panic!("missing consequence was skipped")
    };
    assert_eq!(due.events.len(), 2);
    assert_eq!(current, unchanged);
}

#[test]
fn complete_bounded_schedule_prefix_retains_replayable_remaining_backlog() {
    let current = checkpoint(schedule_state());
    let definition = content();
    let destinations = [AdmittedScheduleDestination {
        event: id(8),
        definition: &definition,
        entity: entity(4),
        expected_location: None,
        destination: entity(5),
    }];
    let mut bounds = schedule_limits();
    bounds.directors.world.selected_events = 1;
    bounds.schedule.selection = bounds.directors.world;
    let outcome = schedule_compose(
        &current,
        &destinations,
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        bounds,
    )
    .unwrap();
    let DirectorStaging::Staged(staged) = outcome else {
        panic!("bounded complete prefix refused")
    };
    assert_eq!(staged.world().events.len(), 1);
    assert_eq!(staged.world().remaining_due(), 1);
    assert_eq!(
        staged
            .candidate()
            .state()
            .continuity
            .catch_up
            .as_ref()
            .unwrap()
            .pending_events,
        vec![id(9)]
    );
    assert_eq!(
        staged
            .candidate()
            .state()
            .entities
            .first()
            .unwrap()
            .location,
        Some(entity(5))
    );
    let continuation = compose(staged.candidate(), None, None, limits()).unwrap();
    let DirectorStaging::WorldPending(due) = continuation else {
        panic!("remaining event disappeared")
    };
    assert_eq!(due.events.first().unwrap().id, id(9));
}

#[test]
fn schedule_source_location_and_time_admission_refusals_leave_current_unchanged() {
    let current = checkpoint(schedule_state());
    let unchanged = current.clone();
    let definition = content();
    let wrong = [AdmittedScheduleDestination {
        event: id(8),
        definition: &definition,
        entity: entity(4),
        expected_location: Some(entity(6)),
        destination: entity(5),
    }];
    assert_eq!(
        schedule_compose(
            &current,
            &wrong,
            DirectorCandidates {
                interaction: None,
                narrative: None
            },
            schedule_limits()
        ),
        Err(DirectorError::Schedule(
            ScheduleAdvancementError::StaleLocation
        )),
    );
    assert_eq!(
        compose_schedule_candidates(
            &current,
            current.pins(),
            ScheduleDirectorRequest {
                accepted_time: None,
                destinations: &[],
                environmental: EnvironmentalRequest::NotApplicable,
            },
            DirectorCandidates {
                interaction: None,
                narrative: None
            },
            ReferenceInventory {
                rules: &[],
                content: &[],
                resources: &[],
                assets: &[]
            },
            schedule_limits(),
        ),
        Err(DirectorError::Schedule(
            ScheduleAdvancementError::TimeNotAccepted
        )),
    );
    assert_eq!(current, unchanged);
}

#[test]
fn stale_narrative_snapshot_cannot_overwrite_selected_world_schedule() {
    let current = checkpoint(schedule_state());
    let definition = content();
    let destinations = [
        AdmittedScheduleDestination {
            event: id(8),
            definition: &definition,
            entity: entity(4),
            expected_location: None,
            destination: entity(5),
        },
        AdmittedScheduleDestination {
            event: id(9),
            definition: &definition,
            entity: entity(4),
            expected_location: Some(entity(5)),
            destination: entity(6),
        },
    ];
    let mut narrative = schedule_state();
    narrative.narrative.open_threads.push(content());
    assert_eq!(
        schedule_compose(
            &current,
            &destinations,
            DirectorCandidates {
                interaction: None,
                narrative: Some(checkpoint(narrative))
            },
            schedule_limits()
        ),
        Err(DirectorError::AuthorityConflict(DirectorStage::Narrative)),
    );
    assert_eq!(current.state().logical_time, time(9));
    assert_eq!(current.state().entities.first().unwrap().location, None);
}

fn alternative_thread() -> ContentReference {
    ContentReference {
        package: content().package,
        entry: label("fixture-independent-thread"),
    }
}

fn admitted_content() -> Vec<ContentReference> {
    vec![content(), alternative_thread()]
}

fn source_backed_schedule_state() -> GameState {
    let mut supplied = schedule_state();
    let source = FactId::from_bytes(&[7; 16]).unwrap();
    let effect = EffectId::from_bytes(&[20; 16]).unwrap();
    let operation = df_types::OperationId::from_bytes(&[5; 16]).unwrap();
    supplied.facts.push(GameFact {
        id: source,
        revision: basis().revision,
        operation,
        ordinal: 0,
        cause: None,
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: content(),
            subjects: vec![entity(4)],
        },
    });
    supplied
        .narrative
        .open_threads
        .extend([content(), alternative_thread()]);
    supplied.intents.push(DurableIntent {
        id: effect,
        basis: basis(),
        operation,
        slot: 0,
        kind: EffectKind::PublishPresentation,
        job: None,
        timer: None,
        generation: 1,
        status: DurableStatus::Pending,
        definition: content(),
    });
    supplied.decisions.push(AcceptedDecision {
        operation,
        revision: basis().revision,
        facts: vec![source],
        draws: vec![],
        effects: vec![effect],
        source_policy: label("fixture-admitted-source-policy"),
        semantic_output: None,
    });
    supplied
}

#[test]
fn actual_world_and_narrative_producers_compose_without_changing_declared_effects_or_facts() {
    use df_narrative::{
        ProgressLimits, ThreadCheckpointRequest, ThreadConsequenceSelection, ThreadDisposition,
    };
    let current = checkpoint(source_backed_schedule_state());
    let unchanged = current.clone();
    let definition = content();
    let destinations = [
        AdmittedScheduleDestination {
            event: id(8),
            definition: &definition,
            entity: entity(4),
            expected_location: None,
            destination: entity(5),
        },
        AdmittedScheduleDestination {
            event: id(9),
            definition: &definition,
            entity: entity(4),
            expected_location: Some(entity(5)),
            destination: entity(6),
        },
    ];
    let policy = label("fixture-admitted-thread-policy");
    let selections = [ThreadConsequenceSelection {
        thread: &definition,
        event_definition: &definition,
        source: FactId::from_bytes(&[7; 16]).unwrap(),
        // Source admission supplies Resolve; the reducer never infers it from a fact.
        disposition: ThreadDisposition::Resolve,
    }];
    let mut interaction_state = current.state().clone();
    interaction_state.relationships.push(Relationship {
        subject: entity(4),
        object: entity(5),
        policy: content(),
        state: label("fixture-authored-reaction"),
        trust: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        affection: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        respect: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        fear: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        suspicion: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        debt: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
        familiarity: RelationshipAxisState {
            value: label("fixture-authored-reaction"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline { source: content() },
        },
    });
    let interaction = checkpoint(interaction_state);
    let composed = compose_schedule_thread_progress(
        &current,
        current.pins(),
        ScheduleDirectorRequest {
            accepted_time: Some(DueSelectionRequest {
                expected_basis: current.basis(),
                target_time: time(12),
                paused: false,
                deadline_remaining: Duration::from_secs(1),
                policy: &definition,
            }),
            destinations: &destinations,
            environmental: EnvironmentalRequest::NotApplicable,
        },
        Some(interaction.clone()),
        ThreadCheckpointRequest {
            expected_basis: current.basis(),
            admitted_pins: current.pins(),
            policy: &policy,
            expected_policy: &policy,
            inventory: ReferenceInventory {
                rules: &[rule()],
                content: &admitted_content(),
                resources: &resource_constraints(),
                assets: &[],
            },
            checkpoint_limits: checkpoint_limits(),
            selections: &selections,
        },
        ScheduleThreadProgressLimits {
            directors: schedule_limits(),
            narrative: ProgressLimits {
                records: 32,
                consequences: 4,
                work: 1024,
            },
        },
    )
    .unwrap();
    let DirectorStaging::Staged(staged) = composed else {
        panic!("actual producer refused")
    };
    assert_eq!(staged.narrative_policy(), Some(&policy));
    assert_eq!(
        staged.narrative_evidence().first().unwrap().source,
        selections[0].source
    );
    assert_eq!(
        staged.candidate().state().relationships,
        interaction.state().relationships
    );
    let effects = effect_emission::declare_accepted_effects(
        staged.candidate(),
        staged.basis(),
        staged.pins(),
        df_types::OperationId::from_bytes(&[5; 16]).unwrap(),
        effect_emission::EffectEmissionLimits {
            maximum_scan_records: 32,
            maximum_effects: 4,
            maximum_comparisons: 512,
            maximum_retained_bytes: 65536,
        },
    )
    .unwrap();
    assert_eq!(effects, current.state().intents);
    assert!(
        effects
            .iter()
            .all(|effect| effect.status == DurableStatus::Pending)
    );
    assert_eq!(
        staged.candidate().state().narrative.open_threads,
        vec![alternative_thread()]
    );
    assert_eq!(staged.candidate().state().facts, current.state().facts);
    assert_eq!(
        staged.candidate().state().decisions,
        current.state().decisions
    );
    assert_eq!(
        staged
            .candidate()
            .state()
            .entities
            .first()
            .unwrap()
            .location,
        Some(entity(6))
    );
    assert_eq!(current, unchanged);
}

#[test]
fn actual_narrative_producer_refuses_absent_source_before_engine_selection() {
    use df_narrative::{
        CheckpointProgressError, ProgressError, ProgressLimits, ThreadCheckpointRequest,
        ThreadConsequenceSelection, ThreadDisposition, stage_checkpoint_thread_progress,
    };
    let current = checkpoint(source_backed_schedule_state());
    let unchanged = current.clone();
    let definition = content();
    let policy = label("fixture-admitted-thread-policy");
    let selections = [ThreadConsequenceSelection {
        thread: &definition,
        event_definition: &definition,
        source: FactId::from_bytes(&[99; 16]).unwrap(),
        disposition: ThreadDisposition::Resolve,
    }];
    assert_eq!(
        stage_checkpoint_thread_progress(
            &current,
            ThreadCheckpointRequest {
                expected_basis: current.basis(),
                admitted_pins: current.pins(),
                policy: &policy,
                expected_policy: &policy,
                inventory: ReferenceInventory {
                    rules: &[rule()],
                    content: &admitted_content(),
                    resources: &resource_constraints(),
                    assets: &[],
                },
                checkpoint_limits: checkpoint_limits(),
                selections: &selections,
            },
            ProgressLimits {
                records: 32,
                consequences: 4,
                work: 1024
            },
        ),
        Err(CheckpointProgressError::Progress(
            ProgressError::MissingFact
        )),
    );
    assert_eq!(current, unchanged);
}

#[test]
fn actual_effect_inspection_keeps_staged_schedule_pending_when_registration_is_missing() {
    struct MissingExecutor;
    impl effect_emission::EffectRegistrationInspector for MissingExecutor {
        type Refusal = EffectKind;

        fn inspect(&self, intent: &DurableIntent) -> Result<(), Self::Refusal> {
            // Faithful unavailable native inventory; this fixture never dispatches work.
            Err(intent.kind)
        }
    }
    let current = checkpoint(source_backed_schedule_state());
    let definition = content();
    let destinations = [
        AdmittedScheduleDestination {
            event: id(8),
            definition: &definition,
            entity: entity(4),
            expected_location: None,
            destination: entity(5),
        },
        AdmittedScheduleDestination {
            event: id(9),
            definition: &definition,
            entity: entity(4),
            expected_location: Some(entity(5)),
            destination: entity(6),
        },
    ];
    let staged = schedule_compose(
        &current,
        &destinations,
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        schedule_limits(),
    )
    .unwrap();
    let DirectorStaging::Staged(staged) = staged else {
        panic!("mapped schedule refused")
    };
    let predecessor = Basis {
        revision: revision(2, 7),
        ..basis()
    };
    assert_eq!(
        effect_emission::inspect_staged_effects(
            staged.candidate(),
            predecessor,
            staged.pins(),
            df_types::OperationId::from_bytes(&[5; 16]).unwrap(),
            effect_emission::EffectEmissionLimits {
                maximum_scan_records: 32,
                maximum_effects: 4,
                maximum_comparisons: 512,
                maximum_retained_bytes: 65536,
            },
            &MissingExecutor,
        ),
        Err(effect_emission::EffectInspectionError::Registration {
            effect: EffectId::from_bytes(&[20; 16]).unwrap(),
            kind: EffectKind::PublishPresentation,
            refusal: EffectKind::PublishPresentation,
        }),
    );
    assert_eq!(staged.candidate().state().intents, current.state().intents);
    assert!(
        current
            .state()
            .intents
            .iter()
            .all(|intent| intent.status == DurableStatus::Pending)
    );
}

#[test]
fn actual_narrative_continue_preserves_alternatives_and_consumed_source_is_not_reapplied() {
    use df_narrative::{
        CheckpointProgressError, ProgressError, ProgressLimits, ThreadCheckpointRequest,
        ThreadConsequenceSelection, ThreadDisposition, stage_checkpoint_thread_progress,
    };
    let current = checkpoint(source_backed_schedule_state());
    let unchanged = current.clone();
    let definition = content();
    let policy = label("fixture-admitted-thread-policy");
    let selections = [ThreadConsequenceSelection {
        thread: &definition,
        event_definition: &definition,
        source: FactId::from_bytes(&[7; 16]).unwrap(),
        disposition: ThreadDisposition::Continue,
    }];
    let progress_limits = ProgressLimits {
        records: 32,
        consequences: 4,
        work: 1024,
    };
    let narrative = stage_checkpoint_thread_progress(
        &current,
        ThreadCheckpointRequest {
            expected_basis: current.basis(),
            admitted_pins: current.pins(),
            policy: &policy,
            expected_policy: &policy,
            inventory: ReferenceInventory {
                rules: &[rule()],
                content: &admitted_content(),
                resources: &resource_constraints(),
                assets: &[],
            },
            checkpoint_limits: checkpoint_limits(),
            selections: &selections,
        },
        progress_limits,
    )
    .unwrap();
    let composed = compose(&current, None, Some(narrative.checkpoint), limits()).unwrap();
    let DirectorStaging::Staged(staged) = composed else {
        panic!("Continue producer refused")
    };
    assert_eq!(
        staged.candidate().state().narrative.open_threads,
        vec![content(), alternative_thread()]
    );
    assert_eq!(
        staged.candidate().state().narrative.accepted_facts,
        vec![selections.first().unwrap().source]
    );
    assert_eq!(
        stage_checkpoint_thread_progress(
            staged.candidate(),
            ThreadCheckpointRequest {
                expected_basis: staged.basis(),
                admitted_pins: staged.pins(),
                policy: &policy,
                expected_policy: &policy,
                inventory: ReferenceInventory {
                    rules: &[rule()],
                    content: &admitted_content(),
                    resources: &resource_constraints(),
                    assets: &[],
                },
                checkpoint_limits: checkpoint_limits(),
                selections: &selections,
            },
            progress_limits,
        ),
        Err(CheckpointProgressError::Progress(
            ProgressError::RepeatedConsequence
        )),
    );
    assert_eq!(staged.candidate().state().facts, current.state().facts);
    assert_eq!(current, unchanged);
}

fn environmental_schedule_state() -> GameState {
    let mut supplied = source_backed_schedule_state();
    let history = FactId::from_bytes(&[7; 16]).unwrap();
    for (location, id, ordinal) in [(entity(5), 21, 1), (entity(6), 22, 2)] {
        let cause = FactId::from_bytes(&[id; 16]).unwrap();
        supplied.facts.push(GameFact {
            id: cause,
            revision: basis().revision,
            operation: df_types::OperationId::from_bytes(&[5; 16]).unwrap(),
            ordinal,
            cause: Some(history),
            audience: AudienceScope::Shared,
            value: FactValue::ContentEvent {
                definition: content(),
                subjects: vec![location],
            },
        });
        supplied.decisions.first_mut().unwrap().facts.push(cause);
        supplied.continuity.environment.push(EnvironmentalState {
            location,
            definition: content(),
            change_facts: vec![history],
        });
        supplied.continuity.scenes.push(SceneIdentityRevision {
            scene: location,
            revision: label("fixture-scene-source"),
            source_facts: vec![history],
            geometry: AssetReference {
                key: label("fixture-geometry-reference"),
                digest: ContentDigest([6; 32]),
                byte_length: 128,
                kind: AssetKind::TacticalGeometry,
            },
            canonical_pack: label("fixture-pack-source"),
        });
    }
    supplied.continuity.canonical_packs.push(CanonicalPack {
        revision: label("fixture-pack-source"),
        digest: ContentDigest([8; 32]),
        bible: VisualBible {
            revision: label("fixture-bible-source"),
            definition: content(),
            palette: vec![],
            style: String::new(),
            references: vec![],
        },
        identities: vec![],
    });
    supplied
}

fn environmental_destinations(current: &Checkpoint) -> Vec<AdmittedScheduleDestination<'_>> {
    let definition = &current.state().narrative.definition;
    vec![
        AdmittedScheduleDestination {
            event: id(8),
            definition,
            entity: entity(4),
            expected_location: None,
            destination: entity(5),
        },
        AdmittedScheduleDestination {
            event: id(9),
            definition,
            entity: entity(4),
            expected_location: Some(entity(5)),
            destination: entity(6),
        },
    ]
}

fn environmental_replacements(current: &Checkpoint) -> Vec<EnvironmentalState> {
    current
        .state()
        .continuity
        .environment
        .iter()
        .map(|environment| {
            let mut replacement = environment.clone();
            let source = if environment.location == entity(5) {
                21
            } else {
                22
            };
            replacement
                .change_facts
                .push(FactId::from_bytes(&[source; 16]).unwrap());
            replacement
        })
        .collect()
}

fn environmental_changes<'a>(
    current: &'a Checkpoint,
    replacements: &'a [EnvironmentalState],
) -> Vec<AdmittedEnvironmentalChange<'a>> {
    replacements
        .iter()
        .map(|replacement| AdmittedEnvironmentalChange {
            expected: current
                .state()
                .continuity
                .environment
                .iter()
                .find(|record| record.location == replacement.location)
                .unwrap(),
            replacement,
            geometry: current
                .state()
                .continuity
                .scenes
                .iter()
                .find(|record| record.scene == replacement.location)
                .unwrap(),
        })
        .collect()
}

fn environmental_request<'a>(
    current: &'a Checkpoint,
    destinations: &'a [AdmittedScheduleDestination<'a>],
    changes: Option<&'a [AdmittedEnvironmentalChange<'a>]>,
) -> ScheduleDirectorRequest<'a> {
    ScheduleDirectorRequest {
        accepted_time: Some(DueSelectionRequest {
            expected_basis: current.basis(),
            target_time: time(12),
            paused: false,
            deadline_remaining: Duration::from_secs(1),
            policy: &current.state().narrative.definition,
        }),
        destinations,
        environmental: EnvironmentalRequest::Required {
            admitted_changes: changes,
            limits: EnvironmentalDeltaLimits {
                input_records: 256,
                changes: 16,
                output_bytes: 65536,
            },
        },
    }
}

fn environmental_assets(current: &Checkpoint) -> Vec<AssetReference> {
    current
        .state()
        .continuity
        .scenes
        .iter()
        .map(|scene| scene.geometry.clone())
        .collect()
}

fn compose_environment<'a>(
    current: &'a Checkpoint,
    destinations: &[AdmittedScheduleDestination<'_>],
    changes: Option<&[AdmittedEnvironmentalChange<'_>]>,
    candidates: DirectorCandidates,
) -> Result<DirectorStaging<'a>, DirectorError> {
    compose_schedule_candidates(
        current,
        current.pins(),
        environmental_request(current, destinations, changes),
        candidates,
        ReferenceInventory {
            rules: &[rule()],
            content: &admitted_content(),
            resources: &resource_constraints(),
            assets: &environmental_assets(current),
        },
        schedule_limits(),
    )
}

#[test]
fn actual_environment_and_schedule_proposals_merge_with_policy_in_one_owned_candidate() {
    let current = checkpoint(environmental_schedule_state());
    let unchanged = current.clone();
    let destinations = environmental_destinations(&current);
    let replacements = environmental_replacements(&current);
    let mut changes = environmental_changes(&current, &replacements);
    changes.reverse();
    let mut interaction = current.state().clone();
    interaction.relationships = interaction_state().relationships;
    let output = compose_environment(
        &current,
        &destinations,
        Some(&changes),
        DirectorCandidates {
            interaction: Some(checkpoint(interaction)),
            narrative: None,
        },
    )
    .unwrap();
    let DirectorStaging::Staged(staged) = output else {
        panic!("admitted environmental work did not stage")
    };
    assert_eq!(staged.basis(), current.basis());
    assert_eq!(staged.pins(), current.pins());
    assert_eq!(staged.candidate().state().logical_time, time(12));
    assert_eq!(
        staged.candidate().state().continuity.environment,
        replacements
    );
    assert_eq!(
        staged.candidate().state().continuity.scenes,
        current.state().continuity.scenes
    );
    assert_eq!(
        staged
            .candidate()
            .state()
            .entities
            .first()
            .unwrap()
            .location,
        Some(entity(6))
    );
    assert_eq!(
        staged.candidate().state().continuity.catch_up.as_ref(),
        Some(&staged.world().cursor)
    );
    assert_eq!(staged.candidate().state().facts, current.state().facts);
    assert_eq!(
        staged.candidate().state().decisions,
        current.state().decisions
    );
    assert_eq!(staged.candidate().state().intents, current.state().intents);
    assert_eq!(current, unchanged);
}

#[test]
fn missing_environmental_producer_is_typed_pending_without_advancing_accepted_time() {
    let current = checkpoint(environmental_schedule_state());
    let unchanged = current.clone();
    let destinations = environmental_destinations(&current);
    assert_eq!(
        compose_environment(
            &current,
            &destinations,
            None,
            DirectorCandidates {
                interaction: None,
                narrative: None
            }
        ),
        Ok(DirectorStaging::MissingEnvironmentalProducer)
    );
    assert_eq!(current, unchanged);
}

#[test]
fn last_environment_geometry_failure_discards_the_whole_schedule_candidate() {
    let current = checkpoint(environmental_schedule_state());
    let unchanged = current.clone();
    let destinations = environmental_destinations(&current);
    let replacements = environmental_replacements(&current);
    let first = current.state().continuity.environment.first().unwrap();
    let last = current.state().continuity.environment.last().unwrap();
    let first_geometry = current.state().continuity.scenes.first().unwrap();
    let mut wrong_geometry = current.state().continuity.scenes.last().unwrap().clone();
    wrong_geometry.geometry.digest = ContentDigest([99; 32]);
    let changes = [
        AdmittedEnvironmentalChange {
            expected: first,
            replacement: replacements.first().unwrap(),
            geometry: first_geometry,
        },
        AdmittedEnvironmentalChange {
            expected: last,
            replacement: replacements.last().unwrap(),
            geometry: &wrong_geometry,
        },
    ];
    assert_eq!(
        compose_environment(
            &current,
            &destinations,
            Some(&changes),
            DirectorCandidates {
                interaction: None,
                narrative: None
            }
        ),
        Err(DirectorError::Environment(
            EnvironmentalDeltaError::StaleGeometry
        ))
    );
    assert_eq!(current, unchanged);
}

#[test]
fn last_environment_source_cause_failure_discards_earlier_valid_changes() {
    let mut state = environmental_schedule_state();
    let last = state.facts.last_mut().unwrap();
    last.value = FactValue::ContentEvent {
        definition: content(),
        subjects: vec![entity(4)],
    };
    let current = checkpoint(state);
    let unchanged = current.clone();
    let destinations = environmental_destinations(&current);
    let replacements = environmental_replacements(&current);
    let changes = environmental_changes(&current, &replacements);
    assert_eq!(
        compose_environment(
            &current,
            &destinations,
            Some(&changes),
            DirectorCandidates {
                interaction: None,
                narrative: None
            }
        ),
        Err(DirectorError::Environment(
            EnvironmentalDeltaError::InvalidFactSource
        ))
    );
    assert_eq!(current, unchanged);
}

#[test]
fn last_narrative_failure_cannot_publish_world_time_cursor_or_environmental_work() {
    let current = checkpoint(environmental_schedule_state());
    let unchanged = current.clone();
    let destinations = environmental_destinations(&current);
    let replacements = environmental_replacements(&current);
    let changes = environmental_changes(&current, &replacements);
    assert_eq!(
        compose_environment(
            &current,
            &destinations,
            Some(&changes),
            DirectorCandidates {
                interaction: None,
                narrative: Some(current.clone())
            }
        ),
        Err(DirectorError::AuthorityConflict(DirectorStage::Narrative))
    );
    assert_eq!(current, unchanged);
}

#[test]
fn environmental_schedule_binding_checks_session_run_revision_and_every_source_pin() {
    let current = checkpoint(environmental_schedule_state());
    let unchanged = current.clone();
    let destinations = environmental_destinations(&current);
    let replacements = environmental_replacements(&current);
    let changes = environmental_changes(&current, &replacements);
    for (expected, error) in [
        (
            Basis {
                session: SessionId::from_bytes(&[99; 16]).unwrap(),
                ..basis()
            },
            CheckpointError::WrongSession,
        ),
        (
            Basis {
                run: RunId::from_bytes(&[99; 16]).unwrap(),
                ..basis()
            },
            CheckpointError::WrongRun,
        ),
        (
            Basis {
                revision: revision(2, 7),
                ..basis()
            },
            CheckpointError::StaleBasis,
        ),
    ] {
        let mut request = environmental_request(&current, &destinations, Some(&changes));
        request.accepted_time.as_mut().unwrap().expected_basis = expected;
        assert_eq!(
            compose_schedule_candidates(
                &current,
                current.pins(),
                request,
                DirectorCandidates {
                    interaction: None,
                    narrative: None
                },
                ReferenceInventory {
                    rules: &[],
                    content: &[],
                    resources: &[],
                    assets: &[]
                },
                schedule_limits()
            ),
            Err(DirectorError::Binding(error))
        );
    }
    for selector in 0..3 {
        let mut pins = current.pins().clone();
        let error = match selector {
            0 => {
                pins.rules.handler_digest = ContentDigest([99; 32]);
                CheckpointError::RulesMismatch
            }
            1 => {
                pins.content.package_digest = ContentDigest([99; 32]);
                CheckpointError::ContentMismatch
            }
            _ => {
                pins.build = BuildIdentity::new(
                    Some("other"),
                    Some("native"),
                    Some("wasm"),
                    Some("config"),
                    Some("content"),
                )
                .unwrap();
                CheckpointError::BuildMismatch
            }
        };
        assert_eq!(
            compose_schedule_candidates(
                &current,
                &pins,
                environmental_request(&current, &destinations, Some(&changes)),
                DirectorCandidates {
                    interaction: None,
                    narrative: None
                },
                ReferenceInventory {
                    rules: &[],
                    content: &[],
                    resources: &[],
                    assets: &[]
                },
                schedule_limits()
            ),
            Err(DirectorError::Binding(error))
        );
    }
    assert_eq!(current, unchanged);
}

#[test]
fn environmental_candidate_constructor_failure_leaves_source_and_schedules_untouched() {
    let current = checkpoint(environmental_schedule_state());
    let unchanged = current.clone();
    let destinations = environmental_destinations(&current);
    let replacements = environmental_replacements(&current);
    let changes = environmental_changes(&current, &replacements);
    assert_eq!(
        compose_schedule_candidates(
            &current,
            current.pins(),
            environmental_request(&current, &destinations, Some(&changes)),
            DirectorCandidates {
                interaction: None,
                narrative: None
            },
            ReferenceInventory {
                rules: &[rule()],
                content: &admitted_content(),
                resources: &resource_constraints(),
                assets: &[]
            },
            schedule_limits()
        ),
        Err(DirectorError::InvalidCandidate(
            CheckpointError::InvalidReference
        ))
    );
    assert_eq!(current, unchanged);
}

#[test]
fn environmental_output_budget_overflow_is_rejected_before_any_staging() {
    let current = checkpoint(environmental_schedule_state());
    let unchanged = current.clone();
    let destinations = environmental_destinations(&current);
    let replacements = environmental_replacements(&current);
    let changes = environmental_changes(&current, &replacements);
    let mut request = environmental_request(&current, &destinations, Some(&changes));
    let EnvironmentalRequest::Required { limits, .. } = &mut request.environmental else {
        panic!("fixture requires environmental work")
    };
    limits.output_bytes = usize::MAX;
    assert_eq!(
        compose_schedule_candidates(
            &current,
            current.pins(),
            request,
            DirectorCandidates {
                interaction: None,
                narrative: None
            },
            ReferenceInventory {
                rules: &[],
                content: &[],
                resources: &[],
                assets: &[]
            },
            schedule_limits()
        ),
        Err(DirectorError::Capacity)
    );
    assert_eq!(current, unchanged);
}

#[test]
fn environmental_entry_preserves_rules_pending_before_missing_producer_or_world_work() {
    let mut state = environmental_schedule_state();
    state.pending.push(PendingResolution {
        id: ResolutionId::from_bytes(&[9; 16]).unwrap(),
        basis: basis(),
        continuation: label("fixture-existing-rules-continuation"),
        window: ResolutionWindow {
            id: WindowId::from_bytes(&[10; 16]).unwrap(),
            phase: TriggerPhase::BeforeDraw,
            causal_fact: FactId::from_bytes(&[7; 16]).unwrap(),
            source: rule(),
            timer: None,
        },
        next: PendingInput::Choice {
            remaining: vec![OfferedResponse {
                participant: member(3),
                offer: label("fixture-offer"),
                options: vec![label("fixture-option")],
                source: rule(),
            }],
        },
        choices: vec![],
        draw_ordinals: vec![],
        spent: vec![],
        rulings: vec![],
    });
    let current = checkpoint(state);
    let unchanged = current.clone();
    let destinations = environmental_destinations(&current);
    let output = compose_environment(
        &current,
        &destinations,
        None,
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
    )
    .unwrap();
    let DirectorStaging::RulesPending(pending) = output else {
        panic!("existing rules were skipped")
    };
    assert_eq!(pending, current.state().pending);
    assert_eq!(current, unchanged);
}

#[test]
fn environmental_entry_keeps_unmapped_due_work_pending_before_any_environmental_merge() {
    let current = checkpoint(environmental_schedule_state());
    let unchanged = current.clone();
    let replacements = environmental_replacements(&current);
    let changes = environmental_changes(&current, &replacements);
    let output = compose_environment(
        &current,
        &[],
        Some(&changes),
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
    )
    .unwrap();
    let DirectorStaging::WorldPending(due) = output else {
        panic!("unmapped due work was skipped")
    };
    assert_eq!(due.events.len(), 2);
    assert_eq!(current, unchanged);
}

struct NarrativeAdmission {
    policy: RevisionLabel,
    rules: Vec<RuleReference>,
    content: Vec<ContentReference>,
    resources: Vec<ResourceConstraint>,
}

impl NarrativeAdmission {
    fn new() -> Self {
        Self {
            policy: label("fixture-admitted-thread-policy"),
            rules: vec![rule()],
            content: admitted_content(),
            resources: resource_constraints(),
        }
    }

    fn request<'a>(
        &'a self,
        current: &'a Checkpoint,
        selections: &'a [df_narrative::ThreadConsequenceSelection<'a>],
    ) -> df_narrative::ThreadCheckpointRequest<'a> {
        df_narrative::ThreadCheckpointRequest {
            expected_basis: current.basis(),
            admitted_pins: current.pins(),
            policy: &self.policy,
            expected_policy: &self.policy,
            inventory: ReferenceInventory {
                rules: &self.rules,
                content: &self.content,
                resources: &self.resources,
                assets: &[],
            },
            checkpoint_limits: checkpoint_limits(),
            selections,
        }
    }
}

fn thread_limits() -> ScheduleThreadProgressLimits {
    ScheduleThreadProgressLimits {
        directors: schedule_limits(),
        narrative: df_narrative::ProgressLimits {
            records: 32,
            consequences: 4,
            work: 1024,
        },
    }
}

fn compose_threads<'a>(
    current: &'a Checkpoint,
    narrative: df_narrative::ThreadCheckpointRequest<'_>,
    limits: ScheduleThreadProgressLimits,
) -> Result<DirectorStaging<'a>, DirectorError> {
    compose_schedule_thread_progress(
        current,
        current.pins(),
        ScheduleDirectorRequest {
            accepted_time: Some(DueSelectionRequest {
                expected_basis: current.basis(),
                target_time: time(12),
                paused: false,
                deadline_remaining: Duration::from_secs(1),
                policy: &current.state().narrative.definition,
            }),
            destinations: &[],
            environmental: EnvironmentalRequest::NotApplicable,
        },
        None,
        narrative,
        limits,
    )
}

#[test]
fn engine_narrative_continue_is_deterministic_and_preserves_private_facts_without_grants() {
    use df_narrative::{ThreadConsequenceSelection, ThreadDisposition};
    let mut state = source_backed_schedule_state();
    state.schedules.clear();
    state.facts[0].audience = AudienceScope::Members(vec![member(3)]);
    let current = checkpoint(state);
    let unchanged = current.clone();
    let admission = NarrativeAdmission::new();
    let definition = content();
    let selections = [ThreadConsequenceSelection {
        thread: &definition,
        event_definition: &definition,
        source: FactId::from_bytes(&[7; 16]).unwrap(),
        disposition: ThreadDisposition::Continue,
    }];
    let first = compose_threads(
        &current,
        admission.request(&current, &selections),
        thread_limits(),
    )
    .unwrap();
    let second = compose_threads(
        &current,
        admission.request(&current, &selections),
        thread_limits(),
    )
    .unwrap();
    assert_eq!(first, second);
    let DirectorStaging::Staged(staged) = first else {
        panic!("Continue refused")
    };
    assert_eq!(
        staged.candidate().state().narrative.open_threads,
        current.state().narrative.open_threads
    );
    assert_eq!(
        staged.candidate().state().narrative.accepted_facts,
        vec![selections[0].source]
    );
    assert_eq!(staged.narrative_policy(), Some(&admission.policy));
    assert_eq!(
        staged.narrative_evidence()[0].disposition,
        ThreadDisposition::Continue
    );
    let mut protected = staged.candidate().state().clone();
    protected.narrative = current.state().narrative.clone();
    protected.logical_time = current.state().logical_time;
    protected.continuity.catch_up = current.state().continuity.catch_up.clone();
    assert_eq!(protected, *current.state());
    assert_eq!(staged.candidate().basis(), current.basis());
    assert_eq!(staged.candidate().pins(), current.pins());
    assert_eq!(current, unchanged);

    let retry = compose_threads(
        staged.candidate(),
        admission.request(staged.candidate(), &selections),
        thread_limits(),
    );
    assert_eq!(
        retry,
        Err(DirectorError::Narrative(
            df_narrative::CheckpointProgressError::Progress(
                df_narrative::ProgressError::RepeatedConsequence,
            )
        ))
    );
    assert_eq!(current, unchanged);
}

#[test]
fn engine_narrative_refuses_stale_versions_and_discards_all_staged_world_changes() {
    use df_narrative::{CheckpointProgressError, ProgressError};
    let mut state = source_backed_schedule_state();
    state.schedules.clear();
    let current = checkpoint(state);
    let unchanged = current.clone();
    let admission = NarrativeAdmission::new();

    for expected_basis in [
        Basis {
            revision: revision(2, 7),
            ..current.basis()
        },
        Basis {
            revision: revision(3, 8),
            ..current.basis()
        },
        Basis {
            run: RunId::from_bytes(&[99; 16]).unwrap(),
            ..current.basis()
        },
        Basis {
            session: SessionId::from_bytes(&[99; 16]).unwrap(),
            ..current.basis()
        },
    ] {
        let mut request = admission.request(&current, &[]);
        request.expected_basis = expected_basis;
        let expected = current
            .validate_resume(expected_basis, current.pins())
            .unwrap_err();
        assert_eq!(
            compose_threads(&current, request, thread_limits()),
            Err(DirectorError::Narrative(CheckpointProgressError::Binding(
                expected
            )))
        );
    }
    for component in 0..3 {
        let mut stale = current.pins().clone();
        let expected = match component {
            0 => {
                stale.content.package_digest = ContentDigest([99; 32]);
                CheckpointError::ContentMismatch
            }
            1 => {
                stale.rules.source_manifest_digest = ContentDigest([99; 32]);
                CheckpointError::RulesMismatch
            }
            _ => {
                stale.build = BuildIdentity::new(
                    Some("different-source"),
                    Some("fixture-native-1"),
                    Some("fixture-wasm-1"),
                    Some("fixture-config-1"),
                    Some("fixture-content-1"),
                )
                .unwrap();
                CheckpointError::BuildMismatch
            }
        };
        let mut request = admission.request(&current, &[]);
        request.admitted_pins = &stale;
        assert_eq!(
            compose_threads(&current, request, thread_limits()),
            Err(DirectorError::Narrative(CheckpointProgressError::Binding(
                expected
            )))
        );
    }
    let stale_policy = label("stale-thread-policy");
    let mut request = admission.request(&current, &[]);
    request.expected_policy = &stale_policy;
    assert_eq!(
        compose_threads(&current, request, thread_limits()),
        Err(DirectorError::Narrative(CheckpointProgressError::Progress(
            ProgressError::StalePolicy
        )))
    );
    assert_eq!(current, unchanged);
}

#[test]
fn engine_narrative_missing_source_and_exhausted_work_expose_no_candidate() {
    use df_narrative::{
        CheckpointProgressError, ProgressError, ThreadConsequenceSelection, ThreadDisposition,
    };
    let mut state = source_backed_schedule_state();
    state.schedules.clear();
    let current = checkpoint(state);
    let unchanged = current.clone();
    let admission = NarrativeAdmission::new();
    let definition = content();
    let selections = [ThreadConsequenceSelection {
        thread: &definition,
        event_definition: &definition,
        source: FactId::from_bytes(&[99; 16]).unwrap(),
        disposition: ThreadDisposition::Resolve,
    }];
    assert_eq!(
        compose_threads(
            &current,
            admission.request(&current, &selections),
            thread_limits()
        ),
        Err(DirectorError::Narrative(CheckpointProgressError::Progress(
            ProgressError::MissingFact
        )))
    );
    let mut bounds = thread_limits();
    bounds.narrative.work = 0;
    assert_eq!(
        compose_threads(&current, admission.request(&current, &[]), bounds),
        Err(DirectorError::Narrative(CheckpointProgressError::Progress(
            ProgressError::Capacity
        )))
    );
    let mut request = admission.request(&current, &[]);
    request.checkpoint_limits.maximum_retained_bytes *= 2;
    assert_eq!(
        compose_threads(&current, request, thread_limits()),
        Err(DirectorError::InvalidLimits)
    );
    assert_eq!(current, unchanged);
}

#[test]
fn engine_narrative_reserves_evidence_within_the_total_composition_byte_budget() {
    use df_narrative::{ThreadConsequenceSelection, ThreadDisposition};
    let mut state = source_backed_schedule_state();
    state.schedules.clear();
    let current = checkpoint(state);
    let unchanged = current.clone();
    let admission = NarrativeAdmission::new();
    let definition = content();
    let selections = [ThreadConsequenceSelection {
        thread: &definition,
        event_definition: &definition,
        source: FactId::from_bytes(&[7; 16]).unwrap(),
        disposition: ThreadDisposition::Continue,
    }];
    let mut bounds = thread_limits();
    bounds.directors.directors.maximum_pass_bytes = current.retained_bytes().unwrap()
        + 3 * bounds.directors.directors.maximum_checkpoint_bytes
        + bounds.directors.schedule.output_bytes;
    assert!(matches!(
        schedule_compose(
            &current,
            &[],
            DirectorCandidates {
                interaction: None,
                narrative: None
            },
            bounds.directors
        )
        .unwrap(),
        DirectorStaging::Staged(_)
    ));
    assert_eq!(
        compose_threads(&current, admission.request(&current, &selections), bounds),
        Err(DirectorError::Capacity)
    );
    assert_eq!(current, unchanged);
}

#[test]
fn engine_narrative_waits_for_unresolved_world_work_before_examining_consequences() {
    use df_narrative::{ThreadConsequenceSelection, ThreadDisposition};
    let current = checkpoint(source_backed_schedule_state());
    let unchanged = current.clone();
    let admission = NarrativeAdmission::new();
    let definition = content();
    let missing_source = [ThreadConsequenceSelection {
        thread: &definition,
        event_definition: &definition,
        source: FactId::from_bytes(&[99; 16]).unwrap(),
        disposition: ThreadDisposition::Resolve,
    }];
    let output = compose_threads(
        &current,
        admission.request(&current, &missing_source),
        thread_limits(),
    )
    .unwrap();
    let DirectorStaging::WorldPending(world) = output else {
        panic!("unresolved World work skipped")
    };
    assert_eq!(world.events.len(), 2);
    assert_eq!(current, unchanged);
}
