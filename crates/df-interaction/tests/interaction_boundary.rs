use std::cell::Cell;

use df_interaction::conversation::*;
use df_interaction::debts::*;
use df_interaction::relationships::*;
use df_interaction::speech::*;

use df_interaction::reactions::{
    NoReactionReason, ReactionEntry, ReactionError, ReactionLimit, ReactionLimits, ReactionOutcome,
    ReactionPolicy, ReactionRequest, ReactionSourceOwner, ReactionSourceRefusal, react,
};
use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, LocaleTag, MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId,
    SessionId, SessionRevision,
};

fn label(text: &str) -> RevisionLabel {
    RevisionLabel::new(Some(text)).unwrap()
}
fn content(entry: &str) -> ContentReference {
    ContentReference {
        package: label("fixture-package-v1"),
        entry: label(entry),
    }
}
fn entity(byte: u8) -> EntityId {
    EntityId::from_bytes(&[byte; 16]).unwrap()
}
fn fact_id(byte: u8) -> FactId {
    FactId::from_bytes(&[byte; 16]).unwrap()
}
fn record(byte: u8) -> RecordId {
    RecordId::from_bytes(&[byte; 16]).unwrap()
}
fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 3),
    }
}
fn pins() -> CheckpointPins {
    CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: label("fixture-rules"),
            catalog: label("fixture-catalog"),
            catalog_digest: ContentDigest([1; 32]),
            source_manifest: label("fixture-sources"),
            source_manifest_digest: ContentDigest([2; 32]),
            handler: label("fixture-handler"),
            handler_digest: ContentDigest([3; 32]),
        },
        content: ContentPins {
            content: label("fixture-content"),
            content_digest: ContentDigest([4; 32]),
            package: label("fixture-package-v1"),
            package_digest: ContentDigest([5; 32]),
        },
        build: BuildIdentity::new(
            Some("fixture-source"),
            Some("fixture-native"),
            Some("fixture-wasm"),
            Some("fixture-config"),
            Some("fixture-content"),
        )
        .unwrap(),
    }
}
fn entry() -> ReactionEntry {
    ReactionEntry {
        source: content("reaction-v1"),
        event: content("accepted-help-event"),
        personality: content("authored-personality"),
        motivation: content("authored-motivation"),
        relationship_policy: content("relationship-policy"),
        from_state: label("reserved"),
        to_state: label("receptive"),
    }
}
fn request() -> ReactionRequest {
    ReactionRequest {
        expected_basis: basis(),
        npc: entity(3),
        target: entity(4),
        event: fact_id(7),
        witness: record(8),
    }
}
fn state() -> GameState {
    let time = LogicalTime {
        ticks: 10,
        ticks_per_second: 1,
    };
    let operation = OperationId::from_bytes(&[5; 16]).unwrap();
    GameState {
        mode: ExecutionMode::Replay,
        logical_time: time,
        members: vec![],
        entities: [entity(3), entity(4)]
            .into_iter()
            .map(|id| WorldEntity {
                id,
                definition: content("entity"),
                location: None,
                position: None,
                identity_revision: label("identity-v1"),
            })
            .collect(),
        characters: vec![],
        resources: vec![],
        inventory: vec![],
        facts: [(fact_id(6), 0, None), (fact_id(7), 1, Some(fact_id(6)))]
            .into_iter()
            .map(|(id, ordinal, cause)| GameFact {
                id,
                revision: basis().revision,
                operation,
                ordinal,
                cause,
                audience: AudienceScope::Host,
                value: FactValue::ContentEvent {
                    definition: content("accepted-help-event"),
                    subjects: vec![entity(4)],
                },
            })
            .collect(),
        draws: vec![],
        decisions: vec![AcceptedDecision {
            operation,
            revision: basis().revision,
            facts: vec![fact_id(6), fact_id(7)],
            draws: vec![],
            effects: vec![],
            source_policy: label("accepted-policy-v1"),
            semantic_output: None,
        }],
        pending: vec![],
        intents: vec![],
        timers: vec![],
        active_effects: vec![],
        knowledge: vec![],
        beliefs: vec![],
        memories: vec![],
        schedules: vec![],
        threats: vec![],
        relationships: vec![Relationship {
            subject: entity(3),
            object: entity(4),
            policy: content("relationship-policy"),
            state: label("reserved"),
            trust: RelationshipAxisState {
                value: label("reserved"),
                provenance: RelationshipAxisProvenance::AuthoredBaseline {
                    source: content("relationship-policy"),
                },
            },
            affection: RelationshipAxisState {
                value: label("reserved"),
                provenance: RelationshipAxisProvenance::AuthoredBaseline {
                    source: content("relationship-policy"),
                },
            },
            respect: RelationshipAxisState {
                value: label("reserved"),
                provenance: RelationshipAxisProvenance::AuthoredBaseline {
                    source: content("relationship-policy"),
                },
            },
            fear: RelationshipAxisState {
                value: label("reserved"),
                provenance: RelationshipAxisProvenance::AuthoredBaseline {
                    source: content("relationship-policy"),
                },
            },
            suspicion: RelationshipAxisState {
                value: label("reserved"),
                provenance: RelationshipAxisProvenance::AuthoredBaseline {
                    source: content("relationship-policy"),
                },
            },
            debt: RelationshipAxisState {
                value: label("reserved"),
                provenance: RelationshipAxisProvenance::AuthoredBaseline {
                    source: content("relationship-policy"),
                },
            },
            familiarity: RelationshipAxisState {
                value: label("reserved"),
                provenance: RelationshipAxisProvenance::AuthoredBaseline {
                    source: content("relationship-policy"),
                },
            },
        }],
        conversations: vec![],
        obligations: vec![],
        narrative: NarrativeState {
            definition: content("general"),
            active_beats: vec![],
            completed_beats: vec![],
            open_threads: vec![],
            accepted_facts: vec![],
            remaining_budget: 0,
        },
        encounters: vec![],
        activity: vec![],
        tempo: TempoState {
            policy: content("general"),
            presentation_ticks: 0,
            intensity: 0,
            inertia: 0,
            fatigue: vec![],
        },
        presentation: vec![],
        continuity: ContinuityState {
            creation: vec![],
            simulation: vec![],
            catch_up: None,
            environment: vec![],
            travel: vec![],
            witnesses: vec![WitnessRecord {
                id: record(8),
                observer: entity(3),
                fact: fact_id(7),
                perceived_at: time,
                source: content("witness-policy"),
            }],
            rumors: vec![],
            journal: vec![],
            summaries: vec![],
            retrieval: vec![],
            retrieved: vec![],
            consolidation: vec![],
            npcs: vec![NpcState {
                entity: entity(3),
                personality: content("authored-personality"),
                role: content("authored-personality"),
                motivations: vec![content("authored-motivation")],
                goals: vec![],
                needs: vec![],
                fears: vec![],
                known_facts: vec![fact_id(7)],
                beliefs: vec![],
                secrets: vec![],
            }],
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
        },
    }
}
fn inventory() -> Vec<ContentReference> {
    [
        "general",
        "entity",
        "accepted-help-event",
        "obligation-agreement",
        "reaction-v1",
        "authored-personality",
        "authored-motivation",
        "relationship-policy",
        "witness-policy",
        "other-personality",
        "other-motivation",
        "other-event",
    ]
    .into_iter()
    .map(content)
    .collect()
}

