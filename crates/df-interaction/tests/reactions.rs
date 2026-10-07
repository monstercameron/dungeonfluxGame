use std::cell::Cell;

use df_interaction::reactions::{
    NoReactionReason, ReactionEntry, ReactionError, ReactionLimit, ReactionLimits, ReactionOutcome,
    ReactionPolicy, ReactionRequest, ReactionSourceOwner, ReactionSourceRefusal, react,
};
use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
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

#[test]
fn accepted_witnessed_event_proposes_exact_state_and_preserves_evidence_and_checkpoint() {
    let checkpoint = checkpoint(state());
    let before = checkpoint.clone();
    let first = run(&checkpoint, request(), &[entry()]).unwrap();
    assert_eq!(first, run(&checkpoint, request(), &[entry()]).unwrap());
    let ReactionOutcome::Proposed(proposal) = first else {
        panic!("expected reaction proposal");
    };
    assert_eq!(proposal.expected_basis, checkpoint.basis());
    assert_eq!(proposal.content_pins, checkpoint.pins().content);
    assert_eq!(proposal.original, checkpoint.state().relationships[0]);
    let mut expected = proposal.original.clone();
    expected.state = label("receptive");
    assert_eq!(proposal.proposed, expected);
    assert_eq!(proposal.policy_entry, entry());
    assert_eq!(proposal.event, checkpoint.state().facts[1].id);
    assert_eq!(proposal.cause, Some(fact_id(6)));
    assert_eq!(
        proposal.witness,
        checkpoint.state().continuity.witnesses[0].id
    );
    assert_eq!(
        proposal.accepted_operation,
        checkpoint.state().decisions[0].operation
    );
    assert_eq!(
        proposal.accepted_revision,
        checkpoint.state().decisions[0].revision
    );
    assert_eq!(checkpoint, before);
}

#[test]
fn ignorance_missing_or_wrong_witness_never_changes_relationships() {
    let mut unknown = state();
    unknown.continuity.npcs[0].known_facts.clear();
    let unknown = checkpoint(unknown);
    assert_eq!(
        run(&unknown, request(), &[entry()]),
        Ok(ReactionOutcome::NoReaction(NoReactionReason::NotKnown))
    );
    let mut unseen = state();
    unseen.continuity.witnesses[0].observer = entity(4);
    let unseen = checkpoint(unseen);
    assert_eq!(
        run(&unseen, request(), &[entry()]),
        Ok(ReactionOutcome::NoReaction(NoReactionReason::NotWitnessed))
    );
    let supplied = checkpoint(state());
    let mut missing = request();
    missing.witness = record(99);
    assert_eq!(
        run(&supplied, missing, &[entry()]),
        Ok(ReactionOutcome::NoReaction(NoReactionReason::NotWitnessed))
    );
}

#[test]
fn exact_basis_and_accepted_decision_are_required() {
    let supplied = checkpoint(state());
    let mut stale = request();
    stale.expected_basis.revision = SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 2);
    assert_eq!(
        run(&supplied, stale, &[entry()]),
        Err(ReactionError::StaleBasis)
    );
    let mut wrong_run = request();
    wrong_run.expected_basis.run = RunId::from_bytes(&[99; 16]).unwrap();
    assert_eq!(
        run(&supplied, wrong_run, &[entry()]),
        Err(ReactionError::StaleBasis)
    );
    let mut unaccepted = state();
    unaccepted.decisions.clear();
    assert_eq!(
        run(&checkpoint(unaccepted), request(), &[entry()]),
        Err(ReactionError::UnacceptedEvent)
    );
}

#[test]
fn reordered_accepted_event_membership_refuses_without_changing_checkpoint() {
    let mut supplied = state();
    supplied.decisions[0].facts.reverse();
    let supplied = checkpoint(supplied);
    let before = supplied.clone();

    assert_eq!(
        run(&supplied, request(), &[entry()]),
        Err(ReactionError::UnacceptedEvent)
    );
    assert_eq!(
        run(&supplied, request(), &[entry()]),
        Err(ReactionError::UnacceptedEvent)
    );
    assert_eq!(supplied, before);
}

