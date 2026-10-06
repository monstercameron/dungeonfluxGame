use std::cell::Cell;

use df_combat::movement::{MovementFacts, SelectedMovement};
use df_combat::movement_staging::{BoundMovementHandler, MovementBindingError};
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_model::checkpoint::*;
use df_model::commands::CommandLimits;
use df_rules::preconditions::{
    CurrentRuleContext, PreconditionLimits, PreconditionedCommandHandler, PreconditionedRejection,
    RuleDependency, RulePreconditions,
};
use df_rules::{
    DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandHandler, RulesCommandInput,
};
use df_types::OperationId;

#[path = "support/registered_fixture.rs"]
mod fixture;
use fixture::*;

const BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureRefusal {
    Unsupported,
}

// A synthetic canonical decision, never a movement cost or source-qualified mechanic.
struct SourceHandler {
    pins: CheckpointPins,
    source: Option<RuleReference>,
    calls: Cell<usize>,
    command: Cell<*const GameInput>,
    draws: Cell<*const ActualDraw>,
    refuse: bool,
}

impl SourceHandler {
    fn new(source: Option<RuleReference>) -> Self {
        Self {
            pins: pins(),
            source,
            calls: Cell::new(0),
            command: Cell::new(std::ptr::null()),
            draws: Cell::new(std::ptr::null()),
            refuse: false,
        }
    }
}

impl RulesCommandHandler for SourceHandler {
    type Rejection = FixtureRefusal;

    fn pins(&self) -> &CheckpointPins {
        &self.pins
    }

    fn bound_source(&self) -> Option<&RuleReference> {
        self.source.as_ref()
    }

    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        self.calls.set(self.calls.get() + 1);
        self.command.set(input.command);
        self.draws.set(input.supplied_draws.as_ptr());
        if self.refuse {
            return Err(FixtureRefusal::Unsupported);
        }
        let GameInput::Game(command) = input.command else {
            panic!("fixture requires a canonical game command");
        };
        assert!(input.supplied_draws.is_empty());
        let mut next_basis = current.basis();
        next_basis.revision = next_basis.revision.next_sequence().unwrap();
        let mut next_state = current.state().clone();
        for pending in &mut next_state.pending {
            pending.basis = next_basis;
        }
        next_state.decisions.push(AcceptedDecision {
            operation: command.operation,
            revision: next_basis.revision,
            facts: vec![],
            draws: vec![],
            effects: vec![],
            source_policy: label("fixture-movement-policy"),
            semantic_output: None,
        });
        checkpoint_at_basis(next_state, &[rule()], next_basis, current.pins().clone())
            .map_err(|_| FixtureRefusal::Unsupported)
    }
}

fn prepared() -> Checkpoint {
    let mut state = state();
    state.facts.push(fact(7, 0));
    state.pending.push(pending());
    checkpoint(state).unwrap()
}

fn input(current: &Checkpoint) -> GameInput {
    GameInput::Game(CommandInput {
        basis: current.basis(),
        observed_revision: current.basis().revision,
        operation: OperationId::from_bytes(&[20; 16]).unwrap(),
        member: member(3),
        command: GameCommand::ProposeAction {
            actor: entity(4),
            action: content(),
            targets: vec![entity(4)],
            choices: vec![],
        },
    })
}

fn selected<'a>(
    current: &'a Checkpoint,
    source: &'a RuleReference,
    definition: &'a ContentReference,
) -> SelectedMovement<'a, ()> {
    SelectedMovement {
        basis: current.basis(),
        pins: current.pins(),
        policy: definition,
        encounter: RecordId::from_bytes(&[30; 16]).unwrap(),
        offer: RecordId::from_bytes(&[31; 16]).unwrap(),
        path: &(),
        facts: MovementFacts {
            actor: entity(4),
            origin_location: entity(4),
            origin_position: Position { x: 0, y: 0, z: 0 },
            destination: entity(4),
            destination_position: Position { x: 1, y: 0, z: 0 },
            objective: definition,
            source,
        },
        objective_preference: 0,
    }
}

type StagingError = InvocationError<PreconditionedRejection<MovementBindingError<FixtureRefusal>>>;

