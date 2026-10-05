use df_knowledge::perception::{ObserverScope, PerceptionError, PerceptionLimits};
use df_model::checkpoint::*;
use df_narrative::{
    AdmittedThreatEvidence, ThreatRelevanceError, ThreatRelevanceLimits, ThreatRelevanceRequest,
    ThreatRelevanceSelection, select_threat_relevance,
};
use df_types::{
    BuildIdentity, MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId,
    SessionRevision,
};

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}

fn content(entry: &str) -> ContentReference {
    ContentReference {
        package: label("fixture-package"),
        entry: label(entry),
    }
}

fn member(value: u8) -> MemberId {
    MemberId::from_bytes(&[value; 16]).unwrap()
}

fn fact_id(value: u8) -> FactId {
    FactId::from_bytes(&[value; 16]).unwrap()
}

fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 8),
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
            package: label("fixture-package"),
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

struct Fixture {
    admitted: Vec<ContentReference>,
    state: GameState,
}

impl Fixture {
    fn new() -> Self {
        let admitted = vec![
            content("campaign"),
            content("shown-clock"),
            content("secret-clock"),
            content("relevance-policy"),
            content("permitted-event"),
            content("hidden-event"),
        ];
        let facts = [
            AudienceScope::Shared,
            AudienceScope::Members(vec![member(3)]),
            AudienceScope::Members(vec![member(5)]),
            AudienceScope::Host,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, audience)| GameFact {
            id: fact_id(10 + u8::try_from(index).unwrap()),
            revision: basis().revision,
            operation: OperationId::from_bytes(&[6; 16]).unwrap(),
            ordinal: u32::try_from(index).unwrap(),
            cause: None,
            audience,
            value: FactValue::ContentEvent {
                definition: admitted[if index < 2 { 4 } else { 5 }].clone(),
                subjects: vec![],
            },
        })
        .collect();
        let state = GameState {
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
            entities: vec![],
            characters: vec![],
            resources: vec![],
            inventory: vec![],
            facts,
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
            threats: vec![
                ThreatClock {
                    id: RecordId::from_bytes(&[30; 16]).unwrap(),
                    definition: admitted[1].clone(),
                    progress: 2,
                    capacity: 10,
                },
                ThreatClock {
                    id: RecordId::from_bytes(&[31; 16]).unwrap(),
                    definition: admitted[2].clone(),
                    progress: 7,
                    capacity: 20,
                },
            ],
            relationships: vec![],
            conversations: vec![],
            obligations: vec![],
            narrative: NarrativeState {
                definition: admitted[0].clone(),
                active_beats: vec![],
                completed_beats: vec![],
                open_threads: vec![],
                accepted_facts: vec![],
                remaining_budget: 9,
            },
            encounters: vec![],
            activity: vec![],
            tempo: TempoState {
                policy: admitted[0].clone(),
                presentation_ticks: 0,
                intensity: 0,
                inertia: 0,
                fatigue: vec![],
            },
            presentation: vec![],
            continuity: continuity(),
        };
        Self { admitted, state }
    }

    fn checkpoint(&self, state: GameState) -> Checkpoint {
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis(),
            pins(),
            state,
            ReferenceInventory {
                rules: &[],
                content: &self.admitted,
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
        .unwrap()
    }

    fn request<'a>(
        &'a self,
        current: &'a Checkpoint,
        mappings: Option<&'a [AdmittedThreatEvidence<'a>]>,
        observer: ObserverScope,
    ) -> ThreatRelevanceRequest<'a> {
        ThreatRelevanceRequest {
            expected_basis: current.basis(),
            admitted_pins: current.pins(),
            observer,
            policy: &self.admitted[3],
            expected_policy: &self.admitted[3],
            admitted_content: &self.admitted,
            mappings,
        }
    }
}