#[test]
fn sparse_accepted_event_membership_cannot_replace_its_canonical_ordinal() {
    let mut exact = state();
    exact.facts[1].cause = None;
    let exact = checkpoint(exact);
    let exact_before = exact.clone();
    let valid = run(&exact, request(), &[entry()]).unwrap();
    let ReactionOutcome::Proposed(proposal) = &valid else {
        panic!("expected exact-ordinal event proposal");
    };
    assert_eq!(proposal.event, fact_id(7));
    assert_eq!(proposal.cause, None);
    assert_eq!(valid, run(&exact, request(), &[entry()]).unwrap());
    assert_eq!(exact, exact_before);

    let mut sparse = exact.state().clone();
    sparse.decisions[0].facts = vec![fact_id(7)];
    let sparse = checkpoint(sparse);
    let sparse_before = sparse.clone();
    assert_eq!(
        run(&sparse, request(), &[entry()]),
        Err(ReactionError::UnacceptedEvent)
    );
    assert_eq!(sparse, sparse_before);
}

#[test]
fn exact_event_membership_does_not_admit_a_reordered_direct_cause() {
    let mut exact = state();
    let mut filler = exact.facts[0].clone();
    filler.id = fact_id(9);
    exact.facts[0].ordinal = 1;
    exact.facts[1].ordinal = 2;
    exact.facts.insert(0, filler);
    exact.decisions[0].facts = vec![fact_id(9), fact_id(6), fact_id(7)];
    let exact = checkpoint(exact);
    let exact_before = exact.clone();
    let valid = run(&exact, request(), &[entry()]).unwrap();
    let ReactionOutcome::Proposed(proposal) = &valid else {
        panic!("expected exact-ordinal cause proposal");
    };
    assert_eq!(proposal.event, fact_id(7));
    assert_eq!(proposal.cause, Some(fact_id(6)));
    assert_eq!(valid, run(&exact, request(), &[entry()]).unwrap());
    assert_eq!(exact, exact_before);

    let mut reordered = exact.state().clone();
    reordered.decisions[0].facts.swap(0, 1);
    let reordered = checkpoint(reordered);
    let reordered_before = reordered.clone();
    assert_eq!(
        reordered.state().decisions[0].facts[2],
        reordered.state().facts[2].id
    );
    assert_eq!(
        run(&reordered, request(), &[entry()]),
        Err(ReactionError::UnacceptedEvent)
    );
    assert_eq!(reordered, reordered_before);
}

#[test]
fn authored_identity_motivation_event_and_state_must_match_without_fallback() {
    let supplied = checkpoint(state());
    for change in 0..4 {
        let mut policy = entry();
        match change {
            0 => policy.personality = content("other-personality"),
            1 => policy.motivation = content("other-motivation"),
            2 => policy.event = content("other-event"),
            _ => policy.from_state = label("other-state"),
        }
        assert_eq!(
            run(&supplied, request(), &[policy]),
            Ok(ReactionOutcome::NoReaction(
                NoReactionReason::NoMatchingPolicy
            ))
        );
    }
    let mut unchanged = entry();
    unchanged.to_state = unchanged.from_state.clone();
    assert_eq!(
        run(&supplied, request(), &[unchanged]),
        Ok(ReactionOutcome::NoReaction(
            NoReactionReason::UnchangedState
        ))
    );
}

