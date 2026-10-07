// Cross-module compiled-handler sentinels; none establishes a source mechanics golden.
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_model::commands::{CommandLimits, validate_client_command};
use df_rules::{
    DispatchError, DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandHandler,
    RulesCommandInput,
};
use std::cell::{Cell, RefCell};
include!("fixtures.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureRejection {
    UnsupportedMechanic,
}

struct FixtureHandler {
    supplied_pins: CheckpointPins,
    candidate: Checkpoint,
    reject: bool,
    calls: Cell<usize>,
    observed_draws: RefCell<Vec<ActualDraw>>,
}

impl RulesCommandHandler for FixtureHandler {
    type Rejection = FixtureRejection;

    fn pins(&self) -> &CheckpointPins {
        &self.supplied_pins
    }

    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        _: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        self.calls.set(self.calls.get() + 1);
        self.observed_draws.replace(input.supplied_draws.to_vec());
        if self.reject {
            Err(FixtureRejection::UnsupportedMechanic)
        } else {
            Ok(self.candidate.clone())
        }
    }
}

fn input() -> GameInput {
    GameInput::Game(CommandInput {
        basis: basis(),
        observed_revision: basis().revision,
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        member: member(3),
        command: GameCommand::Speak {
            speaker: entity(4),
            text: "fixture".into(),
            conversation: None,
        },
    })
}

fn without_draws(command: &GameInput) -> RulesCommandInput<'_> {
    RulesCommandInput {
        command,
        supplied_draws: &[],
    }
}

fn candidate_with(
    current: &Checkpoint,
    candidate_basis: Basis,
    candidate_pins: CheckpointPins,
    decision: bool,
) -> Checkpoint {
    let mut supplied = current.state().clone();
    for pending in &mut supplied.pending {
        pending.basis = candidate_basis;
    }
    if decision {
        supplied.decisions.push(AcceptedDecision {
            operation: OperationId::from_bytes(&[6; 16]).unwrap(),
            revision: candidate_basis.revision,
            facts: vec![],
            draws: vec![],
            effects: vec![],
            source_policy: label("fixture-source-policy"),
            semantic_output: None,
        });
    }
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        candidate_basis,
        candidate_pins,
        supplied,
        ReferenceInventory {
            rules: &[rule()],
            content: &[content()],
            resources: &resource_constraints(),
            assets: &[],
        },
        limits(),
    )
    .unwrap()
}

fn fixture(current: &Checkpoint) -> FixtureHandler {
    let mut next = current.basis();
    next.revision = next.revision.next_sequence().unwrap();
    FixtureHandler {
        supplied_pins: current.pins().clone(),
        candidate: candidate_with(current, next, current.pins().clone(), true),
        reject: false,
        calls: Cell::new(0),
        observed_draws: RefCell::new(vec![]),
    }
}

fn registry<'a>(
    pins: &'a CheckpointPins,
    entries: &'a [CatalogEntry<'a, RuleReference>],
    registrations: &'a [HandlerRegistration<'a, FixtureHandler>],
) -> DispatchRegistry<'a, FixtureHandler> {
    let catalog = CatalogSnapshot::from_published(
        &pins.rules.catalog,
        pins,
        b"synthetic-source-index",
        entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 4,
            max_item_bytes: 64,
            max_total_item_bytes: 128,
        },
    )
    .unwrap();
    DispatchRegistry::from_catalog(catalog, registrations, 4).unwrap()
}

#[test]
fn canonical_admission_then_exact_source_selection_invokes_one_compiled_handler() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let request = input();
    let selector = label("fixture-selector");
    let source = rule();
    let handler = fixture(&current);
    let entries = [CatalogEntry::new(&source, b"opaque-source-parameters")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);
    validate_client_command(
        &request,
        &current,
        ReferenceInventory {
            rules: &[rule()],
            content: &[content()],
            resources: &[],
            assets: &[],
        },
        CommandLimits {
            maximum_records: 8,
            maximum_text_bytes: 32,
            maximum_retained_bytes: 8192,
        },
    )
    .unwrap();

    let candidate = registry
        .stage(
            current.pins(),
            &selector,
            &source,
            without_draws(&request),
            &current,
            1024 * 1024,
        )
        .unwrap();

    assert_eq!(handler.calls.get(), 1);
    assert_eq!(candidate, handler.candidate);
    assert_eq!(current, before);
    assert_eq!(candidate.state().resources, current.state().resources);
    assert!(candidate.state().draws.is_empty());
}

#[test]
fn unknown_or_unsupported_source_never_invokes_the_known_handler() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let selector = label("fixture-selector");
    let source = rule();
    let handler = fixture(&current);
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);
    let mut other = source.clone();
    other.clause = label("unsupported-clause");

    assert_eq!(
        registry.stage(
            current.pins(),
            &label("unknown"),
            &source,
            without_draws(&input()),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::Dispatch(DispatchError::UnknownHandler))
    );
    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &other,
            without_draws(&input()),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::Dispatch(DispatchError::UnsupportedSource))
    );
    assert_eq!(handler.calls.get(), 0);
    assert_eq!(current, before);
}

#[test]
fn registry_or_handler_pin_mismatch_refuses_before_invocation() {
    let current = checkpoint(state()).unwrap();
    let selector = label("fixture-selector");
    let source = rule();
    let mut handler = fixture(&current);
    handler.supplied_pins.rules.handler_digest = ContentDigest([90; 32]);
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);
    let mut changed = current.pins().clone();
    changed.content.content_digest = ContentDigest([91; 32]);

    assert_eq!(
        registry.stage(
            &changed,
            &selector,
            &source,
            without_draws(&input()),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::Dispatch(DispatchError::PinsMismatch))
    );
    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            without_draws(&input()),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::HandlerRulesMismatch)
    );
    assert_eq!(handler.calls.get(), 0);
}

#[test]
fn typed_mechanical_rejection_returns_no_candidate_or_mutation() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let selector = label("fixture-selector");
    let source = rule();
    let mut handler = fixture(&current);
    handler.reject = true;
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);

    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            without_draws(&input()),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::Handler(
            FixtureRejection::UnsupportedMechanic
        ))
    );
    assert_eq!(handler.calls.get(), 1);
    assert_eq!(current, before);
}