fn registered_stage(
    current: &Checkpoint,
    handler: &SourceHandler,
    movement: &SelectedMovement<'_, ()>,
    movement_source: &RuleReference,
    registered_source: &RuleReference,
    input: RulesCommandInput<'_>,
) -> Result<Checkpoint, StagingError> {
    let definition = content();
    let rules = if registered_source == movement_source {
        vec![movement_source.clone()]
    } else {
        vec![movement_source.clone(), registered_source.clone()]
    };
    let contents = [definition.clone()];
    let resources = resource_constraints();
    let inventory = || ReferenceInventory {
        rules: &rules,
        content: &contents,
        resources: &resources,
        assets: &[],
    };
    let bound = BoundMovementHandler {
        handler,
        selected: movement,
        prepared: current,
        source: movement_source,
        action: &definition,
    };
    let dependencies = [RuleDependency::Entity(entity(4))];
    let preconditioned = PreconditionedCommandHandler::new(
        &bound,
        registered_source,
        CurrentRuleContext {
            checkpoint: current,
            basis: current.basis(),
            pins: current.pins(),
            inventory: inventory(),
            command_limits: CommandLimits {
                maximum_records: 16,
                maximum_text_bytes: 256,
                maximum_retained_bytes: BYTES,
            },
        },
        RulePreconditions {
            prepared: current,
            sources: &rules,
            dependencies: &dependencies,
        },
        PreconditionLimits {
            maximum_dependencies: 8,
            maximum_comparisons: BYTES,
            maximum_checkpoint_bytes: BYTES,
        },
    );
    let entries = [CatalogEntry::new(
        registered_source,
        b"synthetic movement binding",
    )];
    let catalog = CatalogSnapshot::from_published(
        &current.pins().rules.catalog,
        current.pins(),
        b"synthetic publication",
        &entries,
        CatalogLimits {
            max_complete_bytes: 128,
            max_entries: 4,
            max_item_bytes: 128,
            max_total_item_bytes: 128,
        },
    )
    .unwrap();
    let selector = label("fixture-movement-selector");
    let registrations = [HandlerRegistration::new(
        &selector,
        registered_source,
        &preconditioned,
    )];
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 1).unwrap();
    registry.stage(
        current.pins(),
        &selector,
        registered_source,
        input,
        current,
        BYTES,
    )
}

#[test]
fn matching_and_unbound_handlers_reach_registered_preconditioned_staging_once() {
    let current = prepared();
    let before = current.clone();
    let source = rule();
    let definition = content();
    let movement = selected(&current, &source, &definition);
    let command = input(&current);
    let draws: [ActualDraw; 0] = [];
    for bound_source in [Some(source.clone()), None] {
        let handler = SourceHandler::new(bound_source);
        let candidate = registered_stage(
            &current,
            &handler,
            &movement,
            &source,
            &source,
            RulesCommandInput {
                command: &command,
                supplied_draws: &draws,
            },
        )
        .unwrap();
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(handler.command.get(), &command as *const GameInput);
        assert_eq!(handler.draws.get(), draws.as_ptr());
        assert_eq!(
            candidate.basis().revision,
            current.basis().revision.next_sequence().unwrap()
        );
        let mut expected_state = current.state().clone();
        for pending in &mut expected_state.pending {
            pending.basis = candidate.basis();
        }
        expected_state
            .decisions
            .push(candidate.state().decisions.last().unwrap().clone());
        assert_eq!(candidate.state(), &expected_state);
        assert_eq!(candidate.pins(), current.pins());
        assert_eq!(current, before);
    }
}

#[test]
fn every_mismatched_inner_source_component_refuses_before_registered_handler_invocation() {
    let current = prepared();
    let before = current.clone();
    let source = rule();
    let definition = content();
    let movement = selected(&current, &source, &definition);
    let command = input(&current);
    for component in 0..4 {
        let mut foreign = source.clone();
        match component {
            0 => foreign.catalog = label("foreign-catalog"),
            1 => foreign.source = label("foreign-source"),
            2 => foreign.entry = label("foreign-entry"),
            _ => foreign.clause = label("foreign-clause"),
        }
        let handler = SourceHandler::new(Some(foreign));
        assert_eq!(
            registered_stage(
                &current,
                &handler,
                &movement,
                &source,
                &source,
                RulesCommandInput {
                    command: &command,
                    supplied_draws: &[]
                },
            ),
            Err(InvocationError::Handler(PreconditionedRejection::Handler(
                MovementBindingError::SourceMismatch
            )))
        );
        assert_eq!(handler.calls.get(), 0);
        assert!(handler.command.get().is_null() && handler.draws.get().is_null());
        assert_eq!(current, before);
    }
}

#[test]
fn movement_fixed_source_is_visible_to_the_registered_precondition_wrapper() {
    let current = prepared();
    let before = current.clone();
    let source = rule();
    let mut foreign = source.clone();
    foreign.clause = label("foreign-clause");
    let definition = content();
    let movement = selected(&current, &source, &definition);
    let handler = SourceHandler::new(None);
    let command = input(&current);
    assert_eq!(
        registered_stage(
            &current,
            &handler,
            &movement,
            &source,
            &foreign,
            RulesCommandInput {
                command: &command,
                supplied_draws: &[]
            },
        ),
        Err(InvocationError::Handler(
            PreconditionedRejection::HandlerSourceMismatch
        ))
    );
    assert_eq!(handler.calls.get(), 0);
    assert_eq!(current, before);
}

