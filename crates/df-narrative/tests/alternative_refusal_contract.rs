//! Proposed integration test for df-narrative/tests/alternative_refusal_contract.rs.
//! The content owner, not this selector, admits the two authored alternatives.
use df_model::checkpoint::*;
use df_narrative::*;
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

struct Fixture {
    references: Vec<ContentReference>,
    rules: Vec<RuleReference>,
    resources: Vec<ResourceConstraint>,
    checkpoint: Checkpoint,
    source: FactId,
    policy: RevisionLabel,
}

impl Fixture {
    fn new() -> Self {
        let references = [
            "fixture-entry-1",
            "opening",
            "courier-answer-seal",
            "courier-answer-escort",
            "ask-courier",
            "escort-courier",
            "packet-thread",
            "begin-story",
            "refusal-recorded",
        ]
        .map(content)
        .to_vec();
        let source = fact_id(20);
        let operation = OperationId::from_bytes(&[21; 16]).unwrap();
        let mut state = model::state();
        state.narrative.active_beats = vec![references[1].clone()];
        state.narrative.open_threads = vec![references[6].clone()];
        state.narrative.accepted_facts = vec![source];
        state.facts.push(GameFact {
            id: source,
            revision: model::basis().revision,
            operation,
            ordinal: 0,
            cause: None,
            audience: AudienceScope::Shared,
            value: FactValue::ContentEvent {
                definition: references[7].clone(),
                subjects: vec![],
            },
        });
        state.decisions.push(AcceptedDecision {
            operation,
            revision: model::basis().revision,
            facts: vec![source],
            draws: vec![],
            effects: vec![],
            source_policy: model::label("admitted-begin-story"),
            semantic_output: None,
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
            references,
            rules,
            resources,
            checkpoint,
            source,
            policy: model::label("authored-courier-alternatives"),
        }
    }

    fn inventory(&self) -> ReferenceInventory<'_> {
        ReferenceInventory {
            rules: &self.rules,
            content: &self.references,
            resources: &self.resources,
            assets: &[],
        }
    }

    fn checkpoint_with(&self, state: GameState) -> Checkpoint {
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            self.checkpoint.basis(),
            self.checkpoint.pins().clone(),
            state,
            self.inventory(),
            model::limits(),
        )
        .unwrap()
    }

    fn beat_limits() -> BeatSelectionLimits {
        BeatSelectionLimits {
            records: 100,
            alternatives: 2,
            work: 4096,
        }
    }

    fn progress_limits() -> ProgressLimits {
        ProgressLimits {
            records: 100,
            consequences: 2,
            work: 4096,
        }
    }

    fn select(
        &self,
        current: &Checkpoint,
        selection: &ContentReference,
        cause: FactId,
        expected_basis: Basis,
        expected_policy: &RevisionLabel,
        duplicate: bool,
    ) -> Result<Checkpoint, BeatSelectionError> {
        let causes = [BeatCause {
            fact: cause,
            event: &self.references[7],
            consumed_by_narrative: true,
        }];
        let threads = [&self.references[6]].map(Clone::clone);
        let ask = AdmittedBeatAlternative {
            selection: &self.references[4],
            from: &self.references[1],
            to: &self.references[2],
            causes: &causes,
            required_threads: &threads,
            opened_thread: None,
        };
        let escort = AdmittedBeatAlternative {
            selection: &self.references[5],
            from: &self.references[1],
            to: &self.references[3],
            causes: &causes,
            required_threads: &threads,
            opened_thread: None,
        };
        let repeated_ask = AdmittedBeatAlternative {
            selection: &self.references[4],
            from: &self.references[1],
            to: &self.references[2],
            causes: &causes,
            required_threads: &threads,
            opened_thread: None,
        };
        let alternatives = if duplicate {
            [ask, repeated_ask]
        } else {
            [ask, escort]
        };
        stage_checkpoint_beat_selection(
            current,
            CheckpointBeatRequest {
                expected_basis,
                admitted_pins: self.checkpoint.pins(),
                policy: &self.policy,
                expected_policy,
                recipient: model::member(3),
                selection,
                alternatives: &alternatives,
                inventory: self.inventory(),
                checkpoint_limits: model::limits(),
            },
            Self::beat_limits(),
        )
    }

    fn progress(
        &self,
        current: &Checkpoint,
        selections: &[ThreadConsequenceSelection<'_>],
    ) -> Result<CheckpointThreadProgressProposal, CheckpointProgressError> {
        stage_checkpoint_thread_progress(
            current,
            ThreadCheckpointRequest {
                expected_basis: current.basis(),
                admitted_pins: current.pins(),
                policy: &self.policy,
                expected_policy: &self.policy,
                inventory: self.inventory(),
                checkpoint_limits: model::limits(),
                selections,
            },
            Self::progress_limits(),
        )
    }
}