#[test]
fn malformed_candidate_basis_decision_pins_and_capacity_are_explicit_refusals() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let selector = label("fixture-selector");
    let source = rule();
    for case in 0..4 {
        let mut handler = fixture(&current);
        let expected = match case {
            0 => {
                handler.candidate =
                    candidate_with(&current, current.basis(), current.pins().clone(), true);
                InvocationError::CandidateBasisMismatch
            }
            1 => {
                handler.candidate = candidate_with(
                    &current,
                    handler.candidate.basis(),
                    current.pins().clone(),
                    false,
                );
                InvocationError::MissingAcceptedDecision
            }
            2 => {
                let mut changed = current.pins().clone();
                changed.rules.handler_digest = ContentDigest([90; 32]);
                handler.candidate =
                    candidate_with(&current, handler.candidate.basis(), changed, true);
                InvocationError::CandidateRulesMismatch
            }
            _ => InvocationError::Capacity,
        };
        let entries = [CatalogEntry::new(&source, b"source")];
        let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
        let registry = registry(current.pins(), &entries, &registrations);
        let maximum = if case == 3 { 1 } else { 1024 * 1024 };

        assert_eq!(
            registry.stage(
                current.pins(),
                &selector,
                &source,
                without_draws(&input()),
                &current,
                maximum,
            ),
            Err(expected)
        );
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
    }
}

#[test]
fn earlier_same_epoch_input_remains_admissible_but_future_input_never_invokes() {
    let current = checkpoint(state()).unwrap();
    let selector = label("fixture-selector");
    let source = rule();
    let handler = fixture(&current);
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);
    let mut request = input();
    if let GameInput::Game(command) = &mut request {
        command.basis.revision = revision(2, 7);
        command.observed_revision = revision(2, 7);
    }
    assert!(
        registry
            .stage(
                current.pins(),
                &selector,
                &source,
                without_draws(&request),
                &current,
                1024 * 1024,
            )
            .is_ok()
    );
    assert_eq!(handler.calls.get(), 1);
    if let GameInput::Game(command) = &mut request {
        command.basis.revision = revision(2, 9);
    }
    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            without_draws(&request),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::InputBasisMismatch)
    );
    assert_eq!(handler.calls.get(), 1);
}

#[test]
fn zero_output_bound_refuses_before_any_handler_invocation() {
    let current = checkpoint(state()).unwrap();
    let selector = label("fixture-selector");
    let source = rule();
    let handler = fixture(&current);
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);

    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            without_draws(&input()),
            &current,
            0,
        ),
        Err(InvocationError::Capacity)
    );
    assert_eq!(handler.calls.get(), 0);
}

#[test]
fn previously_accepted_operation_is_never_staged_again() {
    let initial = checkpoint(state()).unwrap();
    let handler = fixture(&initial);
    let current = handler.candidate.clone();
    let selector = label("fixture-selector");
    let source = rule();
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);

    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            without_draws(&input()),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::AlreadyAccepted)
    );
    assert_eq!(handler.calls.get(), 0);
}

#[test]
fn revision_exhaustion_refuses_before_handler_invocation() {
    let initial = checkpoint(state()).unwrap();
    let handler = fixture(&initial);
    let mut exhausted = initial.basis();
    exhausted.revision = revision(2, u64::MAX);
    let current = candidate_with(&initial, exhausted, initial.pins().clone(), false);
    let selector = label("fixture-selector");
    let source = rule();
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);

    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            without_draws(&input()),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::Revision(
            df_types::RevisionError::SequenceOverflow
        ))
    );
    assert_eq!(handler.calls.get(), 0);
}

fn roll_input() -> GameInput {
    let mut request = input();
    if let GameInput::Game(command) = &mut request {
        command.command = GameCommand::SubmitRoll {
            resolution: ResolutionId::from_bytes(&[10; 16]).unwrap(),
            window: WindowId::from_bytes(&[11; 16]).unwrap(),
        };
    }
    request
}

fn draw(ordinal: u32, sides: u32, value: u32) -> ActualDraw {
    ActualDraw {
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        ordinal,
        resolution: ResolutionId::from_bytes(&[10; 16]).unwrap(),
        window: WindowId::from_bytes(&[11; 16]).unwrap(),
        sides,
        value,
        source: rule(),
    }
}

fn roll_current(retained: Vec<ActualDraw>) -> Checkpoint {
    let mut supplied = state();
    let cause = FactId::from_bytes(&[12; 16]).unwrap();
    supplied.facts.push(GameFact {
        id: cause,
        revision: basis().revision,
        operation: OperationId::from_bytes(&[7; 16]).unwrap(),
        ordinal: 0,
        cause: None,
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: content(),
            subjects: vec![],
        },
    });
    for (index, record) in retained.iter().enumerate() {
        supplied.facts.push(GameFact {
            id: FactId::from_bytes(&[30 + u8::try_from(index).unwrap(); 16]).unwrap(),
            revision: basis().revision,
            operation: record.operation,
            ordinal: record.ordinal,
            cause: Some(cause),
            audience: AudienceScope::Shared,
            value: FactValue::DrawAccepted {
                operation: record.operation,
                ordinal: record.ordinal,
            },
        });
    }
    supplied.draws = retained;
    supplied.pending.push(PendingResolution {
        id: ResolutionId::from_bytes(&[10; 16]).unwrap(),
        basis: basis(),
        continuation: label("fixture-roll-continuation"),
        window: ResolutionWindow {
            id: WindowId::from_bytes(&[11; 16]).unwrap(),
            phase: TriggerPhase::BeforeDraw,
            causal_fact: cause,
            source: rule(),
            timer: None,
        },
        next: PendingInput::Roll {
            participant: member(3),
            sides: vec![20, 6],
            source: rule(),
        },
        choices: vec![],
        draw_ordinals: supplied
            .draws
            .iter()
            .map(|record| (record.operation, record.ordinal))
            .collect(),
        spent: vec![],
        rulings: vec![],
    });
    checkpoint(supplied).unwrap()
}

