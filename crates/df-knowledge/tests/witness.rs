use df_knowledge::witness::{
    WitnessEligibility, WitnessError, WitnessLimits, WitnessRoute, propose_witness_grants,
};
use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId,
    SessionRevision,
};

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}
fn member(value: u8) -> MemberId {
    MemberId::from_bytes(&[value; 16]).unwrap()
}
fn entity(value: u8) -> EntityId {
    EntityId::from_bytes(&[value; 16]).unwrap()
}
fn record(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}
fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(2).unwrap(), 8),
    }
}
fn policy() -> ContentReference {
    ContentReference {
        package: label("witness-fixture-1"),
        entry: label("admitted-contact-policy-1"),
    }
}
fn other_policy() -> ContentReference {
    ContentReference {
        package: label("witness-fixture-1"),
        entry: label("other-admitted-policy-1"),
    }
}
fn pins() -> CheckpointPins {
    CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: label("fixture-rules-1"),
            catalog: label("fixture-catalog-1"),
            catalog_digest: ContentDigest([1; 32]),
            source_manifest: label("fixture-source-1"),
            source_manifest_digest: ContentDigest([2; 32]),
            handler: label("fixture-handler-1"),
            handler_digest: ContentDigest([3; 32]),
        },
        content: ContentPins {
            content: label("witness-fixture-1"),
            content_digest: ContentDigest([4; 32]),
            package: label("witness-fixture-1"),
            package_digest: ContentDigest([5; 32]),
        },
        build: BuildIdentity::new(
            Some("source-1"),
            Some("native-1"),
            Some("wasm-1"),
            Some("config-1"),
            Some("witness-fixture-1"),
        )
        .unwrap(),
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
fn state() -> GameState {
    let operation = OperationId::from_bytes(&[7; 16]).unwrap();
    let facts = [
        AudienceScope::Shared,
        AudienceScope::Members(vec![member(3)]),
        AudienceScope::Host,
    ]
    .into_iter()
    .enumerate()
    .map(|(ordinal, audience)| GameFact {
        id: FactId::from_bytes(&[10 + ordinal as u8; 16]).unwrap(),
        revision: basis().revision,
        operation,
        ordinal: ordinal as u32,
        cause: None,
        audience,
        value: FactValue::ContentEvent {
            definition: policy(),
            subjects: vec![entity(4)],
        },
    })
    .collect::<Vec<_>>();
    let mut continuity = continuity();
    continuity.witnesses = facts
        .iter()
        .enumerate()
        .map(|(index, fact)| WitnessRecord {
            id: record(40 + index as u8),
            observer: entity(4),
            fact: fact.id,
            perceived_at: LogicalTime {
                ticks: 120,
                ticks_per_second: 10,
            },
            source: policy(),
        })
        .collect();
    GameState {
        mode: ExecutionMode::Replay,
        logical_time: LogicalTime {
            ticks: 120,
            ticks_per_second: 10,
        },
        members: vec![
            MembershipLink {
                member: member(3),
                character: Some(entity(4)),
            },
            MembershipLink {
                member: member(5),
                character: Some(entity(6)),
            },
        ],
        entities: [4, 6]
            .into_iter()
            .map(|value| WorldEntity {
                id: entity(value),
                definition: policy(),
                location: None,
                position: None,
                identity_revision: label("entity-1"),
            })
            .collect(),
        characters: vec![],
        resources: vec![],
        inventory: vec![],
        draws: vec![],
        decisions: vec![AcceptedDecision {
            operation,
            revision: basis().revision,
            facts: facts.iter().map(|fact| fact.id).collect(),
            draws: vec![],
            effects: vec![],
            source_policy: label("accepted-world-source-1"),
            semantic_output: None,
        }],
        facts,
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
            subject: entity(4),
            object: entity(6),
            policy: policy(),
            state: label("source-contact-allowed"),
        }],
        conversations: vec![],
        obligations: vec![],
        narrative: NarrativeState {
            definition: policy(),
            active_beats: vec![],
            completed_beats: vec![],
            open_threads: vec![],
            accepted_facts: vec![],
            remaining_budget: 0,
        },
        encounters: vec![],
        activity: vec![],
        tempo: TempoState {
            policy: policy(),
            presentation_ticks: 0,
            intensity: 0,
            inertia: 0,
            fatigue: vec![],
        },
        presentation: vec![],
        continuity,
    }
}
fn checkpoint_at(basis: Basis, state: GameState) -> Checkpoint {
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis,
        pins(),
        state,
        ReferenceInventory {
            rules: &[],
            content: &[policy(), other_policy()],
            resources: &[],
            assets: &[],
        },
        CheckpointLimits {
            maximum_records: 256,
            maximum_text_bytes: 256,
            maximum_total_text_bytes: 8192,
            maximum_retained_bytes: 1024 * 1024,
        },
    )
    .unwrap()
}
fn checkpoint(state: GameState) -> Checkpoint {
    checkpoint_at(basis(), state)
}
fn limits() -> WitnessLimits {
    WitnessLimits {
        maximum_candidates: 16,
        maximum_scan_records: 256,
        maximum_record_comparisons: 256,
        maximum_grants: 16,
        maximum_grant_bytes: 16 * std::mem::size_of::<KnowledgeGrant>(),
    }
}
fn direct(witness: u8) -> WitnessEligibility<'static> {
    WitnessEligibility {
        witness: record(witness),
        recipient: member(3),
        route: WitnessRoute::Sight,
    }
}
fn grants(
    current: &Checkpoint,
    eligible: &[WitnessEligibility<'_>],
) -> Result<Vec<KnowledgeGrant>, WitnessError> {
    propose_witness_grants(
        current,
        current.basis(),
        &pins(),
        &policy(),
        eligible,
        limits(),
    )
    .map(|proposal| proposal.grants().to_vec())
}

#[test]
fn actual_current_witnesses_propose_ordered_direct_sight_and_hearing_grants_without_mutation() {
    let current = checkpoint(state());
    let before = current.clone();
    let eligible = [
        direct(41),
        WitnessEligibility {
            route: WitnessRoute::Hearing,
            ..direct(40)
        },
        direct(41),
    ];
    let proposal = propose_witness_grants(
        &current,
        current.basis(),
        &pins(),
        &policy(),
        &eligible,
        limits(),
    )
    .unwrap();
    assert_eq!(proposal.basis(), current.basis());
    assert!(std::ptr::eq(proposal.pins(), current.pins()));
    assert_eq!(
        proposal.grants(),
        &[
            KnowledgeGrant {
                observer: member(3),
                fact: current.state().facts[1].id,
                source: current.state().facts[1].id
            },
            KnowledgeGrant {
                observer: member(3),
                fact: current.state().facts[0].id,
                source: current.state().facts[0].id
            },
        ]
    );
    assert_eq!(current, before);
    assert!(grants(&current, &[]).unwrap().is_empty());
    assert!(!format!("{proposal:?}").contains("admitted-contact-policy"));
}

#[test]
fn generated_grants_rebuild_a_valid_checkpoint_candidate_and_retry_is_a_no_op() {
    let current = checkpoint(state());
    let before = current.clone();
    let accepted = grants(&current, &[direct(40), direct(41)]).unwrap();
    let mut candidate_state = current.state().clone();
    candidate_state.knowledge.extend(accepted.clone());
    let mut next_basis = current.basis();
    next_basis.revision = next_basis.revision.next_sequence().unwrap();
    let candidate = checkpoint_at(next_basis, candidate_state.clone());
    assert_eq!(candidate.state(), &candidate_state);
    assert_eq!(candidate.state().facts, current.state().facts);
    assert_eq!(
        candidate.state().continuity.witnesses,
        current.state().continuity.witnesses
    );
    assert_eq!(
        candidate.state().relationships,
        current.state().relationships
    );
    assert_eq!(candidate.state().decisions, current.state().decisions);
    assert!(
        grants(&candidate, &[direct(40), direct(41)])
            .unwrap()
            .is_empty()
    );
    assert_eq!(current, before);
}

#[test]
fn indirect_transmission_requires_actual_direction_policy_state_and_supplied_contact_eligibility() {
    let current = checkpoint(state());
    let permitted = label("source-contact-allowed");
    let eligible = WitnessEligibility {
        witness: record(40),
        recipient: member(5),
        route: WitnessRoute::Relationship {
            permitted_state: &permitted,
        },
    };
    assert_eq!(
        grants(&current, &[eligible]).unwrap(),
        vec![KnowledgeGrant {
            observer: member(5),
            fact: current.state().facts[0].id,
            source: current.state().facts[0].id
        }]
    );
    for changed in 0..4 {
        let mut supplied = state();
        match changed {
            0 => supplied.relationships.clear(),
            1 => {
                supplied.relationships[0].subject = entity(6);
                supplied.relationships[0].object = entity(4);
            }
            2 => supplied.relationships[0].state = label("source-contact-denied"),
            _ => supplied.relationships[0].policy = other_policy(),
        }
        assert_eq!(
            grants(&checkpoint(supplied), &[eligible]),
            Err(WitnessError::IneligibleRoute)
        );
    }
    assert!(grants(&current, &[]).unwrap().is_empty());
    let wrong_direct = WitnessEligibility {
        route: WitnessRoute::Hearing,
        ..eligible
    };
    assert_eq!(
        grants(&current, &[wrong_direct]),
        Err(WitnessError::IneligibleRoute)
    );
}

#[test]
fn host_private_other_and_removed_current_audience_cannot_be_widened_by_existing_grants() {
    let mut supplied = state();
    supplied.knowledge.push(KnowledgeGrant {
        observer: member(5),
        fact: supplied.facts[1].id,
        source: supplied.facts[1].id,
    });
    let current = checkpoint(supplied);
    let permitted = label("source-contact-allowed");
    let private = WitnessEligibility {
        witness: record(41),
        recipient: member(5),
        route: WitnessRoute::Relationship {
            permitted_state: &permitted,
        },
    };
    assert_eq!(
        grants(&current, &[private]),
        Err(WitnessError::AudienceDenied)
    );
    assert_eq!(
        grants(&current, &[direct(42)]),
        Err(WitnessError::AudienceDenied)
    );
    let mut removed = current.state().clone();
    removed.facts[1].audience = AudienceScope::Host;
    assert_eq!(
        grants(&checkpoint(removed), &[direct(41)]),
        Err(WitnessError::AudienceDenied)
    );
    let mut shared = current.state().clone();
    shared.facts[1].audience = AudienceScope::Members(vec![member(3), member(5)]);
    shared.knowledge.clear();
    assert_eq!(grants(&checkpoint(shared), &[private]).unwrap().len(), 1);
}

#[test]
fn missing_member_character_witness_or_accepted_cause_is_a_typed_refusal() {
    let current = checkpoint(state());
    assert_eq!(
        grants(
            &current,
            &[WitnessEligibility {
                recipient: member(99),
                ..direct(40)
            }]
        ),
        Err(WitnessError::ObserverUnavailable)
    );
    assert_eq!(
        grants(&current, &[direct(99)]),
        Err(WitnessError::UnknownWitness)
    );
    let mut unbound = state();
    unbound.members[0].character = None;
    assert_eq!(
        grants(&checkpoint(unbound), &[direct(40)]),
        Err(WitnessError::ObserverUnavailable)
    );
    let mut removed = state();
    removed.members.remove(0);
    removed.facts[1].audience = AudienceScope::Host;
    assert_eq!(
        grants(&checkpoint(removed), &[direct(40)]),
        Err(WitnessError::ObserverUnavailable)
    );
    for omit_decision in [false, true] {
        let mut unaccepted = state();
        if omit_decision {
            unaccepted.decisions.clear();
        } else {
            unaccepted.decisions[0].facts.remove(0);
        }
        assert_eq!(
            grants(&checkpoint(unaccepted), &[direct(40)]),
            Err(WitnessError::MissingAcceptedCause)
        );
    }
    // Canonical construction rejects a mismatched decision retaining this fact
    // ID. Structurally valid unrelated decisions still cannot accept the fact.
    for wrong_operation in [false, true] {
        let mut unrelated = state();
        unrelated.decisions[0].facts.clear();
        if wrong_operation {
            unrelated.decisions[0].operation = OperationId::from_bytes(&[99; 16]).unwrap();
        } else {
            unrelated.decisions[0].revision =
                SessionRevision::new(RecoveryEpoch::new(2).unwrap(), 7);
        }
        assert_eq!(
            grants(&checkpoint(unrelated), &[direct(40)]),
            Err(WitnessError::MissingAcceptedCause)
        );
    }
}

#[test]
fn current_witness_time_source_and_character_rebinding_are_revalidated() {
    for changed in 0..3 {
        let mut supplied = state();
        match changed {
            0 => supplied.continuity.witnesses[0].perceived_at.ticks = 121,
            1 => {
                supplied.continuity.witnesses[0]
                    .perceived_at
                    .ticks_per_second = 1
            }
            _ => {
                supplied.members[0].character = Some(entity(6));
                supplied.members[1].character = Some(entity(4));
            }
        }
        assert_eq!(
            grants(&checkpoint(supplied), &[direct(40)]),
            Err(if changed == 2 {
                WitnessError::IneligibleRoute
            } else {
                WitnessError::InvalidWitnessTime
            })
        );
    }
    let current = checkpoint(state());
    let mut wrong_policy = policy();
    wrong_policy.entry = label("not-the-admitted-witness-source");
    assert_eq!(
        propose_witness_grants(
            &current,
            current.basis(),
            &pins(),
            &wrong_policy,
            &[direct(40)],
            limits()
        )
        .unwrap_err(),
        WitnessError::SourceMismatch
    );
    let mut changed_source = state();
    changed_source.continuity.witnesses[0].source = other_policy();
    assert_eq!(
        grants(&checkpoint(changed_source), &[direct(40)]),
        Err(WitnessError::SourceMismatch)
    );
}

#[test]
fn every_current_basis_component_and_full_pin_family_is_required() {
    let current = checkpoint(state());
    for changed in 0..4 {
        let mut expected = basis();
        let error = match changed {
            0 => {
                expected.session = SessionId::from_bytes(&[99; 16]).unwrap();
                CheckpointError::WrongSession
            }
            1 => {
                expected.run = RunId::from_bytes(&[99; 16]).unwrap();
                CheckpointError::WrongRun
            }
            2 => {
                expected.revision = SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 8);
                CheckpointError::StaleBasis
            }
            _ => {
                expected.revision = expected.revision.next_sequence().unwrap();
                CheckpointError::StaleBasis
            }
        };
        assert_eq!(
            propose_witness_grants(
                &current,
                expected,
                &pins(),
                &policy(),
                &[direct(40)],
                limits()
            )
            .unwrap_err(),
            WitnessError::Checkpoint(error)
        );
    }
    for changed in 0..12 {
        let mut admitted = pins();
        let error = match changed {
            0..=7 => {
                match changed {
                    0 => admitted.rules.mode = RulesMode::DisclosedCustom,
                    1 => admitted.rules.ruleset = label("changed"),
                    2 => admitted.rules.catalog = label("changed"),
                    3 => admitted.rules.catalog_digest.0[0] ^= 1,
                    4 => admitted.rules.source_manifest = label("changed"),
                    5 => admitted.rules.source_manifest_digest.0[0] ^= 1,
                    6 => admitted.rules.handler = label("changed"),
                    _ => admitted.rules.handler_digest.0[0] ^= 1,
                }
                CheckpointError::RulesMismatch
            }
            _ => {
                match changed {
                    8 => admitted.content.content = label("changed"),
                    9 => admitted.content.content_digest.0[0] ^= 1,
                    10 => admitted.content.package = label("changed"),
                    _ => admitted.content.package_digest.0[0] ^= 1,
                }
                CheckpointError::ContentMismatch
            }
        };
        assert_eq!(
            propose_witness_grants(
                &current,
                basis(),
                &admitted,
                &policy(),
                &[direct(40)],
                limits()
            )
            .unwrap_err(),
            WitnessError::Checkpoint(error)
        );
    }
    for changed in 0..5 {
        let mut admitted = pins();
        let mut fields = [
            "source-1",
            "native-1",
            "wasm-1",
            "config-1",
            "witness-fixture-1",
        ];
        fields[changed] = "changed";
        admitted.build = BuildIdentity::new(
            Some(fields[0]),
            Some(fields[1]),
            Some(fields[2]),
            Some(fields[3]),
            Some(fields[4]),
        )
        .unwrap();
        assert_eq!(
            propose_witness_grants(
                &current,
                basis(),
                &admitted,
                &policy(),
                &[direct(40)],
                limits()
            )
            .unwrap_err(),
            WitnessError::Checkpoint(CheckpointError::BuildMismatch)
        );
    }
}

