use std::time::Duration;

use df_ai::expression::{
    ExpressionClause, GroundedClause, GroundedValue, QualificationLimits, SpeechEnvelope,
    prepare_expression_request, qualify_expression,
};
use df_interaction::speech::{
    ClaimPerceptionLimits, DeclaredUncertainty, ObserverScope, PerceptionLimits, SpeechLimits,
    SpeechObservation, SpeechProposal, SpeechSourceError, SpeechSourceOwner, SpeechVersions,
    admit_speech, project_expression_context,
};
use df_model::checkpoint::*;
use df_provider_api::{
    CheckedRequest, RequestBinding, RequestError, RequestIdentity, RequestLimits,
    RequestOwnerState, RequestUsage,
};
use df_testkit::{FakeFailure, FakeLimits, FakeOutcome, FakeProvider, FakeProviderError, FakeStep};
use df_types::{
    BuildIdentity, LocaleTag, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId,
    SessionRevision, Usage, UsageUnit,
};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("faithful_modality_contract.json")).unwrap()
}

fn fixture_request_bytes(fixture: &Value) -> Vec<u8> {
    let (pairs, remainder) = fixture["request_hex"]
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}

fn id<T>(byte: u8) -> T
where
    T: FromIdBytes,
{
    T::from_id_bytes(&[byte; 16])
}

trait FromIdBytes {
    fn from_id_bytes(bytes: &[u8; 16]) -> Self;
}

macro_rules! id_type {
    ($($type:ty),+ $(,)?) => {
        $(impl FromIdBytes for $type {
            fn from_id_bytes(bytes: &[u8; 16]) -> Self { Self::from_bytes(bytes).unwrap() }
        })+
    };
}
id_type!(
    SessionId,
    RunId,
    OperationId,
    EntityId,
    RecordId,
    FactId,
    JobId
);

fn basis() -> Basis {
    Basis {
        session: id::<SessionId>(1),
        run: id::<RunId>(2),
        revision: SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 2),
    }
}

fn content() -> ContentReference {
    ContentReference {
        package: label("fixture-package"),
        entry: label("fixture-entry"),
    }
}

fn rule() -> RuleReference {
    RuleReference {
        catalog: label("fixture-catalog"),
        source: label("fixture-source"),
        entry: label("fixture-entry"),
        clause: label("fixture-clause"),
    }
}

fn pins() -> CheckpointPins {
    CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::DisclosedCustom,
            ruleset: label("fixture-rules"),
            catalog: label("fixture-catalog"),
            catalog_digest: ContentDigest([1; 32]),
            source_manifest: label("fixture-source"),
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

fn state(claim: &str) -> GameState {
    let mut state = GameState {
        mode: ExecutionMode::Replay,
        logical_time: LogicalTime {
            ticks: 1,
            ticks_per_second: 1,
        },
        members: vec![],
        entities: vec![],
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
        continuity: ContinuityState {
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
        },
    };
    state.facts.push(GameFact {
        id: id::<FactId>(10),
        revision: basis().revision,
        operation: id::<OperationId>(6),
        ordinal: 0,
        cause: None,
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: content(),
            subjects: vec![],
        },
    });
    state.entities.push(WorldEntity {
        id: id::<EntityId>(4),
        definition: content(),
        location: None,
        position: Some(Position { x: 0, y: 0, z: 0 }),
        identity_revision: label("fixture-entity"),
    });
    state.beliefs.push(AttributedClaim {
        id: id::<RecordId>(30),
        holder: id::<EntityId>(4),
        subject: id::<EntityId>(4),
        claim: claim.to_owned(),
        evidence: vec![id::<FactId>(10)],
        audience: AudienceScope::Shared,
        source: content(),
    });
    state
}

fn checkpoint(claim: &str) -> Checkpoint {
    let references = [content()];
    let rules = [rule()];
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis(),
        pins(),
        state(claim),
        ReferenceInventory {
            rules: &rules,
            content: &references,
            resources: &[],
            assets: &[],
        },
        CheckpointLimits {
            maximum_records: 100,
            maximum_text_bytes: 256,
            maximum_total_text_bytes: 1024,
            maximum_retained_bytes: 1024 * 1024,
        },
    )
    .unwrap()
}