fn draw_candidate(current: &Checkpoint, supplied_draws: &[ActualDraw]) -> Checkpoint {
    let mut next = current.basis();
    next.revision = next.revision.next_sequence().unwrap();
    let operation = OperationId::from_bytes(&[6; 16]).unwrap();
    let mut supplied = current.state().clone();
    supplied.pending.clear();
    let mut fact_ids = vec![];
    let first_fact_ordinal = u32::try_from(
        supplied
            .facts
            .iter()
            .filter(|fact| fact.operation == operation)
            .count(),
    )
    .unwrap();
    for (index, record) in supplied_draws.iter().enumerate() {
        let id = FactId::from_bytes(&[60 + u8::try_from(index).unwrap(); 16]).unwrap();
        supplied.facts.push(GameFact {
            id,
            revision: next.revision,
            operation,
            ordinal: first_fact_ordinal
                .checked_add(u32::try_from(index).unwrap())
                .unwrap(),
            cause: None,
            audience: AudienceScope::Shared,
            value: FactValue::DrawAccepted {
                operation,
                ordinal: record.ordinal,
            },
        });
        fact_ids.push(id);
    }
    supplied.draws.extend_from_slice(supplied_draws);
    supplied.decisions.push(AcceptedDecision {
        operation,
        revision: next.revision,
        facts: fact_ids,
        draws: supplied
            .draws
            .iter()
            .filter(|record| record.operation == operation)
            .map(|record| record.ordinal)
            .collect(),
        effects: vec![],
        source_policy: label("fixture-source-policy"),
        semantic_output: None,
    });
    rebuilt(current, next, supplied)
}

fn rebuilt(current: &Checkpoint, basis: Basis, state: GameState) -> Checkpoint {
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis,
        current.pins().clone(),
        state,
        ReferenceInventory {
            rules: &[rule()],
            content: &[content()],
            resources: &resource_constraints(),
            assets: &[],
        },
        limits(),
    )
    .unwrap()
}

fn invoke_roll(
    handler: &FixtureHandler,
    current: &Checkpoint,
    request: &GameInput,
    draws: &[ActualDraw],
    maximum: usize,
) -> Result<Checkpoint, InvocationError<FixtureRejection>> {
    let selector = label("fixture-selector");
    let source = rule();
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, handler)];
    registry(current.pins(), &entries, &registrations).stage(
        current.pins(),
        &selector,
        &source,
        RulesCommandInput {
            command: request,
            supplied_draws: draws,
        },
        current,
        maximum,
    )
}

#[test]
fn supplied_roll_reaches_handler_and_is_accounted_once_without_mutating_current() {
    let current = roll_current(vec![]);
    let before = current.clone();
    let draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    let mut handler = fixture(&current);
    handler.candidate = draw_candidate(&current, &draws);
    let first = invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024).unwrap();
    let second = invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024).unwrap();
    assert_eq!(first, second);
    assert_eq!(*handler.observed_draws.borrow(), draws);
    assert_eq!(first.state().draws, draws);
    assert_eq!(first.state().decisions.last().unwrap().draws, vec![0, 1]);
    assert_eq!(
        first
            .state()
            .facts
            .iter()
            .filter(|fact| matches!(fact.value, FactValue::DrawAccepted { .. }))
            .count(),
        2
    );
    assert_eq!(current, before);
    assert_eq!(first.state().resources, current.state().resources);
}

#[test]
fn accepted_roll_retry_never_invokes_or_consumes_again() {
    let initial = roll_current(vec![]);
    let draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    let mut handler = fixture(&initial);
    handler.candidate = draw_candidate(&initial, &draws);
    let current = handler.candidate.clone();
    assert_eq!(
        invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024),
        Err(InvocationError::AlreadyAccepted)
    );
    assert_eq!(handler.calls.get(), 0);
}

#[test]
fn malformed_supplied_draws_refuse_before_invocation_and_preserve_current() {
    let current = roll_current(vec![]);
    let before = current.clone();
    for case in 0..10 {
        let handler = fixture(&current);
        let mut draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
        let expected = match case {
            0 => {
                draws[0].operation = OperationId::from_bytes(&[90; 16]).unwrap();
                InvocationError::DrawOperationMismatch
            }
            1 => {
                draws[0].source.catalog = label("foreign-catalog");
                InvocationError::DrawSourceMismatch
            }
            2 => {
                draws[0].value = 0;
                InvocationError::InvalidDrawInput
            }
            3 => {
                draws[0].value = 21;
                InvocationError::InvalidDrawInput
            }
            4 => {
                draws[0].sides = 0;
                InvocationError::InvalidDrawInput
            }
            5 => {
                draws[1].ordinal = 0;
                InvocationError::DrawAlreadyConsumed
            }
            6 => {
                draws[1].ordinal = 2;
                InvocationError::DrawOrderMismatch
            }
            7 => {
                draws[0].resolution = ResolutionId::from_bytes(&[90; 16]).unwrap();
                InvocationError::RollInputMismatch
            }
            8 => {
                draws[0].source.clause = label("foreign-clause");
                InvocationError::RollInputMismatch
            }
            _ => {
                draws[0].window = WindowId::from_bytes(&[90; 16]).unwrap();
                InvocationError::RollInputMismatch
            }
        };
        assert_eq!(
            invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024),
            Err(expected),
            "case {case}"
        );
        assert_eq!(handler.calls.get(), 0);
        assert_eq!(current, before);
    }
}

#[test]
fn pending_roll_participant_window_kind_count_and_sides_are_authoritative() {
    let current = roll_current(vec![]);
    for case in 0..7 {
        let mut request = roll_input();
        let mut draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
        let mut state = current.state().clone();
        match case {
            0 => {
                if let GameInput::Game(command) = &mut request {
                    command.member = member(90);
                }
            }
            1 => {
                if let GameInput::Game(command) = &mut request {
                    command.command = GameCommand::SubmitRoll {
                        resolution: ResolutionId::from_bytes(&[90; 16]).unwrap(),
                        window: draws[0].window,
                    };
                }
            }
            2 => {
                state.pending.clear();
            }
            3 => {
                state.pending[0].next = PendingInput::Choice {
                    remaining: vec![OfferedResponse {
                        participant: member(3),
                        offer: label("fixture-offer"),
                        options: vec![label("fixture-choice")],
                        source: rule(),
                    }],
                };
            }
            4 => {
                draws.pop();
            }
            5 => {
                draws[0].sides = 19;
            }
            _ => {
                draws.clear();
            }
        }
        let current = rebuilt(&current, current.basis(), state);
        let handler = fixture(&current);
        assert_eq!(
            invoke_roll(&handler, &current, &request, &draws, 1024 * 1024),
            Err(InvocationError::RollInputMismatch),
            "case {case}"
        );
        assert_eq!(handler.calls.get(), 0);
    }
}

