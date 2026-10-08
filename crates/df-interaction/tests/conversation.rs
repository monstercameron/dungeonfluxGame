use std::cell::Cell;

use df_interaction::conversation::*;
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
    let content_entries = vec![content(), alternate_topic()];
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

fn alternate_topic() -> ContentReference {
    ContentReference {
        package: label("fixture-package-1"),
        entry: label("fixture-topic-2"),
    }
}

fn conversation_state() -> GameState {
    let mut current = state();
    for value in [5, 6, 7] {
        current.entities.push(WorldEntity {
            id: entity(value),
            definition: content(),
            location: None,
            position: None,
            identity_revision: label("fixture-entity-1"),
        });
    }
    current.members[0].character = Some(entity(5));
    current.members[1].character = Some(entity(6));
    for (value, ordinal, audience) in [
        (10, 0, AudienceScope::Shared),
        (11, 1, AudienceScope::Members(vec![member(3)])),
        (12, 2, AudienceScope::Host),
    ] {
        let mut value = fact(value, ordinal);
        value.audience = audience;
        current.facts.push(value);
    }
    current.decisions.push(AcceptedDecision {
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        revision: basis().revision,
        facts: current.facts.iter().map(|fact| fact.id).collect(),
        draws: vec![],
        effects: vec![],
        source_policy: label("fixture-source-policy"),
        semantic_output: None,
    });
    for (id, evidence, audience, text) in [
        (30, 0, AudienceScope::Shared, "Public attributed claim."),
        (
            31,
            1,
            AudienceScope::Members(vec![member(3)]),
            "Explicit private claim.",
        ),
        (32, 2, AudienceScope::Host, "Hidden counterpart."),
    ] {
        current.beliefs.push(AttributedClaim {
            id: claim_id(id),
            holder: entity(4),
            subject: entity(4),
            claim: text.to_owned(),
            evidence: vec![current.facts[evidence].id],
            audience,
            source: content(),
        });
    }
    current.continuity.npcs.push(NpcState {
        entity: entity(4),
        role: content(),
        personality: content(),
        motivations: vec![],
        goals: vec![],
        needs: vec![],
        fears: vec![],
        known_facts: vec![current.facts[1].id],
        beliefs: vec![claim_id(31)],
        secrets: vec![SecretPolicy {
            holder: entity(4),
            claims: vec![claim_id(31)],
            policy: content(),
            permitted_audience: AudienceScope::Members(vec![member(3)]),
        }],
    });
    current.conversations.push(ConversationState {
        id: claim_id(50),
        participants: vec![entity(4), entity(5), entity(6)],
        topic: content(),
        accepted_facts: vec![current.facts[0].id],
    });
    current
}