#[test]
fn ambiguous_matches_refuse_in_both_orders_and_preserve_input() {
    let supplied = checkpoint(state());
    let before = supplied.clone();
    let first = entry();
    let mut second = entry();
    second.to_state = label("concerned");
    assert_eq!(
        run(&supplied, request(), &[first.clone(), second.clone()]),
        Err(ReactionError::AmbiguousPolicy)
    );
    assert_eq!(
        run(&supplied, request(), &[second, first]),
        Err(ReactionError::AmbiguousPolicy)
    );
    assert_eq!(supplied, before);
    let mut duplicate_direction = state();
    let mut conflicting = duplicate_direction.relationships[0].clone();
    conflicting.state = label("concerned");
    duplicate_direction.relationships.push(conflicting);
    let original = duplicate_direction.clone();
    for reverse in [false, true] {
        let mut supplied = duplicate_direction.clone();
        if reverse {
            supplied.relationships.reverse();
        }
        let before = supplied.clone();
        assert_eq!(
            try_checkpoint_with_pins(supplied.clone(), pins()),
            Err(CheckpointError::DuplicateIdentity)
        );
        assert_eq!(supplied, before);
    }
    assert_eq!(duplicate_direction, original);
}

#[test]
fn policy_pin_and_capacity_refusals_precede_private_lookup() {
    let supplied = checkpoint(state());
    let entries = [entry()];
    let admitted = inventory();
    let owner = source_owner(&entries);
    let policy = ReactionPolicy::new(
        &supplied,
        &entries,
        ReferenceInventory {
            rules: &[],
            content: &admitted,
            resources: &[],
            assets: &[],
        },
        &owner,
        limits(),
    )
    .unwrap();
    let mut changed_pins = pins();
    changed_pins.content.content_digest = ContentDigest([99; 32]);
    let changed = checkpoint_with_pins(state(), changed_pins);
    assert_eq!(
        react(&changed, request(), &policy),
        Err(ReactionError::ContentMismatch)
    );
    for (mut bounded, expected) in [
        (limits(), ReactionLimit::Entries),
        (limits(), ReactionLimit::PolicyBytes),
        (limits(), ReactionLimit::Work),
    ] {
        match expected {
            ReactionLimit::Entries => bounded.maximum_entries = 0,
            ReactionLimit::PolicyBytes => bounded.maximum_policy_bytes = 0,
            ReactionLimit::Work => bounded.maximum_work = 0,
            _ => unreachable!(),
        }
        assert!(matches!(ReactionPolicy::new(&supplied, &entries,
            ReferenceInventory { rules: &[], content: &admitted, resources: &[], assets: &[] },
            &owner, bounded), Err(ReactionError::LimitExceeded(limit)) if limit == expected));
    }
    let mut wrong_package = entry();
    wrong_package.source.package = label("unadmitted-package");
    assert_eq!(
        run(&supplied, request(), &[wrong_package]),
        Err(ReactionError::SourceUnavailable)
    );
    assert_eq!(
        run(&supplied, request(), &vec![entry(); 5]),
        Err(ReactionError::LimitExceeded(ReactionLimit::Entries))
    );
}

#[test]
fn wrong_direction_unrelated_subject_and_future_perception_cannot_react() {
    let mut reversed = state();
    reversed.relationships[0].subject = entity(4);
    reversed.relationships[0].object = entity(3);
    assert_eq!(
        run(&checkpoint(reversed), request(), &[entry()]),
        Err(ReactionError::UnknownRelationship)
    );
    let supplied = checkpoint(state());
    let mut unrelated = request();
    unrelated.target = entity(3);
    assert_eq!(
        run(&supplied, unrelated, &[entry()]),
        Ok(ReactionOutcome::NoReaction(
            NoReactionReason::UnrelatedTarget
        ))
    );
    let mut future = state();
    future.continuity.witnesses[0].perceived_at.ticks = 11;
    assert_eq!(
        run(&checkpoint(future), request(), &[entry()]),
        Err(ReactionError::InvalidPerceptionTime)
    );
}