#[test]
fn candidate_cannot_drop_add_or_alter_explicit_draws() {
    let current = roll_current(vec![]);
    let before = current.clone();
    let draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    for case in 0..3 {
        let mut handler = fixture(&current);
        let mut candidate_draws = draws.clone();
        match case {
            0 => {
                candidate_draws.pop();
            }
            1 => {
                candidate_draws.push(draw(2, 8, 3));
            }
            _ => {
                candidate_draws[0].value = 1;
            }
        }
        handler.candidate = draw_candidate(&current, &candidate_draws);
        assert_eq!(
            invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024),
            Err(InvocationError::CandidateDrawMismatch)
        );
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
    }
}

#[test]
fn candidate_must_link_each_new_draw_once_to_its_decision_and_facts() {
    let current = roll_current(vec![]);
    let draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    for case in 0..5 {
        let mut handler = fixture(&current);
        let candidate = draw_candidate(&current, &draws);
        let mut state = candidate.state().clone();
        match case {
            0 => {
                state.decisions[0].draws.pop();
            }
            1 => {
                state.decisions[0].draws.reverse();
            }
            2 => {
                state.decisions[0].facts.pop();
            }
            3 => {
                state.facts.last_mut().unwrap().value = FactValue::DrawAccepted {
                    operation: draws[0].operation,
                    ordinal: 0,
                };
            }
            _ => {
                state.facts.last_mut().unwrap().value = FactValue::ContentEvent {
                    definition: content(),
                    subjects: vec![],
                };
            }
        }
        handler.candidate = rebuilt(&current, candidate.basis(), state);
        assert_eq!(
            invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024),
            Err(InvocationError::CandidateDrawAccountingMismatch),
            "case {case}"
        );
    }
}

#[test]
fn retained_same_operation_draws_continue_and_are_not_recorded_as_new_facts() {
    let retained = vec![draw(0, 8, 5)];
    let current = roll_current(retained.clone());
    let draws = vec![draw(1, 20, 17), draw(2, 6, 4)];
    let mut handler = fixture(&current);
    handler.candidate = draw_candidate(&current, &draws);
    let candidate = invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024).unwrap();
    assert_eq!(
        candidate.state().decisions.last().unwrap().draws,
        vec![0, 1, 2]
    );
    assert!(candidate.state().facts.starts_with(&current.state().facts));
    assert_eq!(
        candidate.state().facts.len(),
        current.state().facts.len() + 2
    );
    assert!(candidate.state().draws.starts_with(&retained));
    let reused = vec![draw(0, 20, 17), draw(1, 6, 4)];
    assert_eq!(
        invoke_roll(&handler, &current, &roll_input(), &reused, 1024 * 1024),
        Err(InvocationError::DrawAlreadyConsumed)
    );
    assert_eq!(handler.calls.get(), 1);
}

#[test]
fn retained_other_operation_draws_cannot_be_rewritten_by_a_new_roll() {
    let mut prior = draw(0, 8, 5);
    prior.operation = OperationId::from_bytes(&[9; 16]).unwrap();
    let current = roll_current(vec![prior]);
    let draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    let mut handler = fixture(&current);
    handler.candidate = draw_candidate(&current, &draws);
    assert!(invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024).is_ok());
    let mut state = handler.candidate.state().clone();
    state.draws[0].value = 1;
    handler.candidate = rebuilt(&current, handler.candidate.basis(), state);
    assert_eq!(
        invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024),
        Err(InvocationError::CandidateDrawMismatch)
    );
}

#[test]
fn supplied_draw_memory_budget_refuses_before_handler_invocation() {
    let current = roll_current(vec![]);
    let draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    let handler = fixture(&current);
    let record_bytes = std::mem::size_of_val(draws.as_slice());
    for maximum in [record_bytes - 1, record_bytes] {
        assert_eq!(
            invoke_roll(&handler, &current, &roll_input(), &draws, maximum),
            Err(InvocationError::Capacity)
        );
    }
    assert_eq!(handler.calls.get(), 0);
}

fn current_with_accepted_history() -> Checkpoint {
    let mut prior = draw(0, 8, 5);
    prior.operation = OperationId::from_bytes(&[9; 16]).unwrap();
    let current = roll_current(vec![prior]);
    let mut state = current.state().clone();
    state.decisions.push(AcceptedDecision {
        operation: OperationId::from_bytes(&[9; 16]).unwrap(),
        revision: current.basis().revision,
        facts: vec![FactId::from_bytes(&[30; 16]).unwrap()],
        draws: vec![0],
        effects: vec![],
        source_policy: label("fixture-source-policy"),
        semantic_output: Some("retained-server-outcome".into()),
    });
    rebuilt(&current, current.basis(), state)
}

#[test]
fn new_roll_preserves_accepted_history_and_prior_retry_never_reaches_handler() {
    let current = current_with_accepted_history();
    let before = current.clone();
    let draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    let mut handler = fixture(&current);
    handler.candidate = draw_candidate(&current, &draws);
    let candidate = invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024).unwrap();
    assert!(
        candidate
            .state()
            .decisions
            .starts_with(&current.state().decisions)
    );
    assert_eq!(
        candidate.state().decisions.len(),
        current.state().decisions.len() + 1
    );
    assert_eq!(current, before);
    let mut retry = roll_input();
    if let GameInput::Game(command) = &mut retry {
        command.operation = OperationId::from_bytes(&[9; 16]).unwrap();
    }
    assert_eq!(
        invoke_roll(&handler, &candidate, &retry, &[], 1024 * 1024),
        Err(InvocationError::AlreadyAccepted)
    );
    assert_eq!(handler.calls.get(), 1);
}

