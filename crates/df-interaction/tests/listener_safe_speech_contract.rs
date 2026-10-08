use df_interaction::speech::*;
use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, LocaleTag, MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId,
    SessionId, SessionRevision,
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
            mode: RulesMode::DisclosedCustom,
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
        members: vec![
            MembershipLink {
                member: member(3),
                character: None,
            },
            MembershipLink {
                member: member(5),
                character: None,
            },
        ],
        entities: vec![WorldEntity {
            id: entity(4),
            definition: content(),
            location: None,
            position: Some(Position { x: 0, y: 0, z: 0 }),
            identity_revision: label("fixture-entity-1"),
        }],
        characters: vec![],
        resources: vec![],
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
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis(),
        pins(),
        state,
        ReferenceInventory {
            rules: &rules,
            content: &content_entries,
            resources: &[],
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
            subjects: vec![],
        },
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

fn claim_id(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}
fn versions() -> SpeechVersions {
    SpeechVersions {
        access: label("private-access-authority"),
        contract: label("expression-contract-1"),
        profile: label("grounded-profile-1"),
        locale: LocaleTag::parse("en-US").unwrap(),
        provider: label("fixture-provider"),
        model: label("fixture-model"),
        format: label("expression-input-1"),
        quote: label("SECRET-QUOTE"),
    }
}
fn speech_limits() -> SpeechLimits {
    SpeechLimits {
        maximum_plan_claims: 8,
        maximum_slots: 16,
        maximum_claim_bytes: 256,
        maximum_selected_text_and_evidence_bytes: 4096,
        maximum_comparisons: 4096,
        facts: PerceptionLimits {
            maximum_scan_records: 5000,
            maximum_member_comparisons: 5000,
            maximum_selected_facts: 128,
        },
        claims: ClaimPerceptionLimits {
            maximum_scan_records: 5000,
            maximum_record_comparisons: 5000,
            maximum_selected_claims: 128,
        },
    }
}
fn slots() -> Vec<GroundedSlot> {
    vec![
        GroundedSlot {
            kind: SlotKind::Name,
            start: 0,
            end: 3,
        },
        GroundedSlot {
            kind: SlotKind::Outcome,
            start: 4,
            end: 7,
        },
        GroundedSlot {
            kind: SlotKind::Number,
            start: 8,
            end: 10,
        },
    ]
}
fn proposal() -> SpeechProposal {
    SpeechProposal {
        source: content(),
        claims: (30..34)
            .map(|id| PlannedClaim {
                claim: claim_id(id),
                intent: SpeechIntent::Claim,
                uncertainty: DeclaredUncertainty::NoneDeclared,
                slots: if id == 30 { slots() } else { vec![] },
            })
            .collect(),
    }
}
struct RegisteredSource {
    access: bool,
    deceit: bool,
}
impl SpeechSourceOwner for RegisteredSource {
    fn validate(
        &self,
        current: Basis,
        admitted: &CheckpointPins,
        version: &SpeechVersions,
        request: &SpeechProposal,
    ) -> Result<(), SpeechSourceError> {
        if !self.access {
            return Err(SpeechSourceError::AccessDenied);
        }
        if current != basis() || admitted != &pins() || version != &versions() {
            return Err(SpeechSourceError::Stale);
        }
        if request.source != content() {
            return Err(SpeechSourceError::Unsupported);
        }
        for item in &request.claims {
            if !(30..34).any(|id| claim_id(id) == item.claim) {
                return Err(SpeechSourceError::Unsupported);
            }
            if item.intent == SpeechIntent::ApprovedDeceit && !self.deceit {
                return Err(SpeechSourceError::Unsupported);
            }
            let expected = if item.claim == claim_id(30) {
                slots()
            } else {
                vec![]
            };
            if item.slots != expected {
                return Err(SpeechSourceError::Unsupported);
            }
        }
        Ok(())
    }
}
fn observation<'a>(
    current: &'a Checkpoint,
    version: &'a SpeechVersions,
    source: Option<&'a dyn SpeechSourceOwner>,
    observer: ObserverScope,
) -> SpeechObservation<'a> {
    SpeechObservation {
        checkpoint: current,
        basis: basis(),
        pins: current.pins(),
        observer,
        versions: version,
        source_owner: source,
        limits: speech_limits(),
    }
}
fn speech_state(hidden: &str) -> GameState {
    let mut s = state();
    for (index, audience) in [
        AudienceScope::Shared,
        AudienceScope::Members(vec![member(3)]),
        AudienceScope::Members(vec![member(5)]),
        AudienceScope::Host,
    ]
    .into_iter()
    .enumerate()
    {
        let mut f = fact(10 + index as u8, index as u32);
        f.audience = audience;
        s.facts.push(f);
    }
    s.decisions.push(AcceptedDecision {
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        revision: basis().revision,
        facts: s.facts.iter().map(|f| f.id).collect(),
        draws: vec![],
        effects: vec![],
        source_policy: label("fixture-source-policy"),
        semantic_output: Some(hidden.to_owned()),
    });
    for (id, text, evidence, audience) in [
        (30, "Ada has 13 coins.", 0, AudienceScope::Shared),
        (
            31,
            "PRIVATE-MEMBER-3",
            1,
            AudienceScope::Members(vec![member(3)]),
        ),
        (32, hidden, 3, AudienceScope::Host),
        (
            33,
            "A rumor backed by member 5 evidence",
            2,
            AudienceScope::Shared,
        ),
    ] {
        s.beliefs.push(AttributedClaim {
            id: claim_id(id),
            holder: entity(4),
            subject: entity(4),
            claim: text.to_owned(),
            evidence: vec![s.facts[evidence].id],
            audience,
            source: content(),
        });
    }
    s.continuity.npcs.push(NpcState {
        entity: entity(4),
        role: content(),
        personality: content(),
        motivations: vec![],
        goals: vec![],
        needs: vec![],
        fears: vec![],
        known_facts: vec![s.facts[3].id],
        beliefs: vec![claim_id(32)],
        secrets: vec![SecretPolicy {
            holder: entity(4),
            claims: vec![claim_id(32)],
            policy: content(),
            permitted_audience: AudienceScope::Members(vec![member(3)]),
        }],
    });
    s
}
fn hidden_variant() -> GameState {
    let mut s = speech_state("SECRET-B-HOST-REASONING");
    let changed = FactId::from_bytes(&[19; 16]).unwrap();
    s.facts[3].id = changed;
    s.facts[3].value = FactValue::ContentEvent {
        definition: content(),
        subjects: vec![entity(4)],
    };
    s.decisions[0].facts[3] = changed;
    s.beliefs[2].evidence[0] = changed;
    s.continuity.npcs[0].known_facts[0] = changed;
    s
}
fn narrow_private_claim(mut s: GameState) -> GameState {
    s.continuity.npcs[0].secrets.push(SecretPolicy {
        holder: entity(4),
        claims: vec![claim_id(31)],
        policy: content(),
        permitted_audience: AudienceScope::Members(vec![member(5)]),
    });
    s
}