/// Faithful local trusted-owner fixture; no production content/auth issuer is implied.
struct Source {
    allowed: Cell<bool>,
    shared: bool,
}
impl Source {
    fn new(shared: bool) -> Self {
        Self {
            allowed: Cell::new(true),
            shared,
        }
    }
}
impl ConversationSourceOwner for Source {
    fn validate(
        &self,
        current: Basis,
        admitted: &CheckpointPins,
        conversation: &ConversationState,
        permission: &ConversationPermission,
        claims: &[RecordId],
    ) -> Result<(), ConversationSourceError> {
        if !self.allowed.get() || (permission.recipient == ObserverScope::Shared && !self.shared) {
            return Err(ConversationSourceError::AccessDenied);
        }
        if current != basis() || admitted != &pins() {
            return Err(ConversationSourceError::Stale);
        }
        if permission.policy != content()
            || permission.topic != conversation.topic
            || permission.speaker != entity(4)
            || permission.window.opens_at.ticks < 110
            || permission.window.expires_at.ticks > 130
            || claims
                .iter()
                .any(|id| !(30..33).any(|value| claim_id(value) == *id))
        {
            return Err(ConversationSourceError::Unsupported);
        }
        Ok(())
    }
}
impl SpeechSourceOwner for Source {
    fn validate(
        &self,
        current: Basis,
        admitted: &CheckpointPins,
        version: &SpeechVersions,
        proposal: &SpeechProposal,
    ) -> Result<(), SpeechSourceError> {
        if !self.allowed.get() {
            return Err(SpeechSourceError::AccessDenied);
        }
        if current != basis() || admitted != &pins() || version != &versions() {
            return Err(SpeechSourceError::Stale);
        }
        if proposal.source != content()
            || proposal.claims.iter().any(|claim| {
                !(30..33).any(|value| claim_id(value) == claim.claim)
                    || claim.intent != SpeechIntent::Claim
                    || !claim.slots.is_empty()
            })
        {
            return Err(SpeechSourceError::Unsupported);
        }
        Ok(())
    }
}
fn conversation_limits() -> ConversationLimits {
    ConversationLimits {
        maximum_participants: 4,
        maximum_accepted_facts: 4,
        maximum_disclosures: 4,
        maximum_comparisons: 1000,
        maximum_window_ticks: 30,
    }
}
fn observation<'a>(
    current: &'a Checkpoint,
    versions: &'a SpeechVersions,
    source: &'a Source,
    observer: ObserverScope,
) -> ConversationObservation<'a> {
    ConversationObservation {
        speech: SpeechObservation {
            checkpoint: current,
            basis: current.basis(),
            pins: current.pins(),
            observer,
            versions,
            source_owner: Some(source),
            limits: speech_limits(),
        },
        source_owner: Some(source),
        limits: conversation_limits(),
    }
}
fn permission(observer: ObserverScope, ids: &[u8]) -> ConversationPermission {
    ConversationPermission {
        policy: content(),
        conversation: claim_id(50),
        topic: content(),
        speaker: entity(4),
        recipient: observer,
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
        disclosures: ids
            .iter()
            .filter(|id| **id == 31)
            .map(|id| SecretDisclosure {
                claim: claim_id(*id),
                policy: content(),
            })
            .collect(),
    }
}
fn proposal(ids: &[u8]) -> SpeechProposal {
    SpeechProposal {
        source: content(),
        claims: ids
            .iter()
            .map(|id| PlannedClaim {
                claim: claim_id(*id),
                intent: SpeechIntent::Claim,
                uncertainty: DeclaredUncertainty::NoneDeclared,
                slots: vec![],
            })
            .collect(),
    }
}
fn admitted(
    current: &ConversationObservation<'_>,
    ids: &[u8],
) -> Result<ConversationPlan, ConversationError> {
    admit_conversation(
        permission(current.speech.observer, ids),
        proposal(ids),
        current,
    )
}

#[test]
fn current_admitted_topic_speaker_and_finite_window_select_exact_scope() {
    let checkpoint = checkpoint(conversation_state()).unwrap();
    let before = checkpoint.clone();
    let versions = versions();
    let source = Source::new(false);
    let current = observation(
        &checkpoint,
        &versions,
        &source,
        ObserverScope::Member(member(3)),
    );
    let plan = admitted(&current, &[30, 31]).unwrap();
    let context = project_conversation(&plan, &current).unwrap();
    assert_eq!(context.speaker(), entity(4));
    assert_eq!(context.metadata().permitted_claims, 2);
    assert_eq!(
        context
            .claims()
            .iter()
            .map(|claim| claim.id)
            .collect::<Vec<_>>(),
        vec![claim_id(30), claim_id(31)]
    );
    assert_eq!(context.claims()[1].text, "Explicit private claim.");
    assert_eq!(
        context.claims()[1].text.as_ptr(),
        checkpoint.state().beliefs[1].claim.as_ptr()
    );
    context.validate_current(&current).unwrap();
    assert_eq!(checkpoint, before);
    assert!(!format!("{plan:?} {context:?}").contains("Explicit private claim"));
}