fn checkpoint(state: GameState) -> Checkpoint {
    checkpoint_with_pins(state, pins())
}

fn checkpoint_with_pins(state: GameState, pins: CheckpointPins) -> Checkpoint {
    try_checkpoint_with_pins(state, pins).unwrap()
}

fn try_checkpoint_with_pins(
    state: GameState,
    pins: CheckpointPins,
) -> Result<Checkpoint, CheckpointError> {
    let admitted = inventory();
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis(),
        pins,
        state,
        ReferenceInventory {
            rules: &[],
            content: &admitted,
            resources: &[],
            assets: &[],
        },
        CheckpointLimits {
            maximum_records: 100,
            maximum_text_bytes: 256,
            maximum_total_text_bytes: 4096,
            maximum_retained_bytes: 1024 * 1024,
        },
    )
}

struct SourceOwner {
    basis: Basis,
    pins: ContentPins,
    entries: Vec<ReactionEntry>,
    refusal: Cell<Option<ReactionSourceRefusal>>,
}

impl ReactionSourceOwner for SourceOwner {
    fn validate_entry(
        &self,
        supplied: Basis,
        pins: &ContentPins,
        entry: &ReactionEntry,
    ) -> Result<(), ReactionSourceRefusal> {
        if let Some(refusal) = self.refusal.get() {
            return Err(refusal);
        }
        if supplied != self.basis || pins != &self.pins {
            return Err(ReactionSourceRefusal::StaleAdmission);
        }
        if !self.entries.contains(entry) {
            return Err(ReactionSourceRefusal::NotAdmitted);
        }
        Ok(())
    }
}