#[test]
fn listener_projection_uses_canonical_knowledge_and_all_scope_channels() {
    let c = checkpoint(speech_state("SECRET-A-HOST-REASONING")).unwrap();
    let original = c.clone();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let admitted = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let plan = admit_speech(proposal(), &admitted).unwrap();
    for (scope, expected) in [
        (ObserverScope::Shared, vec![30]),
        (ObserverScope::Member(member(3)), vec![30, 31]),
        (ObserverScope::Member(member(5)), vec![30, 33]),
    ] {
        let observed = observation(&c, &v, Some(&source), scope);
        let public = project_expression_context(&plan, &observed).unwrap();
        assert_eq!(
            public.claims().iter().map(|c| c.id).collect::<Vec<_>>(),
            expected.into_iter().map(claim_id).collect::<Vec<_>>()
        );
        assert_eq!(
            public.claims()[0].text.as_ptr(),
            c.state().beliefs[0].claim.as_ptr()
        );
        assert_eq!(public.claims()[0].slots[2].value, "13");
    }
    assert_eq!(c, original);
}

#[test]
fn paired_hidden_reasoning_states_have_identical_owned_public_projection() {
    let a = checkpoint(speech_state("SECRET-A-HOST-REASONING")).unwrap();
    let b = checkpoint(hidden_variant()).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let oa = observation(&a, &v, Some(&source), ObserverScope::Shared);
    let ob = observation(&b, &v, Some(&source), ObserverScope::Shared);
    let pa = admit_speech(proposal(), &oa).unwrap();
    let pb = admit_speech(proposal(), &ob).unwrap();
    let ca = project_expression_context(&pa, &oa).unwrap();
    let cb = project_expression_context(&pb, &ob).unwrap();
    assert!(ca.claims() == cb.claims());
    assert_eq!(ca.metadata(), cb.metadata());
    assert!(ca.semantic_basis() == cb.semantic_basis());
    assert!(ca.validate_current(&ob).is_ok());
    assert!(!format!("{ca:?}").contains("SECRET"));
}