#[test]
fn source_inventory_presence_does_not_grant_authored_policy_or_rights() {
    let supplied = checkpoint(state());
    let entries = [entry()];
    let admitted = inventory();
    let owner = source_owner(&entries);
    for refusal in [
        ReactionSourceRefusal::NotAdmitted,
        ReactionSourceRefusal::AccessDenied,
        ReactionSourceRefusal::UnsupportedPolicy,
        ReactionSourceRefusal::StaleAdmission,
    ] {
        owner.refusal.set(Some(refusal));
        assert!(matches!(ReactionPolicy::new(&supplied, &entries,
            ReferenceInventory { rules: &[], content: &admitted, resources: &[], assets: &[] },
            &owner, limits()), Err(ReactionError::SourceRefused(actual)) if actual == refusal));
    }
    let unapproved = source_owner(&[]);
    assert!(matches!(
        ReactionPolicy::new(
            &supplied,
            &entries,
            ReferenceInventory {
                rules: &[],
                content: &admitted,
                resources: &[],
                assets: &[]
            },
            &unapproved,
            limits()
        ),
        Err(ReactionError::SourceRefused(
            ReactionSourceRefusal::NotAdmitted
        ))
    ));
    let mut missing = inventory();
    missing.retain(|reference| reference != &entries[0].source);
    owner.refusal.set(None);
    assert!(matches!(
        ReactionPolicy::new(
            &supplied,
            &entries,
            ReferenceInventory {
                rules: &[],
                content: &missing,
                resources: &[],
                assets: &[]
            },
            &owner,
            limits()
        ),
        Err(ReactionError::SourceUnavailable)
    ));
}

#[test]
fn native_source_revocation_after_construction_refuses_without_partial_output() {
    let supplied = checkpoint(state());
    let before = supplied.clone();
    let entries = [entry()];
    let admitted = inventory();
    let owner = source_owner(&entries);
    let policy = ReactionPolicy::new(
        &supplied,
        &entries,
        ReferenceInventory {
            rules: &[],
            content: &admitted,
            resources: &[],
            assets: &[],
        },
        &owner,
        limits(),
    )
    .unwrap();
    assert!(matches!(
        react(&supplied, request(), &policy),
        Ok(ReactionOutcome::Proposed(_))
    ));
    owner.refusal.set(Some(ReactionSourceRefusal::AccessDenied));
    assert_eq!(
        react(&supplied, request(), &policy),
        Err(ReactionError::SourceRefused(
            ReactionSourceRefusal::AccessDenied
        ))
    );
    assert_eq!(supplied, before);
}

#[test]
fn proposal_budget_counts_both_relationships_and_all_axis_provenance_before_cloning() {
    let mut supplied_state = state();
    let axis_sources: Vec<_> = (0..7)
        .map(|index| content(&format!("axis-{index}-{}", "s".repeat(104))))
        .collect();
    let relationship = &mut supplied_state.relationships[0];
    for (index, axis) in [
        &mut relationship.trust,
        &mut relationship.affection,
        &mut relationship.respect,
        &mut relationship.fear,
        &mut relationship.suspicion,
        &mut relationship.debt,
        &mut relationship.familiarity,
    ]
    .into_iter()
    .enumerate()
    {
        axis.value = label(&format!("value-{index}-{}", "v".repeat(104)));
        axis.provenance = if index == 3 {
            RelationshipAxisProvenance::AcceptedFact {
                source: axis_sources[index].clone(),
                fact: fact_id(7),
                source_policy: label("accepted-policy-v1"),
                witness: Some(record(8)),
            }
        } else {
            RelationshipAxisProvenance::AuthoredBaseline {
                source: axis_sources[index].clone(),
            }
        };
    }
    let mut admitted = inventory();
    admitted.extend(axis_sources.iter().cloned());
    let supplied = Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis(),
        pins(),
        supplied_state,
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
    .unwrap();
    let before = supplied.clone();
    let entries = [entry()];
    let owner = source_owner(&entries);
    let mut bounded = limits();
    // This cap fits the old policy/state-only estimate but not two cloned seven-axis records.
    bounded.maximum_proposal_bytes = 4096;
    let policy = ReactionPolicy::new(
        &supplied,
        &entries,
        ReferenceInventory {
            rules: &[],
            content: &admitted,
            resources: &[],
            assets: &[],
        },
        &owner,
        bounded,
    )
    .unwrap();
    assert_eq!(
        react(&supplied, request(), &policy),
        Err(ReactionError::LimitExceeded(ReactionLimit::ProposalBytes))
    );
    assert_eq!(supplied, before);

    bounded.maximum_proposal_bytes = 16 * 1024;
    let policy = ReactionPolicy::new(
        &supplied,
        &entries,
        ReferenceInventory {
            rules: &[],
            content: &admitted,
            resources: &[],
            assets: &[],
        },
        &owner,
        bounded,
    )
    .unwrap();
    let ReactionOutcome::Proposed(proposal) = react(&supplied, request(), &policy).unwrap() else {
        panic!("sufficient proposal budget must preserve the full relationship");
    };
    let mut expected = before.state().relationships[0].clone();
    assert_eq!(proposal.original, expected);
    expected.state = label("receptive");
    assert_eq!(proposal.proposed, expected);
    assert_eq!(supplied, before);
}

