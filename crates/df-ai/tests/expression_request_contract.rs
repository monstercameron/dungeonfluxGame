use df_ai::expression::*;
use df_interaction::conversation::*;
use df_interaction::speech::*;
use df_model::checkpoint::*;
use df_provider_api::{
    RequestBinding, RequestError, RequestIdentity, RequestIdentityField, RequestLimits,
    RequestOwnerState, RequestUsage,
};
use df_types::{
    BuildIdentity, LocaleTag, MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId,
    SessionId, SessionRevision, Usage, UsageUnit,
};
use std::cell::Cell;
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

fn request_binding(ctx: &ExpressionContext<'_>) -> RequestBinding<ExpressionBasis> {
    RequestBinding {
        identity: RequestIdentity {
            basis: ctx.basis(),
            job: JobId::from_bytes(&[5; 16]).unwrap(),
            operation: OperationId::from_bytes(&[6; 16]).unwrap(),
            generation: 7,
        },
        semantic_basis: ctx.semantic_basis().clone(),
        mode: ExecutionMode::Replay,
        deadline: Duration::from_secs(2),
    }
}
fn request_owner(
    binding: &RequestBinding<ExpressionBasis>,
) -> RequestOwnerState<'_, ExpressionBasis> {
    RequestOwnerState {
        current: Some(binding),
        elapsed: Duration::ZERO,
        cancelled: false,
    }
}
fn request_limits() -> RequestLimits {
    RequestLimits::new(
        16384,
        100,
        Duration::from_secs(2),
        Usage::new(100, UsageUnit::Token),
    )
    .unwrap()
}
fn request_work() -> RequestUsage {
    RequestUsage::new(
        10,
        Duration::from_millis(20),
        Usage::new(10, UsageUnit::Token),
    )
}

struct ConversationOwner {
    speech_allowed: Cell<bool>,
    conversation_allowed: Cell<bool>,
}

impl SpeechSourceOwner for ConversationOwner {
    fn validate(
        &self,
        current: Basis,
        admitted: &CheckpointPins,
        version: &SpeechVersions,
        proposal: &SpeechProposal,
    ) -> Result<(), SpeechSourceError> {
        SpeechSourceOwner::validate(
            &RegisteredSource {
                access: self.speech_allowed.get(),
                deceit: false,
            },
            current,
            admitted,
            version,
            proposal,
        )
    }
}

impl ConversationSourceOwner for ConversationOwner {
    fn validate(
        &self,
        current: Basis,
        admitted: &CheckpointPins,
        conversation: &ConversationState,
        permission: &ConversationPermission,
        claims: &[RecordId],
    ) -> Result<(), ConversationSourceError> {
        if !self.conversation_allowed.get() {
            return Err(ConversationSourceError::AccessDenied);
        }
        if current != basis()
            || admitted != &pins()
            || conversation.id != claim_id(50)
            || permission.policy != content()
            || permission.topic != conversation.topic
            || permission.speaker != entity(4)
            || permission.recipient != ObserverScope::Shared
            || claims != [claim_id(30)]
        {
            return Err(ConversationSourceError::Unsupported);
        }
        Ok(())
    }
}

fn conversation_state(hidden: &str) -> GameState {
    let mut current = speech_state(hidden);
    current.conversations.push(ConversationState {
        id: claim_id(50),
        participants: vec![entity(4)],
        topic: content(),
        accepted_facts: vec![],
    });
    current
}

fn conversation_observation<'a>(
    current: &'a Checkpoint,
    version: &'a SpeechVersions,
    source: &'a ConversationOwner,
) -> ConversationObservation<'a> {
    ConversationObservation {
        speech: observation(current, version, Some(source), ObserverScope::Shared),
        source_owner: Some(source),
        limits: ConversationLimits {
            maximum_participants: 2,
            maximum_accepted_facts: 2,
            maximum_disclosures: 0,
            maximum_comparisons: 100,
            maximum_window_ticks: 30,
        },
    }
}

fn conversation_permission() -> ConversationPermission {
    ConversationPermission {
        policy: content(),
        conversation: claim_id(50),
        topic: content(),
        speaker: entity(4),
        recipient: ObserverScope::Shared,
        window: PermissionWindow {
            opens_at: LogicalTime {
                ticks: 110,
                ticks_per_second: 10,
            },
            expires_at: LogicalTime {
                ticks: 130,
                ticks_per_second: 10,
            },
        },
        disclosures: vec![],
    }
}

