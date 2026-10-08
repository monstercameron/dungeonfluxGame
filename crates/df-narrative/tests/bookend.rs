use df_knowledge::perception::{ClaimPerceptionLimits, ObserverScope, PerceptionLimits};
use df_model::checkpoint::*;
use df_narrative::{
    BookendEvidenceError, BookendEvidenceLimits, BookendEvidenceRequest, select_bookend_evidence,
};
use df_types::{OperationId, RevisionLabel};

#[expect(
    dead_code,
    reason = "shared canonical checkpoint fixture has other consumers"
)]
#[path = "../../df-engine/tests/support/fixture_model.rs"]
mod model;

fn content(entry: &str) -> ContentReference {
    ContentReference {
        package: model::pins().content.package,
        entry: model::label(entry),
    }
}

fn fact_id(value: u8) -> FactId {
    FactId::from_bytes(&[value; 16]).unwrap()
}

fn record_id(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}

fn fact(
    value: u8,
    operation: u8,
    revision: df_types::SessionRevision,
    audience: AudienceScope,
) -> GameFact {
    GameFact {
        id: fact_id(value),
        revision,
        operation: OperationId::from_bytes(&[operation; 16]).unwrap(),
        ordinal: 0,
        cause: None,
        audience,
        value: FactValue::ContentEvent {
            definition: content("established-event"),
            subjects: vec![],
        },
    }
}

fn decision(fact: &GameFact) -> AcceptedDecision {
    AcceptedDecision {
        operation: fact.operation,
        revision: fact.revision,
        facts: vec![fact.id],
        draws: vec![],
        effects: vec![],
        source_policy: model::label("admitted-event"),
        semantic_output: None,
    }
}

struct Fixture {
    checkpoint: Checkpoint,
    references: Vec<ContentReference>,
    rules: Vec<RuleReference>,
    resources: Vec<ResourceConstraint>,
    thread: ContentReference,
}

impl Fixture {
    fn new() -> Self {
        let references = [
            "fixture-entry-1",
            "established-event",
            "open-thread",
            "closed-thread",
        ]
        .map(content)
        .to_vec();
        let thread = references[2].clone();
        let mut state = model::state();
        state.narrative.open_threads.push(thread.clone());
        let shared = fact(20, 21, model::revision(2, 7), AudienceScope::Shared);
        let member = fact(
            22,
            23,
            model::basis().revision,
            AudienceScope::Members(vec![model::member(3)]),
        );
        let host = fact(24, 25, model::basis().revision, AudienceScope::Host);
        let orphan = fact(26, 27, model::basis().revision, AudienceScope::Shared);
        state.decisions = [&shared, &member, &host].map(decision).to_vec();
        state.facts = vec![shared, member, host, orphan];
        state.beliefs.push(AttributedClaim {
            id: record_id(30),
            holder: model::entity(4),
            subject: model::entity(4),
            claim: "An attributed belief, not established truth".into(),
            evidence: vec![fact_id(20)],
            audience: AudienceScope::Members(vec![model::member(3)]),
            source: content("established-event"),
        });
        let rules = vec![model::rule()];
        let resources = model::resource_constraints();
        let checkpoint = Checkpoint::new(
            CHECKPOINT_SCHEMA,
            model::basis(),
            model::pins(),
            state,
            ReferenceInventory {
                rules: &rules,
                content: &references,
                resources: &resources,
                assets: &[],
            },
            model::limits(),
        )
        .unwrap();
        Self {
            checkpoint,
            references,
            rules,
            resources,
            thread,
        }
    }

    fn checkpoint_with(&self, state: GameState) -> Checkpoint {
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            self.checkpoint.basis(),
            self.checkpoint.pins().clone(),
            state,
            ReferenceInventory {
                rules: &self.rules,
                content: &self.references,
                resources: &self.resources,
                assets: &[],
            },
            model::limits(),
        )
        .unwrap()
    }

    fn spec(&self, kind: BookendKind, audience: AudienceScope) -> BookendSpec {
        BookendSpec {
            id: record_id(40),
            basis: self.checkpoint.basis(),
            kind,
            from: model::revision(2, 7),
            through: model::basis().revision,
            audience,
            locale: df_types::LocaleTag::parse("en").unwrap(),
            policy: content("fixture-entry-1"),
            maximum_shots: 1,
            maximum_duration_ticks: 40,
            maximum_text_bytes: 128,
            maximum_asset_bytes: 256,
            expires: LogicalTime {
                ticks: 240,
                ticks_per_second: 10,
            },
            mode: ExecutionMode::PreparedOnly,
            budget_reservation: RevisionLabel::new(Some("synthetic-budget")).unwrap(),
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "fixture varies each independent admission input in refusal cases"
    )]
    fn select(
        &self,
        current: &Checkpoint,
        spec: &BookendSpec,
        requested: &FactSelection,
        pins: &CheckpointPins,
        observer: ObserverScope,
        admitted_threads: &[ContentReference],
        thread: Option<&ContentReference>,
        maximum_work: usize,
    ) -> Result<(FactSelection, Option<ContentReference>), BookendEvidenceError> {
        select_bookend_evidence(
            current,
            BookendEvidenceRequest {
                spec,
                selected: requested,
                admitted_pins: pins,
                observer,
                admitted_threads,
                selected_thread: thread,
            },
            BookendEvidenceLimits {
                facts: PerceptionLimits {
                    maximum_scan_records: 100,
                    maximum_member_comparisons: 100,
                    maximum_selected_facts: 100,
                },
                claims: ClaimPerceptionLimits {
                    maximum_scan_records: 100,
                    maximum_record_comparisons: 100,
                    maximum_selected_claims: 100,
                },
                maximum_work,
            },
        )
    }
}