#[test]
fn secret_policy_narrows_and_private_evidence_never_widens_public_context() {
    let mut s = narrow_private_claim(speech_state("SECRET-A-HOST-REASONING"));
    s.knowledge.push(KnowledgeGrant {
        observer: member(3),
        fact: s.facts[3].id,
        source: s.facts[0].id,
    });
    let c = checkpoint(s).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let p = admit_speech(proposal(), &o).unwrap();
    for (scope, expected) in [
        (ObserverScope::Shared, vec![30]),
        (ObserverScope::Member(member(3)), vec![30]),
        (ObserverScope::Member(member(5)), vec![30, 33]),
    ] {
        let o = observation(&c, &v, Some(&source), scope);
        let out = project_expression_context(&p, &o).unwrap();
        assert_eq!(
            out.claims().iter().map(|x| x.id).collect::<Vec<_>>(),
            expected.into_iter().map(claim_id).collect::<Vec<_>>()
        );
        assert!(
            !out.claims()
                .iter()
                .flat_map(|x| x.evidence)
                .any(|id| *id == c.state().facts[3].id)
        );
    }
}

#[test]
fn source_basis_access_and_missing_owner_refuse_before_public_context() {
    let c = checkpoint(speech_state("SECRET")).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let missing = observation(&c, &v, None, ObserverScope::Shared);
    assert!(matches!(
        admit_speech(proposal(), &missing),
        Err(SpeechError::SourceUnavailable)
    ));
    let denied = RegisteredSource {
        access: false,
        deceit: false,
    };
    let denied_o = observation(&c, &v, Some(&denied), ObserverScope::Shared);
    assert!(matches!(
        admit_speech(proposal(), &denied_o),
        Err(SpeechError::Source(SpeechSourceError::AccessDenied))
    ));
    let o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let p = admit_speech(proposal(), &o).unwrap();
    let mut stale = observation(&c, &v, Some(&source), ObserverScope::Shared);
    stale.basis.revision = revision(2, 9);
    assert!(matches!(
        project_expression_context(&p, &stale),
        Err(SpeechError::Checkpoint(_))
    ));
    let mut changed = v.clone();
    changed.access = label("access-2");
    let changed_o = observation(&c, &changed, Some(&source), ObserverScope::Shared);
    assert!(matches!(
        project_expression_context(&p, &changed_o),
        Err(SpeechError::StalePlan)
    ));
    let mut pins_changed = c.pins().clone();
    pins_changed.rules.handler = label("handler-2");
    let mut pin_o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    pin_o.pins = &pins_changed;
    assert!(matches!(
        project_expression_context(&p, &pin_o),
        Err(SpeechError::Checkpoint(_))
    ));
}

