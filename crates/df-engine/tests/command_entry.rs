// Test-only supplied checkpoints: no fixture handler reduces game mechanics.
#[path = "support/fixture_model.rs"]
mod fixture_model;
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::command_entry::*;
use df_model::checkpoint::*;
use df_model::commands::{CommandError, CommandLimits};
use df_rules::{DispatchError, DispatchRegistry, HandlerRegistration, InvocationError};
use fixture_model::*;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Unsupported,
}
struct SuppliedHandler {
    pins: CheckpointPins,
    calls: Cell<usize>,
    candidate: Checkpoint,
    refuse: bool,
}
impl RulesCommandHandler for SuppliedHandler {
    type Rejection = Refusal;
    fn pins(&self) -> &CheckpointPins {
        &self.pins
    }
    fn stage(&self, input: RulesCommandInput<'_>, _: &Checkpoint) -> Result<Checkpoint, Refusal> {
        assert!(input.supplied_draws.is_empty());
        self.calls.set(self.calls.get() + 1);
        if self.refuse {
            Err(Refusal::Unsupported)
        } else {
            Ok(self.candidate.clone())
        }
    }
}
fn handler() -> SuppliedHandler {
    SuppliedHandler {
        pins: pins(),
        calls: Cell::new(0),
        candidate: accepted(),
        refuse: false,
    }
}
fn bounds() -> CommandEntryLimits {
    CommandEntryLimits {
        command: CommandLimits {
            maximum_records: 8,
            maximum_text_bytes: 32,
            maximum_retained_bytes: 8192,
        },
        maximum_staged_bytes: 1024 * 1024,
    }
}
fn direct(
    input: &GameInput,
    current: &Checkpoint,
    handler: &SuppliedHandler,
) -> Result<Checkpoint, CommandRejection<Refusal>> {
    let rules = [rule()];
    let contents = [content()];
    let resources = resource_constraints();
    decide_command(
        RulesCommandInput {
            command: input,
            supplied_draws: &[],
        },
        current,
        basis(),
        &pins(),
        ReferenceInventory {
            rules: &rules,
            content: &contents,
            resources: &resources,
            assets: &[],
        },
        bounds(),
        handler,
    )
}
fn catalog<'a>(
    pins: &'a CheckpointPins,
    entries: &'a [CatalogEntry<'a, RuleReference>],
) -> CatalogSnapshot<'a, df_types::RevisionLabel, CheckpointPins, RuleReference> {
    CatalogSnapshot::from_published(
        &pins.rules.catalog,
        pins,
        b"complete synthetic source",
        entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 4,
            max_item_bytes: 64,
            max_total_item_bytes: 128,
        },
    )
    .unwrap()
}
fn registered<H: RulesCommandHandler>(
    input: &GameInput,
    current: &Checkpoint,
    registry: &DispatchRegistry<'_, H>,
    selector: &df_types::RevisionLabel,
    source: &RuleReference,
) -> Result<Checkpoint, CommandRejection<H::Rejection>> {
    let pins = pins();
    let rules = [rule()];
    let contents = [content()];
    let resources = resource_constraints();
    decide_registered_command(
        RulesCommandInput {
            command: input,
            supplied_draws: &[],
        },
        current,
        CommandEntryContext {
            current_basis: current.basis(),
            admitted_pins: &pins,
            inventory: ReferenceInventory {
                rules: &rules,
                content: &contents,
                resources: &resources,
                assets: &[],
            },
            limits: bounds(),
        },
        registry,
        selector,
        source,
    )
}
#[test]
fn rejected_action_preserves_input_checkpoint_and_never_invokes_handler() {
    let current = checkpoint(state()).unwrap();
    let original = current.clone();
    let mut input = action();
    let GameInput::Game(command) = &mut input else {
        panic!("fixture")
    };
    command.member = member(99);
    let saved = input.clone();
    let handler = handler();
    assert_eq!(
        direct(&input, &current, &handler),
        Err(CommandRejection::Structural(CommandError::UnknownMember))
    );
    assert_eq!(current, original);
    assert_eq!(input, saved);
    assert_eq!(handler.calls.get(), 0);
}
#[test]
fn supplied_canonical_candidate_returns_without_applying_to_current() {
    let current = checkpoint(state()).unwrap();
    let original = current.clone();
    let input = action();
    let saved = input.clone();
    let handler = handler();
    assert_eq!(
        direct(&input, &current, &handler).unwrap(),
        handler.candidate
    );
    assert_eq!(current, original);
    assert_eq!(input, saved);
    assert_eq!(handler.calls.get(), 1);
    assert!(current.state().decisions.is_empty());
    assert!(current.state().draws.is_empty());
    assert!(current.state().intents.is_empty());
}
#[test]
fn typed_rules_rejection_never_returns_a_candidate() {
    let current = checkpoint(state()).unwrap();
    let mut handler = handler();
    handler.refuse = true;
    assert_eq!(
        direct(&action(), &current, &handler),
        Err(CommandRejection::Invocation(InvocationError::Handler(
            Refusal::Unsupported
        )))
    );
    assert_eq!(handler.calls.get(), 1);
    assert!(current.state().decisions.is_empty());
}
#[test]
fn current_source_recovery_and_staged_output_fences_refuse_without_mutation() {
    let current = checkpoint(state()).unwrap();
    let mut stale_handler = handler();
    stale_handler.pins.rules.handler_digest = ContentDigest([99; 32]);
    assert_eq!(
        direct(&action(), &current, &stale_handler),
        Err(CommandRejection::Invocation(
            InvocationError::HandlerRulesMismatch
        ))
    );
    assert_eq!(stale_handler.calls.get(), 0);
    let mut unavailable = state();
    unavailable
        .continuity
        .recovery
        .unavailable_sources
        .push(label("unavailable"));
    assert_eq!(
        direct(&action(), &checkpoint(unavailable).unwrap(), &stale_handler),
        Err(CommandRejection::Checkpoint(
            CheckpointError::UnavailableCheckpoint
        ))
    );
    let mut handler = handler();
    handler.candidate = current.clone();
    assert_eq!(
        direct(&action(), &current, &handler),
        Err(CommandRejection::Invocation(
            InvocationError::CandidateBasisMismatch
        ))
    );
    assert_eq!(current, checkpoint(state()).unwrap());
}
#[test]
fn unknown_registered_clause_never_uses_a_fallback_handler() {
    let pins = pins();
    let source = rule();
    let selector = label("compiled-source-selector");
    let handler = handler();
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();
    assert_eq!(
        registered(
            &action(),
            &checkpoint(state()).unwrap(),
            &registry,
            &label("missing"),
            &source
        ),
        Err(CommandRejection::Invocation(InvocationError::Dispatch(
            DispatchError::UnknownHandler
        )))
    );
    let mut wrong = source.clone();
    wrong.clause = label("missing-clause");
    assert_eq!(
        registered(
            &action(),
            &checkpoint(state()).unwrap(),
            &registry,
            &selector,
            &wrong
        ),
        Err(CommandRejection::Invocation(InvocationError::Dispatch(
            DispatchError::UnsupportedSource
        )))
    );
    assert_eq!(handler.calls.get(), 0);
}
#[test]
fn registered_handler_stages_only_for_complete_current_pins() {
    let mut catalog_pins = pins();
    catalog_pins.rules.handler_digest = ContentDigest([99; 32]);
    let source = rule();
    let selector = label("compiled-source-selector");
    let handler = handler();
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(&catalog_pins, &entries), &registrations, 1)
            .unwrap();
    assert_eq!(
        registered(
            &action(),
            &checkpoint(state()).unwrap(),
            &registry,
            &selector,
            &source
        ),
        Err(CommandRejection::Invocation(InvocationError::Dispatch(
            DispatchError::PinsMismatch
        )))
    );
    assert_eq!(handler.calls.get(), 0);
}
#[test]
fn input_capacity_refusal_precedes_source_selection_and_handler_call() {
    let current = checkpoint(state()).unwrap();
    let handler = handler();
    let mut input = action();
    let GameInput::Game(command) = &mut input else {
        panic!("fixture")
    };
    command.command = GameCommand::Speak {
        speaker: entity(4),
        text: "x".repeat(33),
        conversation: None,
    };
    assert_eq!(
        direct(&input, &current, &handler),
        Err(CommandRejection::Structural(CommandError::Capacity))
    );
    assert_eq!(handler.calls.get(), 0);
}
#[test]
fn surviving_older_input_observations_retain_canonical_model_policy() {
    let current = checkpoint(state()).unwrap();
    let handler = handler();
    let mut input = action();
    let GameInput::Game(command) = &mut input else {
        panic!("fixture")
    };
    command.basis.revision = revision(2, 7);
    command.observed_revision = revision(2, 7);
    assert!(direct(&input, &current, &handler).is_ok());
    assert_eq!(handler.calls.get(), 1);
}
#[test]
fn selected_dependency_change_blocks_inner_handler_but_unselected_time_change_survives() {
    use df_rules::preconditions::{
        CurrentRuleContext, PreconditionError, PreconditionLimits, PreconditionedCommandHandler,
        PreconditionedRejection, RuleDependency, RulePreconditions,
    };
    let prepared = checkpoint(state()).unwrap();
    let pins = pins();
    let rules = [rule()];
    let contents = [content()];
    let resources = resource_constraints();
    let selector = label("compiled-source-selector");
    let deps = [RuleDependency::Resource {
        owner: entity(4),
        resource: label("fixture-resource-1"),
    }];
    for selected_changed in [true, false] {
        let mut state = state();
        if selected_changed {
            state.resources.first_mut().unwrap().value = 3;
        } else {
            state.logical_time.ticks += 1;
        }
        let current = checkpoint(state.clone()).unwrap();
        let mut handler = handler();
        handler.candidate = accepted_from(state);
        let guarded = PreconditionedCommandHandler::new(
            &handler,
            &rules[0],
            CurrentRuleContext {
                checkpoint: &current,
                basis: current.basis(),
                pins: &pins,
                inventory: ReferenceInventory {
                    rules: &rules,
                    content: &contents,
                    resources: &resources,
                    assets: &[],
                },
                command_limits: bounds().command,
            },
            RulePreconditions {
                prepared: &prepared,
                sources: &rules,
                dependencies: &deps,
            },
            PreconditionLimits {
                maximum_dependencies: 8,
                maximum_comparisons: 4 * 1024 * 1024,
                maximum_checkpoint_bytes: 1024 * 1024,
            },
        );
        let entries = [CatalogEntry::new(&rules[0], b"source")];
        let registrations = [HandlerRegistration::new(&selector, &rules[0], &guarded)];
        let registry =
            DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();
        let result = registered(&action(), &current, &registry, &selector, &rules[0]);
        if selected_changed {
            assert_eq!(
                result,
                Err(CommandRejection::Invocation(InvocationError::Handler(
                    PreconditionedRejection::Precondition(PreconditionError::StaleResource)
                )))
            );
            assert_eq!(handler.calls.get(), 0);
        } else {
            assert!(result.is_ok());
            assert_eq!(handler.calls.get(), 1);
        }
    }
}
#[test]
fn canonical_handler_output_passes_to_directors_without_another_revision_or_mechanical_change() {
    use df_engine::director_staging::{
        DirectorCandidates, DirectorLimits, DirectorStaging, compose_director_candidates,
    };
    use df_world::{DueSelectionLimits, DueSelectionRequest};
    use std::time::Duration;
    let current = checkpoint(state()).unwrap();
    let handler = handler();
    let staged = direct(&action(), &current, &handler).unwrap();
    let unchanged = staged.clone();
    let policy = content();
    let directors = compose_director_candidates(
        &staged,
        staged.pins(),
        DueSelectionRequest {
            expected_basis: staged.basis(),
            target_time: staged.state().logical_time,
            paused: false,
            deadline_remaining: Duration::from_secs(1),
            policy: &policy,
        },
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        DirectorLimits {
            maximum_checkpoint_bytes: 1024 * 1024,
            maximum_pass_bytes: 8 * 1024 * 1024,
            maximum_relationships: 8,
            world: DueSelectionLimits {
                queue_events: 8,
                selected_events: 4,
                output_bytes: 64 * 1024,
            },
        },
    )
    .unwrap();
    let DirectorStaging::Staged(directors) = directors else {
        panic!("fixture has no pending work")
    };
    assert_eq!(directors.candidate(), &unchanged);
    assert_eq!(staged, unchanged);
    assert_eq!(current, checkpoint(state()).unwrap());
    assert_eq!(handler.calls.get(), 1);
}