#[test]
fn fixed_fixture_work_and_output_bounds_fail_instead_of_truncating_evidence() {
    let supplied = checkpoint(state());
    let entries = [entry()];
    let admitted = inventory();
    let owner = source_owner(&entries);
    let mut bounded = limits();
    // Eleven admitted references, five entry slots and one entry cost 56 units.
    bounded.maximum_work = 55;
    assert!(matches!(
        ReactionPolicy::new(
            &supplied,
            &entries,
            ReferenceInventory {
                rules: &[],
                content: &admitted,
                resources: &[],
                assets: &[]
            },
            &owner,
            bounded
        ),
        Err(ReactionError::LimitExceeded(ReactionLimit::Work))
    ));
    bounded.maximum_work = 56;
    let policy = ReactionPolicy::new(
        &supplied,
        &entries,
        ReferenceInventory {
            rules: &[],
            content: &admitted,
            resources: &[],
            assets: &[],
        },
        &owner,
        bounded,
    )
    .unwrap();
    assert_eq!(
        react(&supplied, request(), &policy),
        Err(ReactionError::LimitExceeded(ReactionLimit::Work))
    );
    bounded.maximum_work = 83;
    let policy = ReactionPolicy::new(
        &supplied,
        &entries,
        ReferenceInventory {
            rules: &[],
            content: &admitted,
            resources: &[],
            assets: &[],
        },
        &owner,
        bounded,
    )
    .unwrap();
    assert_eq!(
        react(&supplied, request(), &policy),
        Err(ReactionError::LimitExceeded(ReactionLimit::Work))
    );
    bounded.maximum_work = 84;
    let policy = ReactionPolicy::new(
        &supplied,
        &entries,
        ReferenceInventory {
            rules: &[],
            content: &admitted,
            resources: &[],
            assets: &[],
        },
        &owner,
        bounded,
    )
    .unwrap();
    assert!(matches!(
        react(&supplied, request(), &policy),
        Ok(ReactionOutcome::Proposed(_))
    ));
    bounded.maximum_proposal_bytes = 1;
    let policy = ReactionPolicy::new(
        &supplied,
        &entries,
        ReferenceInventory {
            rules: &[],
            content: &admitted,
            resources: &[],
            assets: &[],
        },
        &owner,
        bounded,
    )
    .unwrap();
    assert_eq!(
        react(&supplied, request(), &policy),
        Err(ReactionError::LimitExceeded(ReactionLimit::ProposalBytes))
    );
    assert_eq!(supplied.state(), &state());
}