fn conversation_proposal() -> SpeechProposal {
    SpeechProposal {
        source: content(),
        claims: vec![PlannedClaim {
            claim: claim_id(30),
            intent: SpeechIntent::Claim,
            uncertainty: DeclaredUncertainty::NoneDeclared,
            slots: slots(),
        }],
    }
}
fn prepared<'a>(
    c: &'a Checkpoint,
    plan: &'a SpeechActPlan,
    v: &'a SpeechVersions,
    source: &'a RegisteredSource,
    scope: ObserverScope,
) -> PreparedExpression<'a> {
    let o = observation(c, v, Some(source), scope);
    let ctx = project_expression_context(plan, &o).unwrap();
    let current = request_binding(&ctx);
    prepare_expression_request(
        ctx,
        &o,
        request_binding_from(&current),
        request_work(),
        request_limits(),
        request_owner(&current),
    )
    .unwrap()
}
fn request_binding_from(b: &RequestBinding<ExpressionBasis>) -> RequestBinding<ExpressionBasis> {
    RequestBinding {
        identity: b.identity,
        semantic_basis: b.semantic_basis.clone(),
        mode: b.mode,
        deadline: b.deadline,
    }
}
fn has(bytes: &[u8], value: &[u8]) -> bool {
    bytes.windows(value.len()).any(|window| window == value)
}
fn response(p: &PreparedExpression<'_>) -> SpeechEnvelope {
    SpeechEnvelope {
        clauses: p
            .context()
            .claims()
            .iter()
            .map(|c| {
                ExpressionClause::Grounded(GroundedClause {
                    claim_id: c.id,
                    holder: c.holder,
                    subject: c.subject,
                    intent: c.intent,
                    uncertainty: c.uncertainty,
                    text: c.text.to_owned(),
                    slots: c
                        .slots
                        .iter()
                        .map(|s| GroundedValue {
                            kind: s.kind,
                            value: s.value.to_owned(),
                        })
                        .collect(),
                })
            })
            .collect(),
    }
}
fn qualification_limits() -> QualificationLimits {
    QualificationLimits {
        maximum_clauses: 8,
        maximum_output_bytes: 4096,
        maximum_slots: 16,
    }
}

#[test]
fn adapter_created_checked_request_and_public_metadata_are_equal_for_hidden_secret_pairs() {
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
    let ra = prepared(&a, &pa, &v, &source, ObserverScope::Shared);
    let rb = prepared(&b, &pb, &v, &source, ObserverScope::Shared);
    assert_eq!(ra.request().payload(), rb.request().payload());
    assert_eq!(ra.metadata(), rb.metadata());
    assert!(has(ra.request().payload(), b"Ada has 13 coins."));
    for secret in [
        b"SECRET-A-HOST-REASONING".as_slice(),
        b"SECRET-B-HOST-REASONING",
        b"PRIVATE-MEMBER-3",
        b"private-access-authority",
        b"SECRET-QUOTE",
    ] {
        assert!(!has(ra.request().payload(), secret));
        assert!(!format!("{:?}", ra.request()).contains(std::str::from_utf8(secret).unwrap()));
    }
    for raw_secret_id in [11_u8, 12, 13, 19, 31, 32, 33] {
        assert!(!has(ra.request().payload(), &[raw_secret_id; 16]));
    }
    assert!(ra.request().binding().semantic_basis == rb.request().binding().semantic_basis);
}

#[test]
fn actual_request_prompt_retrieval_style_and_asset_fields_follow_each_listener_scope() {
    let c = checkpoint(speech_state("SECRET-A-HOST-REASONING")).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let p = admit_speech(proposal(), &o).unwrap();
    for (scope, private, rumor) in [
        (ObserverScope::Shared, false, false),
        (ObserverScope::Member(member(3)), true, false),
        (ObserverScope::Member(member(5)), false, true),
    ] {
        let r = prepared(&c, &p, &v, &source, scope);
        let bytes = r.request().payload();
        assert_eq!(has(bytes, b"PRIVATE-MEMBER-3"), private);
        assert_eq!(has(bytes, &[31; 16]), private);
        assert_eq!(has(bytes, &[11; 16]), private);
        assert_eq!(has(bytes, b"A rumor backed by member 5 evidence"), rumor);
        assert_eq!(has(bytes, &[33; 16]), rumor);
        assert_eq!(has(bytes, &[12; 16]), rumor);
        assert!(!has(bytes, b"SECRET-A-HOST-REASONING"));
        assert!(!has(bytes, &[32; 16]));
    }
}