#[test]
fn unselected_and_refused_route_leave_player_choice_and_canonical_state_untouched() {
    let fixture = Fixture::new();
    let current = &fixture.checkpoint;
    let original = current.clone();
    let refusal = fixture.select(
        current,
        &fixture.references[8],
        fixture.source,
        current.basis(),
        &fixture.policy,
        false,
    );
    assert_eq!(refusal, Err(BeatSelectionError::Selection));
    assert_eq!(current, &original);
    assert!(current.state().narrative.completed_beats.is_empty());
    assert_eq!(
        current.state().narrative.active_beats,
        [fixture.references[1].clone()]
    );
    assert!(current.state().characters[0].choices.is_empty());
    assert_eq!(current.state().resources[0].value, 4);
    assert_eq!(current.state().facts.len(), 1);
    assert_eq!(current.state().decisions.len(), 1);
}

#[test]
fn explicitly_selected_alternative_stages_only_authored_route_and_preserves_cause() {
    let fixture = Fixture::new();
    let current = &fixture.checkpoint;
    for (selection, destination) in [(4, 2), (5, 3)] {
        let staged = fixture
            .select(
                current,
                &fixture.references[selection],
                fixture.source,
                current.basis(),
                &fixture.policy,
                false,
            )
            .unwrap();
        let narrative = &staged.state().narrative;
        assert_eq!(
            narrative.active_beats,
            [fixture.references[destination].clone()]
        );
        assert_eq!(narrative.completed_beats, [fixture.references[1].clone()]);
        assert_eq!(narrative.accepted_facts, [fixture.source]);
        assert_eq!(
            narrative.open_threads,
            current.state().narrative.open_threads
        );
        let mut protected = staged.state().clone();
        protected.narrative = current.state().narrative.clone();
        assert_eq!(protected, *current.state());
        assert_eq!(staged.basis(), current.basis());
        assert_eq!(staged.pins(), current.pins());
        assert_eq!(
            current.state().narrative.active_beats,
            [fixture.references[1].clone()]
        );
    }
}

#[test]
fn repeated_stale_foreign_and_ambiguous_route_requests_refuse_atomically() {
    let fixture = Fixture::new();
    let current = &fixture.checkpoint;
    let ask = &fixture.references[4];
    let selected = fixture
        .select(
            current,
            ask,
            fixture.source,
            current.basis(),
            &fixture.policy,
            false,
        )
        .unwrap();
    assert_eq!(
        fixture.select(
            &selected,
            ask,
            fixture.source,
            selected.basis(),
            &fixture.policy,
            false
        ),
        Err(BeatSelectionError::Prerequisite)
    );
    assert_eq!(
        fixture.select(
            current,
            ask,
            fixture.source,
            current.basis(),
            &fixture.policy,
            true
        ),
        Err(BeatSelectionError::Selection)
    );
    assert_eq!(
        fixture.select(
            current,
            ask,
            fact_id(99),
            current.basis(),
            &fixture.policy,
            false
        ),
        Err(BeatSelectionError::Source)
    );
    assert_eq!(
        fixture.select(
            current,
            ask,
            fixture.source,
            Basis {
                revision: current.basis().revision.next_sequence().unwrap(),
                ..current.basis()
            },
            &fixture.policy,
            false,
        ),
        Err(BeatSelectionError::Binding(CheckpointError::StaleBasis))
    );
    assert_eq!(
        fixture.select(
            current,
            ask,
            fixture.source,
            current.basis(),
            &model::label("foreign-policy"),
            false
        ),
        Err(BeatSelectionError::StalePolicy)
    );
    let mut unsupported = current.state().clone();
    unsupported.decisions.clear();
    let unsupported = fixture.checkpoint_with(unsupported);
    assert_eq!(
        fixture.select(
            &unsupported,
            ask,
            fixture.source,
            unsupported.basis(),
            &fixture.policy,
            false
        ),
        Err(BeatSelectionError::Source)
    );
    assert_eq!(current, &fixture.checkpoint);
    assert_eq!(selected.state().facts, current.state().facts);
}