#[path = "character_reference.rs"]
mod character_reference;

fn fact_projection_checkpoint(candidate: &Checkpoint, state: GameState) -> Checkpoint {
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        candidate.basis(),
        candidate.pins().clone(),
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

fn projection_fact(candidate: &Checkpoint, id: u8, ordinal: u32) -> GameFact {
    GameFact {
        id: FactId::from_bytes(&[id; 16]).unwrap(),
        revision: candidate.basis().revision,
        operation: operation(),
        ordinal,
        cause: candidate.state().facts.last().map(|fact| fact.id),
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: content(),
            subjects: vec![entity(4)],
        },
    }
}

#[test]
fn actual_direct_and_registered_entries_refuse_unowned_backdated_and_unordered_facts() {
    for with_history in [false, true] {
        let mut state = state();
        if with_history {
            let id = FactId::from_bytes(&[40; 16]).unwrap();
            let prior = df_types::OperationId::from_bytes(&[99; 16]).unwrap();
            state.facts.push(GameFact {
                id,
                revision: basis().revision,
                operation: prior,
                ordinal: 0,
                cause: None,
                audience: AudienceScope::Shared,
                value: FactValue::ContentEvent {
                    definition: content(),
                    subjects: vec![],
                },
            });
            state.decisions.push(AcceptedDecision {
                operation: prior,
                revision: basis().revision,
                facts: vec![id],
                draws: vec![],
                effects: vec![],
                source_policy: label("fixture-prior-fact-policy"),
                semantic_output: None,
            });
        }
        let current = checkpoint(state).unwrap();
        let original = current.clone();
        let input = action();
        let saved_input = input.clone();
        for case in 0..6 {
            let mut handler = handler();
            let candidate = accepted_from(current.state().clone());
            let mut state = candidate.state().clone();
            let mut first = projection_fact(&candidate, 80, 0);
            if case == 1 || case == 3 {
                first.operation = df_types::OperationId::from_bytes(&[98; 16]).unwrap();
            }
            if case == 2 || case == 3 {
                first.revision = current.basis().revision;
            }
            state.facts.push(first);
            if case >= 4 {
                let mut second = projection_fact(&candidate, 81, 1);
                second.cause = Some(state.facts.last().unwrap().id);
                state.facts.push(second);
                let first = state.facts[state.facts.len() - 2].id;
                let second = state.facts.last().unwrap().id;
                state.decisions.last_mut().unwrap().facts = if case == 4 {
                    vec![second, first]
                } else {
                    vec![first]
                };
            }
            // Canonical construction accepts every fixture before either public engine entry.
            handler.candidate = fact_projection_checkpoint(&candidate, state);
            let saved_candidate = handler.candidate.clone();
            let expected = Err(CommandRejection::Invocation(
                InvocationError::CandidateFactAccountingMismatch,
            ));
            assert_eq!(
                direct(&input, &current, &handler),
                expected,
                "direct {with_history} {case}"
            );
            let pins = pins();
            let source = rule();
            let selector = label("compiled-source-selector");
            let entries = [CatalogEntry::new(&source, b"source")];
            let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
            let registry =
                DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1)
                    .unwrap();
            assert_eq!(
                registered(&input, &current, &registry, &selector, &source),
                expected,
                "registered {with_history} {case}"
            );
            assert_eq!(handler.calls.get(), 2);
            assert_eq!(handler.candidate, saved_candidate);
            assert_eq!(current, original);
            assert_eq!(input, saved_input);
        }
    }
}