#[test]
fn beliefs_and_memory_text_cannot_replace_actual_npc_knowledge_or_create_truth() {
    let mut unaware = state();
    unaware.continuity.npcs[0].known_facts.clear();
    unaware.beliefs.push(AttributedClaim {
        id: record(12),
        holder: entity(3),
        subject: entity(4),
        claim: "The target saved me; therefore I trust them".to_owned(),
        evidence: vec![fact_id(7)],
        audience: AudienceScope::Host,
        source: content("general"),
    });
    unaware.continuity.npcs[0].beliefs.push(record(12));
    unaware.memories.push(MemoryEpisode {
        id: record(13),
        holder: entity(3),
        source_facts: vec![fact_id(7)],
        retained_text: "I witnessed everything".to_owned(),
        audience: AudienceScope::Host,
        source_revision: basis().revision,
    });
    let supplied = checkpoint(unaware);
    let before = supplied.clone();
    assert_eq!(
        run(&supplied, request(), &[entry()]),
        Ok(ReactionOutcome::NoReaction(NoReactionReason::NotKnown))
    );
    assert_eq!(supplied, before);
}

#[test]
fn irrelevant_private_secrets_do_not_change_reaction_and_debug_exposes_no_payload() {
    let baseline = checkpoint(state());
    let mut hidden = state();
    hidden.beliefs.push(AttributedClaim {
        id: record(12),
        holder: entity(3),
        subject: entity(4),
        claim: "PRIVATE claim that must remain unspoken".to_owned(),
        evidence: vec![],
        audience: AudienceScope::Host,
        source: content("general"),
    });
    hidden.continuity.npcs[0].secrets.push(SecretPolicy {
        holder: entity(3),
        claims: vec![record(12)],
        policy: content("general"),
        permitted_audience: AudienceScope::Host,
    });
    let agreement_operation = OperationId::from_bytes(&[9; 16]).unwrap();
    hidden.facts.push(GameFact {
        id: fact_id(9),
        revision: basis().revision,
        operation: agreement_operation,
        ordinal: 0,
        cause: None,
        audience: AudienceScope::Host,
        value: FactValue::ContentEvent {
            definition: content("general"),
            subjects: vec![entity(3), entity(4)],
        },
    });
    hidden.decisions.push(AcceptedDecision {
        operation: agreement_operation,
        revision: basis().revision,
        facts: vec![fact_id(9)],
        draws: vec![],
        effects: vec![],
        source_policy: label("explicit-agreement-policy"),
        semantic_output: None,
    });
    hidden.obligations.push(Obligation {
        id: record(14),
        obligor: entity(3),
        beneficiary: entity(4),
        definition: content("general"),
        terms: content("general"),
        due: Some(hidden.logical_time),
        agreement: ObligationAgreement {
            source: content("general"),
            fact: fact_id(9),
            source_policy: label("explicit-agreement-policy"),
            at: hidden.logical_time,
        },
        status: ObligationStatus::Active,
        transition: None,
    });
    let hidden = checkpoint(hidden);
    let before = hidden.clone();
    let result = run(&hidden, request(), &[entry()]).unwrap();
    assert_eq!(result, run(&baseline, request(), &[entry()]).unwrap());
    let diagnostic = format!("{result:?}");
    for private in [
        "PRIVATE",
        "reserved",
        "receptive",
        "reaction-v1",
        "accepted-help-event",
    ] {
        assert!(!diagnostic.contains(private));
    }
    assert_eq!(hidden, before);
}

#[test]
fn supported_categorical_event_is_not_inferred_from_other_committed_fact_families() {
    let mut unsupported = state();
    unsupported.facts[1].value = FactValue::EntityMoved {
        entity: entity(4),
        destination: entity(3),
        position: Position { x: 0, y: 0, z: 0 },
    };
    let supplied = checkpoint(unsupported);
    assert_eq!(
        run(&supplied, request(), &[entry()]),
        Ok(ReactionOutcome::NoReaction(
            NoReactionReason::UnsupportedEvent
        ))
    );
    let mut unlisted = state();
    unlisted.decisions[0].facts = vec![fact_id(6)];
    assert_eq!(
        run(&checkpoint(unlisted), request(), &[entry()]),
        Err(ReactionError::UnacceptedEvent)
    );
    let mut unaccepted_cause = state();
    unaccepted_cause.decisions[0].facts = vec![fact_id(7)];
    assert_eq!(
        run(&checkpoint(unaccepted_cause), request(), &[entry()]),
        Err(ReactionError::UnacceptedEvent)
    );
}