struct Source;
impl SpeechSourceOwner for Source {
    fn validate(
        &self,
        _basis: Basis,
        _pins: &CheckpointPins,
        _versions: &SpeechVersions,
        _proposal: &SpeechProposal,
    ) -> Result<(), SpeechSourceError> {
        Ok(())
    }
}

fn versions() -> SpeechVersions {
    SpeechVersions {
        access: label("fixture-access"),
        contract: label("fixture-contract"),
        profile: label("fixture-profile"),
        locale: LocaleTag::parse("en-US").unwrap(),
        provider: label("fixture-provider"),
        model: label("fixture-model"),
        format: label("fixture-format"),
        quote: label("fixture-quote"),
    }
}

fn observation<'a>(
    checkpoint: &'a Checkpoint,
    versions: &'a SpeechVersions,
    source: &'a Source,
) -> SpeechObservation<'a> {
    SpeechObservation {
        checkpoint,
        basis: basis(),
        pins: checkpoint.pins(),
        observer: ObserverScope::Shared,
        versions,
        source_owner: Some(source),
        limits: SpeechLimits {
            maximum_plan_claims: 1,
            maximum_slots: 0,
            maximum_claim_bytes: 256,
            maximum_selected_text_and_evidence_bytes: 512,
            maximum_comparisons: 20,
            facts: PerceptionLimits {
                maximum_scan_records: 100,
                maximum_member_comparisons: 100,
                maximum_selected_facts: 10,
            },
            claims: ClaimPerceptionLimits {
                maximum_scan_records: 100,
                maximum_record_comparisons: 100,
                maximum_selected_claims: 10,
            },
        },
    }
}

fn request_binding<Semantic: Clone>(
    semantic_basis: Semantic,
    mode: ExecutionMode,
) -> RequestBinding<Semantic> {
    RequestBinding {
        identity: RequestIdentity {
            basis: basis(),
            job: id::<JobId>(5),
            operation: id::<OperationId>(6),
            generation: 7,
        },
        semantic_basis,
        mode,
        deadline: Duration::from_secs(2),
    }
}

fn owner<Semantic>(binding: &RequestBinding<Semantic>) -> RequestOwnerState<'_, Semantic> {
    RequestOwnerState {
        current: Some(binding),
        elapsed: Duration::ZERO,
        cancelled: false,
    }
}

fn declared(tokens: u128) -> RequestUsage {
    RequestUsage::new(
        tokens,
        Duration::from_millis(20),
        Usage::new(tokens, UsageUnit::Token),
    )
}

fn checked(
    payload: &[u8],
    tokens: u128,
) -> (CheckedRequest<&'static str>, RequestBinding<&'static str>) {
    let current = request_binding("scope", ExecutionMode::Replay);
    let request = CheckedRequest::new(
        request_binding("scope", ExecutionMode::Replay),
        payload,
        declared(tokens),
        RequestLimits::new(
            16_384,
            100,
            Duration::from_secs(2),
            Usage::new(100, UsageUnit::Token),
        )
        .unwrap(),
        owner(&current),
    )
    .unwrap();
    (request, current)
}

fn limits() -> FakeLimits {
    FakeLimits {
        maximum_steps: 4,
        maximum_request_bytes: 16_384,
        maximum_response_bytes: 256,
    }
}