#[test]
fn nonparticipant_wrong_topic_expired_or_stale_window_refuses() {
    let checkpoint = checkpoint(conversation_state()).unwrap();
    let versions = versions();
    let source = Source::new(false);
    let current = observation(
        &checkpoint,
        &versions,
        &source,
        ObserverScope::Member(member(3)),
    );
    for (mut scope, expected) in [
        (
            permission(current.speech.observer, &[30]),
            ConversationError::SpeakerUnavailable,
        ),
        (
            permission(current.speech.observer, &[30]),
            ConversationError::TopicChanged,
        ),
        (
            permission(current.speech.observer, &[30]),
            ConversationError::WindowExpired,
        ),
        (
            permission(current.speech.observer, &[30]),
            ConversationError::WindowNotOpen,
        ),
        (
            permission(current.speech.observer, &[30]),
            ConversationError::InvalidWindow,
        ),
    ] {
        match expected {
            ConversationError::SpeakerUnavailable => scope.speaker = entity(7),
            ConversationError::TopicChanged => scope.topic = alternate_topic(),
            ConversationError::WindowExpired => scope.window.expires_at.ticks = 120,
            ConversationError::WindowNotOpen => scope.window.opens_at.ticks = 121,
            ConversationError::InvalidWindow => scope.window.expires_at.ticks_per_second = 11,
            _ => unreachable!(),
        }
        assert_eq!(
            admit_conversation(scope, proposal(&[30]), &current).unwrap_err(),
            expected
        );
    }
    let mut scope = permission(current.speech.observer, &[30]);
    scope.speaker = entity(5); // A participant cannot speak another holder's literal claims.
    assert_eq!(
        admit_conversation(scope, proposal(&[30]), &current).unwrap_err(),
        ConversationError::SpeakerUnavailable
    );
}

#[test]
fn explicit_secret_selection_never_widens_claim_or_secret_audience() {
    let versions = versions();
    let source = Source::new(true);
    for (observer, ids) in [
        (ObserverScope::Shared, vec![31]),
        (ObserverScope::Member(member(5)), vec![30, 31]),
        (ObserverScope::Member(member(3)), vec![32]),
    ] {
        let checkpoint = checkpoint(conversation_state()).unwrap();
        let before = checkpoint.clone();
        let current = observation(&checkpoint, &versions, &source, observer);
        assert_eq!(
            admitted(&current, &ids).unwrap_err(),
            ConversationError::AudienceDenied
        );
        assert_eq!(checkpoint, before);
    }
    let mut state = conversation_state();
    state.beliefs[1].audience = AudienceScope::Shared;
    state.beliefs[1].evidence = vec![state.facts[0].id];
    let checkpoint = checkpoint(state).unwrap();
    let public = observation(&checkpoint, &versions, &source, ObserverScope::Shared);
    assert_eq!(
        admitted(&public, &[31]).unwrap_err(),
        ConversationError::AudienceDenied
    );
    let private = observation(
        &checkpoint,
        &versions,
        &source,
        ObserverScope::Member(member(3)),
    );
    admitted(&private, &[31]).unwrap();
}

#[test]
fn shared_output_requires_explicit_owner_admission_and_public_ceiling() {
    let checkpoint = checkpoint(conversation_state()).unwrap();
    let versions = versions();
    let source = Source::new(false);
    let current = observation(&checkpoint, &versions, &source, ObserverScope::Shared);
    assert_eq!(
        admitted(&current, &[30]).unwrap_err(),
        ConversationError::Source(ConversationSourceError::AccessDenied)
    );
    let public_source = Source::new(true);
    let public = observation(
        &checkpoint,
        &versions,
        &public_source,
        ObserverScope::Shared,
    );
    let plan = admitted(&public, &[30]).unwrap();
    assert_eq!(
        project_conversation(&plan, &public).unwrap().claims()[0].text,
        "Public attributed claim."
    );
    assert_eq!(
        admitted(&public, &[30, 31]).unwrap_err(),
        ConversationError::AudienceDenied
    );
}

