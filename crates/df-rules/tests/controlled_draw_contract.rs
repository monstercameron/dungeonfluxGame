// Synthetic structural source/draw fixtures; no standard-rulebook or native supplier qualification.
#[path = "../src/controlled_draw_contract.rs"]
mod controlled_draw_contract;
use controlled_draw_contract::prepare_controlled_roll;
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_model::commands::CommandLimits;
use df_rules::current_responses::{ResponseError, ResponsePreparationError};
use df_rules::preconditions::{
    CurrentRuleContext, PreconditionLimits, PreconditionedCommandHandler, RuleDependency,
    RulePreconditions,
};
use df_rules::{
    DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandHandler, RulesCommandInput,
    stage_handler,
};
use std::cell::{Cell, RefCell};
include!("fixtures.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Rejection {
    UnsupportedMechanic,
}
struct Handler {
    pins: CheckpointPins,
    source: RuleReference,
    candidate: Checkpoint,
    calls: Cell<usize>,
    observed: RefCell<Vec<ActualDraw>>,
    reject: bool,
}
impl RulesCommandHandler for Handler {
    type Rejection = Rejection;
    fn pins(&self) -> &CheckpointPins {
        &self.pins
    }
    fn bound_source(&self) -> Option<&RuleReference> {
        Some(&self.source)
    }
    fn stage(&self, input: RulesCommandInput<'_>, _: &Checkpoint) -> Result<Checkpoint, Rejection> {
        self.calls.set(self.calls.get() + 1);
        self.observed.replace(input.supplied_draws.to_vec());
        if self.reject {
            Err(Rejection::UnsupportedMechanic)
        } else {
            Ok(self.candidate.clone())
        }
    }
}
fn handler(current: &Checkpoint, draws: &[ActualDraw]) -> Handler {
    Handler {
        pins: current.pins().clone(),
        source: rule(),
        candidate: draw_candidate(current, draws),
        calls: Cell::new(0),
        observed: RefCell::new(vec![]),
        reject: false,
    }
}
fn request() -> GameInput {
    GameInput::Game(CommandInput {
        basis: basis(),
        observed_revision: basis().revision,
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        member: member(3),
        command: GameCommand::SubmitRoll {
            resolution: ResolutionId::from_bytes(&[10; 16]).unwrap(),
            window: WindowId::from_bytes(&[11; 16]).unwrap(),
        },
    })
}
fn invoke(
    handler: &Handler,
    current: &Checkpoint,
    request: &GameInput,
    draws: &[ActualDraw],
) -> Result<Checkpoint, ResponsePreparationError<Rejection>> {
    let sources = [rule()];
    let contents = [content()];
    let resources = resource_constraints();
    let context = || CurrentRuleContext {
        checkpoint: current,
        basis: current.basis(),
        pins: current.pins(),
        inventory: ReferenceInventory {
            rules: &sources,
            content: &contents,
            resources: &resources,
            assets: &[],
        },
        command_limits: CommandLimits {
            maximum_records: 100,
            maximum_text_bytes: 256,
            maximum_retained_bytes: 1024 * 1024,
        },
    };
    let dependencies = [
        RuleDependency::PendingResolution(ResolutionId::from_bytes(&[10; 16]).unwrap()),
        RuleDependency::LogicalTime,
    ];
    let wrapped = PreconditionedCommandHandler::new(
        handler,
        &sources[0],
        context(),
        RulePreconditions {
            prepared: current,
            sources: &sources,
            dependencies: &dependencies,
        },
        PreconditionLimits {
            maximum_dependencies: 3,
            maximum_comparisons: 12_582_942,
            maximum_checkpoint_bytes: 1024 * 1024,
        },
    );
    let entries = [CatalogEntry::new(&sources[0], b"synthetic-roll-source")];
    let selector = label("fixture-selector");
    let registrations = [HandlerRegistration::new(&selector, &sources[0], &wrapped)];
    let catalog = CatalogSnapshot::from_published(
        &current.pins().rules.catalog,
        current.pins(),
        b"synthetic-source-index",
        &entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 4,
            max_item_bytes: 64,
            max_total_item_bytes: 128,
        },
    )
    .unwrap();
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 4).unwrap();
    prepare_controlled_roll(
        RulesCommandInput {
            command: request,
            supplied_draws: draws,
        },
        context(),
        &registry,
        &selector,
        1024 * 1024,
    )
}
fn supplied() -> Vec<ActualDraw> {
    vec![draw(0, 20, 1), draw(1, 6, 6)]
}