#[test]
fn canonical_expression_request_and_candidate_round_trip_through_json_script() {
    let fixture = fixture();
    let claim = fixture["claim"].as_str().unwrap();
    let checkpoint = checkpoint(claim);
    let versions = versions();
    let source = Source;
    let observation = observation(&checkpoint, &versions, &source);
    let plan = admit_speech(
        SpeechProposal {
            source: content(),
            claims: vec![df_interaction::speech::PlannedClaim {
                claim: id::<RecordId>(30),
                intent: df_interaction::speech::SpeechIntent::Claim,
                uncertainty: DeclaredUncertainty::NoneDeclared,
                slots: vec![],
            }],
        },
        &observation,
    )
    .unwrap();
    let context = project_expression_context(&plan, &observation).unwrap();
    let binding = request_binding(context.semantic_basis().clone(), ExecutionMode::Replay);
    let request = prepare_expression_request(
        context,
        &observation,
        request_binding(binding.semantic_basis.clone(), ExecutionMode::Replay),
        declared(fixture["declared_tokens"].as_u64().unwrap().into()),
        RequestLimits::new(
            16_384,
            100,
            Duration::from_secs(2),
            Usage::new(100, UsageUnit::Token),
        )
        .unwrap(),
        owner(&binding),
    )
    .unwrap();
    let payload = request.request().payload();
    let expected_payload = fixture_request_bytes(&fixture);
    assert_eq!(payload, expected_payload);
    assert!(payload.starts_with(fixture["request_marker"].as_str().unwrap().as_bytes()));
    assert!(
        payload
            .windows(claim.len())
            .any(|part| part == claim.as_bytes())
    );
    assert_eq!(
        request.request().binding().semantic_basis,
        binding.semantic_basis
    );
    let response = fixture["response"].as_str().unwrap().as_bytes().to_vec();
    let actual = Usage::new(
        fixture["actual_tokens"].as_u64().unwrap().into(),
        UsageUnit::Token,
    );
    let mut fake = FakeProvider::new(
        vec![FakeStep {
            request_bytes: expected_payload,
            declared_usage: request.request().usage(),
            outcome: FakeOutcome::Success {
                bytes: response.clone(),
                usage: actual,
            },
        }],
        limits(),
    )
    .unwrap();
    let outcome = fake.take(request.request(), owner(&binding)).unwrap();
    assert!(!format!("{outcome:?}").contains(claim));
    let bytes = outcome.bytes;
    let usage = outcome.usage;
    assert_eq!(bytes, response);
    assert_eq!(usage, actual);
    let clause = request.context().claims().first().unwrap();
    let envelope = SpeechEnvelope {
        clauses: vec![ExpressionClause::Grounded(GroundedClause {
            claim_id: clause.id,
            holder: clause.holder,
            subject: clause.subject,
            intent: clause.intent,
            uncertainty: clause.uncertainty,
            text: String::from_utf8(bytes).unwrap(),
            slots: Vec::<GroundedValue>::new(),
        })],
    };
    let qualified = qualify_expression(
        &request,
        &observation,
        owner(&binding),
        envelope,
        QualificationLimits {
            maximum_clauses: 1,
            maximum_output_bytes: 256,
            maximum_slots: 0,
        },
    )
    .unwrap();
    assert_eq!(qualified.clauses()[0].text, claim);
    assert_eq!(fake.consumed(), 1);
    assert_eq!(fake.finish(), Ok(()));
}

#[test]
fn failures_and_terminal_cancellation_keep_explicit_partial_usage() {
    let fixture = fixture();
    let (request, current) = checked(b"canonical bytes", 10);
    let steps = [
        FakeOutcome::Failure {
            kind: FakeFailure::RateLimited,
            usage: Usage::new(
                fixture["failed_tokens"].as_u64().unwrap().into(),
                UsageUnit::Token,
            ),
        },
        FakeOutcome::Cancelled {
            usage: Usage::new(
                fixture["cancelled_tokens"].as_u64().unwrap().into(),
                UsageUnit::Token,
            ),
        },
    ];
    let mut fake = FakeProvider::new(
        steps
            .into_iter()
            .map(|outcome| FakeStep {
                request_bytes: b"canonical bytes".to_vec(),
                declared_usage: declared(10),
                outcome,
            })
            .collect(),
        limits(),
    )
    .unwrap();
    assert_eq!(
        fake.finish(),
        Err(FakeProviderError::Unused { remaining: 2 })
    );
    assert_eq!(
        fake.take(&request, owner(&current)),
        Err(FakeProviderError::ProviderFailure {
            kind: FakeFailure::RateLimited,
            usage: Usage::new(3, UsageUnit::Token),
        })
    );
    assert_eq!(
        fake.take(&request, owner(&current)),
        Err(FakeProviderError::Cancelled {
            usage: Usage::new(2, UsageUnit::Token),
        })
    );
    assert_eq!(fake.consumed(), 2);
    assert_eq!(fake.finish(), Ok(()));
    assert_eq!(
        fake.take(&request, owner(&current)),
        Err(FakeProviderError::Exhausted)
    );
}