fn limits() -> ThreatRelevanceLimits {
    ThreatRelevanceLimits {
        maximum_checkpoint_bytes: 1024 * 1024,
        maximum_candidates: 16,
        maximum_evidence_records: 32,
        maximum_clock_records: 16,
        maximum_content_records: 16,
        maximum_reference_bytes: 256,
        maximum_work: 4096,
        maximum_output_bytes: 4096,
        perception: PerceptionLimits {
            maximum_scan_records: 32,
            maximum_member_comparisons: 32,
            maximum_selected_facts: 16,
        },
    }
}

fn ids(selection: &ThreatRelevanceSelection<'_>) -> Vec<FactId> {
    selection.facts().iter().map(|fact| fact.id).collect()
}

#[test]
fn actual_checkpoint_perception_selects_only_current_observer_evidence_without_mutation() {
    let fixture = Fixture::new();
    let current = fixture.checkpoint(fixture.state.clone());
    let original = current.clone();
    let evidence = [[fact_id(10)], [fact_id(11)], [fact_id(12)], [fact_id(13)]];
    let mappings = evidence
        .iter()
        .map(|evidence| AdmittedThreatEvidence {
            expected: &current.state().threats[0],
            evidence,
        })
        .collect::<Vec<_>>();
    for (observer, expected) in [
        (ObserverScope::Shared, vec![fact_id(10)]),
        (
            ObserverScope::Member(member(3)),
            vec![fact_id(10), fact_id(11)],
        ),
        (
            ObserverScope::Member(member(5)),
            vec![fact_id(10), fact_id(12)],
        ),
    ] {
        let selection = select_threat_relevance(
            &current,
            fixture.request(&current, Some(&mappings), observer),
            limits(),
        )
        .unwrap();
        assert_eq!(ids(&selection), expected);
        assert_eq!(
            format!("{selection:?}"),
            format!(
                "ThreatRelevanceSelection {{ facts: {}, .. }}",
                expected.len()
            )
        );
        assert_eq!(selection.basis(), current.basis());
        assert!(std::ptr::eq(selection.pins(), current.pins()));
        assert!(std::ptr::eq(selection.policy(), &fixture.admitted[3]));
        for selected in selection.facts() {
            assert!(
                current
                    .state()
                    .facts
                    .iter()
                    .any(|fact| std::ptr::eq(fact, *selected))
            );
        }
        assert!(selection.accounted_output_bytes() <= limits().maximum_output_bytes);
    }
    assert_eq!(current, original);
}

#[test]
fn mixed_visible_and_hidden_evidence_omits_entire_mapping_before_clock_validation() {
    let fixture = Fixture::new();
    let current = fixture.checkpoint(fixture.state.clone());
    let mut stale_secret = current.state().threats[1].clone();
    stale_secret.id = RecordId::from_bytes(&[99; 16]).unwrap();
    stale_secret.progress = 99;
    let mixed = [fact_id(10), fact_id(13)];
    let unknown = [fact_id(10), fact_id(99)];
    for evidence in [&mixed[..], &unknown[..], &[][..]] {
        let mappings = [AdmittedThreatEvidence {
            expected: &stale_secret,
            evidence,
        }];
        let selection = select_threat_relevance(
            &current,
            fixture.request(&current, Some(&mappings), ObserverScope::Shared),
            limits(),
        )
        .unwrap();
        assert!(selection.facts().is_empty());
    }
}