#[test]
fn missing_npc_event_and_directional_relationship_have_typed_refusals() {
    let supplied = checkpoint(state());
    let mut missing_npc = request();
    missing_npc.npc = entity(99);
    assert_eq!(
        run(&supplied, missing_npc, &[entry()]),
        Err(ReactionError::UnknownNpc)
    );
    let mut missing_event = request();
    missing_event.event = fact_id(99);
    assert_eq!(
        run(&supplied, missing_event, &[entry()]),
        Err(ReactionError::UnknownEvent)
    );
    let mut missing_relation = state();
    missing_relation.relationships.clear();
    assert_eq!(
        run(&checkpoint(missing_relation), request(), &[entry()]),
        Err(ReactionError::UnknownRelationship)
    );
    let mut wrong_policy = entry();
    wrong_policy.relationship_policy = content("general");
    assert_eq!(
        run(&supplied, request(), &[wrong_policy]),
        Ok(ReactionOutcome::NoReaction(
            NoReactionReason::NoMatchingPolicy
        ))
    );
}

#[test]
fn source_basis_and_witness_event_observer_and_units_remain_exact() {
    let supplied = checkpoint(state());
    let entries = [entry()];
    let admitted = inventory();
    let mut owner = source_owner(&entries);
    owner.basis.revision = SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 2);
    assert!(matches!(
        ReactionPolicy::new(
            &supplied,
            &entries,
            ReferenceInventory {
                rules: &[],
                content: &admitted,
                resources: &[],
                assets: &[]
            },
            &owner,
            limits()
        ),
        Err(ReactionError::SourceRefused(
            ReactionSourceRefusal::StaleAdmission
        ))
    ));
    let mut wrong_event = state();
    wrong_event.continuity.witnesses[0].fact = fact_id(6);
    assert_eq!(
        run(&checkpoint(wrong_event), request(), &[entry()]),
        Ok(ReactionOutcome::NoReaction(NoReactionReason::NotWitnessed))
    );
    let mut units = state();
    units.continuity.witnesses[0].perceived_at.ticks_per_second = 2;
    assert_eq!(
        run(&checkpoint(units), request(), &[entry()]),
        Err(ReactionError::InvalidPerceptionTime)
    );
    let mut duplicate = state();
    duplicate.relationships.push(Relationship {
        subject: entity(4),
        object: entity(3),
        policy: content("relationship-policy"),
        state: label("private-inverse-state"),
        trust: RelationshipAxisState {
            value: label("private-inverse-state"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline {
                source: content("relationship-policy"),
            },
        },
        affection: RelationshipAxisState {
            value: label("private-inverse-state"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline {
                source: content("relationship-policy"),
            },
        },
        respect: RelationshipAxisState {
            value: label("private-inverse-state"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline {
                source: content("relationship-policy"),
            },
        },
        fear: RelationshipAxisState {
            value: label("private-inverse-state"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline {
                source: content("relationship-policy"),
            },
        },
        suspicion: RelationshipAxisState {
            value: label("private-inverse-state"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline {
                source: content("relationship-policy"),
            },
        },
        debt: RelationshipAxisState {
            value: label("private-inverse-state"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline {
                source: content("relationship-policy"),
            },
        },
        familiarity: RelationshipAxisState {
            value: label("private-inverse-state"),
            provenance: RelationshipAxisProvenance::AuthoredBaseline {
                source: content("relationship-policy"),
            },
        },
    });
    let duplicate = checkpoint(duplicate);
    let before = duplicate.clone();
    let ReactionOutcome::Proposed(proposal) = run(&duplicate, request(), &[entry()]).unwrap()
    else {
        panic!("expected directional proposal");
    };
    assert_eq!(proposal.original, duplicate.state().relationships[0]);
    assert_eq!(proposal.proposed.subject, entity(3));
    assert_eq!(proposal.proposed.object, entity(4));
    assert_eq!(duplicate, before);
}