fn selection(audience: AudienceScope, facts: Vec<FactId>, claims: Vec<RecordId>) -> FactSelection {
    FactSelection {
        facts,
        attributed_claims: claims,
        audience,
    }
}

fn outcome(
    result: &Result<(FactSelection, Option<ContentReference>), BookendEvidenceError>,
) -> String {
    match result {
        Ok((selected, thread)) => format!(
            "ok:facts={}:claims={}:thread={}",
            selected.facts.len(),
            selected.attributed_claims.len(),
            thread.is_some()
        ),
        Err(error) => format!("error:{error:?}"),
    }
}

#[test]
fn recap_and_trailer_evidence_contract_has_exact_golden_outcomes() {
    let fixture = Fixture::new();
    let current = &fixture.checkpoint;
    let original = current.clone();
    let member_audience = AudienceScope::Members(vec![model::member(3)]);
    let recap = fixture.spec(BookendKind::Recap, member_audience.clone());
    let selected = selection(
        member_audience.clone(),
        vec![fact_id(20), fact_id(22)],
        vec![record_id(30)],
    );
    let member = ObserverScope::Member(model::member(3));
    let mut observed = Vec::new();

    let accepted = fixture.select(
        current,
        &recap,
        &selected,
        current.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(accepted, Ok((selected.clone(), None)));
    observed.push(("accepted_recap", outcome(&accepted)));
    assert_eq!(
        current.state().beliefs[0].claim,
        "An attributed belief, not established truth"
    );
    assert!(!accepted.as_ref().unwrap().0.facts.contains(&fact_id(24)));

    for (name, invalid, expected) in [
        ("unknown_fact", fact_id(99), BookendEvidenceError::Evidence),
        ("orphan_fact", fact_id(26), BookendEvidenceError::Evidence),
        ("hidden_fact", fact_id(24), BookendEvidenceError::Evidence),
    ] {
        let request = selection(member_audience.clone(), vec![invalid], vec![]);
        let result = fixture.select(
            current,
            &recap,
            &request,
            current.pins(),
            member,
            &[],
            None,
            300,
        );
        assert_eq!(result, Err(expected), "{name}");
        observed.push((name, outcome(&result)));
    }

    let mut short_range = recap.clone();
    short_range.through = model::revision(2, 7);
    let recent = selection(member_audience.clone(), vec![fact_id(22)], vec![]);
    let out_of_range = fixture.select(
        current,
        &short_range,
        &recent,
        current.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(out_of_range, Err(BookendEvidenceError::Evidence));
    observed.push(("out_of_range_fact", outcome(&out_of_range)));

    let reversed = selection(
        member_audience.clone(),
        vec![fact_id(22), fact_id(20)],
        vec![],
    );
    let reordered = fixture.select(
        current,
        &recap,
        &reversed,
        current.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(reordered, Err(BookendEvidenceError::Evidence));
    observed.push(("reordered_chronology", outcome(&reordered)));

    let claim_only = selection(member_audience.clone(), vec![], vec![record_id(30)]);
    let attributed = fixture.select(
        current,
        &recap,
        &claim_only,
        current.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(attributed, Ok((claim_only.clone(), None)));
    observed.push(("attributed_claim", outcome(&attributed)));

    let hidden_claim = selection(AudienceScope::Shared, vec![], vec![record_id(30)]);
    let shared_recap = fixture.spec(BookendKind::Recap, AudienceScope::Shared);
    let denied_claim = fixture.select(
        current,
        &shared_recap,
        &hidden_claim,
        current.pins(),
        ObserverScope::Shared,
        &[],
        None,
        300,
    );
    assert_eq!(denied_claim, Err(BookendEvidenceError::Evidence));
    observed.push(("hidden_claim", outcome(&denied_claim)));

    let unknown_claim = selection(member_audience.clone(), vec![], vec![record_id(31)]);
    let unknown = fixture.select(
        current,
        &recap,
        &unknown_claim,
        current.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(unknown, Err(BookendEvidenceError::Evidence));
    observed.push(("unknown_claim", outcome(&unknown)));

    let mut orphan_claim_state = current.state().clone();
    orphan_claim_state.beliefs[0].evidence = vec![fact_id(26)];
    let orphan_claim_checkpoint = fixture.checkpoint_with(orphan_claim_state);
    let orphan_claim = fixture.select(
        &orphan_claim_checkpoint,
        &recap,
        &claim_only,
        orphan_claim_checkpoint.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(orphan_claim, Err(BookendEvidenceError::Evidence));
    observed.push(("uncommitted_claim_evidence", outcome(&orphan_claim)));

    let mut unsupported_claim_state = current.state().clone();
    unsupported_claim_state.beliefs[0].evidence.clear();
    let unsupported_claim_checkpoint = fixture.checkpoint_with(unsupported_claim_state);
    let unsupported_claim = fixture.select(
        &unsupported_claim_checkpoint,
        &recap,
        &claim_only,
        unsupported_claim_checkpoint.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(unsupported_claim, Err(BookendEvidenceError::Evidence));
    observed.push(("unsupported_claim", outcome(&unsupported_claim)));

    let mut recent_claim_state = current.state().clone();
    recent_claim_state.beliefs[0].evidence = vec![fact_id(22)];
    let recent_claim_checkpoint = fixture.checkpoint_with(recent_claim_state);
    let future_claim_result = fixture.select(
        &recent_claim_checkpoint,
        &short_range,
        &claim_only,
        recent_claim_checkpoint.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(future_claim_result, Err(BookendEvidenceError::Evidence));
    observed.push(("out_of_range_claim_evidence", outcome(&future_claim_result)));

    let mut stale = recap.clone();
    stale.basis.run = df_types::RunId::from_bytes(&[81; 16]).unwrap();
    let stale_run = fixture.select(
        current,
        &stale,
        &selected,
        current.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(
        stale_run,
        Err(BookendEvidenceError::Checkpoint(CheckpointError::WrongRun))
    );
    observed.push(("stale_run", outcome(&stale_run)));

    let mut stale_revision = recap.clone();
    stale_revision.basis.revision = model::revision(2, 9);
    let stale_basis = fixture.select(
        current,
        &stale_revision,
        &selected,
        current.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(
        stale_basis,
        Err(BookendEvidenceError::Checkpoint(
            CheckpointError::StaleBasis
        ))
    );
    observed.push(("stale_basis", outcome(&stale_basis)));

    let mut invalid_range_spec = recap.clone();
    invalid_range_spec.from = model::revision(2, 9);
    let invalid_range = fixture.select(
        current,
        &invalid_range_spec,
        &selected,
        current.pins(),
        member,
        &[],
        None,
        300,
    );
    assert_eq!(invalid_range, Err(BookendEvidenceError::Range));
    observed.push(("invalid_range", outcome(&invalid_range)));

    let mut old_pins = current.pins().clone();
    old_pins.content.content = model::label("old-content");
    let stale_pins = fixture.select(
        current,
        &recap,
        &selected,
        &old_pins,
        member,
        &[],
        None,
        300,
    );
    assert!(matches!(
        stale_pins,
        Err(BookendEvidenceError::Checkpoint(_))
    ));
    observed.push(("stale_pins", outcome(&stale_pins)));

    let mismatch = fixture.select(
        current,
        &recap,
        &selected,
        current.pins(),
        ObserverScope::Shared,
        &[],
        None,
        300,
    );
    assert_eq!(mismatch, Err(BookendEvidenceError::Audience));
    observed.push(("observer_mismatch", outcome(&mismatch)));

    let no_work = fixture.select(
        current,
        &recap,
        &selected,
        current.pins(),
        member,
        &[],
        None,
        0,
    );
    assert_eq!(no_work, Err(BookendEvidenceError::Capacity));
    observed.push(("bounded_work", outcome(&no_work)));

    let limited = select_bookend_evidence(
        current,
        BookendEvidenceRequest {
            spec: &recap,
            selected: &selected,
            admitted_pins: current.pins(),
            observer: member,
            admitted_threads: &[],
            selected_thread: None,
        },
        BookendEvidenceLimits {
            facts: PerceptionLimits {
                maximum_scan_records: 100,
                maximum_member_comparisons: 100,
                maximum_selected_facts: 1,
            },
            claims: ClaimPerceptionLimits {
                maximum_scan_records: 100,
                maximum_record_comparisons: 100,
                maximum_selected_claims: 100,
            },
            maximum_work: 300,
        },
    );
    assert_eq!(limited, Err(BookendEvidenceError::Capacity));
    observed.push(("bounded_selection", outcome(&limited)));

    let trailer = fixture.spec(BookendKind::SpeculativeTrailer, member_audience.clone());
    let empty = selection(member_audience.clone(), vec![], vec![]);
    let allowed = [fixture.thread.clone()];
    let speculative = fixture.select(
        current,
        &trailer,
        &empty,
        current.pins(),
        member,
        &allowed,
        Some(&fixture.thread),
        300,
    );
    assert_eq!(
        speculative,
        Ok((empty.clone(), Some(fixture.thread.clone())))
    );
    observed.push(("authorized_trailer", outcome(&speculative)));

    let future_claim = selection(member_audience.clone(), vec![fact_id(20)], vec![]);
    let promoted = fixture.select(
        current,
        &trailer,
        &future_claim,
        current.pins(),
        member,
        &allowed,
        Some(&fixture.thread),
        300,
    );
    assert_eq!(promoted, Err(BookendEvidenceError::Evidence));
    observed.push(("trailer_fact_refused", outcome(&promoted)));

    let claimed = selection(member_audience.clone(), vec![], vec![record_id(30)]);
    let claim_promoted = fixture.select(
        current,
        &trailer,
        &claimed,
        current.pins(),
        member,
        &allowed,
        Some(&fixture.thread),
        300,
    );
    assert_eq!(claim_promoted, Err(BookendEvidenceError::Evidence));
    observed.push(("trailer_claim_refused", outcome(&claim_promoted)));

    let unavailable = fixture.select(
        current,
        &trailer,
        &empty,
        current.pins(),
        member,
        &[],
        Some(&fixture.thread),
        300,
    );
    assert_eq!(unavailable, Err(BookendEvidenceError::Thread));
    observed.push(("unadmitted_thread", outcome(&unavailable)));

    let closed = content("closed-thread");
    let closed_allowed = [closed.clone()];
    let closed_result = fixture.select(
        current,
        &trailer,
        &empty,
        current.pins(),
        member,
        &closed_allowed,
        Some(&closed),
        300,
    );
    assert_eq!(closed_result, Err(BookendEvidenceError::Thread));
    observed.push(("closed_thread", outcome(&closed_result)));

    let mut hidden_state = current.state().clone();
    hidden_state.facts[2].value = FactValue::ContentEvent {
        definition: content("closed-thread"),
        subjects: vec![],
    };
    let changed_hidden = fixture.checkpoint_with(hidden_state);
    let same_observer = fixture.select(
        &changed_hidden,
        &trailer,
        &empty,
        changed_hidden.pins(),
        member,
        &allowed,
        Some(&fixture.thread),
        300,
    );
    assert_eq!(same_observer, speculative);
    observed.push(("hidden_state_noninterference", outcome(&same_observer)));

    assert_eq!(current, &original);
    assert_eq!(current.basis(), original.basis());
    assert_eq!(current.state().facts, original.state().facts);
    assert_eq!(current.state().draws, original.state().draws);
    assert_eq!(current.state().decisions, original.state().decisions);

    let cases = observed
        .iter()
        .map(|(name, result)| format!("    {{\"case\":\"{name}\",\"outcome\":\"{result}\"}}"))
        .collect::<Vec<_>>()
        .join(",\n");
    let actual = format!(
        "{{\n  \"task\": \"B-C-df-narrative-D04\",\n  \"source\": \"planning/campaign-cinematics.md: Recaps and trailers; planning/narrative-engine.md: Realization and acceptance\",\n  \"decision\": \"Recaps select exact accepted and observer-permitted facts, retaining claims as attributed; trailers select only an admitted current open thread and no fact or claim references.\",\n  \"rejected_alternatives\": [\n    \"Treat a trailer outline as a committed future event\",\n    \"Promote an NPC belief or a summary into canonical truth\",\n    \"Infer thread disclosure from its bare content reference\"\n  ],\n  \"unresolved\": [\n    \"No current df-presentation bookend shot/caption planner or mounted session publisher consumes this selector\",\n    \"Arbitrary image, audio and caption semantics require separate audience and speculative-label validation\",\n    \"Browser, media/provider, PostgreSQL and independent integrated review remain unperformed by this contract\"\n  ],\n  \"cases\": [\n{cases}\n  ]\n}}\n"
    );
    assert_eq!(
        actual,
        include_str!("fixtures/bookend_evidence_contract.json")
    );
}
