use df_api::{CommandFields, RequestError, RequestField, admit_game_command};
use df_model::checkpoint::*;
use df_model::commands::{CommandError, CommandLimits};
use df_protocol::common as wire;
use df_types::{
    BuildIdentity, MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId,
    SessionRevision,
};
use prost::Message;

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
fn limits() -> CheckpointLimits {
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
            ticks: 120,
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
fn checkpoint(state: GameState) -> Result<Checkpoint, CheckpointError> {
    let rules = vec![rule()];
    let content_entries = vec![content()];
    let resource_constraints = resource_constraints();
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis(),
        pins(),
        state,
        ReferenceInventory {
            rules: &rules,
            content: &content_entries,
            resources: &resource_constraints,
            assets: &[],
        },
        limits(),
    )
}
fn fact(value: u8, ordinal: u32) -> GameFact {
    GameFact {
        id: FactId::from_bytes(&[value; 16]).unwrap(),
        revision: basis().revision,
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        ordinal,
        cause: None,
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: content(),
            subjects: vec![entity(4)],
        },
    }
}

fn pending() -> PendingResolution {
    PendingResolution {
        id: ResolutionId::from_bytes(&[9; 16]).unwrap(),
        basis: basis(),
        continuation: label("fixture-continuation-position-1"),
        window: ResolutionWindow {
            id: WindowId::from_bytes(&[10; 16]).unwrap(),
            phase: TriggerPhase::BeforeDraw,
            causal_fact: FactId::from_bytes(&[7; 16]).unwrap(),
            source: rule(),
            timer: None,
        },
        next: PendingInput::Reaction {
            remaining: vec![OfferedResponse {
                participant: member(3),
                offer: label("fixture-offer-1"),
                options: vec![label("fixture-option-1")],
                source: rule(),
            }],
        },
        choices: vec![],
        draw_ordinals: vec![],
        spent: vec![ResourceSpend {
            owner: entity(4),
            resource: label("fixture-resource-1"),
            amount: 1,
            source: rule(),
        }],
        rulings: vec![],
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

struct CommonFields {
    session: wire::SessionId,
    run: wire::RunId,
    operation: wire::OperationId,
    revision: wire::SessionRevision,
}
impl CommonFields {
    fn new(epoch: u64, sequence: u64) -> Self {
        Self {
            session: wire::SessionId::decode(
                wire::SessionId {
                    value: Some(vec![1; 16]),
                }
                .encode_to_vec()
                .as_slice(),
            )
            .unwrap(),
            run: wire::RunId::decode(
                wire::RunId {
                    value: Some(vec![2; 16]),
                }
                .encode_to_vec()
                .as_slice(),
            )
            .unwrap(),
            operation: wire::OperationId::decode(
                wire::OperationId {
                    value: Some(vec![20; 16]),
                }
                .encode_to_vec()
                .as_slice(),
            )
            .unwrap(),
            revision: wire::SessionRevision::decode(
                wire::SessionRevision {
                    epoch: Some(wire::RecoveryEpoch { value: Some(epoch) }),
                    sequence: Some(sequence),
                }
                .encode_to_vec()
                .as_slice(),
            )
            .unwrap(),
        }
    }
    fn borrow(&self) -> CommandFields<'_> {
        CommandFields {
            session: Some(&self.session),
            run: Some(&self.run),
            operation: Some(&self.operation),
            observed_revision: Some(&self.revision),
        }
    }
}
fn command_limits() -> CommandLimits {
    CommandLimits {
        maximum_records: 8,
        maximum_text_bytes: 32,
        maximum_retained_bytes: 8192,
    }
}
fn speak(text: &str) -> GameCommand {
    GameCommand::Speak {
        speaker: entity(4),
        text: text.into(),
        conversation: None,
    }
}
fn inventory<'a>(
    rules: &'a [RuleReference],
    content: &'a [ContentReference],
) -> ReferenceInventory<'a> {
    ReferenceInventory {
        rules,
        content,
        resources: &[],
        assets: &[],
    }
}
fn admit(
    fields: &CommonFields,
    command: GameCommand,
    current: &Checkpoint,
) -> Result<GameInput, RequestError> {
    admit_game_command(
        fields.borrow(),
        member(3),
        command,
        current,
        inventory(&[rule()], &[content()]),
        command_limits(),
    )
}
fn offered_checkpoint() -> Checkpoint {
    let mut state = state();
    state.facts.push(fact(7, 0));
    state.pending.push(pending());
    checkpoint(state).unwrap()
}
fn reaction() -> GameCommand {
    GameCommand::SelectReaction {
        resolution: ResolutionId::from_bytes(&[9; 16]).unwrap(),
        window: WindowId::from_bytes(&[10; 16]).unwrap(),
        offer: label("fixture-offer-1"),
        option: label("fixture-option-1"),
    }
}