fn source_owner(entries: &[ReactionEntry]) -> SourceOwner {
    // Synthetic already-admitted authored entries, not a production content/rights grant.
    SourceOwner {
        basis: basis(),
        pins: pins().content,
        entries: entries.to_vec(),
        refusal: Cell::new(None),
    }
}

fn limits() -> ReactionLimits {
    ReactionLimits {
        maximum_entries: 4,
        maximum_policy_bytes: 64 * 1024,
        maximum_work: 1000,
        maximum_proposal_bytes: 64 * 1024,
    }
}

fn run(
    checkpoint: &Checkpoint,
    request: ReactionRequest,
    entries: &[ReactionEntry],
) -> Result<ReactionOutcome, ReactionError> {
    let admitted = inventory();
    let owner = source_owner(entries);
    let policy = ReactionPolicy::new(
        checkpoint,
        entries,
        ReferenceInventory {
            rules: &[],
            content: &admitted,
            resources: &[],
            assets: &[],
        },
        &owner,
        limits(),
    )?;
    react(checkpoint, request, &policy)
}

fn member(byte: u8) -> MemberId {
    MemberId::from_bytes(&[byte; 16]).unwrap()
}

fn boundary_state() -> GameState {
    let mut current = state();
    current.entities.push(WorldEntity {
        id: entity(11),
        definition: content("entity"),
        location: None,
        position: None,
        identity_revision: label("identity-v1"),
    });
    current.members = vec![
        MembershipLink {
            member: member(9),
            character: Some(entity(4)),
        },
        MembershipLink {
            member: member(10),
            character: Some(entity(11)),
        },
    ];
    current.facts[0].audience = AudienceScope::Shared;
    let mut private = current.facts[0].clone();
    private.id = fact_id(12);
    private.ordinal = 2;
    private.audience = AudienceScope::Members(vec![member(9)]);
    current.facts.push(private);
    current.decisions[0].facts.push(fact_id(12));
    // Agreement is its own accepted event, preserving the reaction's help-event lineage.
    let mut agreement = current.facts[0].clone();
    agreement.id = fact_id(13);
    agreement.ordinal = 3;
    agreement.value = FactValue::ContentEvent {
        definition: content("obligation-agreement"),
        subjects: vec![entity(3), entity(4)],
    };
    current.facts.push(agreement);
    current.decisions[0].facts.push(fact_id(13));
    current.beliefs = vec![
        AttributedClaim {
            id: record(30),
            holder: entity(3),
            subject: entity(4),
            claim: "Mira has 13 coins.".to_owned(),
            evidence: vec![fact_id(6)],
            audience: AudienceScope::Shared,
            source: content("general"),
        },
        AttributedClaim {
            id: record(31),
            holder: entity(3),
            subject: entity(4),
            claim: "PRIVATE-EXPLICIT-SELECTION".to_owned(),
            evidence: vec![fact_id(12)],
            audience: AudienceScope::Members(vec![member(9)]),
            source: content("general"),
        },
        AttributedClaim {
            id: record(32),
            holder: entity(3),
            subject: entity(4),
            claim: "HOST-COUNTERPART".to_owned(),
            evidence: vec![fact_id(7)],
            audience: AudienceScope::Host,
            source: content("general"),
        },
    ];
    current.continuity.npcs[0].beliefs = vec![record(31)];
    current.continuity.npcs[0].secrets = vec![SecretPolicy {
        holder: entity(3),
        claims: vec![record(31)],
        policy: content("general"),
        permitted_audience: AudienceScope::Members(vec![member(9)]),
    }];
    current.conversations.push(ConversationState {
        id: record(40),
        participants: vec![entity(3), entity(4), entity(11)],
        topic: content("general"),
        accepted_facts: vec![fact_id(6)],
    });
    current.obligations.push(Obligation {
        id: record(41),
        obligor: entity(3),
        beneficiary: entity(4),
        definition: content("general"),
        terms: content("general"),
        due: Some(LogicalTime {
            ticks: 15,
            ticks_per_second: 1,
        }),
        agreement: ObligationAgreement {
            source: content("obligation-agreement"),
            fact: fact_id(13),
            source_policy: label("accepted-policy-v1"),
            at: current.logical_time,
        },
        status: ObligationStatus::Active,
        transition: None,
    });
    current
}