#[test]
fn missing_source_owner_invalid_bounds_duplicates_and_oversize_refuse() {
    let checkpoint = checkpoint(conversation_state()).unwrap();
    let versions = versions();
    let source = Source::new(false);
    let observer = ObserverScope::Member(member(3));
    let mut current = observation(&checkpoint, &versions, &source, observer);
    current.source_owner = None;
    assert_eq!(
        admitted(&current, &[30]).unwrap_err(),
        ConversationError::SourceUnavailable
    );
    current.source_owner = Some(&source);
    current.speech.source_owner = None;
    assert_eq!(
        admitted(&current, &[30]).unwrap_err(),
        ConversationError::Speech(SpeechError::SourceUnavailable)
    );
    current.speech.source_owner = Some(&source);
    for expected in [
        ConversationError::InvalidWindow,
        ConversationError::Capacity,
    ] {
        let mut scope = permission(observer, &[30]);
        scope.window.expires_at.ticks = if expected == ConversationError::InvalidWindow {
            110
        } else {
            200
        };
        assert_eq!(
            admit_conversation(scope, proposal(&[30]), &current).unwrap_err(),
            expected
        );
    }
    let mut scope = permission(observer, &[31]);
    scope.disclosures.push(scope.disclosures[0].clone());
    assert_eq!(
        admit_conversation(scope, proposal(&[31]), &current).unwrap_err(),
        ConversationError::DuplicateDisclosure
    );
    assert_eq!(
        admitted(&current, &[30, 30]).unwrap_err(),
        ConversationError::Speech(SpeechError::DuplicateClaim)
    );
    for dimension in 0..5 {
        let mut bounded = observation(&checkpoint, &versions, &source, observer);
        match dimension {
            0 => bounded.limits.maximum_participants = 2,
            1 => bounded.limits.maximum_accepted_facts = 0,
            2 => bounded.limits.maximum_disclosures = 0,
            3 => bounded.limits.maximum_comparisons = 0,
            4 => bounded.speech.limits.maximum_plan_claims = 0,
            _ => unreachable!(),
        }
        assert_eq!(
            admitted(&bounded, &[31]).unwrap_err(),
            ConversationError::Capacity
        );
    }
    let mut scope = permission(observer, &[30]);
    scope.policy = alternate_topic();
    assert_eq!(
        admit_conversation(scope, proposal(&[30]), &current).unwrap_err(),
        ConversationError::Source(ConversationSourceError::Unsupported)
    );
}

#[test]
fn implicit_foreign_and_ambiguous_disclosures_refuse_without_output() {
    let checkpoint = checkpoint(conversation_state()).unwrap();
    let versions = versions();
    let source = Source::new(false);
    let observer = ObserverScope::Member(member(3));
    let current = observation(&checkpoint, &versions, &source, observer);
    let mut scope = permission(observer, &[31]);
    scope.disclosures.clear();
    assert_eq!(
        admit_conversation(scope, proposal(&[31]), &current).unwrap_err(),
        ConversationError::ImplicitDisclosure
    );
    let mut scope = permission(observer, &[31]);
    scope.disclosures[0].policy = alternate_topic();
    assert_eq!(
        admit_conversation(scope, proposal(&[31]), &current).unwrap_err(),
        ConversationError::InvalidDisclosure
    );
    assert_eq!(
        admit_conversation(permission(observer, &[31]), proposal(&[30]), &current).unwrap_err(),
        ConversationError::InvalidDisclosure
    );
    let mut scope = permission(observer, &[30]);
    scope.disclosures.push(SecretDisclosure {
        claim: claim_id(30),
        policy: content(),
    });
    assert_eq!(
        admit_conversation(scope, proposal(&[30]), &current).unwrap_err(),
        ConversationError::InvalidDisclosure
    );
    let mut state = conversation_state();
    let secret = state.continuity.npcs[0].secrets[0].clone();
    state.continuity.npcs[0].secrets.push(secret);
    let ambiguous = super_checkpoint(state);
    let observed = observation(&ambiguous, &versions, &source, observer);
    assert_eq!(
        admitted(&observed, &[31]).unwrap_err(),
        ConversationError::SecretOwnership
    );
}
fn super_checkpoint(state: GameState) -> Checkpoint {
    checkpoint(state).unwrap()
}

#[test]
fn recipient_membership_and_changed_observer_refuse() {
    let versions = versions();
    let source = Source::new(false);
    let checkpoint = checkpoint(conversation_state()).unwrap();
    let current = observation(
        &checkpoint,
        &versions,
        &source,
        ObserverScope::Member(member(3)),
    );
    let plan = admitted(&current, &[30]).unwrap();
    let changed = observation(
        &checkpoint,
        &versions,
        &source,
        ObserverScope::Member(member(5)),
    );
    assert_eq!(
        project_conversation(&plan, &changed).unwrap_err(),
        ConversationError::RecipientChanged
    );
    for change in 0..2 {
        let mut state = conversation_state();
        if change == 0 {
            state.members[0].character = None;
        } else {
            state.conversations[0]
                .participants
                .retain(|id| *id != entity(5));
        }
        let absent = super_checkpoint(state);
        let observed = observation(
            &absent,
            &versions,
            &source,
            ObserverScope::Member(member(3)),
        );
        assert_eq!(
            admitted(&observed, &[30]).unwrap_err(),
            ConversationError::RecipientUnavailable
        );
    }
}

