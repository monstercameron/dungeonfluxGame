use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_intent::candidate::{
    CandidateError, CandidateOwner, CandidatePreparationError, prepare_semantic_candidate,
    validate_semantic_candidate,
};
use df_intent::plan::{PlanError, PlanLimits, validate_plan_steps};
use df_model::checkpoint::*;
use df_model::commands::{CommandError, CommandLimits};
use df_rules::current_responses::ResponsePreparationError;
use df_rules::preconditions::{
    CurrentRuleContext, PreconditionError, PreconditionLimits, PreconditionedCommandHandler,
    PreconditionedRejection, RuleDependency, RulePreconditions,
};
use df_rules::{
    DispatchError, DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandHandler,
    RulesCommandInput,
};
use df_types::OperationId;
use std::cell::Cell;

#[path = "support/candidate_fixture.rs"]
mod candidate_fixture;
use candidate_fixture::*;

fn operation(value: u8) -> OperationId {
    OperationId::from_bytes(&[value; 16]).unwrap()
}

fn command_limits() -> CommandLimits {
    CommandLimits {
        maximum_records: 32,
        maximum_text_bytes: 64,
        maximum_retained_bytes: 8192,
    }
}

fn plan_limits() -> PlanLimits {
    PlanLimits {
        maximum_steps: 3,
        maximum_total_input_bytes: 8192,
        maximum_comparisons: 1024,
        commands: command_limits(),
    }
}

fn owner(current: &Checkpoint) -> CandidateOwner<'_> {
    CandidateOwner {
        basis: current.basis(),
        pins: current.pins(),
        member: member(3),
        operation: operation(31),
    }
}

fn response(kind: u8, request: u8) -> GameInput {
    let pending = pending();
    let command = match kind {
        0 => GameCommand::SelectChoice {
            resolution: pending.id,
            window: pending.window.id,
            offer: label("fixture-offer-1"),
            option: label("fixture-option-1"),
        },
        1 => GameCommand::SelectReaction {
            resolution: pending.id,
            window: pending.window.id,
            offer: label("fixture-offer-1"),
            option: label("fixture-option-1"),
        },
        2 => GameCommand::SubmitRoll {
            resolution: pending.id,
            window: pending.window.id,
        },
        _ => unreachable!(),
    };
    GameInput::Game(CommandInput {
        basis: basis(),
        observed_revision: basis().revision,
        operation: operation(request),
        member: member(3),
        command,
    })
}

fn waiting(kind: u8) -> Checkpoint {
    let mut supplied = state();
    supplied.facts.push(fact(7, 0));
    let mut resolution = pending();
    let offered = vec![OfferedResponse {
        participant: member(3),
        offer: label("fixture-offer-1"),
        options: vec![label("fixture-option-1")],
        source: rule(),
    }];
    resolution.next = match kind {
        0 => PendingInput::Choice { remaining: offered },
        1 => PendingInput::Reaction { remaining: offered },
        2 => PendingInput::Roll {
            participant: member(3),
            sides: vec![20],
            source: rule(),
        },
        _ => unreachable!(),
    };
    supplied.pending.push(resolution);
    checkpoint(supplied).unwrap()
}

fn command(input: &mut GameInput) -> &mut CommandInput {
    let GameInput::Game(command) = input else {
        unreachable!()
    };
    command
}

fn validate<'a>(
    input: &'a GameInput,
    current: &Checkpoint,
) -> Result<&'a CommandInput, CandidateError> {
    validate_semantic_candidate(
        input,
        current,
        owner(current),
        ReferenceInventory {
            rules: &[rule()],
            content: &[content()],
            resources: &[],
            assets: &[],
        },
        command_limits(),
    )
}

fn validate_steps<'a>(
    inputs: &'a [GameInput],
    current: &'a Checkpoint,
    limits: PlanLimits,
) -> Result<df_intent::plan::PlanAdmission<'a>, PlanError> {
    validate_plan_steps(
        inputs,
        current,
        owner(current),
        ReferenceInventory {
            rules: &[rule()],
            content: &[content()],
            resources: &[],
            assets: &[],
        },
        limits,
    )
}

