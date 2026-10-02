use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId,
    SessionRevision,
};

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

#[test]
fn owns_detached_state_and_preserves_exact_resume_basis() {
    let supplied = state();
    let retained = supplied.clone();
    let checkpoint = checkpoint(supplied).unwrap();
    assert_eq!(checkpoint.state(), &retained);
    assert_eq!(checkpoint.basis(), basis());
    assert_eq!(
        checkpoint.validate_resume(basis(), &pins()),
        Ok(&checkpoint)
    );
    let mut unrelated = retained.clone();
    unrelated.resources[0].value = 7;
    assert_eq!(checkpoint.state().resources[0].value, 4);
}
#[test]
fn rejects_reference_to_unadmitted_source_or_content() {
    for changed in 0..4 {
        let mut supplied = state();
        match changed {
            0 => supplied.entities[0].definition.entry = label("fixture-unadmitted"),
            1 => supplied.entities[0].definition.package = label("fixture-other-package"),
            2 => supplied.resources[0].source.clause = label("fixture-unadmitted"),
            3 => supplied.resources[0].source.catalog = label("fixture-other-catalog"),
            _ => unreachable!(),
        }
        assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidReference));
    }
}
#[test]
fn rejects_missing_member_entity_and_duplicate_resource_references() {
    let mut supplied = state();
    supplied.members[0].character = Some(entity(99));
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidReference));
    let mut supplied = state();
    supplied.characters[0].owner = member(99);
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidReference));
    let mut supplied = state();
    supplied.resources[0].owner = entity(99);
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidReference));
    let mut supplied = state();
    supplied.resources.push(supplied.resources[0].clone());
    assert_eq!(
        checkpoint(supplied),
        Err(CheckpointError::DuplicateIdentity)
    );
}
#[test]
fn validates_source_supplied_resource_interval_without_a_universal_cap() {
    for (minimum, maximum, value) in [(0, 8, -1), (0, 8, 9), (9, 8, 8)] {
        let mut supplied = state();
        supplied.resources[0].minimum = minimum;
        supplied.resources[0].maximum = maximum;
        supplied.resources[0].value = value;
        assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidResource));
    }
    let mut supplied = state();
    supplied.resources[0].minimum = -8;
    supplied.resources[0].value = -4;
    let rules = vec![rule()];
    let content_entries = vec![content()];
    let admitted_bounds = vec![ResourceConstraint {
        owner: entity(4),
        resource: label("fixture-resource-1"),
        minimum: -8,
        maximum: 8,
        source: rule(),
    }];
    assert!(
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis(),
            pins(),
            supplied,
            ReferenceInventory {
                rules: &rules,
                content: &content_entries,
                resources: &admitted_bounds,
                assets: &[]
            },
            limits()
        )
        .is_ok()
    );
}
#[test]
fn refuses_same_sequence_from_other_epoch_and_caller_newer_basis() {
    let checkpoint = checkpoint(state()).unwrap();
    for candidate in [
        revision(1, 8),
        revision(2, 7),
        revision(2, 9),
        revision(3, 8),
    ] {
        let mut expected = basis();
        expected.revision = candidate;
        assert_eq!(
            checkpoint.validate_resume(expected, &pins()),
            Err(CheckpointError::StaleBasis)
        );
    }
    let mut expected = basis();
    expected.session = SessionId::from_bytes(&[9; 16]).unwrap();
    assert_eq!(
        checkpoint.validate_resume(expected, &pins()),
        Err(CheckpointError::WrongSession)
    );
    let mut expected = basis();
    expected.run = RunId::from_bytes(&[9; 16]).unwrap();
    assert_eq!(
        checkpoint.validate_resume(expected, &pins()),
        Err(CheckpointError::WrongRun)
    );
}
#[test]
fn refuses_every_exact_rules_pin_change() {
    let checkpoint = checkpoint(state()).unwrap();
    for changed in 0..8 {
        let mut admitted = pins();
        match changed {
            0 => admitted.rules.mode = RulesMode::DisclosedCustom,
            1 => admitted.rules.ruleset = label("fixture-other"),
            2 => admitted.rules.catalog = label("fixture-other"),
            3 => admitted.rules.catalog_digest = ContentDigest([9; 32]),
            4 => admitted.rules.source_manifest = label("fixture-other"),
            5 => admitted.rules.source_manifest_digest = ContentDigest([9; 32]),
            6 => admitted.rules.handler = label("fixture-other"),
            7 => admitted.rules.handler_digest = ContentDigest([9; 32]),
            _ => unreachable!(),
        }
        assert_eq!(
            checkpoint.validate_resume(basis(), &admitted),
            Err(CheckpointError::RulesMismatch)
        );
    }
}
#[test]
fn refuses_content_package_digest_and_build_changes() {
    let checkpoint = checkpoint(state()).unwrap();
    for changed in 0..4 {
        let mut admitted = pins();
        match changed {
            0 => admitted.content.content = label("fixture-other"),
            1 => admitted.content.content_digest = ContentDigest([9; 32]),
            2 => admitted.content.package = label("fixture-other"),
            3 => admitted.content.package_digest = ContentDigest([9; 32]),
            _ => unreachable!(),
        }
        assert_eq!(
            checkpoint.validate_resume(basis(), &admitted),
            Err(CheckpointError::ContentMismatch)
        );
    }
    let mut admitted = pins();
    admitted.build = BuildIdentity::new(
        Some("fixture-other"),
        Some("fixture-native-1"),
        Some("fixture-wasm-1"),
        Some("fixture-config-1"),
        Some("fixture-content-1"),
    )
    .unwrap();
    assert_eq!(
        checkpoint.validate_resume(basis(), &admitted),
        Err(CheckpointError::BuildMismatch)
    );
}
#[test]
fn accepts_historical_facts_without_rewriting_their_recovery_epoch() {
    let mut supplied = state();
    let mut historical = fact(7, 0);
    historical.revision = revision(1, 40);
    supplied.facts.push(historical.clone());
    let checkpoint = checkpoint(supplied).unwrap();
    assert_eq!(checkpoint.state().facts, vec![historical]);
}
#[test]
fn rejects_forward_causal_reference_and_out_of_order_facts() {
    let mut supplied = state();
    let mut first = fact(7, 0);
    let next = fact(8, 1);
    first.cause = Some(next.id);
    supplied.facts = vec![first, next];
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidReference));
    let mut supplied = state();
    supplied.facts = vec![fact(7, 1)];
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidOrder));
    let mut supplied = state();
    supplied.facts = vec![fact(7, 0), fact(8, 2)];
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidOrder));
}
#[test]
fn preserves_actual_draw_values_and_rejects_invalid_dice() {
    let draw = ActualDraw {
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        ordinal: 0,
        resolution: ResolutionId::from_bytes(&[7; 16]).unwrap(),
        window: WindowId::from_bytes(&[8; 16]).unwrap(),
        sides: 20,
        value: 13,
        source: rule(),
    };
    let mut supplied = state();
    supplied.draws.push(draw.clone());
    assert_eq!(
        checkpoint(supplied).unwrap().state().draws,
        vec![draw.clone()]
    );
    for (sides, value) in [(0, 1), (20, 0), (20, 21)] {
        let mut supplied = state();
        let mut changed = draw.clone();
        changed.sides = sides;
        changed.value = value;
        supplied.draws.push(changed);
        assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidDraw));
    }
    let mut supplied = state();
    supplied.draws = vec![draw.clone(), draw];
    assert_eq!(
        checkpoint(supplied),
        Err(CheckpointError::DuplicateIdentity)
    );
}
#[test]
fn rejects_decision_fact_from_another_operation() {
    let mut supplied = state();
    let record = fact(7, 0);
    supplied.facts.push(record.clone());
    supplied.decisions.push(AcceptedDecision {
        operation: OperationId::from_bytes(&[8; 16]).unwrap(),
        revision: basis().revision,
        facts: vec![record.id],
        draws: vec![],
        effects: vec![],
        source_policy: label("fixture-policy-1"),
        semantic_output: None,
    });
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidReference));
}
#[test]
fn keeps_attributed_false_claim_separate_from_canonical_facts() {
    let mut supplied = state();
    let record = fact(7, 0);
    supplied.facts.push(record.clone());
    supplied.beliefs.push(AttributedClaim {
        id: RecordId::from_bytes(&[9; 16]).unwrap(),
        holder: entity(4),
        subject: entity(4),
        claim: "synthetic contradictory claim".to_owned(),
        evidence: vec![record.id],
        audience: AudienceScope::Members(vec![member(3)]),
        source: content(),
    });
    let checkpoint = checkpoint(supplied).unwrap();
    assert_eq!(checkpoint.state().facts, vec![record]);
    assert_eq!(
        checkpoint.state().beliefs[0].claim,
        "synthetic contradictory claim"
    );
}
#[test]
fn rejects_zero_time_scale_unknown_private_audience_and_capacity_overflow() {
    let mut supplied = state();
    supplied.logical_time.ticks_per_second = 0;
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidTime));
    let mut supplied = state();
    let mut record = fact(7, 0);
    record.audience = AudienceScope::Members(vec![member(99)]);
    supplied.facts.push(record);
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidReference));
    let rules = vec![rule()];
    let content_entries = vec![content()];
    let resource_constraints = resource_constraints();
    let mut admitted = limits();
    admitted.maximum_records = 1;
    assert_eq!(
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis(),
            pins(),
            state(),
            ReferenceInventory {
                rules: &rules,
                content: &content_entries,
                resources: &resource_constraints,
                assets: &[]
            },
            admitted
        ),
        Err(CheckpointError::Capacity)
    );
}
#[test]
fn rejects_unsupported_schema_before_admitting_state() {
    let rules = vec![rule()];
    let content_entries = vec![content()];
    let resource_constraints = resource_constraints();
    for schema in [0, 2, u16::MAX] {
        assert_eq!(
            Checkpoint::new(
                schema,
                basis(),
                pins(),
                state(),
                ReferenceInventory {
                    rules: &rules,
                    content: &content_entries,
                    resources: &resource_constraints,
                    assets: &[]
                },
                limits()
            ),
            Err(CheckpointError::UnsupportedSchema)
        );
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
#[test]
fn preserves_exact_reaction_window_continuation_and_spent_resources() {
    let mut supplied = state();
    supplied.facts.push(fact(7, 0));
    let mut before = pending();
    before.choices.push(AcceptedChoice {
        participant: member(3),
        offer: label("fixture-old-offer"),
        selected: label("fixture-old-selection"),
        source: rule(),
    });
    supplied.pending.push(before.clone());
    let checkpoint = checkpoint(supplied).unwrap();
    assert_eq!(checkpoint.state().pending, vec![before]);
    assert_eq!(checkpoint.state().resources[0].value, 4);
}
#[test]
fn rejects_stale_pending_basis_empty_offer_and_unknown_spent_resource() {
    for changed in 0..3 {
        let mut supplied = state();
        supplied.facts.push(fact(7, 0));
        let mut continuation = pending();
        match changed {
            0 => continuation.basis.revision = revision(2, 7),
            1 => continuation.next = PendingInput::Choice { remaining: vec![] },
            2 => continuation.spent[0].resource = label("fixture-unknown-resource"),
            _ => unreachable!(),
        }
        supplied.pending.push(continuation);
        assert_eq!(
            checkpoint(supplied),
            Err(match changed {
                0 => CheckpointError::StaleBasis,
                1 => CheckpointError::InvalidContinuation,
                2 => CheckpointError::InvalidReference,
                _ => unreachable!(),
            })
        );
    }
}
#[test]
fn preserves_unknown_dispatch_status_and_refuses_timer_kind_without_owned_instance() {
    let mut supplied = state();
    let intent = DurableIntent {
        id: EffectId::from_bytes(&[11; 16]).unwrap(),
        basis: basis(),
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        slot: 0,
        kind: EffectKind::RunAi,
        job: Some(JobId::from_bytes(&[12; 16]).unwrap()),
        timer: None,
        generation: 2,
        status: DurableStatus::SentUnknown,
        definition: content(),
    };
    supplied.intents.push(intent.clone());
    assert_eq!(
        checkpoint(supplied).unwrap().state().intents,
        vec![intent.clone()]
    );
    let mut supplied = state();
    let mut invalid = intent;
    invalid.kind = EffectKind::ArmTimer;
    invalid.job = None;
    invalid.timer = Some(TimerId::from_bytes(&[13; 16]).unwrap());
    supplied.intents.push(invalid);
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidIntent));
}
#[test]
fn counts_retained_claim_text_in_utf8_bytes() {
    let mut supplied = state();
    supplied.beliefs.push(AttributedClaim {
        id: RecordId::from_bytes(&[9; 16]).unwrap(),
        holder: entity(4),
        subject: entity(4),
        claim: "é".repeat(129),
        evidence: vec![],
        audience: AudienceScope::Shared,
        source: content(),
    });
    assert_eq!(checkpoint(supplied), Err(CheckpointError::Capacity));
}