#[test]
fn canonically_valid_candidate_cannot_drop_an_earlier_accepted_decision() {
    let current = current_with_accepted_history();
    let before = current.clone();
    let draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    let mut handler = fixture(&current);
    let candidate = draw_candidate(&current, &draws);
    let mut state = candidate.state().clone();
    state.decisions.remove(0);
    handler.candidate = rebuilt(&current, candidate.basis(), state);
    assert_eq!(handler.candidate.state().draws, candidate.state().draws);
    assert_eq!(handler.candidate.state().facts, candidate.state().facts);
    assert_eq!(
        invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024),
        Err(InvocationError::CandidateDecisionHistoryMismatch)
    );
    assert_eq!(handler.calls.get(), 1);
    assert_eq!(current, before);
}

#[test]
fn canonically_valid_candidate_cannot_rewrite_accepted_decision_provenance() {
    let current = current_with_accepted_history();
    let before = current.clone();
    let draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    for case in 0..4 {
        let mut handler = fixture(&current);
        let candidate = draw_candidate(&current, &draws);
        let mut state = candidate.state().clone();
        match case {
            0 => {
                state.decisions[0].semantic_output = Some("changed-server-outcome".into());
            }
            1 => {
                state.decisions[0].source_policy = label("changed-source-policy");
            }
            2 => {
                state.decisions[0].draws.clear();
            }
            _ => {
                state.decisions[0].facts.clear();
            }
        }
        handler.candidate = rebuilt(&current, candidate.basis(), state);
        assert_eq!(
            invoke_roll(&handler, &current, &roll_input(), &draws, 1024 * 1024),
            Err(InvocationError::CandidateDecisionHistoryMismatch),
            "case {case}"
        );
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
    }
}

fn with_foreign_decision(
    current: &Checkpoint,
    candidate: &Checkpoint,
    revision: SessionRevision,
) -> Checkpoint {
    let mut state = candidate.state().clone();
    state.decisions.push(AcceptedDecision {
        operation: OperationId::from_bytes(&[99; 16]).unwrap(),
        revision,
        facts: vec![],
        draws: vec![],
        effects: vec![],
        source_policy: label("fixture-foreign-policy"),
        semantic_output: None,
    });
    rebuilt(current, candidate.basis(), state)
}

#[test]
fn canonical_requested_plus_foreign_decision_is_not_one_atomic_command() {
    for with_history in [false, true] {
        let current = if with_history {
            current_with_accepted_history()
        } else {
            checkpoint(state()).unwrap()
        };
        let before = current.clone();
        for backdated in [false, true] {
            let mut handler = fixture(&current);
            let foreign_revision = if backdated {
                current.basis().revision
            } else {
                handler.candidate.basis().revision
            };
            handler.candidate =
                with_foreign_decision(&current, &handler.candidate, foreign_revision);
            assert_eq!(
                handler.candidate.state().decisions.len(),
                current.state().decisions.len() + 2
            );
            assert!(
                handler
                    .candidate
                    .state()
                    .decisions
                    .starts_with(&current.state().decisions)
            );

            assert_eq!(
                invoke_roll(&handler, &current, &input(), &[], 1024 * 1024),
                Err(InvocationError::CandidateDecisionHistoryMismatch),
                "history {with_history}, backdated {backdated}"
            );
            assert_eq!(handler.calls.get(), 1);
            assert!(handler.observed_draws.borrow().is_empty());
            assert_eq!(current, before);
        }
    }
}

#[test]
fn one_atomic_decision_preserves_historical_prefix_and_zero_draw_outcome() {
    for with_history in [false, true] {
        let current = if with_history {
            current_with_accepted_history()
        } else {
            checkpoint(state()).unwrap()
        };
        let before = current.clone();
        let handler = fixture(&current);

        let candidate = invoke_roll(&handler, &current, &input(), &[], 1024 * 1024).unwrap();

        assert!(
            candidate
                .state()
                .decisions
                .starts_with(&current.state().decisions)
        );
        assert_eq!(
            candidate.state().decisions.len(),
            current.state().decisions.len() + 1
        );
        let appended = candidate.state().decisions.last().unwrap();
        assert_eq!(
            appended.operation,
            OperationId::from_bytes(&[6; 16]).unwrap()
        );
        assert_eq!(
            appended.revision,
            current.basis().revision.next_sequence().unwrap()
        );
        assert!(appended.draws.is_empty());
        assert_eq!(candidate.state().facts, current.state().facts);
        assert_eq!(candidate.state().draws, current.state().draws);
        assert_eq!(handler.calls.get(), 1);
        assert!(handler.observed_draws.borrow().is_empty());
        assert_eq!(current, before);
    }
}

#[test]
fn atomic_decision_count_preserves_existing_capacity_history_and_draw_refusal_priority() {
    for case in 0..4 {
        let current = if case < 2 {
            checkpoint(state()).unwrap()
        } else {
            current_with_accepted_history()
        };
        let before = current.clone();
        let mut handler = fixture(&current);
        if case == 1 {
            handler.candidate = candidate_with(
                &current,
                handler.candidate.basis(),
                current.pins().clone(),
                false,
            );
        }
        handler.candidate = with_foreign_decision(
            &current,
            &handler.candidate,
            handler.candidate.basis().revision,
        );
        let expected = match case {
            0 => InvocationError::Capacity,
            1 => InvocationError::MissingAcceptedDecision,
            2 => {
                let mut state = handler.candidate.state().clone();
                state.decisions.first_mut().unwrap().source_policy = label("rewritten-history");
                handler.candidate = rebuilt(&current, handler.candidate.basis(), state);
                InvocationError::CandidateDecisionHistoryMismatch
            }
            _ => {
                let mut state = handler.candidate.state().clone();
                state.draws.first_mut().unwrap().value = 1;
                handler.candidate = rebuilt(&current, handler.candidate.basis(), state);
                InvocationError::CandidateDrawMismatch
            }
        };
        let maximum = if case < 2 { 1 } else { 1024 * 1024 };

        assert_eq!(
            invoke_roll(&handler, &current, &input(), &[], maximum),
            Err(expected),
            "case {case}"
        );
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
    }
}

fn alternate_intent_definition() -> ContentReference {
    ContentReference {
        package: content().package,
        entry: label("fixture-alternate-intent"),
    }
}