#[test]
fn typed_candidate_schema_preserves_native_actor_request_and_source_basis() {
    for kind in 0..3 {
        let current = waiting(kind);
        let before = current.clone();
        let input = response(kind, 31);
        let GameInput::Game(expected) = &input else {
            unreachable!()
        };
        let admitted = validate(&input, &current).unwrap();
        assert!(std::ptr::eq(admitted, expected));
        assert_eq!(admitted.member, member(3));
        assert_eq!(admitted.operation, operation(31));
        assert_eq!(admitted.basis, current.basis());
        assert_eq!(admitted.observed_revision, current.basis().revision);
        assert_eq!(current.state().members[0].character, Some(entity(4)));
        assert_eq!(current.state().pending[0].window.source, rule());
        assert_eq!(current, before);
    }
}

#[test]
fn proposal_cannot_rebind_the_independently_admitted_actor_or_request() {
    let current = waiting(0);
    let before = current.clone();
    let mut other_actor = response(0, 31);
    command(&mut other_actor).member = member(5);
    assert_eq!(
        validate(&other_actor, &current),
        Err(CandidateError::MemberMismatch)
    );
    assert_eq!(
        validate(&response(0, 32), &current),
        Err(CandidateError::OperationMismatch)
    );
    assert_eq!(current, before);
}

#[test]
fn current_owner_basis_and_complete_source_pins_are_not_client_observations() {
    let current = waiting(1);
    let mut input = response(1, 31);
    command(&mut input).basis.revision = revision(2, 7);
    command(&mut input).observed_revision = revision(2, 7);
    assert!(validate(&input, &current).is_ok());

    let mut native = owner(&current);
    native.basis.revision = revision(2, 7);
    assert_eq!(
        validate_semantic_candidate(
            &input,
            &current,
            native,
            ReferenceInventory {
                rules: &[rule()],
                content: &[],
                resources: &[],
                assets: &[]
            },
            command_limits(),
        ),
        Err(CandidateError::Snapshot(CheckpointError::StaleBasis))
    );

    let mut changed = current.pins().clone();
    changed.rules.handler = label("changed-handler-version");
    let mut native = owner(&current);
    native.pins = &changed;
    assert_eq!(
        validate_semantic_candidate(
            &input,
            &current,
            native,
            ReferenceInventory {
                rules: &[rule()],
                content: &[],
                resources: &[],
                assets: &[]
            },
            command_limits(),
        ),
        Err(CandidateError::Snapshot(CheckpointError::RulesMismatch))
    );
}

#[test]
fn selected_source_must_still_exist_in_the_trusted_current_inventory() {
    let current = waiting(1);
    let before = current.clone();
    assert_eq!(
        validate_semantic_candidate(
            &response(1, 31),
            &current,
            owner(&current),
            ReferenceInventory {
                rules: &[],
                content: &[],
                resources: &[],
                assets: &[]
            },
            command_limits(),
        ),
        Err(CandidateError::Command(CommandError::InvalidReference))
    );
    assert_eq!(current, before);
}

#[test]
fn ordered_step_schema_preserves_exact_head_and_unapproved_tail() {
    let current = waiting(0);
    let before = current.clone();
    let mut tail = response(0, 32);
    let GameCommand::SelectChoice { option, .. } = &mut command(&mut tail).command else {
        unreachable!()
    };
    *option = label("fixture-option-next");
    let steps = [response(0, 31), tail];
    let admitted = validate_steps(&steps, &current, plan_limits()).unwrap();
    let GameInput::Game(head) = &steps[0] else {
        unreachable!()
    };
    assert!(std::ptr::eq(admitted.first(), head));
    assert!(std::ptr::eq(admitted.requires_revalidation(), &steps[1..]));
    assert_eq!(admitted.basis(), current.basis());
    assert!(std::ptr::eq(admitted.pins(), current.pins()));
    assert_eq!(admitted.first().operation, operation(31));
    assert_eq!(current, before);
}