struct SpeechOwner {
    allowed: Cell<bool>,
}
impl SpeechSourceOwner for SpeechOwner {
    fn validate(
        &self,
        supplied: Basis,
        pins: &CheckpointPins,
        versions: &SpeechVersions,
        proposal: &SpeechProposal,
    ) -> Result<(), SpeechSourceError> {
        if !self.allowed.get() {
            return Err(SpeechSourceError::AccessDenied);
        }
        if supplied != basis() || pins != &crate_pins() || versions != &speech_versions() {
            return Err(SpeechSourceError::Stale);
        }
        if proposal.source != content("general")
            || proposal.claims.iter().any(|claim| {
                ![record(30), record(31), record(32)].contains(&claim.claim)
                    || !claim.slots.is_empty()
            })
        {
            return Err(SpeechSourceError::Unsupported);
        }
        Ok(())
    }
}
fn crate_pins() -> CheckpointPins {
    pins()
}
impl ConversationSourceOwner for SpeechOwner {
    fn validate(
        &self,
        supplied: Basis,
        pins: &CheckpointPins,
        conversation: &ConversationState,
        scope: &ConversationPermission,
        claims: &[RecordId],
    ) -> Result<(), ConversationSourceError> {
        if !self.allowed.get() {
            return Err(ConversationSourceError::AccessDenied);
        }
        if supplied != basis() || pins != &crate_pins() {
            return Err(ConversationSourceError::Stale);
        }
        if scope.policy != content("general")
            || conversation.id != record(40)
            || scope.topic != content("general")
            || scope.speaker != entity(3)
            || scope.window.opens_at.ticks != 5
            || scope.window.expires_at.ticks > 15
            || claims
                .iter()
                .any(|claim| ![record(30), record(31), record(32)].contains(claim))
        {
            return Err(ConversationSourceError::Unsupported);
        }
        Ok(())
    }
}
fn speech_versions() -> SpeechVersions {
    SpeechVersions {
        access: label("current-access"),
        contract: label("claim-contract-v1"),
        profile: label("literal-profile-v1"),
        locale: LocaleTag::parse("en-US").unwrap(),
        provider: label("fixture-provider"),
        model: label("fixture-model"),
        format: label("fixture-format"),
        quote: label("private-quote"),
    }
}
fn conversation_observation<'a>(
    checkpoint: &'a Checkpoint,
    versions: &'a SpeechVersions,
    source: &'a SpeechOwner,
    observer: ObserverScope,
) -> ConversationObservation<'a> {
    ConversationObservation {
        source_owner: Some(source),
        limits: ConversationLimits {
            maximum_participants: 3,
            maximum_accepted_facts: 2,
            maximum_disclosures: 2,
            maximum_comparisons: 1000,
            maximum_window_ticks: 10,
        },
        speech: SpeechObservation {
            checkpoint,
            basis: checkpoint.basis(),
            pins: checkpoint.pins(),
            observer,
            versions,
            source_owner: Some(source),
            limits: SpeechLimits {
                maximum_plan_claims: 3,
                maximum_slots: 3,
                maximum_claim_bytes: 256,
                maximum_selected_text_and_evidence_bytes: 2048,
                maximum_comparisons: 1000,
                facts: PerceptionLimits {
                    maximum_scan_records: 100,
                    maximum_member_comparisons: 100,
                    maximum_selected_facts: 10,
                },
                claims: ClaimPerceptionLimits {
                    maximum_scan_records: 1000,
                    maximum_record_comparisons: 1000,
                    maximum_selected_claims: 10,
                },
            },
        },
    }
}
fn scope(observer: ObserverScope, secret: bool) -> ConversationPermission {
    ConversationPermission {
        policy: content("general"),
        conversation: record(40),
        topic: content("general"),
        speaker: entity(3),
        recipient: observer,
        window: PermissionWindow {
            opens_at: LogicalTime {
                ticks: 5,
                ticks_per_second: 1,
            },
            expires_at: LogicalTime {
                ticks: 15,
                ticks_per_second: 1,
            },
        },
        disclosures: if secret {
            vec![SecretDisclosure {
                claim: record(31),
                policy: content("general"),
            }]
        } else {
            vec![]
        },
    }
}
fn speech(secret: bool, intent: SpeechIntent) -> SpeechProposal {
    let ids = if secret {
        vec![record(30), record(31)]
    } else {
        vec![record(30)]
    };
    SpeechProposal {
        source: content("general"),
        claims: ids
            .into_iter()
            .map(|claim| PlannedClaim {
                claim,
                intent,
                uncertainty: DeclaredUncertainty::Explicit,
                slots: vec![],
            })
            .collect(),
    }
}