fn intent_checkpoint(current: &Checkpoint, basis: Basis, state: GameState) -> Checkpoint {
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis,
        current.pins().clone(),
        state,
        ReferenceInventory {
            rules: &[rule()],
            content: &[content(), alternate_intent_definition()],
            resources: &resource_constraints(),
            assets: &[],
        },
        limits(),
    )
    .unwrap()
}

fn current_with_retained_intents(base: &Checkpoint, referenced: bool, timer: bool) -> Checkpoint {
    let mut state = base.state().clone();
    for value in [40, 44] {
        let operation = OperationId::from_bytes(&[value + 30; 16]).unwrap();
        let id = EffectId::from_bytes(&[value; 16]).unwrap();
        state.intents.push(DurableIntent {
            id,
            basis: base.basis(),
            operation,
            slot: 0,
            kind: if timer && value == 40 {
                EffectKind::ArmTimer
            } else {
                EffectKind::RunAi
            },
            job: if timer && value == 40 {
                None
            } else {
                Some(JobId::from_bytes(&[value + 1; 16]).unwrap())
            },
            timer: if timer && value == 40 {
                Some(TimerId::from_bytes(&[42; 16]).unwrap())
            } else {
                None
            },
            generation: 1,
            status: DurableStatus::Pending,
            definition: content(),
        });
        state.decisions.push(AcceptedDecision {
            operation,
            revision: base.basis().revision,
            facts: vec![],
            draws: vec![],
            effects: if referenced || value != 40 {
                vec![id]
            } else {
                vec![]
            },
            source_policy: label("fixture-intent-policy"),
            semantic_output: None,
        });
    }
    if timer {
        for value in [42, 43] {
            state.timers.push(OwnedTimer {
                id: TimerId::from_bytes(&[value; 16]).unwrap(),
                basis: base.basis(),
                generation: 1,
                due: state.logical_time,
                source: rule(),
                status: DurableStatus::Pending,
            });
        }
    }
    intent_checkpoint(base, base.basis(), state)
}

#[test]
fn retained_intent_binding_changes_refuse_canonically_valid_candidates() {
    for field in [
        "id",
        "basis",
        "operation",
        "slot",
        "kind",
        "job",
        "timer",
        "generation",
        "definition",
    ] {
        // Unreferenced intents exercise identity/operation preservation independently of the
        // canonical accepted-decision effect-reference validator.
        let current = current_with_retained_intents(
            &checkpoint(state()).unwrap(),
            field != "id" && field != "operation",
            field == "timer",
        );
        let before = current.clone();
        let mut handler = fixture(&current);
        let mut state = handler.candidate.state().clone();
        let intent = &mut state.intents[0];
        match field {
            "id" => intent.id = EffectId::from_bytes(&[50; 16]).unwrap(),
            "basis" => intent.basis.revision = revision(2, 7),
            "operation" => intent.operation = OperationId::from_bytes(&[72; 16]).unwrap(),
            "slot" => intent.slot = 1,
            "kind" => intent.kind = EffectKind::RunMedia,
            "job" => intent.job = Some(JobId::from_bytes(&[50; 16]).unwrap()),
            "timer" => intent.timer = Some(TimerId::from_bytes(&[43; 16]).unwrap()),
            "generation" => intent.generation = 2,
            "definition" => intent.definition = alternate_intent_definition(),
            _ => unreachable!(),
        }
        // Construction proves the candidate passes canonical shape/reference validation.
        handler.candidate = intent_checkpoint(&current, handler.candidate.basis(), state);
        assert!(
            handler
                .candidate
                .state()
                .decisions
                .starts_with(&current.state().decisions)
        );
        assert_eq!(
            invoke_roll(&handler, &current, &input(), &[], 1024 * 1024),
            Err(InvocationError::CandidateIntentBindingMismatch),
            "field {field}"
        );
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
    }
}

#[test]
fn unreferenced_retained_intent_deletion_is_refused() {
    let current = current_with_retained_intents(&checkpoint(state()).unwrap(), false, false);
    let before = current.clone();
    let mut handler = fixture(&current);
    let mut state = handler.candidate.state().clone();
    state.intents.remove(0);
    handler.candidate = intent_checkpoint(&current, handler.candidate.basis(), state);

    assert_eq!(
        invoke_roll(&handler, &current, &input(), &[], 1024 * 1024),
        Err(InvocationError::CandidateIntentBindingMismatch)
    );
    assert_eq!(handler.calls.get(), 1);
    assert_eq!(current, before);
}

#[test]
fn retained_intent_status_only_updates_including_completion_are_accepted() {
    for status in [
        DurableStatus::Pending,
        DurableStatus::Claimed,
        DurableStatus::SentUnknown,
        DurableStatus::Completed,
        DurableStatus::Failed,
        DurableStatus::Cancelled,
    ] {
        let current = current_with_retained_intents(&checkpoint(state()).unwrap(), true, false);
        let before = current.clone();
        let mut handler = fixture(&current);
        let mut state = handler.candidate.state().clone();
        state.intents[0].status = status;
        handler.candidate = intent_checkpoint(&current, handler.candidate.basis(), state);

        let candidate = invoke_roll(&handler, &current, &input(), &[], 1024 * 1024).unwrap();
        let mut expected = current.state().intents.clone();
        expected[0].status = status;
        assert_eq!(candidate.state().intents, expected);
        assert!(
            candidate
                .state()
                .decisions
                .starts_with(&current.state().decisions)
        );
        assert_eq!(candidate.state().facts, current.state().facts);
        assert_eq!(candidate.state().draws, current.state().draws);
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
    }
}

#[test]
fn reordered_retained_intents_and_new_requested_effect_are_accepted() {
    let current = current_with_retained_intents(&checkpoint(state()).unwrap(), true, true);
    let before = current.clone();
    let mut handler = fixture(&current);
    let mut state = handler.candidate.state().clone();
    state.intents.reverse();
    let id = EffectId::from_bytes(&[60; 16]).unwrap();
    state.intents.insert(
        1,
        DurableIntent {
            id,
            basis: handler.candidate.basis(),
            operation: OperationId::from_bytes(&[6; 16]).unwrap(),
            slot: 0,
            kind: EffectKind::PublishPresentation,
            job: None,
            timer: None,
            generation: 1,
            status: DurableStatus::Pending,
            definition: content(),
        },
    );
    state.decisions.last_mut().unwrap().effects.push(id);
    handler.candidate = intent_checkpoint(&current, handler.candidate.basis(), state);

    let candidate = invoke_roll(&handler, &current, &input(), &[], 1024 * 1024).unwrap();
    for prior in &current.state().intents {
        assert_eq!(
            candidate
                .state()
                .intents
                .iter()
                .find(|intent| intent.id == prior.id),
            Some(prior)
        );
    }
    assert_eq!(
        candidate.state().intents.len(),
        current.state().intents.len() + 1
    );
    assert_eq!(
        candidate.state().decisions.last().unwrap().effects,
        vec![id]
    );
    assert_eq!(handler.calls.get(), 1);
    assert_eq!(current, before);
}