#[test]
fn all_capacity_dimensions_and_late_failure_publish_no_partial_proposal() {
    let current = checkpoint(state());
    let original = current.clone();
    let eligible = [direct(40), direct(41)];
    for (bounded, error) in [
        (
            WitnessLimits {
                maximum_candidates: 1,
                ..limits()
            },
            WitnessError::CandidateCapacity,
        ),
        (
            WitnessLimits {
                maximum_scan_records: 0,
                ..limits()
            },
            WitnessError::ScanCapacity,
        ),
        (
            WitnessLimits {
                maximum_record_comparisons: 0,
                ..limits()
            },
            WitnessError::ComparisonCapacity,
        ),
        (
            WitnessLimits {
                maximum_scan_records: 5,
                ..limits()
            },
            WitnessError::ScanCapacity,
        ),
        (
            WitnessLimits {
                maximum_record_comparisons: 5,
                ..limits()
            },
            WitnessError::ComparisonCapacity,
        ),
        (
            WitnessLimits {
                maximum_grants: 1,
                ..limits()
            },
            WitnessError::GrantCapacity,
        ),
        (
            WitnessLimits {
                maximum_grant_bytes: std::mem::size_of::<KnowledgeGrant>(),
                ..limits()
            },
            WitnessError::GrantByteCapacity,
        ),
    ] {
        assert_eq!(
            propose_witness_grants(&current, basis(), &pins(), &policy(), &eligible, bounded)
                .unwrap_err(),
            error
        );
        assert_eq!(current, original);
    }
    assert_eq!(
        grants(&current, &[direct(40), direct(99)]),
        Err(WitnessError::UnknownWitness)
    );
    assert_eq!(
        grants(&current, &[direct(40), direct(42)]),
        Err(WitnessError::AudienceDenied)
    );
    assert_eq!(current, original);
}