impl RelationshipOwner for SourceOwner {
    type Subject = EntityId;
    type Basis = Basis;
    type Value = RevisionLabel;
    type Provenance = RelationshipAxisProvenance;
    type Refusal = ReactionSourceRefusal;
    fn validate_axis_change(
        &self,
        axis: RelationshipAxis,
        current: &RelationshipAxisValue<Self::Value, Self::Provenance>,
        proposed: &RelationshipAxisValue<Self::Value, Self::Provenance>,
    ) -> Result<(), Self::Refusal> {
        if let Some(refusal) = self.refusal.get() {
            return Err(refusal);
        }
        if axis != RelationshipAxis::Trust
            || current.value() != &label("reserved")
            || proposed.value() != &label("receptive")
            || proposed.provenance() != &axis_provenance()
        {
            return Err(ReactionSourceRefusal::UnsupportedPolicy);
        }
        Ok(())
    }
}
fn axis_provenance() -> RelationshipAxisProvenance {
    RelationshipAxisProvenance::AcceptedFact {
        source: content("reaction-v1"),
        fact: fact_id(7),
        source_policy: label("accepted-policy-v1"),
        witness: Some(record(8)),
    }
}

// The existing generic contract borrows the canonical model; it defines no durable alias.
// Its caller verifies the canonical logical timebase before passing ordered tick values.
struct BorrowedObligation<'a> {
    current: &'a Obligation,
    basis: Basis,
    at: u64,
    due: Option<u64>,
}
impl ObligationView for BorrowedObligation<'_> {
    type Id = RecordId;
    type Party = EntityId;
    type Terms = ContentReference;
    type Basis = Basis;
    type LogicalTime = u64;
    type Provenance = ObligationAgreement;
    fn obligation_id(&self) -> &Self::Id {
        &self.current.id
    }
    fn debtor(&self) -> &Self::Party {
        &self.current.obligor
    }
    fn creditor(&self) -> &Self::Party {
        &self.current.beneficiary
    }
    fn terms(&self) -> &Self::Terms {
        &self.current.terms
    }
    fn basis(&self) -> &Self::Basis {
        &self.basis
    }
    fn status(&self) -> DebtStatus {
        match self.current.status {
            ObligationStatus::Active => DebtStatus::Active,
            ObligationStatus::Fulfilled => DebtStatus::Fulfilled,
            ObligationStatus::Broken => DebtStatus::Broken,
            ObligationStatus::Expired => DebtStatus::Expired,
            ObligationStatus::Cancelled => DebtStatus::Cancelled,
        }
    }
    fn last_transition_at(&self) -> &Self::LogicalTime {
        &self.at
    }
    fn due_at(&self) -> Option<&Self::LogicalTime> {
        self.due.as_ref()
    }
    fn agreement_provenance(&self) -> &Self::Provenance {
        &self.current.agreement
    }
}