#[test]
fn retained_intent_guard_preserves_capacity_draw_history_and_atomic_refusal_priority() {
    for case in 0..4 {
        let base = if case == 0 {
            checkpoint(state()).unwrap()
        } else {
            current_with_accepted_history()
        };
        let current = current_with_retained_intents(&base, true, false);
        let before = current.clone();
        let mut handler = fixture(&current);
        let mut state = handler.candidate.state().clone();
        state.intents[0].generation += 1;
        let expected = match case {
            0 => InvocationError::Capacity,
            1 => {
                state.draws[0].value = 1;
                InvocationError::CandidateDrawMismatch
            }
            2 => {
                state.decisions[0].source_policy = label("rewritten-intent-history");
                InvocationError::CandidateDecisionHistoryMismatch
            }
            _ => {
                state.decisions.push(AcceptedDecision {
                    operation: OperationId::from_bytes(&[99; 16]).unwrap(),
                    revision: handler.candidate.basis().revision,
                    facts: vec![],
                    draws: vec![],
                    effects: vec![],
                    source_policy: label("fixture-foreign-policy"),
                    semantic_output: None,
                });
                InvocationError::CandidateDecisionHistoryMismatch
            }
        };
        handler.candidate = intent_checkpoint(&current, handler.candidate.basis(), state);
        let maximum = if case == 0 { 1 } else { 1024 * 1024 };

        assert_eq!(
            invoke_roll(&handler, &current, &input(), &[], maximum),
            Err(expected),
            "case {case}"
        );
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
    }
}

fn appended_content_fact(candidate: &Checkpoint, id: u8, ordinal: u32) -> GameFact {
    GameFact {
        id: FactId::from_bytes(&[id; 16]).unwrap(),
        revision: candidate.basis().revision,
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        ordinal,
        cause: candidate.state().facts.last().map(|fact| fact.id),
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: content(),
            subjects: vec![entity(4)],
        },
    }
}

fn invalid_fact_projection(current: &Checkpoint, candidate: &Checkpoint, case: u8) -> Checkpoint {
    let mut state = candidate.state().clone();
    let mut first = appended_content_fact(candidate, 80, 0);
    if case == 1 || case == 3 {
        first.operation = OperationId::from_bytes(&[99; 16]).unwrap();
    }
    if case == 2 || case == 3 {
        first.revision = current.basis().revision;
    }
    state.facts.push(first);
    if case >= 4 {
        let mut second = appended_content_fact(candidate, 81, 1);
        second.cause = Some(state.facts.last().unwrap().id);
        state.facts.push(second);
        let ids: Vec<_> = state
            .facts
            .iter()
            .rev()
            .take(2)
            .map(|fact| fact.id)
            .collect();
        state.decisions.last_mut().unwrap().facts = if case == 4 { ids } else { vec![ids[1]] };
    }
    // All cases are canonically valid; the complete operation projection is the new guard.
    rebuilt(current, candidate.basis(), state)
}

#[test]
fn registered_staging_refuses_orphan_foreign_backdated_and_unordered_appended_facts() {
    for with_history in [false, true] {
        let current = if with_history {
            current_with_accepted_history()
        } else {
            checkpoint(state()).unwrap()
        };
        let before = current.clone();
        let request = input();
        let saved_input = request.clone();
        for case in 0..6 {
            let mut handler = fixture(&current);
            handler.candidate = invalid_fact_projection(&current, &handler.candidate, case);
            assert!(
                handler
                    .candidate
                    .state()
                    .facts
                    .starts_with(&current.state().facts)
            );
            assert!(
                handler
                    .candidate
                    .state()
                    .decisions
                    .starts_with(&current.state().decisions)
            );

            assert_eq!(
                invoke_roll(&handler, &current, &request, &[], 1024 * 1024),
                Err(InvocationError::CandidateFactAccountingMismatch),
                "history {with_history}, case {case}"
            );
            assert_eq!(handler.calls.get(), 1);
            assert!(handler.observed_draws.borrow().is_empty());
            assert_eq!(current, before);
            assert_eq!(request, saved_input);
        }
    }
}

#[test]
fn exact_ordered_fact_projection_allows_every_canonical_value_and_repeatable_staging() {
    let current = current_with_accepted_history();
    let before = current.clone();
    let draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    let request = roll_input();
    let saved_input = request.clone();
    let mut later = current.state().logical_time;
    later.ticks += 1;
    let values = vec![
        FactValue::EntityCreated {
            entity: entity(4),
            definition: content(),
        },
        FactValue::EntityMoved {
            entity: entity(4),
            destination: entity(4),
            position: Position { x: 1, y: 0, z: 0 },
        },
        FactValue::ResourceChanged {
            entity: entity(4),
            resource: label("fixture-resource-1"),
            before: 4,
            after: 3,
            source: rule(),
        },
        FactValue::ChoiceAccepted {
            resolution: draws[0].resolution,
            window: draws[0].window,
            choice: AcceptedChoice {
                participant: member(3),
                offer: label("fixture-offer"),
                selected: label("fixture-selected"),
                source: rule(),
            },
        },
        FactValue::RulingAccepted {
            resolution: draws[0].resolution,
            window: draws[0].window,
            ruling: ScopedRuling {
                adjudicator: member(3),
                selected: label("fixture-ruling"),
                source: rule(),
                audience: AudienceScope::Shared,
            },
        },
        FactValue::TimeAdvanced {
            before: current.state().logical_time,
            after: later,
        },
        FactValue::ContentEvent {
            definition: content(),
            subjects: vec![entity(4)],
        },
    ];
    for value in values {
        let mut handler = fixture(&current);
        let candidate = draw_candidate(&current, &draws);
        let mut state = candidate.state().clone();
        let mut fact = appended_content_fact(&candidate, 80, 2);
        fact.value = value.clone();
        state.decisions.last_mut().unwrap().facts.push(fact.id);
        state.facts.push(fact);
        handler.candidate = rebuilt(&current, candidate.basis(), state);

        let first = invoke_roll(&handler, &current, &request, &draws, 1024 * 1024).unwrap();
        let retry = invoke_roll(&handler, &current, &request, &draws, 1024 * 1024).unwrap();
        assert_eq!(first, retry);
        assert_eq!(first.state().facts.last().unwrap().value, value);
        assert!(first.state().facts.starts_with(&current.state().facts));
        assert_eq!(
            first.state().facts[current.state().facts.len()..]
                .iter()
                .map(|fact| fact.id)
                .collect::<Vec<_>>(),
            first.state().decisions.last().unwrap().facts
        );
        assert_eq!(first.state().draws[current.state().draws.len()..], draws);
        assert_eq!(handler.calls.get(), 2);
        assert_eq!(current, before);
        assert_eq!(request, saved_input);
    }
}