#[test]
fn next_step_requires_the_new_current_offer_and_fresh_owner_admission() {
    let current = waiting(0);
    let mut tail = response(0, 32);
    let GameCommand::SelectChoice { option, .. } = &mut command(&mut tail).command else {
        unreachable!()
    };
    *option = label("fixture-option-next");
    let steps = [response(0, 31), tail];
    let admitted = validate_steps(&steps, &current, plan_limits()).unwrap();
    let next = &admitted.requires_revalidation()[0];
    assert_eq!(
        validate_semantic_candidate(
            next,
            &current,
            CandidateOwner {
                operation: operation(32),
                ..owner(&current)
            },
            ReferenceInventory {
                rules: &[rule()],
                content: &[],
                resources: &[],
                assets: &[]
            },
            command_limits(),
        ),
        Err(CandidateError::Command(CommandError::UnofferedResponse))
    );

    let mut next_basis = current.basis();
    next_basis.revision = next_basis.revision.next_sequence().unwrap();
    let mut next_state = current.state().clone();
    let pending = next_state.pending.first_mut().unwrap();
    pending.basis = next_basis;
    let PendingInput::Choice { remaining } = &mut pending.next else {
        unreachable!()
    };
    remaining.first_mut().unwrap().options = vec![label("fixture-option-next")];
    let new_current = checkpoint_at_basis(next_state, &[rule()], next_basis, pins()).unwrap();
    let fresh = CandidateOwner {
        basis: new_current.basis(),
        pins: new_current.pins(),
        member: member(3),
        operation: operation(32),
    };
    assert!(
        validate_semantic_candidate(
            next,
            &new_current,
            fresh,
            ReferenceInventory {
                rules: &[rule()],
                content: &[],
                resources: &[],
                assets: &[]
            },
            command_limits(),
        )
        .is_ok()
    );
    assert_eq!(
        validate_semantic_candidate(
            next,
            &new_current,
            CandidateOwner {
                operation: operation(32),
                ..owner(&current)
            },
            ReferenceInventory {
                rules: &[rule()],
                content: &[],
                resources: &[],
                assets: &[]
            },
            command_limits(),
        ),
        Err(CandidateError::Snapshot(CheckpointError::StaleBasis))
    );
}

#[test]
fn every_step_retains_the_native_actor_session_run_and_recovery_scope() {
    let current = waiting(1);
    let before = current.clone();
    for mismatch in 0..4 {
        let mut steps = [response(1, 31), response(1, 32)];
        let tail = command(&mut steps[1]);
        let expected = match mismatch {
            0 => {
                tail.member = member(5);
                PlanError::MemberMismatch { index: 1 }
            }
            1 => {
                tail.basis.run = df_types::RunId::from_bytes(&[99; 16]).unwrap();
                PlanError::ScopeMismatch { index: 1 }
            }
            2 => {
                tail.observed_revision = revision(3, 0);
                PlanError::ScopeMismatch { index: 1 }
            }
            3 => {
                tail.basis.revision = revision(2, 9);
                PlanError::ScopeMismatch { index: 1 }
            }
            _ => unreachable!(),
        };
        assert_eq!(
            validate_steps(&steps, &current, plan_limits()).map(|_| ()),
            Err(expected)
        );
        assert_eq!(current, before);
    }
}

#[test]
fn ordered_steps_have_explicit_count_byte_comparison_and_request_identity_bounds() {
    let current = waiting(1);
    let steps = [response(1, 31), response(1, 32)];
    assert_eq!(
        validate_steps(&[], &current, plan_limits()).map(|_| ()),
        Err(PlanError::EmptyPlan)
    );
    for capacity in 0..3 {
        let mut limits = plan_limits();
        match capacity {
            0 => limits.maximum_steps = 1,
            1 => limits.maximum_total_input_bytes = 1,
            2 => limits.maximum_comparisons = 0,
            _ => unreachable!(),
        }
        assert_eq!(
            validate_steps(&steps, &current, limits).map(|_| ()),
            Err(PlanError::Capacity)
        );
    }
    let duplicate = [response(1, 31), response(1, 31)];
    assert_eq!(
        validate_steps(&duplicate, &current, plan_limits()).map(|_| ()),
        Err(PlanError::DuplicateOperation)
    );
}