#[test]
fn slot_shape_duplicate_and_work_limits_return_no_partial_projection() {
    let c = checkpoint(speech_state("SECRET")).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let mut invalid = proposal();
    invalid.claims[0].slots[2].end = 999;
    assert!(matches!(
        admit_speech(invalid, &o),
        Err(SpeechError::InvalidSlot)
    ));
    let mut duplicate = proposal();
    duplicate.claims[1].claim = duplicate.claims[0].claim;
    assert!(matches!(
        admit_speech(duplicate, &o),
        Err(SpeechError::DuplicateClaim)
    ));
    let mut tiny = observation(&c, &v, Some(&source), ObserverScope::Shared);
    tiny.limits.maximum_plan_claims = 0;
    assert!(matches!(
        admit_speech(proposal(), &tiny),
        Err(SpeechError::Capacity)
    ));
    let plan = admit_speech(proposal(), &o).unwrap();
    tiny.limits = speech_limits();
    tiny.limits.maximum_plan_claims = 0;
    assert!(matches!(
        project_expression_context(&plan, &tiny),
        Err(SpeechError::Capacity)
    ));
    tiny.limits = speech_limits();
    tiny.limits.maximum_slots = 0;
    assert!(matches!(
        project_expression_context(&plan, &tiny),
        Err(SpeechError::Capacity)
    ));
    tiny.limits = speech_limits();
    tiny.limits.maximum_claim_bytes = 1;
    assert!(matches!(
        project_expression_context(&plan, &tiny),
        Err(SpeechError::Capacity)
    ));
    tiny.limits = speech_limits();
    tiny.limits.maximum_selected_text_and_evidence_bytes = 0;
    assert!(matches!(
        project_expression_context(&plan, &tiny),
        Err(SpeechError::Capacity)
    ));
    tiny.limits = speech_limits();
    tiny.limits.claims.maximum_scan_records = 0;
    assert!(matches!(
        project_expression_context(&plan, &tiny),
        Err(SpeechError::Perception(_))
    ));
}

#[test]
fn current_listener_permission_is_rechecked_and_hidden_only_changes_are_allowed() {
    let a = checkpoint(speech_state("SECRET-A-HOST-REASONING")).unwrap();
    let revoked = checkpoint(narrow_private_claim(speech_state(
        "SECRET-A-HOST-REASONING",
    )))
    .unwrap();
    let hidden = checkpoint(hidden_variant()).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let o = observation(&a, &v, Some(&source), ObserverScope::Member(member(3)));
    let p = admit_speech(proposal(), &o).unwrap();
    let ctx = project_expression_context(&p, &o).unwrap();
    let revoked_o = observation(
        &revoked,
        &v,
        Some(&source),
        ObserverScope::Member(member(3)),
    );
    assert_eq!(
        ctx.validate_current(&revoked_o),
        Err(SpeechError::ContextChanged)
    );
    let hidden_o = observation(&hidden, &v, Some(&source), ObserverScope::Member(member(3)));
    assert!(ctx.validate_current(&hidden_o).is_ok());
    let other = observation(&a, &v, Some(&source), ObserverScope::Member(member(5)));
    assert_eq!(
        ctx.validate_current(&other),
        Err(SpeechError::ContextChanged)
    );
}

#[test]
fn approved_deceit_stays_attributed_and_does_not_change_world_truth() {
    let c = checkpoint(speech_state("SECRET")).unwrap();
    let before = c.clone();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: true,
    };
    let o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let mut request = proposal();
    request.claims[0].intent = SpeechIntent::ApprovedDeceit;
    request.claims[0].uncertainty = DeclaredUncertainty::Explicit;
    let p = admit_speech(request, &o).unwrap();
    let ctx = project_expression_context(&p, &o).unwrap();
    assert_eq!(ctx.claims()[0].intent, SpeechIntent::ApprovedDeceit);
    assert_eq!(ctx.claims()[0].holder, entity(4));
    assert_eq!(ctx.claims()[0].text, "Ada has 13 coins.");
    let denied = RegisteredSource {
        access: true,
        deceit: false,
    };
    let denied_o = observation(&c, &v, Some(&denied), ObserverScope::Shared);
    assert!(matches!(
        project_expression_context(&p, &denied_o),
        Err(SpeechError::Source(SpeechSourceError::Unsupported))
    ));
    assert_eq!(c, before);
}

#[test]
fn frozen_listener_contract_retains_original_privacy_outcome_and_explicit_gaps() {
    let frozen = include_str!("fixtures/listener_safe_speech_contract.json");
    assert!(frozen.contains("private reasoning facts never enter public provider input"));
    assert!(frozen.contains("native source owner and egress remain unperformed"));
}