#[test]
fn roll_command_carries_only_pending_identity_and_outcomes_pass_unchanged() {
    // Exhaustive variant field pattern: adding a face/outcome field breaks this contract.
    let GameInput::Game(CommandInput {
        command: GameCommand::SubmitRoll { resolution, window },
        ..
    }) = request()
    else {
        panic!("canonical roll request");
    };
    assert_eq!(resolution, ResolutionId::from_bytes(&[10; 16]).unwrap());
    assert_eq!(window, WindowId::from_bytes(&[11; 16]).unwrap());
    let current = roll_current(vec![]);
    let before = current.clone();
    let draws = supplied();
    let handler = handler(&current, &draws);
    let candidate = invoke(&handler, &current, &request(), &draws).unwrap();
    assert_eq!(*handler.observed.borrow(), draws);
    assert_eq!(candidate.state().draws, draws);
    assert_eq!(candidate.state().decisions[0].draws, vec![0, 1]);
    assert_eq!(candidate.state().resources, current.state().resources);
    assert_eq!(current, before);
    // Repeated pure preparation gives the same detached candidate, not durable consumption.
    assert_eq!(
        invoke(&handler, &current, &request(), &draws).unwrap(),
        candidate
    );
}
#[test]
fn narrative_text_cannot_choose_a_roll_or_reach_the_handler() {
    let current = roll_current(vec![]);
    let handler = handler(&current, &supplied());
    let mut input = request();
    if let GameInput::Game(command) = &mut input {
        command.command = GameCommand::Speak {
            speaker: entity(4),
            text: "choose natural 20".into(),
            conversation: None,
        };
    }
    assert_eq!(
        invoke(&handler, &current, &input, &supplied()),
        Err(ResponsePreparationError::Response(
            ResponseError::UnsupportedCommand
        ))
    );
    assert_eq!(handler.calls.get(), 0);
}
#[test]
fn missing_and_extra_draws_never_receive_defaults() {
    let current = roll_current(vec![]);
    for draws in [
        vec![],
        vec![draw(0, 20, 1)],
        vec![draw(0, 20, 1), draw(1, 6, 6), draw(2, 6, 3)],
    ] {
        let handler = handler(&current, &supplied());
        assert_eq!(
            invoke(&handler, &current, &request(), &draws),
            Err(ResponsePreparationError::Invocation(
                InvocationError::RollInputMismatch
            ))
        );
        assert_eq!(handler.calls.get(), 0);
    }
}
#[test]
fn reversed_duplicate_and_gapped_ordinals_are_refused_before_handler() {
    let current = roll_current(vec![]);
    for (draws, expected) in [
        (
            vec![draw(1, 20, 1), draw(0, 6, 6)],
            InvocationError::DrawOrderMismatch,
        ),
        (
            vec![draw(0, 20, 1), draw(0, 6, 6)],
            InvocationError::DrawAlreadyConsumed,
        ),
        (
            vec![draw(0, 20, 1), draw(2, 6, 6)],
            InvocationError::DrawOrderMismatch,
        ),
    ] {
        let handler = handler(&current, &supplied());
        assert_eq!(
            invoke(&handler, &current, &request(), &draws),
            Err(ResponsePreparationError::Invocation(expected))
        );
        assert_eq!(handler.calls.get(), 0);
    }
}
#[test]
fn invalid_faces_and_zero_sides_are_not_clamped_or_rerolled() {
    let current = roll_current(vec![]);
    for first in [draw(0, 20, 0), draw(0, 20, 21), draw(0, 0, 1)] {
        let handler = handler(&current, &supplied());
        assert_eq!(
            invoke(&handler, &current, &request(), &[first, draw(1, 6, 6)]),
            Err(ResponsePreparationError::Invocation(
                InvocationError::InvalidDrawInput
            ))
        );
        assert_eq!(handler.calls.get(), 0);
    }
}
#[test]
fn operation_and_catalog_scope_are_checked_before_handler() {
    let current = roll_current(vec![]);
    let mut foreign_operation = supplied();
    foreign_operation[0].operation = OperationId::from_bytes(&[9; 16]).unwrap();
    let mut foreign_catalog = supplied();
    foreign_catalog[0].source.catalog = label("foreign-catalog");
    for (draws, error) in [
        (foreign_operation, InvocationError::DrawOperationMismatch),
        (foreign_catalog, InvocationError::DrawSourceMismatch),
    ] {
        let handler = handler(&current, &supplied());
        assert_eq!(
            invoke(&handler, &current, &request(), &draws),
            Err(ResponsePreparationError::Invocation(error))
        );
        assert_eq!(handler.calls.get(), 0);
    }
}
#[test]
fn pending_roll_identity_source_and_die_shape_cannot_be_substituted() {
    let current = roll_current(vec![]);
    let variants = (0..4).map(|case| {
        let mut draws = supplied();
        match case {
            0 => draws[0].resolution = ResolutionId::from_bytes(&[9; 16]).unwrap(),
            1 => draws[0].window = WindowId::from_bytes(&[9; 16]).unwrap(),
            2 => draws[0].source.clause = label("foreign-clause"),
            _ => draws[0].sides = 19,
        }
        draws
    });
    for draws in variants {
        let handler = handler(&current, &supplied());
        assert_eq!(
            invoke(&handler, &current, &request(), &draws),
            Err(ResponsePreparationError::Invocation(
                InvocationError::RollInputMismatch
            ))
        );
        assert_eq!(handler.calls.get(), 0);
    }
}
#[test]
fn handler_cannot_replace_omit_or_append_actual_draws() {
    let current = roll_current(vec![]);
    let before = current.clone();
    let actual = supplied();
    for substituted in [
        vec![draw(0, 20, 20), draw(1, 6, 6)],
        vec![draw(0, 20, 1)],
        vec![draw(0, 20, 1), draw(1, 6, 6), draw(2, 6, 2)],
    ] {
        let handler = handler(&current, &substituted);
        assert_eq!(
            invoke(&handler, &current, &request(), &actual),
            Err(ResponsePreparationError::Invocation(
                InvocationError::CandidateDrawMismatch
            ))
        );
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
    }
}
#[test]
fn retained_draw_cannot_be_consumed_twice() {
    let current = roll_current(vec![draw(0, 20, 1)]);
    let handler = handler(&current, &[draw(1, 20, 1), draw(2, 6, 6)]);
    assert_eq!(
        invoke(&handler, &current, &request(), &supplied()),
        Err(ResponsePreparationError::Invocation(
            InvocationError::DrawAlreadyConsumed
        ))
    );
    assert_eq!(handler.calls.get(), 0);
}
#[test]
fn already_accepted_operation_never_invokes_or_rerolls() {
    let current = roll_current(vec![]);
    let first = handler(&current, &supplied());
    let accepted = first.candidate.clone();
    let retry = Handler {
        pins: accepted.pins().clone(),
        source: rule(),
        candidate: accepted.clone(),
        calls: Cell::new(0),
        observed: RefCell::new(vec![]),
        reject: false,
    };
    assert_eq!(
        stage_handler(
            &retry,
            accepted.pins(),
            RulesCommandInput {
                command: &request(),
                supplied_draws: &supplied()
            },
            &accepted,
            1024 * 1024
        ),
        Err(InvocationError::AlreadyAccepted)
    );
    assert_eq!(retry.calls.get(), 0);
}
#[test]
fn typed_unsupported_mechanic_preserves_draws_and_current_state() {
    let current = roll_current(vec![]);
    let before = current.clone();
    let mut handler = handler(&current, &supplied());
    handler.reject = true;
    assert_eq!(
        invoke(&handler, &current, &request(), &supplied()),
        Err(ResponsePreparationError::Invocation(
            InvocationError::Handler(df_rules::preconditions::PreconditionedRejection::Handler(
                Rejection::UnsupportedMechanic
            ))
        ))
    );
    assert_eq!(current, before);
    assert_eq!(*handler.observed.borrow(), supplied());
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

#[test]
fn each_draw_requires_its_exact_ordered_decision_and_fact_accounting() {
    let current = roll_current(vec![]);
    let before = current.clone();
    let draws = supplied();
    for case in 0..5 {
        let mut handler = handler(&current, &draws);
        let mut changed = handler.candidate.state().clone();
        match case {
            0 => {
                changed.decisions[0].draws.pop();
            }
            1 => changed.decisions[0].draws.reverse(),
            2 => {
                changed.decisions[0].facts.pop();
            }
            3 => {
                changed.facts.last_mut().unwrap().value = FactValue::DrawAccepted {
                    operation: draws[0].operation,
                    ordinal: 0,
                }
            }
            _ => {
                changed.facts.last_mut().unwrap().value = FactValue::ContentEvent {
                    definition: content(),
                    subjects: vec![],
                }
            }
        }
        handler.candidate = rebuilt(&current, handler.candidate.basis(), changed);
        assert_eq!(
            invoke(&handler, &current, &request(), &draws),
            Err(ResponsePreparationError::Invocation(
                InvocationError::CandidateDrawAccountingMismatch
            )),
            "case {case}"
        );
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
    }
}
#[test]
fn continuation_preserves_retained_draws_and_does_not_reemit_their_facts() {
    let retained = vec![draw(0, 8, 5)];
    let current = roll_current(retained.clone());
    let draws = vec![draw(1, 20, 1), draw(2, 6, 6)];
    let handler = handler(&current, &draws);
    let candidate = invoke(&handler, &current, &request(), &draws).unwrap();
    assert!(candidate.state().draws.starts_with(&retained));
    assert!(candidate.state().facts.starts_with(&current.state().facts));
    assert_eq!(
        candidate.state().facts.len(),
        current.state().facts.len() + 2
    );
    assert_eq!(candidate.state().decisions[0].draws, vec![0, 1, 2]);
}