#[test]
fn retained_request_identity_requires_lookup_instead_of_continuation_replay() {
    let current = waiting(1);
    let mut retained = current.state().clone();
    retained.decisions.push(AcceptedDecision {
        operation: operation(32),
        revision: current.basis().revision,
        facts: vec![],
        draws: vec![],
        effects: vec![],
        source_policy: label("fixture-accepted-policy"),
        semantic_output: None,
    });
    let current = checkpoint(retained).unwrap();
    let before = current.clone();
    assert_eq!(
        validate_steps(&[response(1, 31), response(1, 32)], &current, plan_limits()).map(|_| ()),
        Err(PlanError::LookupRequired {
            operation: operation(32)
        })
    );
    assert_eq!(current, before);
}

#[test]
fn speech_and_general_action_cannot_become_a_typed_candidate_or_plan_step() {
    let current = waiting(1);
    for proposed in [
        GameCommand::Speak {
            speaker: entity(4),
            text: "Maybe I attack?".to_owned(),
            conversation: None,
        },
        GameCommand::ProposeAction {
            actor: entity(4),
            action: content(),
            targets: vec![],
            choices: vec![],
        },
    ] {
        let mut input = response(1, 32);
        command(&mut input).command = proposed;
        assert_eq!(
            validate(&input, &current),
            Err(CandidateError::UnsupportedVariant)
        );
        assert_eq!(
            validate_steps(&[response(1, 31), input], &current, plan_limits()).map(|_| ()),
            Err(PlanError::UnsupportedStep { index: 1 })
        );
    }
}

struct RecordedDrawHandler<'a> {
    pins: &'a CheckpointPins,
    expected: RulesCommandInput<'a>,
    calls: Cell<usize>,
}

impl RulesCommandHandler for RecordedDrawHandler<'_> {
    type Rejection = CheckpointError;

    fn pins(&self) -> &CheckpointPins {
        self.pins
    }

    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        self.calls.set(self.calls.get() + 1);
        assert!(std::ptr::eq(input.command, self.expected.command));
        assert!(std::ptr::eq(
            input.supplied_draws,
            self.expected.supplied_draws
        ));
        let GameInput::Game(command) = input.command else {
            unreachable!()
        };
        let mut next_basis = current.basis();
        next_basis.revision = next_basis.revision.next_sequence().unwrap();
        let mut staged = current.state().clone();
        for pending in &mut staged.pending {
            pending.basis = next_basis;
        }
        let mut accepted = Vec::new();
        for (index, draw) in input.supplied_draws.iter().enumerate() {
            let id =
                FactId::from_bytes(&[32_u8.checked_add(index.try_into().unwrap()).unwrap(); 16])
                    .unwrap();
            staged.facts.push(GameFact {
                id,
                revision: next_basis.revision,
                operation: command.operation,
                ordinal: index.try_into().unwrap(),
                cause: None,
                audience: AudienceScope::Shared,
                value: FactValue::DrawAccepted {
                    operation: draw.operation,
                    ordinal: draw.ordinal,
                },
            });
            accepted.push(id);
        }
        staged.draws.extend_from_slice(input.supplied_draws);
        staged.decisions.push(AcceptedDecision {
            operation: command.operation,
            revision: next_basis.revision,
            facts: accepted,
            draws: input
                .supplied_draws
                .iter()
                .map(|draw| draw.ordinal)
                .collect(),
            effects: vec![],
            source_policy: label("fixture-handler-policy"),
            semantic_output: None,
        });
        checkpoint_at_basis(staged, &[rule()], next_basis, self.pins.clone())
    }
}