#[test]
fn hidden_clock_and_fact_changes_with_stale_hidden_mapping_preserve_identical_safe_output() {
    let fixture = Fixture::new();
    let original = fixture.checkpoint(fixture.state.clone());
    let public_evidence = [fact_id(10)];
    let hidden_evidence = [fact_id(13)];
    let mappings = [
        AdmittedThreatEvidence {
            expected: &original.state().threats[0],
            evidence: &public_evidence,
        },
        AdmittedThreatEvidence {
            expected: &original.state().threats[1],
            evidence: &hidden_evidence,
        },
    ];
    let baseline = select_threat_relevance(
        &original,
        fixture.request(&original, Some(&mappings), ObserverScope::Shared),
        limits(),
    )
    .unwrap();
    for variant in 0..4 {
        let mut state = fixture.state.clone();
        match variant {
            0 => {
                state.threats[1].progress = 19;
                state.threats[1].capacity = 21;
                state.threats[1].id = RecordId::from_bytes(&[99; 16]).unwrap();
                state.threats[1].definition = fixture.admitted[5].clone();
                state.facts[3].value = FactValue::ContentEvent {
                    definition: fixture.admitted[2].clone(),
                    subjects: vec![],
                };
            }
            1 => {
                state.threats.pop();
            }
            2 => {
                let mut extra = state.threats[1].clone();
                extra.id = RecordId::from_bytes(&[99; 16]).unwrap();
                state.threats.push(extra);
            }
            3 => {
                state.facts.pop();
                state.threats.pop();
            }
            _ => unreachable!(),
        }
        let current = fixture.checkpoint(state);
        let before = current.clone();
        let selection = select_threat_relevance(
            &current,
            fixture.request(&current, Some(&mappings), ObserverScope::Shared),
            limits(),
        )
        .unwrap();
        assert_eq!(selection.facts(), baseline.facts());
        assert_eq!(format!("{selection:?}"), format!("{baseline:?}"));
        assert_eq!(
            selection.accounted_output_bytes(),
            baseline.accounted_output_bytes()
        );
        assert_eq!(current, before);
    }
}

#[test]
fn duplicate_mapping_and_evidence_order_cannot_change_canonical_union_or_retry() {
    let fixture = Fixture::new();
    let current = fixture.checkpoint(fixture.state.clone());
    let forward = [fact_id(10), fact_id(11)];
    let reverse = [fact_id(11), fact_id(10), fact_id(11)];
    let first = [AdmittedThreatEvidence {
        expected: &current.state().threats[0],
        evidence: &forward,
    }];
    let repeated = [
        AdmittedThreatEvidence {
            expected: &current.state().threats[1],
            evidence: &reverse,
        },
        AdmittedThreatEvidence {
            expected: &current.state().threats[0],
            evidence: &reverse,
        },
        AdmittedThreatEvidence {
            expected: &current.state().threats[0],
            evidence: &forward,
        },
    ];
    let baseline = select_threat_relevance(
        &current,
        fixture.request(&current, Some(&first), ObserverScope::Member(member(3))),
        limits(),
    )
    .unwrap();
    for mappings in [&repeated[..], &repeated[..], &first[..]] {
        let selection = select_threat_relevance(
            &current,
            fixture.request(&current, Some(mappings), ObserverScope::Member(member(3))),
            limits(),
        )
        .unwrap();
        assert_eq!(ids(&selection), vec![fact_id(10), fact_id(11)]);
        assert_eq!(selection.facts(), baseline.facts());
        assert_eq!(
            selection.accounted_output_bytes(),
            baseline.accounted_output_bytes()
        );
    }
}

#[test]
fn missing_owner_admission_and_empty_admitted_batch_have_distinct_outcomes() {
    let fixture = Fixture::new();
    let current = fixture.checkpoint(fixture.state.clone());
    assert_eq!(
        select_threat_relevance(
            &current,
            fixture.request(&current, None, ObserverScope::Shared),
            limits(),
        )
        .unwrap_err(),
        ThreatRelevanceError::NotAdmitted
    );
    let empty = select_threat_relevance(
        &current,
        fixture.request(&current, Some(&[]), ObserverScope::Shared),
        limits(),
    )
    .unwrap();
    assert!(empty.facts().is_empty());
}