#[test]
fn appended_fact_guard_preserves_capacity_draw_history_atomic_and_intent_refusal_priority() {
    for case in 0..5 {
        let base = if case == 0 {
            checkpoint(state()).unwrap()
        } else {
            current_with_accepted_history()
        };
        let current = if case == 3 {
            current_with_retained_intents(&base, true, false)
        } else {
            base
        };
        let before = current.clone();
        let request = input();
        let saved_input = request.clone();
        let mut handler = fixture(&current);
        let candidate = invalid_fact_projection(&current, &handler.candidate, 0);
        let mut state = candidate.state().clone();
        let expected = match case {
            0 => InvocationError::Capacity,
            1 => {
                state.draws[0].value = 1;
                InvocationError::CandidateDrawMismatch
            }
            2 => {
                state.decisions[0].source_policy = label("changed-fact-history");
                InvocationError::CandidateDecisionHistoryMismatch
            }
            3 => {
                state.intents[0].generation += 1;
                InvocationError::CandidateIntentBindingMismatch
            }
            _ => {
                state.decisions.push(AcceptedDecision {
                    operation: OperationId::from_bytes(&[99; 16]).unwrap(),
                    revision: candidate.basis().revision,
                    facts: vec![],
                    draws: vec![],
                    effects: vec![],
                    source_policy: label("fixture-foreign-policy"),
                    semantic_output: None,
                });
                InvocationError::CandidateDecisionHistoryMismatch
            }
        };
        handler.candidate = rebuilt(&current, candidate.basis(), state);
        let maximum = if case == 0 { 1 } else { 1024 * 1024 };
        assert_eq!(
            invoke_roll(&handler, &current, &request, &[], maximum),
            Err(expected),
            "case {case}"
        );
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
        assert_eq!(request, saved_input);
    }
}

#[test]
fn resolve_contract_composes_pinned_source_time_and_borrowed_actual_draws() {
    use df_rules::preconditions::{
        CurrentRuleContext, PreconditionError, PreconditionLimits, RuleDependency,
        RulePreconditions, validate_rule_preconditions,
    };

    let current = roll_current(vec![]);
    let before = current.clone();
    let request = roll_input();
    let saved_request = request.clone();
    let supplied_draws = vec![draw(0, 20, 17), draw(1, 6, 4)];
    let source = rule();
    let selector = label("fixture-selector");
    let sources = [source.clone()];
    let dependencies = [RuleDependency::LogicalTime];
    let contents = [content()];
    let resources = resource_constraints();
    let command_limits = CommandLimits {
        maximum_records: 8,
        maximum_text_bytes: 32,
        maximum_retained_bytes: 8192,
    };
    let precondition_limits = PreconditionLimits {
        maximum_dependencies: 2,
        // (2 + 1) * (2 + 1 + 1 + 1 + 2 * 1 MiB) * (1 + 1), from validate_bounds.
        maximum_comparisons: 12_582_942,
        maximum_checkpoint_bytes: 1024 * 1024,
    };
    let check_preconditions = |prepared: &Checkpoint| {
        let preconditions = RulePreconditions {
            prepared,
            sources: &sources,
            dependencies: &dependencies,
        };
        let context = CurrentRuleContext {
            checkpoint: &current,
            basis: current.basis(),
            pins: current.pins(),
            inventory: ReferenceInventory {
                rules: &sources,
                content: &contents,
                resources: &resources,
                assets: &[],
            },
            command_limits,
        };
        validate_rule_preconditions(&request, context, &preconditions, precondition_limits)
    };

    let mut stale_state = current.state().clone();
    stale_state.logical_time.ticks += 1;
    let stale_prepared = rebuilt(&current, current.basis(), stale_state);
    let refusal = check_preconditions(&stale_prepared);
    assert_eq!(refusal, Err(PreconditionError::StaleTime));
    assert_eq!(current, before);
    assert_eq!(request, saved_request);

    let mut handler = fixture(&current);
    handler.candidate = draw_candidate(&current, &supplied_draws);
    let entries = [CatalogEntry::new(&source, b"opaque-source-parameters")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);

    assert_eq!(check_preconditions(&current), Ok(()));
    let candidate = registry
        .stage(
            current.pins(),
            &selector,
            &source,
            RulesCommandInput {
                command: &request,
                supplied_draws: &supplied_draws,
            },
            &current,
            1024 * 1024,
        )
        .unwrap();

    let operation = match &request {
        GameInput::Game(command) => command.operation,
        _ => unreachable!(),
    };
    assert_eq!(
        candidate.state().decisions.len(),
        before.state().decisions.len() + 1
    );
    assert_eq!(
        candidate.state().decisions.last().unwrap().operation,
        operation
    );
    assert_eq!(candidate.state().draws, supplied_draws);
    assert!(candidate.state().pending.is_empty());
    assert_eq!(handler.observed_draws.borrow().as_slice(), supplied_draws);
    assert_eq!(handler.calls.get(), 1);
    assert_eq!(current, before);
    assert_eq!(request, saved_request);
}