fn prepare(
    input: RulesCommandInput<'_>,
    current: &Checkpoint,
    prepared: &Checkpoint,
    dependencies: &[RuleDependency],
    selector: &df_types::RevisionLabel,
) -> (
    Result<Checkpoint, CandidatePreparationError<CheckpointError>>,
    usize,
) {
    let sources = [rule()];
    let content_entries = [content()];
    let resources = resource_constraints();
    let context = || CurrentRuleContext {
        checkpoint: current,
        basis: current.basis(),
        pins: current.pins(),
        inventory: ReferenceInventory {
            rules: &sources,
            content: &content_entries,
            resources: &resources,
            assets: &[],
        },
        command_limits: command_limits(),
    };
    let handler = RecordedDrawHandler {
        pins: current.pins(),
        expected: input,
        calls: Cell::new(0),
    };
    let guarded = PreconditionedCommandHandler::new(
        &handler,
        &sources[0],
        context(),
        RulePreconditions {
            prepared,
            sources: &sources,
            dependencies,
        },
        PreconditionLimits {
            maximum_dependencies: 8,
            maximum_comparisons: 1_000_000,
            maximum_checkpoint_bytes: 1_000_000,
        },
    );
    let registered = label("fixture-compiled-handler");
    let registrations = [HandlerRegistration::new(&registered, &sources[0], &guarded)];
    let entries = [CatalogEntry::new(&sources[0], b"synthetic-clause")];
    let catalog = CatalogSnapshot::from_published(
        &current.pins().rules.catalog,
        current.pins(),
        b"synthetic-catalog",
        &entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 1,
            max_item_bytes: 64,
            max_total_item_bytes: 64,
        },
    )
    .unwrap();
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 1).unwrap();
    let result = prepare_semantic_candidate(
        input,
        context(),
        owner(current),
        &registry,
        selector,
        1_000_000,
    );
    (result, handler.calls.get())
}

#[test]
fn compiled_source_example_records_only_the_native_supplied_outcome() {
    let current = waiting(2);
    let before = current.clone();
    let candidate = response(2, 31);
    let draws = [ActualDraw {
        operation: operation(31),
        ordinal: 0,
        resolution: pending().id,
        window: pending().window.id,
        sides: 20,
        value: 11,
        source: rule(),
    }];
    let (staged, calls) = prepare(
        RulesCommandInput {
            command: &candidate,
            supplied_draws: &draws,
        },
        &current,
        &current,
        &[RuleDependency::PendingResolution(pending().id)],
        &label("fixture-compiled-handler"),
    );
    let staged = staged.unwrap();
    assert_eq!(calls, 1);
    assert_eq!(staged.state().draws, draws);
    assert_eq!(
        staged.state().decisions.last().unwrap().operation,
        operation(31)
    );
    assert_eq!(staged.state().decisions.last().unwrap().draws, vec![0]);
    assert_eq!(staged.pins(), current.pins());
    assert_eq!(staged.state().members, current.state().members);
    assert_eq!(staged.state().resources, current.state().resources);
    assert_eq!(current, before);
    println!(
        "Intent D01 source decision: {}",
        include_str!("fixtures/intent_step_schema_contract.json")
    );
}

#[test]
fn source_dispatch_refuses_unknown_handler_and_stale_dependencies_before_mechanics() {
    let prepared = waiting(1);
    let candidate = response(1, 31);
    let input = RulesCommandInput {
        command: &candidate,
        supplied_draws: &[],
    };
    let (unknown, calls) = prepare(input, &prepared, &prepared, &[], &label("unknown-handler"));
    assert_eq!(calls, 0);
    assert_eq!(
        unknown,
        Err(CandidatePreparationError::Rules(
            ResponsePreparationError::Invocation(InvocationError::Dispatch(
                DispatchError::UnknownHandler
            ))
        ))
    );

    let mut state = prepared.state().clone();
    state.resources.first_mut().unwrap().value = 3;
    let current = checkpoint(state).unwrap();
    let before = current.clone();
    let (stale, calls) = prepare(
        input,
        &current,
        &prepared,
        &[RuleDependency::Resource {
            owner: entity(4),
            resource: label("fixture-resource-1"),
        }],
        &label("fixture-compiled-handler"),
    );
    assert_eq!(calls, 0);
    assert_eq!(
        stale,
        Err(CandidatePreparationError::Rules(
            ResponsePreparationError::Invocation(InvocationError::Handler(
                PreconditionedRejection::Precondition(PreconditionError::StaleResource)
            ))
        ))
    );
    assert_eq!(current, before);
}