#[test]
fn eligible_wrong_exact_clock_and_source_mapping_refuse_atomically_without_details() {
    let fixture = Fixture::new();
    let current = fixture.checkpoint(fixture.state.clone());
    let original = current.clone();
    let evidence = [fact_id(10)];
    for variant in 0..4 {
        let mut wrong = current.state().threats[1].clone();
        match variant {
            0 => wrong.id = RecordId::from_bytes(&[99; 16]).unwrap(),
            1 => wrong.progress += 1,
            2 => wrong.capacity += 1,
            3 => wrong.definition = fixture.admitted[4].clone(),
            _ => unreachable!(),
        }
        let mappings = [
            AdmittedThreatEvidence {
                expected: &current.state().threats[0],
                evidence: &evidence,
            },
            AdmittedThreatEvidence {
                expected: &wrong,
                evidence: &evidence,
            },
        ];
        let error = select_threat_relevance(
            &current,
            fixture.request(&current, Some(&mappings), ObserverScope::Shared),
            limits(),
        )
        .unwrap_err();
        assert_eq!(error, ThreatRelevanceError::InvalidAdmission);
        assert_eq!(format!("{error:?}"), "InvalidAdmission");
    }
    assert_eq!(current, original);
}

#[test]
fn source_policy_and_definition_must_remain_exactly_admitted() {
    let fixture = Fixture::new();
    let current = fixture.checkpoint(fixture.state.clone());
    let evidence = [fact_id(10)];
    let mappings = [AdmittedThreatEvidence {
        expected: &current.state().threats[0],
        evidence: &evidence,
    }];
    let wrong_policy = content("unadmitted-policy");
    for variant in 0..3 {
        let mut content = fixture.admitted.clone();
        let mut request = fixture.request(&current, Some(&mappings), ObserverScope::Shared);
        match variant {
            0 => request.expected_policy = &wrong_policy,
            1 => {
                content.remove(3);
                request.admitted_content = &content;
            }
            2 => {
                content.remove(1);
                request.admitted_content = &content;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            select_threat_relevance(&current, request, limits()).unwrap_err(),
            ThreatRelevanceError::InvalidAdmission
        );
    }
}

#[test]
fn exact_session_run_revision_and_every_full_pin_component_are_required() {
    let fixture = Fixture::new();
    let current = fixture.checkpoint(fixture.state.clone());
    for variant in 0..3 {
        let mut request = fixture.request(&current, Some(&[]), ObserverScope::Shared);
        let expected = match variant {
            0 => {
                request.expected_basis.session = SessionId::from_bytes(&[99; 16]).unwrap();
                CheckpointError::WrongSession
            }
            1 => {
                request.expected_basis.run = RunId::from_bytes(&[99; 16]).unwrap();
                CheckpointError::WrongRun
            }
            2 => {
                request.expected_basis.revision =
                    request.expected_basis.revision.next_sequence().unwrap();
                CheckpointError::StaleBasis
            }
            _ => unreachable!(),
        };
        assert_eq!(
            select_threat_relevance(&current, request, limits()).unwrap_err(),
            ThreatRelevanceError::Checkpoint(expected)
        );
    }
    for variant in 0..17 {
        let mut changed = current.pins().clone();
        let expected = if variant < 8 {
            match variant {
                0 => changed.rules.mode = RulesMode::DisclosedCustom,
                1 => changed.rules.ruleset = label("other"),
                2 => changed.rules.catalog = label("other"),
                3 => changed.rules.catalog_digest = ContentDigest([99; 32]),
                4 => changed.rules.source_manifest = label("other"),
                5 => changed.rules.source_manifest_digest = ContentDigest([99; 32]),
                6 => changed.rules.handler = label("other"),
                7 => changed.rules.handler_digest = ContentDigest([99; 32]),
                _ => unreachable!(),
            }
            CheckpointError::RulesMismatch
        } else if variant < 12 {
            match variant {
                8 => changed.content.content = label("other"),
                9 => changed.content.content_digest = ContentDigest([99; 32]),
                10 => changed.content.package = label("other"),
                11 => changed.content.package_digest = ContentDigest([99; 32]),
                _ => unreachable!(),
            }
            CheckpointError::ContentMismatch
        } else {
            let mut fields = [
                "fixture-source",
                "fixture-native",
                "fixture-wasm",
                "fixture-config",
                "fixture-content",
            ];
            fields[variant - 12] = "other";
            changed.build = BuildIdentity::new(
                Some(fields[0]),
                Some(fields[1]),
                Some(fields[2]),
                Some(fields[3]),
                Some(fields[4]),
            )
            .unwrap();
            CheckpointError::BuildMismatch
        };
        let mut request = fixture.request(&current, Some(&[]), ObserverScope::Shared);
        request.admitted_pins = &changed;
        assert_eq!(
            select_threat_relevance(&current, request, limits()).unwrap_err(),
            ThreatRelevanceError::Checkpoint(expected)
        );
    }
}

#[test]
fn current_membership_and_audience_withdrawal_cannot_be_widened_by_mapping() {
    let fixture = Fixture::new();
    let original = fixture.checkpoint(fixture.state.clone());
    let evidence = [fact_id(11)];
    let mappings = [AdmittedThreatEvidence {
        expected: &original.state().threats[0],
        evidence: &evidence,
    }];
    let mut withdrawn = fixture.state.clone();
    withdrawn.facts[1].audience = AudienceScope::Host;
    let changed = fixture.checkpoint(withdrawn.clone());
    assert!(
        select_threat_relevance(
            &changed,
            fixture.request(&changed, Some(&mappings), ObserverScope::Member(member(3))),
            limits(),
        )
        .unwrap()
        .facts()
        .is_empty()
    );
    withdrawn.members.retain(|link| link.member != member(3));
    let removed = fixture.checkpoint(withdrawn);
    assert_eq!(
        select_threat_relevance(
            &removed,
            fixture.request(&removed, Some(&mappings), ObserverScope::Member(member(3))),
            limits(),
        )
        .unwrap_err(),
        ThreatRelevanceError::Perception(PerceptionError::ObserverUnavailable)
    );
}

#[test]
fn finite_input_work_perception_and_output_bounds_refuse_without_partial_selection() {
    let fixture = Fixture::new();
    let current = fixture.checkpoint(fixture.state.clone());
    let original = current.clone();
    let evidence = [fact_id(10), fact_id(11)];
    let mappings = [AdmittedThreatEvidence {
        expected: &current.state().threats[0],
        evidence: &evidence,
    }];
    for variant in 0..11 {
        let mut bound = limits();
        let expected = match variant {
            0 => {
                bound.maximum_checkpoint_bytes = 0;
                ThreatRelevanceError::InputCapacity
            }
            1 => {
                bound.maximum_candidates = 0;
                ThreatRelevanceError::InputCapacity
            }
            2 => {
                bound.maximum_evidence_records = 1;
                ThreatRelevanceError::InputCapacity
            }
            3 => {
                bound.maximum_clock_records = 1;
                ThreatRelevanceError::InputCapacity
            }
            4 => {
                bound.maximum_content_records = 1;
                ThreatRelevanceError::InputCapacity
            }
            5 => {
                bound.maximum_reference_bytes = 1;
                ThreatRelevanceError::InputCapacity
            }
            6 => {
                bound.maximum_work = 0;
                ThreatRelevanceError::WorkCapacity
            }
            7 => {
                bound.maximum_output_bytes = 0;
                ThreatRelevanceError::OutputCapacity
            }
            8 => {
                bound.perception.maximum_scan_records = 0;
                ThreatRelevanceError::Perception(PerceptionError::ScanCapacity)
            }
            9 => {
                bound.perception.maximum_member_comparisons = 0;
                ThreatRelevanceError::Perception(PerceptionError::ComparisonCapacity)
            }
            10 => {
                bound.perception.maximum_selected_facts = 0;
                ThreatRelevanceError::Perception(PerceptionError::ResultCapacity)
            }
            _ => unreachable!(),
        };
        assert_eq!(
            select_threat_relevance(
                &current,
                fixture.request(&current, Some(&mappings), ObserverScope::Member(member(3))),
                bound,
            )
            .unwrap_err(),
            expected
        );
    }
    assert_eq!(current, original);
}