#[test]
fn direct_wrapper_refuses_a_different_inner_clause_without_invocation() {
    let current = prepared();
    let before = current.clone();
    let source = rule();
    let mut foreign = source.clone();
    foreign.clause = label("foreign-clause");
    let definition = content();
    let movement = selected(&current, &source, &definition);
    let handler = SourceHandler::new(Some(foreign));
    let bound = BoundMovementHandler {
        handler: &handler,
        selected: &movement,
        prepared: &current,
        source: &source,
        action: &definition,
    };
    assert_eq!(bound.bound_source(), Some(&source));
    assert_eq!(
        bound.stage(
            RulesCommandInput {
                command: &input(&current),
                supplied_draws: &[]
            },
            &current
        ),
        Err(MovementBindingError::SourceMismatch)
    );
    assert_eq!(handler.calls.get(), 0);
    assert_eq!(current, before);
}

#[test]
fn existing_basis_pins_selected_source_and_command_guards_still_precede_invocation() {
    let current = prepared();
    let before = current.clone();
    let source = rule();
    let mut foreign_source = source.clone();
    foreign_source.clause = label("foreign-clause");
    let mut foreign_pins = current.pins().clone();
    foreign_pins.rules.handler_digest = ContentDigest([99; 32]);
    let definition = content();
    let handler = SourceHandler::new(Some(source.clone()));
    for case in 0..4 {
        let mut movement = selected(&current, &source, &definition);
        let mut command = input(&current);
        let expected = match case {
            0 => {
                movement.basis.revision = movement.basis.revision.next_sequence().unwrap();
                MovementBindingError::PreparedBasisMismatch
            }
            1 => {
                movement.pins = &foreign_pins;
                MovementBindingError::PinsMismatch
            }
            2 => {
                movement.facts.source = &foreign_source;
                MovementBindingError::SourceMismatch
            }
            _ => {
                let GameInput::Game(input) = &mut command else {
                    panic!("game fixture")
                };
                let GameCommand::ProposeAction { targets, .. } = &mut input.command else {
                    panic!("movement fixture");
                };
                targets.clear();
                MovementBindingError::CommandMismatch
            }
        };
        let bound = BoundMovementHandler {
            handler: &handler,
            selected: &movement,
            prepared: &current,
            source: &source,
            action: &definition,
        };
        assert_eq!(
            bound.stage(
                RulesCommandInput {
                    command: &command,
                    supplied_draws: &[]
                },
                &current
            ),
            Err(expected)
        );
        assert_eq!(handler.calls.get(), 0);
        assert_eq!(current, before);
    }
}

#[test]
fn matching_registered_handler_refusal_remains_typed_and_does_not_change_current_state() {
    let current = prepared();
    let before = current.clone();
    let source = rule();
    let definition = content();
    let movement = selected(&current, &source, &definition);
    let mut handler = SourceHandler::new(Some(source.clone()));
    handler.refuse = true;
    assert_eq!(
        registered_stage(
            &current,
            &handler,
            &movement,
            &source,
            &source,
            RulesCommandInput {
                command: &input(&current),
                supplied_draws: &[]
            },
        ),
        Err(InvocationError::Handler(PreconditionedRejection::Handler(
            MovementBindingError::Handler(FixtureRefusal::Unsupported)
        )))
    );
    assert_eq!(handler.calls.get(), 1);
    assert_eq!(current, before);
}

#[test]
fn direct_registry_refuses_registration_that_disagrees_with_movement_fixed_source() {
    let current = prepared();
    let before = current.clone();
    let source = rule();
    let mut foreign = source.clone();
    foreign.clause = label("foreign-clause");
    let definition = content();
    let movement = selected(&current, &source, &definition);
    let handler = SourceHandler::new(None);
    let bound = BoundMovementHandler {
        handler: &handler,
        selected: &movement,
        prepared: &current,
        source: &source,
        action: &definition,
    };
    let entries = [CatalogEntry::new(
        &foreign,
        b"synthetic foreign registration",
    )];
    let catalog = CatalogSnapshot::from_published(
        &current.pins().rules.catalog,
        current.pins(),
        b"synthetic publication",
        &entries,
        CatalogLimits {
            max_complete_bytes: 128,
            max_entries: 4,
            max_item_bytes: 128,
            max_total_item_bytes: 128,
        },
    )
    .unwrap();
    let selector = label("fixture-movement-selector");
    let registrations = [HandlerRegistration::new(&selector, &foreign, &bound)];
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 1).unwrap();
    let command = input(&current);
    let draws: [ActualDraw; 0] = [];
    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &foreign,
            RulesCommandInput {
                command: &command,
                supplied_draws: &draws
            },
            &current,
            BYTES,
        ),
        Err(InvocationError::HandlerSourceMismatch)
    );
    assert_eq!(handler.calls.get(), 0);
    assert!(handler.command.get().is_null() && handler.draws.get().is_null());
    assert_eq!(current, before);
}