#[test]
fn current_source_listener_and_evidence_revocation_refuse_before_qualified_release() {
    let c = checkpoint(speech_state("SECRET-A-HOST-REASONING")).unwrap();
    let original = c.clone();
    let narrowed = checkpoint(narrow_private_claim(speech_state(
        "SECRET-A-HOST-REASONING",
    )))
    .unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let o = observation(&c, &v, Some(&source), ObserverScope::Member(member(3)));
    let p = admit_speech(proposal(), &o).unwrap();
    let r = prepared(&c, &p, &v, &source, ObserverScope::Member(member(3)));
    let revoked = observation(
        &narrowed,
        &v,
        Some(&source),
        ObserverScope::Member(member(3)),
    );
    assert!(matches!(
        qualify_expression(
            &r,
            &revoked,
            request_owner(r.request().binding()),
            response(&r),
            qualification_limits()
        ),
        Err(QualificationError::Current(ExpressionRequestError::Speech(
            SpeechError::ContextChanged
        )))
    ));
    let denied = RegisteredSource {
        access: false,
        deceit: false,
    };
    let denied_o = observation(&c, &v, Some(&denied), ObserverScope::Member(member(3)));
    assert!(matches!(
        r.validate_current(&denied_o, request_owner(r.request().binding())),
        Err(ExpressionRequestError::Speech(SpeechError::Source(
            SpeechSourceError::AccessDenied
        )))
    ));
    let mut changed = v.clone();
    changed.contract = label("contract-2");
    let changed_o = observation(
        &c,
        &changed,
        Some(&source),
        ObserverScope::Member(member(3)),
    );
    assert!(matches!(
        r.validate_current(&changed_o, request_owner(r.request().binding())),
        Err(ExpressionRequestError::Speech(SpeechError::StalePlan))
    ));
    assert_eq!(c, original);
}

#[test]
fn actual_request_owner_generation_deadline_cancel_and_bytes_fail_closed() {
    let c = checkpoint(speech_state("SECRET")).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let p = admit_speech(proposal(), &o).unwrap();
    for case in 0..5 {
        let ctx = project_expression_context(&p, &o).unwrap();
        let mut current = request_binding(&ctx);
        if case == 0 {
            current.identity.generation = 0;
        }
        let mut owner = request_owner(&current);
        if case == 1 {
            owner.cancelled = true;
        }
        if case == 2 {
            owner.elapsed = Duration::from_secs(3);
        }
        if case == 3 {
            owner.current = None;
        }
        let limits = if case == 4 {
            RequestLimits::new(
                1,
                100,
                Duration::from_secs(2),
                Usage::new(100, UsageUnit::Token),
            )
            .unwrap()
        } else {
            request_limits()
        };
        let result = prepare_expression_request(
            ctx,
            &o,
            request_binding_from(&current),
            request_work(),
            limits,
            owner,
        );
        let expected = match case {
            0 => ExpressionRequestError::Request(RequestError::InvalidGeneration),
            1 => ExpressionRequestError::Request(RequestError::Cancelled),
            2 => ExpressionRequestError::Request(RequestError::DeadlineExceeded),
            3 => ExpressionRequestError::Request(RequestError::CurrentBasisUnavailable),
            _ => ExpressionRequestError::Capacity,
        };
        assert_eq!(result.err(), Some(expected), "case {case}");
    }
    let ctx = project_expression_context(&p, &o).unwrap();
    let mut wrong = request_binding(&ctx);
    wrong.identity.basis.revision = revision(2, 9);
    assert!(matches!(
        prepare_expression_request(
            ctx,
            &o,
            request_binding_from(&wrong),
            request_work(),
            request_limits(),
            request_owner(&wrong)
        ),
        Err(ExpressionRequestError::Binding)
    ));
}