#[test]
fn accepted_speech_projection_revalidates_current_basis_and_preserves_checkpoint() {
    let checkpoint = checkpoint(conversation_state()).unwrap();
    let before = checkpoint.clone();
    let versions = versions();
    let source = Source::new(false);
    let current = observation(
        &checkpoint,
        &versions,
        &source,
        ObserverScope::Member(member(3)),
    );
    let plan = admitted(&current, &[30, 31]).unwrap();
    let context = project_conversation(&plan, &current).unwrap();
    source.allowed.set(false);
    assert_eq!(
        context.validate_current(&current).unwrap_err(),
        ConversationError::Source(ConversationSourceError::AccessDenied)
    );
    source.allowed.set(true);
    for change in 0..3 {
        let mut stale = observation(&checkpoint, &versions, &source, current.speech.observer);
        match change {
            0 => stale.speech.basis.run = RunId::from_bytes(&[9; 16]).unwrap(),
            1 => stale.speech.basis.revision = revision(3, 8),
            2 => stale.speech.basis.revision = revision(2, 9),
            _ => unreachable!(),
        }
        assert_eq!(
            context.validate_current(&stale).unwrap_err(),
            ConversationError::StalePlan
        );
    }
    assert_eq!(checkpoint, before);
}

#[test]
fn topic_transition_expiry_and_same_basis_changes_require_fresh_admission() {
    let checkpoint = checkpoint(conversation_state()).unwrap();
    let versions = versions();
    let source = Source::new(true);
    let current = observation(
        &checkpoint,
        &versions,
        &source,
        ObserverScope::Member(member(3)),
    );
    let plan = admitted(&current, &[31]).unwrap();
    for (change, expected) in [
        (0, ConversationError::TopicChanged),
        (1, ConversationError::WindowExpired),
        (2, ConversationError::ContextChanged),
        (3, ConversationError::AudienceDenied),
    ] {
        let mut state = conversation_state();
        match change {
            0 => state.conversations[0].topic = alternate_topic(),
            1 => state.logical_time.ticks = 130,
            2 => state.conversations[0]
                .accepted_facts
                .push(state.facts[1].id),
            3 => state.continuity.npcs[0].secrets[0].permitted_audience = AudienceScope::Host,
            _ => unreachable!(),
        }
        let changed = super_checkpoint(state);
        let observed = observation(&changed, &versions, &source, current.speech.observer);
        assert_eq!(
            project_conversation(&plan, &observed).unwrap_err(),
            expected
        );
    }
    let mut transitioned = conversation_state();
    transitioned.conversations[0].topic = alternate_topic();
    let transitioned = super_checkpoint(transitioned);
    let observed = observation(&transitioned, &versions, &source, ObserverScope::Shared);
    let mut scope = permission(ObserverScope::Shared, &[31]);
    scope.topic = alternate_topic();
    assert_eq!(
        admit_conversation(scope, proposal(&[31]), &observed).unwrap_err(),
        ConversationError::AudienceDenied
    );
}

#[test]
fn replay_and_irrelevant_hidden_state_do_not_change_permitted_expression() {
    let versions = versions();
    let source = Source::new(false);
    let mut output = None;
    for secret in ["Hidden variant A", "Hidden variant B"] {
        let mut state = conversation_state();
        state.beliefs[2].claim = secret.to_owned();
        let checkpoint = checkpoint(state).unwrap();
        assert_eq!(checkpoint.state().mode, ExecutionMode::Replay);
        let before = checkpoint.clone();
        let current = observation(
            &checkpoint,
            &versions,
            &source,
            ObserverScope::Member(member(3)),
        );
        let plan = admitted(&current, &[30, 31]).unwrap();
        for _ in 0..2 {
            let context = project_conversation(&plan, &current).unwrap();
            let actual: Vec<_> = context
                .claims()
                .iter()
                .map(|claim| (claim.id, claim.text.to_owned(), claim.evidence.to_vec()))
                .collect();
            if let Some(expected) = &output {
                assert_eq!(&actual, expected);
            } else {
                output = Some(actual);
            }
            context.validate_current(&current).unwrap();
        }
        assert_eq!(checkpoint, before);
    }
}