#[test]
fn registered_ordered_content_and_resource_facts_pass_to_actual_director_composition() {
    use df_engine::director_staging::{
        DirectorCandidates, DirectorLimits, DirectorStaging, compose_director_candidates,
    };
    use df_world::{DueSelectionLimits, DueSelectionRequest};
    use std::time::Duration;
    let current = checkpoint(state()).unwrap();
    let original = current.clone();
    let input = action();
    let saved_input = input.clone();
    let mut handler = handler();
    let candidate = handler.candidate.clone();
    let mut state = candidate.state().clone();
    let first = projection_fact(&candidate, 80, 0);
    let mut second = projection_fact(&candidate, 81, 1);
    second.cause = Some(first.id);
    second.value = FactValue::ResourceChanged {
        entity: entity(4),
        resource: label("fixture-resource-1"),
        before: 4,
        after: 3,
        source: rule(),
    };
    state.resources[0].value = 3;
    state.decisions.last_mut().unwrap().facts = vec![first.id, second.id];
    state.facts.extend([first, second]);
    handler.candidate = fact_projection_checkpoint(&candidate, state);
    let pins = pins();
    let source = rule();
    let selector = label("compiled-source-selector");
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();
    let staged = registered(&input, &current, &registry, &selector, &source).unwrap();
    let retry = registered(&input, &current, &registry, &selector, &source).unwrap();
    assert_eq!(staged, retry);
    assert_eq!(staged, handler.candidate);
    let policy = content();
    let directors = compose_director_candidates(
        &staged,
        staged.pins(),
        DueSelectionRequest {
            expected_basis: staged.basis(),
            target_time: staged.state().logical_time,
            paused: false,
            deadline_remaining: Duration::from_secs(1),
            policy: &policy,
        },
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        DirectorLimits {
            maximum_checkpoint_bytes: 1024 * 1024,
            maximum_pass_bytes: 8 * 1024 * 1024,
            maximum_relationships: 8,
            world: DueSelectionLimits {
                queue_events: 8,
                selected_events: 4,
                output_bytes: 64 * 1024,
            },
        },
    )
    .unwrap();
    let DirectorStaging::Staged(directors) = directors else {
        panic!("fixture has no pending work")
    };
    assert_eq!(directors.candidate(), &staged);
    assert_eq!(
        staged
            .state()
            .facts
            .iter()
            .map(|fact| fact.id)
            .collect::<Vec<_>>(),
        staged.state().decisions.last().unwrap().facts
    );
    assert_eq!(handler.calls.get(), 2);
    assert_eq!(current, original);
    assert_eq!(input, saved_input);
}