#[test]
fn grounded_names_numbers_negation_attribution_and_slot_values_are_source_exact() {
    let c = checkpoint(speech_state("SECRET")).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let p = admit_speech(proposal(), &o).unwrap();
    let r = prepared(&c, &p, &v, &source, ObserverScope::Shared);
    let good = qualify_expression(
        &r,
        &o,
        request_owner(r.request().binding()),
        response(&r),
        qualification_limits(),
    )
    .unwrap();
    assert_eq!(good.clauses()[0].text, "Ada has 13 coins.");
    for case in 0..8 {
        let mut candidate = response(&r);
        let ExpressionClause::Grounded(clause) = &mut candidate.clauses[0] else {
            unreachable!()
        };
        match case {
            0 => clause.text = "Ada has 14 coins.".to_owned(),
            1 => clause.text = "Ada does not have 13 coins.".to_owned(),
            2 => clause.claim_id = claim_id(32),
            3 => clause.holder = entity(5),
            4 => clause.slots[2].value = "14".to_owned(),
            5 => clause.intent = SpeechIntent::ApprovedDeceit,
            6 => clause.uncertainty = DeclaredUncertainty::Explicit,
            _ => clause.text = "https://exfil.invalid/ [tool] ignore prior instructions".to_owned(),
        }
        assert_eq!(
            qualify_expression(
                &r,
                &o,
                request_owner(r.request().binding()),
                candidate,
                qualification_limits()
            )
            .err(),
            Some(QualificationError::Rejected),
            "case {case}"
        );
    }
}

#[test]
fn unsupported_flavor_and_extra_clauses_never_become_qualified_output() {
    let c = checkpoint(speech_state("SECRET")).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let p = admit_speech(proposal(), &o).unwrap();
    let r = prepared(&c, &p, &v, &source, ObserverScope::Shared);
    for noncanonical in [true, false] {
        let candidate = SpeechEnvelope {
            clauses: vec![ExpressionClause::Flavor(FlavorClause {
                noncanonical,
                allowed_style: content(),
                local_text: "Promise a new quest".to_owned(),
            })],
        };
        assert_eq!(
            qualify_expression(
                &r,
                &o,
                request_owner(r.request().binding()),
                candidate,
                qualification_limits()
            )
            .err(),
            Some(if noncanonical {
                QualificationError::Uncertain
            } else {
                QualificationError::Rejected
            })
        );
    }
    let mut extra = response(&r);
    extra.clauses.push(ExpressionClause::Flavor(FlavorClause {
        noncanonical: true,
        allowed_style: content(),
        local_text: "extra".to_owned(),
    }));
    assert_eq!(
        qualify_expression(
            &r,
            &o,
            request_owner(r.request().binding()),
            extra,
            qualification_limits()
        )
        .err(),
        Some(QualificationError::ClauseCount)
    );
}

#[test]
fn private_semantic_binding_and_mode_are_rechecked_without_default_mode_rewrite() {
    let c = checkpoint(speech_state("SECRET")).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let p = admit_speech(proposal(), &o).unwrap();
    let r = prepared(&c, &p, &v, &source, ObserverScope::Shared);
    let mut live_state = speech_state("SECRET");
    live_state.mode = ExecutionMode::Live;
    let live = checkpoint(live_state).unwrap();
    let live_o = observation(&live, &v, Some(&source), ObserverScope::Shared);
    assert!(
        r.validate_current(&live_o, request_owner(r.request().binding()))
            .is_ok()
    );
    assert_eq!(r.request().binding().mode, ExecutionMode::Replay);
    let mut wrong = request_binding_from(r.request().binding());
    wrong.mode = ExecutionMode::Live;
    assert_eq!(
        r.validate_current(&o, request_owner(&wrong)),
        Err(ExpressionRequestError::Request(
            RequestError::IdentityMismatch(RequestIdentityField::Mode)
        ))
    );
    let member_o = observation(&c, &v, Some(&source), ObserverScope::Member(member(3)));
    let other = project_expression_context(&p, &member_o).unwrap();
    wrong = request_binding_from(r.request().binding());
    wrong.semantic_basis = other.semantic_basis().clone();
    assert_eq!(
        r.validate_current(&o, request_owner(&wrong)),
        Err(ExpressionRequestError::Request(
            RequestError::SemanticBasisMismatch
        ))
    );
}