#[test]
fn duplicate_eligibility_is_revalidated_before_a_no_op_and_exact_single_grant_bounds_work() {
    let current = checkpoint(state());
    let exact = WitnessLimits {
        maximum_candidates: 1,
        maximum_scan_records: 5,
        maximum_record_comparisons: 5,
        maximum_grants: 1,
        maximum_grant_bytes: std::mem::size_of::<KnowledgeGrant>(),
    };
    assert_eq!(
        propose_witness_grants(&current, basis(), &pins(), &policy(), &[direct(40)], exact)
            .unwrap()
            .grants()
            .len(),
        1
    );
    let zero = WitnessLimits {
        maximum_candidates: 0,
        maximum_scan_records: 0,
        maximum_record_comparisons: 0,
        maximum_grants: 0,
        maximum_grant_bytes: 0,
    };
    assert!(
        propose_witness_grants(&current, basis(), &pins(), &policy(), &[], zero)
            .unwrap()
            .grants()
            .is_empty()
    );
    let permitted = label("source-contact-allowed");
    let invalid_duplicate = WitnessEligibility {
        route: WitnessRoute::Relationship {
            permitted_state: &permitted,
        },
        ..direct(40)
    };
    assert_eq!(
        grants(&current, &[direct(40), invalid_duplicate]),
        Err(WitnessError::IneligibleRoute)
    );
    let mut already_known = current.state().clone();
    already_known
        .knowledge
        .extend(grants(&current, &[direct(40)]).unwrap());
    assert_eq!(
        grants(&checkpoint(already_known), &[invalid_duplicate]),
        Err(WitnessError::IneligibleRoute)
    );
    assert!(current.state().knowledge.is_empty());
}

#[test]
fn irrelevant_private_claim_and_rumor_text_never_produce_grants_or_change_truth() {
    let current = checkpoint(state());
    let mut hidden = current.state().clone();
    hidden.beliefs.push(AttributedClaim {
        id: record(80),
        holder: entity(6),
        subject: entity(4),
        claim: "A private false claim says everyone witnessed the event".to_owned(),
        evidence: vec![hidden.facts[2].id],
        audience: AudienceScope::Members(vec![member(5)]),
        source: policy(),
    });
    hidden.continuity.rumors.push(RumorTransmission {
        id: record(81),
        claim: record(80),
        sender: entity(6),
        recipient: entity(4),
        evidence: vec![hidden.facts[2].id],
        policy: policy(),
        remaining_hops: 1,
        audience: AudienceScope::Shared,
    });
    let hidden = checkpoint(hidden);
    assert_eq!(
        grants(&hidden, &[direct(40)]),
        grants(&current, &[direct(40)])
    );
    assert!(grants(&hidden, &[]).unwrap().is_empty());
    assert_eq!(hidden.state().facts, current.state().facts);
}