#[test]
fn missing_understanding_thread_and_source_audience_refuse_without_fabricating_access() {
    let fixture = Fixture::new();
    let current = &fixture.checkpoint;
    let ask = &fixture.references[4];
    for case in 0..4 {
        let mut state = current.state().clone();
        let expected = match case {
            0 => {
                state.narrative.accepted_facts.clear();
                BeatSelectionError::Source
            }
            1 => {
                state.narrative.open_threads.clear();
                BeatSelectionError::Prerequisite
            }
            2 => {
                state.facts[0].audience = AudienceScope::Host;
                BeatSelectionError::Recipient
            }
            3 => {
                state.facts[0].value = FactValue::ContentEvent {
                    definition: fixture.references[8].clone(),
                    subjects: vec![],
                };
                BeatSelectionError::Source
            }
            _ => unreachable!(),
        };
        let changed = fixture.checkpoint_with(state);
        let original = changed.clone();
        assert_eq!(
            fixture.select(
                &changed,
                ask,
                fixture.source,
                changed.basis(),
                &fixture.policy,
                false,
            ),
            Err(expected),
            "case {case}"
        );
        assert_eq!(changed, original);
        assert!(changed.state().characters[0].choices.is_empty());
        assert_eq!(changed.state().resources[0].value, 4);
    }
}

#[test]
fn refusal_consequence_retains_thread_until_an_admitted_resolution() {
    let fixture = Fixture::new();
    let current = &fixture.checkpoint;
    let selection = [ThreadConsequenceSelection {
        thread: &fixture.references[6],
        event_definition: &fixture.references[7],
        source: fixture.source,
        disposition: ThreadDisposition::Continue,
    }];
    // The accepted source may be recorded once as a continuing consequence.
    let mut state = current.state().clone();
    state.narrative.accepted_facts.clear();
    let unconsumed = fixture.checkpoint_with(state);
    let continued = fixture.progress(&unconsumed, &selection).unwrap();
    assert_eq!(
        continued.checkpoint.state().narrative.open_threads,
        [fixture.references[6].clone()]
    );
    assert_eq!(
        continued.checkpoint.state().narrative.active_beats,
        unconsumed.state().narrative.active_beats
    );
    assert_eq!(continued.evidence[0].source, fixture.source);
    assert_eq!(
        continued.evidence[0].disposition,
        ThreadDisposition::Continue
    );
    assert_eq!(
        fixture.progress(&continued.checkpoint, &selection),
        Err(CheckpointProgressError::Progress(
            ProgressError::RepeatedConsequence
        ))
    );
    let bad = [ThreadConsequenceSelection {
        thread: &fixture.references[6],
        event_definition: &fixture.references[7],
        source: fact_id(98),
        disposition: ThreadDisposition::Continue,
    }];
    assert_eq!(
        fixture.progress(&unconsumed, &bad),
        Err(CheckpointProgressError::Progress(
            ProgressError::MissingFact
        ))
    );
    assert_eq!(
        unconsumed.state().narrative.open_threads,
        [fixture.references[6].clone()]
    );
}