#[test]
fn output_capacity_and_empty_listener_context_do_not_return_partial_requests_or_clauses() {
    let c = checkpoint(speech_state("SECRET")).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    let p = admit_speech(proposal(), &o).unwrap();
    let r = prepared(&c, &p, &v, &source, ObserverScope::Shared);
    let mut small = qualification_limits();
    small.maximum_output_bytes = 0;
    assert_eq!(
        qualify_expression(
            &r,
            &o,
            request_owner(r.request().binding()),
            response(&r),
            small
        )
        .err(),
        Some(QualificationError::Capacity)
    );
    let private = SpeechProposal {
        source: content(),
        claims: vec![PlannedClaim {
            claim: claim_id(32),
            intent: SpeechIntent::Claim,
            uncertainty: DeclaredUncertainty::NoneDeclared,
            slots: vec![],
        }],
    };
    let private_plan = admit_speech(private, &o).unwrap();
    let ctx = project_expression_context(&private_plan, &o).unwrap();
    let b = request_binding(&ctx);
    assert!(matches!(
        prepare_expression_request(
            ctx,
            &o,
            request_binding_from(&b),
            request_work(),
            request_limits(),
            request_owner(&b)
        ),
        Err(ExpressionRequestError::Empty)
    ));
}

#[test]
fn frozen_expression_contract_names_the_actual_checked_request_and_unperformed_egress() {
    let frozen = include_str!("fixtures/expression_request_contract.json");
    assert!(frozen.contains("df-provider-api::CheckedRequest::new"));
    assert!(frozen.contains("native source owner and egress remain unperformed"));
}

#[test]
fn projection_text_budget_is_distinct_from_complete_checked_request_byte_bound() {
    let c = checkpoint(speech_state("SECRET-A-HOST-REASONING")).unwrap();
    let v = versions();
    let source = RegisteredSource {
        access: true,
        deceit: false,
    };
    let mut o = observation(&c, &v, Some(&source), ObserverScope::Shared);
    // 17 claim bytes + 16 evidence-ID bytes + 8 grounded-slot bytes.
    o.limits.maximum_selected_text_and_evidence_bytes = 41;
    let plan = admit_speech(proposal(), &o).unwrap();
    let context = project_expression_context(&plan, &o).unwrap();
    let binding = request_binding(&context);
    let owner_binding = request_binding_from(&binding);
    let full = RequestLimits::new(
        512,
        100,
        Duration::from_secs(2),
        Usage::new(100, UsageUnit::Token),
    )
    .unwrap();
    let request = prepare_expression_request(
        context,
        &o,
        binding,
        request_work(),
        full,
        request_owner(&owner_binding),
    )
    .unwrap();
    assert!(request.request().payload().len() > 41);
    assert!(request.request().payload().len() <= 512);
    let context = project_expression_context(&plan, &o).unwrap();
    let binding = request_binding(&context);
    let owner_binding = request_binding_from(&binding);
    let tiny = RequestLimits::new(
        64,
        100,
        Duration::from_secs(2),
        Usage::new(100, UsageUnit::Token),
    )
    .unwrap();
    assert!(matches!(
        prepare_expression_request(
            context,
            &o,
            binding,
            request_work(),
            tiny,
            request_owner(&owner_binding)
        ),
        Err(ExpressionRequestError::Capacity)
    ));
}