#[test]
fn checkpoint_cannot_authorize_its_own_resource_maximum() {
    let mut supplied = state();
    supplied.resources[0].maximum = 100;
    supplied.resources[0].value = 99;
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidResource));
    let mut supplied = state();
    supplied.resources[0].minimum = -100;
    supplied.resources[0].value = -99;
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidResource));
    let rules = vec![rule()];
    let content_entries = vec![content()];
    assert_eq!(
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis(),
            pins(),
            state(),
            ReferenceInventory {
                rules: &rules,
                content: &content_entries,
                resources: &[],
                assets: &[]
            },
            limits()
        ),
        Err(CheckpointError::InvalidResource)
    );
}

#[test]
fn rejects_reserved_heap_capacity_even_when_the_vector_has_no_live_records() {
    let normal = checkpoint(state()).unwrap();
    let mut supplied = state();
    supplied.facts = Vec::with_capacity(10_000);
    let rules = vec![rule()];
    let content_entries = vec![content()];
    let resource_constraints = resource_constraints();
    let mut admitted = limits();
    admitted.maximum_retained_bytes = normal.retained_bytes().unwrap() + 128;
    assert_eq!(
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis(),
            pins(),
            supplied,
            ReferenceInventory {
                rules: &rules,
                content: &content_entries,
                resources: &resource_constraints,
                assets: &[]
            },
            admitted
        ),
        Err(CheckpointError::Capacity)
    );
    assert!(normal.retained_bytes().unwrap() > normal.retained_heap_bytes().unwrap());
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