#[test]
fn actual_common_wire_values_map_to_closed_command_with_trusted_member() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let fields = CommonFields::new(2, 8);
    let command = speak("hello");
    assert_eq!(
        admit(&fields, command.clone(), &current).unwrap(),
        GameInput::Game(CommandInput {
            basis: basis(),
            observed_revision: basis().revision,
            operation: OperationId::from_bytes(&[20; 16]).unwrap(),
            member: member(3),
            command,
        })
    );
    assert_eq!(current, before);
}

#[test]
fn older_observation_preserves_live_offer_while_stale_windows_refuse() {
    let current = offered_checkpoint();
    let before = current.clone();
    assert!(admit(&CommonFields::new(2, 0), reaction(), &current).is_ok());
    let stale = GameCommand::SelectReaction {
        window: WindowId::from_bytes(&[11; 16]).unwrap(),
        resolution: ResolutionId::from_bytes(&[9; 16]).unwrap(),
        offer: label("fixture-offer-1"),
        option: label("fixture-option-1"),
    };
    assert_eq!(
        admit(&CommonFields::new(2, 7), stale, &current),
        Err(RequestError::Domain(CommandError::StaleWindow))
    );
    assert_eq!(current, before);
}

#[test]
fn wrong_session_run_retired_epoch_and_future_revision_refuse_without_mutation() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    for (epoch, sequence) in [(1, u64::MAX), (3, 0), (2, 9)] {
        assert_eq!(
            admit(
                &CommonFields::new(epoch, sequence),
                speak("hello"),
                &current
            ),
            Err(RequestError::Domain(CommandError::StaleRevision))
        );
    }
    let mut fields = CommonFields::new(2, 8);
    fields.session.value = Some(vec![99; 16]);
    assert_eq!(
        admit(&fields, speak("hello"), &current),
        Err(RequestError::Domain(CommandError::WrongSession))
    );
    fields.session.value = Some(vec![1; 16]);
    fields.run.value = Some(vec![99; 16]);
    assert_eq!(
        admit(&fields, speak("hello"), &current),
        Err(RequestError::Domain(CommandError::WrongRun))
    );
    assert_eq!(current, before);
}

#[test]
fn absent_and_invalid_wire_operation_never_reach_domain_command() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let mut fields = CommonFields::new(2, 8);
    fields.operation.value = None;
    assert_eq!(
        admit(&fields, speak("hello"), &current),
        Err(RequestError::Missing(RequestField::Operation))
    );
    fields.operation.value = Some(vec![0; 16]);
    assert_eq!(
        admit(&fields, speak("hello"), &current),
        Err(RequestError::Identity {
            field: RequestField::Operation,
            cause: df_types::IdentityError::Zero
        })
    );
    fields.operation.value = Some(vec![20; 16]);
    fields.revision.sequence = None;
    assert_eq!(
        admit(&fields, speak("hello"), &current),
        Err(RequestError::Missing(RequestField::Sequence))
    );
    assert_eq!(current, before);
}

#[test]
fn unoffered_selection_wrong_input_kind_unknown_member_and_capacity_refuse() {
    let current = offered_checkpoint();
    let before = current.clone();
    let fields = CommonFields::new(2, 8);
    let forged = GameCommand::SelectReaction {
        resolution: ResolutionId::from_bytes(&[9; 16]).unwrap(),
        window: WindowId::from_bytes(&[10; 16]).unwrap(),
        offer: label("fixture-offer-1"),
        option: label("unoffered-option"),
    };
    assert_eq!(
        admit(&fields, forged, &current),
        Err(RequestError::Domain(CommandError::UnofferedResponse))
    );
    let wrong_kind = GameCommand::SubmitRoll {
        resolution: ResolutionId::from_bytes(&[9; 16]).unwrap(),
        window: WindowId::from_bytes(&[10; 16]).unwrap(),
    };
    assert_eq!(
        admit(&fields, wrong_kind, &current),
        Err(RequestError::Domain(CommandError::WrongPendingKind))
    );
    assert_eq!(
        admit_game_command(
            fields.borrow(),
            member(99),
            speak("hello"),
            &current,
            inventory(&[rule()], &[content()]),
            command_limits()
        ),
        Err(RequestError::Domain(CommandError::UnknownMember))
    );
    assert_eq!(
        admit(&fields, speak(&"x".repeat(33)), &current),
        Err(RequestError::Domain(CommandError::Capacity))
    );
    let mut bounds = command_limits();
    bounds.maximum_retained_bytes = 1;
    assert_eq!(
        admit_game_command(
            fields.borrow(),
            member(3),
            speak("hello"),
            &current,
            inventory(&[rule()], &[content()]),
            bounds
        ),
        Err(RequestError::Domain(CommandError::Capacity))
    );
    assert_eq!(current, before);
}