#[test]
fn whole_original_interaction_boundary_keeps_proposals_private_and_current() {
    let ledger = include_str!("interaction_boundary.json");
    for required in [
        "ReactionPolicy",
        "ConversationPermission",
        "SpeechActPlan",
        "ObligationView",
        "RelationshipOwner",
        "df-engine/src/relationship_staging.rs",
        "df-ai/src/expression.rs",
        "UNRESOLVED",
        "no_owned_streams_or_runtime_handles",
    ] {
        assert!(ledger.contains(required));
    }
    let current = checkpoint(boundary_state());
    let before = current.clone();
    let entries = vec![entry()];
    let result = run(&current, request(), &entries).unwrap();
    assert_eq!(result, run(&current, request(), &entries).unwrap());
    let ReactionOutcome::Proposed(reaction) = result else {
        panic!("source-admitted reaction expected");
    };
    let mut replacement = current.state().relationships[0].clone();
    replacement.state = label("receptive");
    assert_eq!(reaction.original, current.state().relationships[0]);
    assert_eq!(reaction.proposed, replacement);
    assert_eq!(reaction.event, fact_id(7));
    assert_eq!(reaction.witness, record(8));
    assert_eq!(reaction.accepted_revision, basis().revision);
    assert!(!format!("{reaction:?}").contains("reserved"));
    drop(reaction);
    let mut stale = request();
    stale.expected_basis.run = RunId::from_bytes(&[99; 16]).unwrap();
    assert_eq!(
        run(&current, stale, &entries),
        Err(ReactionError::StaleBasis)
    );
    let mut unknown = boundary_state();
    unknown.continuity.npcs[0].known_facts.clear();
    assert_eq!(
        run(&checkpoint(unknown), request(), &entries),
        Ok(ReactionOutcome::NoReaction(NoReactionReason::NotKnown))
    );
    let mut unwitnessed = request();
    unwitnessed.witness = record(99);
    assert_eq!(
        run(&current, unwitnessed, &entries),
        Ok(ReactionOutcome::NoReaction(NoReactionReason::NotWitnessed))
    );
    let source = source_owner(&entries);
    source
        .refusal
        .set(Some(ReactionSourceRefusal::AccessDenied));
    let admitted = inventory();
    assert!(matches!(
        ReactionPolicy::new(
            &current,
            &entries,
            ReferenceInventory {
                rules: &[],
                content: &admitted,
                resources: &[],
                assets: &[]
            },
            &source,
            limits()
        ),
        Err(ReactionError::SourceRefused(
            ReactionSourceRefusal::AccessDenied
        ))
    ));
    source.refusal.set(None);
    let mut bounded = limits();
    bounded.maximum_entries = 0;
    assert!(matches!(
        ReactionPolicy::new(
            &current,
            &entries,
            ReferenceInventory {
                rules: &[],
                content: &admitted,
                resources: &[],
                assets: &[]
            },
            &source,
            bounded
        ),
        Err(ReactionError::LimitExceeded(ReactionLimit::Entries))
    ));

    let relationship = &current.state().relationships[0];
    let axes = RelationshipAxes::new(
        [
            &relationship.trust,
            &relationship.affection,
            &relationship.respect,
            &relationship.fear,
            &relationship.suspicion,
            &relationship.debt,
            &relationship.familiarity,
        ]
        .map(|axis| RelationshipAxisValue::new(axis.value.clone(), axis.provenance.clone())),
    );
    let proposed = propose_relationship_change(
        &source,
        RelationshipView {
            subject: &relationship.subject,
            target: &relationship.object,
            basis: &basis(),
            axes: &axes,
        },
        RelationshipChange {
            subject: &relationship.subject,
            target: &relationship.object,
            expected_basis: &basis(),
            axis: RelationshipAxis::Trust,
            replacement: RelationshipAxisValue::new(label("receptive"), axis_provenance()),
        },
    )
    .unwrap();
    assert_eq!(
        proposed.axes().axis(RelationshipAxis::Trust).value(),
        &label("receptive")
    );
    for axis in [
        RelationshipAxis::Affection,
        RelationshipAxis::Respect,
        RelationshipAxis::Fear,
        RelationshipAxis::Suspicion,
        RelationshipAxis::Debt,
        RelationshipAxis::Familiarity,
    ] {
        assert_eq!(proposed.axes().axis(axis), axes.axis(axis));
    }
    drop(proposed);
    assert_eq!(relationship.trust.value, label("reserved"));

    let obligation = &current.state().obligations[0];
    assert_eq!(
        obligation.agreement.at.ticks_per_second,
        current.state().logical_time.ticks_per_second
    );
    assert_eq!(
        obligation.due.unwrap().ticks_per_second,
        current.state().logical_time.ticks_per_second
    );
    let borrowed = BorrowedObligation {
        current: obligation,
        basis: basis(),
        at: obligation.agreement.at.ticks,
        due: obligation.due.map(|time| time.ticks),
    };
    let provenance = obligation.agreement.clone();
    let now = current.state().logical_time.ticks;
    let proposed = propose_debt_transition(
        &borrowed,
        DebtTransitionRequest {
            obligation_id: &obligation.id,
            expected_basis: &basis(),
            at: &now,
            action: DebtAction::Cancel,
            authorization: DebtAuthorization::Approved,
            action_provenance: &provenance,
        },
    );
    let DebtTransitionOutcome::Proposed(candidate) = proposed else {
        panic!("explicit cancellation expected");
    };
    assert_eq!(candidate.status, DebtStatus::Cancelled);
    assert_eq!(candidate.agreement_provenance, obligation.agreement);
    drop(candidate);
    assert_eq!(obligation.status, ObligationStatus::Active);
    assert!(matches!(
        propose_debt_transition(
            &borrowed,
            DebtTransitionRequest {
                obligation_id: &obligation.id,
                expected_basis: &basis(),
                at: &now,
                action: DebtAction::Fulfill,
                authorization: DebtAuthorization::NeedsRuling,
                action_provenance: &provenance
            }
        ),
        DebtTransitionOutcome::Refused(DebtTransitionRefusal::NeedsRuling)
    ));

    let owner = SpeechOwner {
        allowed: Cell::new(true),
    };
    let versions = speech_versions();
    let observer = ObserverScope::Member(member(9));
    let observed = conversation_observation(&current, &versions, &owner, observer);
    let plan = admit_conversation(
        scope(observer, true),
        speech(true, SpeechIntent::Claim),
        &observed,
    )
    .unwrap();
    let context = project_conversation(&plan, &observed).unwrap();
    assert_eq!(context.speaker(), entity(3));
    assert_eq!(context.claims().len(), 2);
    assert_eq!(context.claims()[1].text, "PRIVATE-EXPLICIT-SELECTION");
    context.validate_current(&observed).unwrap();
    owner.allowed.set(false);
    assert_eq!(
        context.validate_current(&observed),
        Err(ConversationError::Source(
            ConversationSourceError::AccessDenied
        ))
    );
    owner.allowed.set(true);
    for (observer, expected) in [
        (ObserverScope::Shared, ConversationError::AudienceDenied),
        (
            ObserverScope::Member(member(10)),
            ConversationError::AudienceDenied,
        ),
    ] {
        let forbidden = conversation_observation(&current, &versions, &owner, observer);
        assert_eq!(
            admit_conversation(
                scope(observer, true),
                speech(true, SpeechIntent::Claim),
                &forbidden
            )
            .unwrap_err(),
            expected
        );
    }
    let mut implicit = scope(observed.speech.observer, true);
    implicit.disclosures.clear();
    assert_eq!(
        admit_conversation(implicit, speech(true, SpeechIntent::Claim), &observed).unwrap_err(),
        ConversationError::ImplicitDisclosure
    );
    for mutation in 0..4 {
        let mut scope = scope(observed.speech.observer, false);
        let expected = match mutation {
            0 => {
                scope.topic = content("other-event");
                ConversationError::TopicChanged
            }
            1 => {
                scope.speaker = entity(4);
                ConversationError::SpeakerUnavailable
            }
            2 => {
                scope.window.expires_at.ticks = 10;
                ConversationError::WindowExpired
            }
            3 => {
                scope.recipient = ObserverScope::Shared;
                ConversationError::RecipientChanged
            }
            _ => unreachable!(),
        };
        assert_eq!(
            admit_conversation(scope, speech(false, SpeechIntent::Claim), &observed).unwrap_err(),
            expected
        );
    }
    let mut absent = conversation_observation(&current, &versions, &owner, observer);
    absent.source_owner = None;
    assert_eq!(
        admit_conversation(
            scope(observer, false),
            speech(false, SpeechIntent::Claim),
            &absent
        )
        .unwrap_err(),
        ConversationError::SourceUnavailable
    );
    let mut stale = conversation_observation(&current, &versions, &owner, observer);
    stale.speech.basis.revision = SessionRevision::new(RecoveryEpoch::new(2).unwrap(), 3);
    assert_eq!(
        project_conversation(&plan, &stale).unwrap_err(),
        ConversationError::StalePlan
    );
    for intent in [SpeechIntent::ApprovedDeceit, SpeechIntent::FalseBelief] {
        let attributed =
            admit_conversation(scope(observer, false), speech(false, intent), &observed).unwrap();
        let context = project_conversation(&attributed, &observed).unwrap();
        assert_eq!(context.claims()[0].intent, intent);
        assert_eq!(context.claims()[0].holder, entity(3));
        assert_eq!(context.claims()[0].text, "Mira has 13 coins.");
        assert_eq!(
            context.claims()[0].uncertainty,
            DeclaredUncertainty::Explicit
        );
    }
    let expected: Vec<_> = context
        .claims()
        .iter()
        .map(|claim| (claim.id, claim.text.to_owned()))
        .collect();
    let mut hidden = boundary_state();
    hidden.beliefs[2].claim = "OTHER-HOST-COUNTERPART".to_owned();
    let hidden = checkpoint(hidden);
    let observed_hidden = conversation_observation(&hidden, &versions, &owner, observer);
    let hidden_plan = admit_conversation(
        scope(observer, true),
        speech(true, SpeechIntent::Claim),
        &observed_hidden,
    )
    .unwrap();
    let actual = project_conversation(&hidden_plan, &observed_hidden).unwrap();
    assert_eq!(
        actual
            .claims()
            .iter()
            .map(|claim| (claim.id, claim.text.to_owned()))
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(current, before);
    drop(context);
    drop(plan);
    assert_eq!(current, before);
}