#[test]
fn rejects_missing_family_references_and_spare_nested_capacity() {
    let mut supplied = state();
    supplied.continuity.witnesses.push(WitnessRecord {
        id: RecordId::from_bytes(&[20; 16]).unwrap(),
        observer: entity(4),
        fact: FactId::from_bytes(&[99; 16]).unwrap(),
        perceived_at: supplied.logical_time,
        source: content(),
    });
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidReference));
    let mut supplied = state();
    supplied.continuity.journal.push(JournalEntry {
        id: RecordId::from_bytes(&[20; 16]).unwrap(),
        source_facts: vec![],
        attributed_claims: vec![],
        audience: AudienceScope::Shared,
        text: String::with_capacity(2 * 1024 * 1024),
    });
    assert_eq!(checkpoint(supplied), Err(CheckpointError::Capacity));
}
#[test]
fn forbids_private_payload_routing_to_shared_room_and_locked_listener() {
    use df_types::ClientBindingId;
    let binding = ClientBindingId::from_bytes(&[30; 16]).unwrap();
    for changed in 0..2 {
        let mut supplied = state();
        supplied.continuity.presence.push(ParticipantPresence {
            member: member(3),
            bindings: vec![binding],
            state: PresenceKind::Connected,
            policy: content(),
        });
        supplied.continuity.audio = Some(AudioTopology {
            policy: content(),
            outputs: vec![AudioOutputLease {
                id: RecordId::from_bytes(&[31; 16]).unwrap(),
                destination: if changed == 0 {
                    AudioDestination::PublicRoom(binding)
                } else {
                    AudioDestination::PrivateListener {
                        member: member(3),
                        binding,
                    }
                },
                generation: 1,
                audience: AudienceScope::Members(vec![member(3)]),
                device_unlocked: false,
            }],
            captures: vec![],
        });
        assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidReference));
    }
}
#[test]
fn keeps_presence_separate_from_resources_logical_time_and_gameplay_membership() {
    use df_types::ClientBindingId;
    let mut supplied = state();
    let before_time = supplied.logical_time;
    let before_resource = supplied.resources.clone();
    let before_members = supplied.members.clone();
    supplied.continuity.presence.push(ParticipantPresence {
        member: member(3),
        bindings: vec![ClientBindingId::from_bytes(&[30; 16]).unwrap()],
        state: PresenceKind::VoluntaryAfk,
        policy: content(),
    });
    let checkpoint = checkpoint(supplied).unwrap();
    assert_eq!(checkpoint.state().logical_time, before_time);
    assert_eq!(checkpoint.state().resources, before_resource);
    assert_eq!(checkpoint.state().members, before_members);
}
#[test]
fn derived_summary_cannot_replace_missing_episode_or_canonical_fact() {
    let mut supplied = state();
    supplied.continuity.summaries.push(MemorySummary {
        id: RecordId::from_bytes(&[31; 16]).unwrap(),
        episodes: vec![RecordId::from_bytes(&[99; 16]).unwrap()],
        derived_claims: vec![],
        source_digest: pins().content.content_digest,
        source_revision: basis().revision,
        summarizer: label("fixture-summarizer"),
        model: label("fixture-model"),
        policy: content(),
        audience: AudienceScope::Shared,
        text: "synthetic summary".into(),
        incomplete: false,
    });
    assert_eq!(checkpoint(supplied), Err(CheckpointError::InvalidReference));
}
#[test]
fn redacted_or_unavailable_checkpoint_never_claims_intact_resume() {
    let mut supplied = state();
    supplied
        .continuity
        .recovery
        .redacted_records
        .push(RecordId::from_bytes(&[31; 16]).unwrap());
    let retained = checkpoint(supplied).unwrap();
    assert_eq!(
        retained.validate_resume(basis(), &pins()),
        Err(CheckpointError::RedactedCheckpoint)
    );
    let mut supplied = state();
    supplied
        .continuity
        .recovery
        .unavailable_sources
        .push(label("fixture-source-1"));
    let checkpoint = checkpoint(supplied).unwrap();
    assert_eq!(
        checkpoint.validate_resume(basis(), &pins()),
        Err(CheckpointError::UnavailableCheckpoint)
    );
}
#[test]
fn debug_fork_and_disaster_restore_require_distinct_supplied_origin_bindings() {
    for kind in [RestoreKind::DebugFork, RestoreKind::Disaster] {
        let mut supplied = state();
        supplied.continuity.recovery.origin = Some(RestoreOrigin {
            kind,
            checkpoint_digest: ContentDigest([31; 32]),
            origin: basis(),
            process_generation: 1,
        });
        assert_eq!(
            checkpoint(supplied),
            Err(match kind {
                RestoreKind::DebugFork => CheckpointError::WrongRun,
                RestoreKind::Disaster => CheckpointError::StaleBasis,
            })
        );
    }
    let mut supplied = state();
    let mut origin = basis();
    origin.revision = revision(1, 99);
    supplied.continuity.recovery.origin = Some(RestoreOrigin {
        kind: RestoreKind::Disaster,
        checkpoint_digest: ContentDigest([31; 32]),
        origin,
        process_generation: 2,
    });
    supplied
        .continuity
        .recovery
        .retired_epochs
        .push(RecoveryEpoch::new(1).unwrap());
    supplied
        .continuity
        .recovery
        .lost_ranges
        .push(LostGameRange {
            from: revision(1, 90),
            through: revision(1, 99),
        });
    assert!(checkpoint(supplied).is_ok());
}
#[test]
fn unverified_asset_or_asset_dependency_cycle_rejects() {
    let a = AssetReference {
        key: label("fixture-asset-a"),
        digest: ContentDigest([31; 32]),
        byte_length: 100,
        kind: AssetKind::Image,
    };
    let b = AssetReference {
        key: label("fixture-asset-b"),
        digest: ContentDigest([32; 32]),
        byte_length: 100,
        kind: AssetKind::Image,
    };
    let mut supplied = state();
    supplied.continuity.asset_dependencies = vec![
        AssetDependency {
            asset: a.clone(),
            prerequisites: vec![b.clone()],
        },
        AssetDependency {
            asset: b.clone(),
            prerequisites: vec![a.clone()],
        },
    ];
    assert_eq!(
        checkpoint(supplied.clone()),
        Err(CheckpointError::InvalidReference)
    );
    let rules = vec![rule()];
    let content_entries = vec![content()];
    let constraints = resource_constraints();
    assert_eq!(
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis(),
            pins(),
            supplied,
            ReferenceInventory {
                rules: &rules,
                content: &content_entries,
                resources: &constraints,
                assets: &[a, b]
            },
            limits()
        ),
        Err(CheckpointError::InvalidReference)
    );
}