#[test]
fn conversation_request_uses_only_current_listener_projection_in_hidden_secret_pair() {
    let first = checkpoint(conversation_state("SECRET-A-HOST-REASONING")).unwrap();
    let second = checkpoint(conversation_state("SECRET-B-HOST-REASONING")).unwrap();
    let version = versions();
    let source = ConversationOwner {
        speech_allowed: Cell::new(true),
        conversation_allowed: Cell::new(true),
    };
    let mut payloads = Vec::new();
    for current in [&first, &second] {
        let observed = conversation_observation(current, &version, &source);
        let plan = admit_conversation(
            conversation_permission(),
            conversation_proposal(),
            &observed,
        )
        .unwrap();
        let binding = project_conversation(&plan, &observed)
            .unwrap()
            .with_expression(request_binding);
        let prepared = prepare_conversation_expression_request(
            &plan,
            &observed,
            request_binding_from(&binding),
            request_work(),
            request_limits(),
            request_owner(&binding),
        )
        .unwrap();
        prepared
            .validate_current(&observed, request_owner(&binding))
            .unwrap();
        assert_eq!(prepared.metadata().permitted_claims, 1);
        assert!(!has(prepared.request().payload(), b"SECRET-"));
        let qualified = qualify_conversation_expression(
            &prepared,
            &observed,
            request_owner(&binding),
            SpeechEnvelope {
                clauses: vec![ExpressionClause::Grounded(GroundedClause {
                    claim_id: claim_id(30),
                    holder: entity(4),
                    subject: entity(4),
                    intent: SpeechIntent::Claim,
                    uncertainty: DeclaredUncertainty::NoneDeclared,
                    text: "Ada has 13 coins.".to_owned(),
                    slots: vec![
                        GroundedValue {
                            kind: SlotKind::Name,
                            value: "Ada".to_owned(),
                        },
                        GroundedValue {
                            kind: SlotKind::Outcome,
                            value: "has".to_owned(),
                        },
                        GroundedValue {
                            kind: SlotKind::Number,
                            value: "13".to_owned(),
                        },
                    ],
                })],
            },
            qualification_limits(),
        )
        .unwrap();
        assert_eq!(qualified.clauses().len(), 1);
        payloads.push(prepared.request().payload().to_vec());
    }
    assert_eq!(payloads[0], payloads[1]);
}

#[test]
fn conversation_permission_revocation_refuses_egress_and_qualified_output() {
    let current = checkpoint(conversation_state("SECRET-HOST-ONLY")).unwrap();
    let version = versions();
    let source = ConversationOwner {
        speech_allowed: Cell::new(true),
        conversation_allowed: Cell::new(true),
    };
    let observed = conversation_observation(&current, &version, &source);
    let plan = admit_conversation(
        conversation_permission(),
        conversation_proposal(),
        &observed,
    )
    .unwrap();
    let binding = project_conversation(&plan, &observed)
        .unwrap()
        .with_expression(request_binding);
    source.conversation_allowed.set(false);
    assert!(admit_speech(conversation_proposal(), &observed.speech).is_ok());
    assert!(matches!(
        prepare_conversation_expression_request(
            &plan,
            &observed,
            request_binding_from(&binding),
            request_work(),
            request_limits(),
            request_owner(&binding),
        ),
        Err(ConversationExpressionError::Conversation(
            ConversationError::Source(ConversationSourceError::AccessDenied)
        ))
    ));
    source.conversation_allowed.set(true);
    let prepared = prepare_conversation_expression_request(
        &plan,
        &observed,
        request_binding_from(&binding),
        request_work(),
        request_limits(),
        request_owner(&binding),
    )
    .unwrap();
    source.conversation_allowed.set(false);
    assert_eq!(
        prepared.validate_current(&observed, request_owner(&binding)),
        Err(ConversationExpressionError::Conversation(
            ConversationError::Source(ConversationSourceError::AccessDenied)
        ))
    );
    assert!(matches!(
        qualify_conversation_expression(
            &prepared,
            &observed,
            request_owner(&binding),
            SpeechEnvelope { clauses: vec![] },
            qualification_limits(),
        ),
        Err(ConversationQualificationError::Current(
            ConversationExpressionError::Conversation(ConversationError::Source(
                ConversationSourceError::AccessDenied
            ))
        ))
    ));

    source.conversation_allowed.set(true);
    let mut changed = conversation_state("SECRET-HOST-ONLY");
    changed.conversations[0].topic = ContentReference {
        package: label("fixture-package-1"),
        entry: label("changed-topic-1"),
    };
    let changed = checkpoint(changed).unwrap();
    let changed_observation = conversation_observation(&changed, &version, &source);
    assert_eq!(
        prepared.validate_current(&changed_observation, request_owner(&binding)),
        Err(ConversationExpressionError::Conversation(
            ConversationError::TopicChanged
        ))
    );
}