#[test]
fn cancellation_deadline_stale_binding_and_bad_shape_cannot_consume_script() {
    let (request, current) = checked(b"canonical bytes", 10);
    let mut fake = FakeProvider::new(
        vec![FakeStep {
            request_bytes: b"canonical bytes".to_vec(),
            declared_usage: declared(10),
            outcome: FakeOutcome::Success {
                bytes: b"reply".to_vec(),
                usage: Usage::new(8, UsageUnit::Token),
            },
        }],
        limits(),
    )
    .unwrap();
    assert_eq!(
        fake.take(
            &request,
            RequestOwnerState {
                current: Some(&current),
                elapsed: Duration::ZERO,
                cancelled: true,
            }
        ),
        Err(FakeProviderError::Request(RequestError::Cancelled))
    );
    assert_eq!(
        fake.take(
            &request,
            RequestOwnerState {
                current: Some(&current),
                elapsed: Duration::from_secs(2),
                cancelled: false,
            }
        ),
        Err(FakeProviderError::Request(RequestError::DeadlineExceeded))
    );
    let stale = request_binding("changed", ExecutionMode::Replay);
    assert_eq!(
        fake.take(&request, owner(&stale)),
        Err(FakeProviderError::Request(
            RequestError::SemanticBasisMismatch
        ))
    );
    let (wrong_payload, _) = checked(b"changed bytes", 10);
    assert_eq!(
        fake.take(&wrong_payload, owner(&current)),
        Err(FakeProviderError::PayloadMismatch)
    );
    let (wrong_usage, _) = checked(b"canonical bytes", 9);
    assert_eq!(
        fake.take(&wrong_usage, owner(&current)),
        Err(FakeProviderError::DeclaredUsageMismatch)
    );
    assert_eq!(fake.consumed(), 0);
    assert_eq!(fake.remaining(), 1);
    assert!(matches!(
        fake.take(&request, owner(&current)),
        Ok(df_testkit::FakeSuccess { .. })
    ));
    assert_eq!(fake.finish(), Ok(()));
}

#[test]
fn script_limits_and_units_are_checked_before_any_use() {
    let make = || FakeStep {
        request_bytes: b"a".to_vec(),
        declared_usage: declared(1),
        outcome: FakeOutcome::Success {
            bytes: b"b".to_vec(),
            usage: Usage::new(1, UsageUnit::Token),
        },
    };
    assert!(matches!(
        FakeProvider::new(
            vec![make(), make()],
            FakeLimits {
                maximum_steps: 1,
                ..limits()
            }
        ),
        Err(FakeProviderError::Capacity)
    ));
    assert!(matches!(
        FakeProvider::new(
            vec![make()],
            FakeLimits {
                maximum_request_bytes: 0,
                ..limits()
            }
        ),
        Err(FakeProviderError::InvalidLimits)
    ));
    assert!(matches!(
        FakeProvider::new(
            vec![make()],
            FakeLimits {
                maximum_response_bytes: 0,
                ..limits()
            }
        ),
        Err(FakeProviderError::InvalidLimits)
    ));
    assert!(matches!(
        FakeProvider::new(
            vec![FakeStep {
                request_bytes: b"ab".to_vec(),
                ..make()
            }],
            FakeLimits {
                maximum_request_bytes: 1,
                ..limits()
            }
        ),
        Err(FakeProviderError::Capacity)
    ));
    assert!(matches!(
        FakeProvider::new(
            vec![FakeStep {
                outcome: FakeOutcome::Success {
                    bytes: b"ab".to_vec(),
                    usage: Usage::new(1, UsageUnit::Token),
                },
                ..make()
            }],
            FakeLimits {
                maximum_response_bytes: 1,
                ..limits()
            }
        ),
        Err(FakeProviderError::Capacity)
    ));
    assert!(matches!(
        FakeProvider::new(
            vec![FakeStep {
                outcome: FakeOutcome::Cancelled {
                    usage: Usage::new(1, UsageUnit::Image)
                },
                ..make()
            }],
            limits()
        ),
        Err(FakeProviderError::UnitMismatch)
    ));
}