#[test]
fn native_input_retained_bytes_include_string_and_vector_capacities() {
    let input = GameInput::Game(CommandInput {
        basis: basis(),
        observed_revision: basis().revision,
        operation: OperationId::from_bytes(&[40; 16]).unwrap(),
        member: member(3),
        command: GameCommand::Speak {
            speaker: entity(4),
            text: String::with_capacity(4096),
            conversation: None,
        },
    });
    assert_eq!(input.retained_heap_bytes(), Some(4096));
    assert_eq!(
        input.retained_bytes(),
        Some(std::mem::size_of::<GameInput>() + 4096)
    );
    let input = GameInput::Game(CommandInput {
        basis: basis(),
        observed_revision: basis().revision,
        operation: OperationId::from_bytes(&[40; 16]).unwrap(),
        member: member(3),
        command: GameCommand::SubmitRoll {
            resolution: ResolutionId::from_bytes(&[41; 16]).unwrap(),
            window: WindowId::from_bytes(&[42; 16]).unwrap(),
            values: Vec::with_capacity(16),
        },
    });
    assert_eq!(
        input.retained_heap_bytes(),
        Some(16 * std::mem::size_of::<u32>())
    );
}
#[test]
fn checkpoint_debug_output_omits_private_payload_and_choices() {
    let mut supplied = state();
    supplied.continuity.journal.push(JournalEntry {
        id: RecordId::from_bytes(&[43; 16]).unwrap(),
        source_facts: vec![],
        attributed_claims: vec![],
        audience: AudienceScope::Members(vec![member(3)]),
        text: "synthetic private witness report".into(),
    });
    let checkpoint = checkpoint(supplied).unwrap();
    let diagnostic = format!("{checkpoint:?}");
    assert!(diagnostic.contains("Checkpoint"));
    assert!(!diagnostic.contains("synthetic private witness report"));
    assert!(!diagnostic.contains("journal"));
}
