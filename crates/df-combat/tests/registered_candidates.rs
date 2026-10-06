use std::cell::Cell;

use df_combat::candidates::{CandidateContext, CandidateError, CandidateLimits};
use df_combat::registered_candidates::{
    RegisteredCandidateError, RegisteredCandidateLimits, RegisteredCandidateRequest,
    enumerate_registered_responses,
};
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_model::checkpoint::*;
use df_model::commands::{CommandError, CommandLimits};
use df_rules::current_responses::{ResponseError, ResponsePreparationError};
use df_rules::preconditions::{
    CurrentRuleContext, PreconditionError, PreconditionLimits, PreconditionedCommandHandler,
    PreconditionedRejection, RuleDependency, RulePreconditions,
};
use df_rules::{
    DispatchError, DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandHandler,
    RulesCommandInput,
};
use df_types::{OperationId, RevisionLabel, RunId, SessionId};

#[path = "support/registered_fixture.rs"]
mod fixture;
use fixture::*;

fn operation() -> OperationId {
    OperationId::from_bytes(&[20; 16]).unwrap()
}

fn command(reaction: bool) -> CommandInput {
    let pending = pending();
    let command = if reaction {
        GameCommand::SelectReaction {
            resolution: pending.id,
            window: pending.window.id,
            offer: label("ignored-template"),
            option: label("ignored-template"),
        }
    } else {
        GameCommand::SelectChoice {
            resolution: pending.id,
            window: pending.window.id,
            offer: label("ignored-template"),
            option: label("ignored-template"),
        }
    };
    CommandInput {
        basis: basis(),
        observed_revision: basis().revision,
        operation: operation(),
        member: member(3),
        command,
    }
}

fn waiting(reaction: bool) -> Checkpoint {
    let mut state = state();
    state.facts.push(fact(7, 0));
    state.members.push(MembershipLink {
        member: member(5),
        character: None,
    });
    let mut pending = pending();
    let other = OfferedResponse {
        participant: member(5),
        offer: label("other-member-offer"),
        options: vec![label("other-member-option")],
        source: rule(),
    };
    let first = OfferedResponse {
        participant: member(3),
        offer: label("first-current-offer"),
        options: vec![label("option-a"), label("option-b")],
        source: rule(),
    };
    let mut second = first.clone();
    second.offer = label("second-current-offer");
    second.options = vec![label("option-c")];
    let remaining = vec![first, other, second];
    pending.next = if reaction {
        PendingInput::Reaction { remaining }
    } else {
        PendingInput::Choice { remaining }
    };
    state.pending.push(pending);
    checkpoint(state).unwrap()
}

fn budgets() -> RegisteredCandidateLimits {
    RegisteredCandidateLimits {
        candidates: CandidateLimits {
            max_offers: 8,
            max_identity_comparisons: 28,
            max_candidates: 8,
            max_candidate_bytes: 8192,
        },
        maximum_checkpoint_bytes: 1024 * 1024,
        maximum_inventory_records: 64,
        maximum_rules_preparations: 8,
        maximum_staged_bytes: 1024 * 1024,
    }
}

// Deterministic structural handler fixture: accepts a prepared command by recording its
// operation at the next canonical revision. It supplies no D&D mechanics or source golden.
struct Handler<'a> {
    pins: &'a CheckpointPins,
    calls: Cell<usize>,
    reject: bool,
}
impl RulesCommandHandler for Handler<'_> {
    type Rejection = CheckpointError;
    fn pins(&self) -> &CheckpointPins {
        self.pins
    }
    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, CheckpointError> {
        self.calls.set(self.calls.get() + 1);
        if self.reject {
            return Err(CheckpointError::InvalidReference);
        }
        let GameInput::Game(command) = input.command else {
            return Err(CheckpointError::InvalidReference);
        };
        let mut next = current.basis();
        next.revision = next.revision.next_sequence().unwrap();
        let mut state = current.state().clone();
        state.pending.clear();
        state.decisions.push(AcceptedDecision {
            operation: command.operation,
            revision: next.revision,
            facts: vec![],
            draws: vec![],
            effects: vec![],
            source_policy: label("fixture-handler"),
            semantic_output: None,
        });
        checkpoint_at_basis(state, &[rule()], next, self.pins.clone())
    }
}

fn with_pipeline<R>(
    prepared: &Checkpoint,
    current: &Checkpoint,
    handler: &Handler<'_>,
    dependencies: &[RuleDependency],
    revoked: bool,
    call: impl FnOnce(
        &DispatchRegistry<'_, PreconditionedCommandHandler<'_, Handler<'_>>>,
        CurrentRuleContext<'_>,
        &RevisionLabel,
    ) -> R,
) -> R {
    let sources = [rule()];
    let contents = [content()];
    let resources = resource_constraints();
    let context = || CurrentRuleContext {
        checkpoint: current,
        basis: current.basis(),
        pins: current.pins(),
        inventory: ReferenceInventory {
            rules: if revoked { &[] } else { &sources },
            content: &contents,
            resources: &resources,
            assets: &[],
        },
        command_limits: CommandLimits {
            maximum_records: 128,
            maximum_text_bytes: 128,
            maximum_retained_bytes: 1024 * 1024,
        },
    };
    let wrapped = PreconditionedCommandHandler::new(
        handler,
        &sources[0],
        context(),
        RulePreconditions {
            prepared,
            sources: &sources,
            dependencies,
        },
        PreconditionLimits {
            maximum_dependencies: 16,
            maximum_comparisons: 8 * 1024 * 1024,
            maximum_checkpoint_bytes: 1024 * 1024,
        },
    );
    let selector = label("fixture-compiled-handler");
    let registrations = [HandlerRegistration::new(&selector, &sources[0], &wrapped)];
    let entries = [CatalogEntry::new(&sources[0], b"synthetic-clause-fixture")];
    let catalog = CatalogSnapshot::from_published(
        &current.pins().rules.catalog,
        current.pins(),
        b"synthetic-clause-fixture",
        &entries,
        CatalogLimits {
            max_complete_bytes: 1024,
            max_entries: 8,
            max_item_bytes: 1024,
            max_total_item_bytes: 1024,
        },
    )
    .unwrap();
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 8).unwrap();
    call(&registry, context(), &selector)
}

#[test]
fn actual_registry_intent_preconditions_admit_only_exact_current_member_responses() {
    for reaction in [false, true] {
        let current = waiting(reaction);
        let before = current.clone();
        let handler = Handler {
            pins: current.pins(),
            calls: Cell::new(0),
            reject: false,
        };
        with_pipeline(
            &current,
            &current,
            &handler,
            &[RuleDependency::PendingResolution(pending().id)],
            false,
            |registry, context, selector| {
                let current_basis = current.basis();
                let template = command(reaction);
                let admitted = enumerate_registered_responses(
                    context,
                    registry,
                    RegisteredCandidateRequest {
                        template: &template,
                        current: CandidateContext {
                            basis: &current_basis,
                            pins: current.pins(),
                        },
                        member: member(3),
                        operation: operation(),
                        selector,
                    },
                    budgets(),
                )
                .unwrap();
                let canonical = match &current.state().pending[0].next {
                    PendingInput::Choice { remaining } | PendingInput::Reaction { remaining } => {
                        remaining
                    }
                    _ => unreachable!(),
                };
                assert_eq!(admitted.offers().len(), 2);
                assert!(std::ptr::eq(admitted.offers()[0], &canonical[0]));
                assert!(std::ptr::eq(admitted.offers()[1], &canonical[2]));
                assert!(std::ptr::eq(admitted.pins(), current.pins()));
                assert_eq!(handler.calls.get(), 3);
            },
        );
        assert_eq!(current, before);
        assert!(current.state().decisions.is_empty());
        assert!(current.state().draws.is_empty());
    }
}

#[test]
fn original_template_scope_is_admitted_before_any_registered_handler_runs() {
    for reaction in [false, true] {
        let current = waiting(reaction);
        let before = current.clone();
        for case in 0..8 {
            let mut template = command(reaction);
            let expected = match case {
                0 => {
                    template.basis.session = SessionId::from_bytes(&[21; 16]).unwrap();
                    CommandError::WrongSession
                }
                1 => {
                    template.basis.run = RunId::from_bytes(&[22; 16]).unwrap();
                    CommandError::WrongRun
                }
                2 => {
                    template.basis.revision = revision(1, 8);
                    CommandError::StaleRevision
                }
                3 => {
                    template.basis.revision = revision(3, 8);
                    CommandError::StaleRevision
                }
                4 => {
                    template.basis.revision = revision(2, 9);
                    CommandError::StaleRevision
                }
                5 => {
                    template.observed_revision = revision(1, 8);
                    CommandError::StaleRevision
                }
                6 => {
                    template.observed_revision = revision(3, 8);
                    CommandError::StaleRevision
                }
                7 => {
                    template.observed_revision = revision(2, 9);
                    CommandError::StaleRevision
                }
                _ => unreachable!(),
            };
            let handler = Handler {
                pins: current.pins(),
                calls: Cell::new(0),
                reject: false,
            };
            with_pipeline(
                &current,
                &current,
                &handler,
                &[RuleDependency::PendingResolution(pending().id)],
                false,
                |registry, context, selector| {
                    let current_basis = current.basis();
                    let result = enumerate_registered_responses(
                        context,
                        registry,
                        RegisteredCandidateRequest {
                            template: &template,
                            current: CandidateContext {
                                basis: &current_basis,
                                pins: current.pins(),
                            },
                            member: member(3),
                            operation: operation(),
                            selector,
                        },
                        budgets(),
                    );
                    assert_eq!(
                        result.err(),
                        Some(RegisteredCandidateError::Enumeration(
                            CandidateError::Owner(ResponsePreparationError::Response(
                                ResponseError::Admission(expected)
                            ),)
                        )),
                        "reaction={reaction}, case={case}",
                    );
                },
            );
            assert_eq!(handler.calls.get(), 0, "reaction={reaction}, case={case}");
            assert_eq!(current, before);
            assert!(current.state().decisions.is_empty());
            assert!(current.state().draws.is_empty());
        }
    }
}

#[test]
fn older_same_epoch_template_revisions_select_exact_current_responses() {
    for reaction in [false, true] {
        let current = waiting(reaction);
        let before = current.clone();
        // Basis and observation are admitted independently by the canonical model.
        for (basis_revision, observed_revision) in [
            (revision(2, 7), revision(2, 8)),
            (revision(2, 8), revision(2, 7)),
            (revision(2, 7), revision(2, 6)),
        ] {
            let mut template = command(reaction);
            template.basis.revision = basis_revision;
            template.observed_revision = observed_revision;
            let handler = Handler {
                pins: current.pins(),
                calls: Cell::new(0),
                reject: false,
            };
            with_pipeline(
                &current,
                &current,
                &handler,
                &[RuleDependency::PendingResolution(pending().id)],
                false,
                |registry, context, selector| {
                    let current_basis = current.basis();
                    let admitted = enumerate_registered_responses(
                        context,
                        registry,
                        RegisteredCandidateRequest {
                            template: &template,
                            current: CandidateContext {
                                basis: &current_basis,
                                pins: current.pins(),
                            },
                            member: member(3),
                            operation: operation(),
                            selector,
                        },
                        budgets(),
                    )
                    .unwrap();
                    let canonical = match &current.state().pending[0].next {
                        PendingInput::Choice { remaining }
                        | PendingInput::Reaction { remaining } => remaining,
                        _ => unreachable!(),
                    };
                    assert_eq!(admitted.offers().len(), 2);
                    assert!(std::ptr::eq(admitted.offers()[0], &canonical[0]));
                    assert!(std::ptr::eq(admitted.offers()[1], &canonical[2]));
                    assert_eq!(*admitted.basis(), current.basis());
                    assert!(std::ptr::eq(admitted.pins(), current.pins()));
                },
            );
            assert_eq!(handler.calls.get(), 3);
            assert_eq!(current, before);
            assert!(current.state().decisions.is_empty());
            assert!(current.state().draws.is_empty());
        }
    }
}

#[test]
fn revoked_source_and_unknown_registration_refuse_without_handler_or_partial_receipt() {
    let current = waiting(true);
    for revoked in [false, true] {
        let handler = Handler {
            pins: current.pins(),
            calls: Cell::new(0),
            reject: false,
        };
        with_pipeline(
            &current,
            &current,
            &handler,
            &[],
            revoked,
            |registry, context, selector| {
                let current_basis = current.basis();
                let template = command(true);
                let unknown = label("unknown-handler");
                let error = enumerate_registered_responses(
                    context,
                    registry,
                    RegisteredCandidateRequest {
                        template: &template,
                        current: CandidateContext {
                            basis: &current_basis,
                            pins: current.pins(),
                        },
                        member: member(3),
                        operation: operation(),
                        selector: if revoked { selector } else { &unknown },
                    },
                    budgets(),
                )
                .err();
                let expected = if revoked {
                    RegisteredCandidateError::Window(CommandError::InvalidReference)
                } else {
                    RegisteredCandidateError::Enumeration(CandidateError::Owner(
                        ResponsePreparationError::Invocation(InvocationError::Dispatch(
                            DispatchError::UnknownHandler,
                        )),
                    ))
                };
                assert_eq!(error, Some(expected));
                assert_eq!(handler.calls.get(), 0);
            },
        );
    }
}

#[test]
fn changed_reviewed_resource_dependency_refuses_before_inner_handler() {
    let prepared = waiting(true);
    let mut state = prepared.state().clone();
    state.resources[0].value = 3;
    let current = checkpoint(state).unwrap();
    let before = current.clone();
    let handler = Handler {
        pins: current.pins(),
        calls: Cell::new(0),
        reject: false,
    };
    let dependency = [RuleDependency::Resource {
        owner: entity(4),
        resource: label("fixture-resource-1"),
    }];
    with_pipeline(
        &prepared,
        &current,
        &handler,
        &dependency,
        false,
        |registry, context, selector| {
            let current_basis = current.basis();
            let template = command(true);
            let error = enumerate_registered_responses(
                context,
                registry,
                RegisteredCandidateRequest {
                    template: &template,
                    current: CandidateContext {
                        basis: &current_basis,
                        pins: current.pins(),
                    },
                    member: member(3),
                    operation: operation(),
                    selector,
                },
                budgets(),
            )
            .err();
            assert_eq!(
                error,
                Some(RegisteredCandidateError::Enumeration(
                    CandidateError::Owner(ResponsePreparationError::Invocation(
                        InvocationError::Handler(PreconditionedRejection::Precondition(
                            PreconditionError::StaleResource
                        ))
                    ))
                ))
            );
            assert_eq!(handler.calls.get(), 0);
        },
    );
    assert_eq!(current, before);
}

#[test]
fn stale_native_basis_pins_window_and_wrong_pending_kind_never_run_handler() {
    let current = waiting(true);
    let handler = Handler {
        pins: current.pins(),
        calls: Cell::new(0),
        reject: false,
    };
    for case in 0..4 {
        with_pipeline(
            &current,
            &current,
            &handler,
            &[],
            false,
            |registry, context, selector| {
                let mut current_basis = current.basis();
                let mut current_pins = current.pins().clone();
                let mut template = command(true);
                match case {
                    0 => current_basis.revision = revision(2, 7),
                    1 => current_pins.rules.handler_digest = ContentDigest([99; 32]),
                    2 => {
                        let GameCommand::SelectReaction { window, .. } = &mut template.command
                        else {
                            unreachable!()
                        };
                        *window = WindowId::from_bytes(&[99; 16]).unwrap();
                    }
                    _ => template = command(false),
                }
                let error = enumerate_registered_responses(
                    context,
                    registry,
                    RegisteredCandidateRequest {
                        template: &template,
                        current: CandidateContext {
                            basis: &current_basis,
                            pins: &current_pins,
                        },
                        member: member(3),
                        operation: operation(),
                        selector,
                    },
                    budgets(),
                )
                .err();
                assert_eq!(
                    error,
                    Some(match case {
                        0 => RegisteredCandidateError::StaleBasis,
                        1 => RegisteredCandidateError::StalePins,
                        2 => RegisteredCandidateError::Window(CommandError::StaleWindow),
                        _ => RegisteredCandidateError::Window(CommandError::WrongPendingKind),
                    })
                );
            },
        );
    }
    assert_eq!(handler.calls.get(), 0);
}

#[test]
fn changed_pending_response_dependency_and_source_handler_refusal_are_preserved() {
    let prepared = waiting(true);
    let mut state = prepared.state().clone();
    let PendingInput::Reaction { remaining } = &mut state.pending[0].next else {
        unreachable!()
    };
    remaining[0].options = vec![label("changed-current-option")];
    let current = checkpoint(state).unwrap();
    for reject in [false, true] {
        let handler = Handler {
            pins: current.pins(),
            calls: Cell::new(0),
            reject,
        };
        let dependencies = if reject {
            vec![]
        } else {
            vec![RuleDependency::PendingResolution(pending().id)]
        };
        with_pipeline(
            &prepared,
            &current,
            &handler,
            &dependencies,
            false,
            |registry, context, selector| {
                let current_basis = current.basis();
                let template = command(true);
                let error = enumerate_registered_responses(
                    context,
                    registry,
                    RegisteredCandidateRequest {
                        template: &template,
                        current: CandidateContext {
                            basis: &current_basis,
                            pins: current.pins(),
                        },
                        member: member(3),
                        operation: operation(),
                        selector,
                    },
                    budgets(),
                )
                .err();
                let rejection = if reject {
                    PreconditionedRejection::Handler(CheckpointError::InvalidReference)
                } else {
                    PreconditionedRejection::Precondition(PreconditionError::StalePending)
                };
                assert_eq!(
                    error,
                    Some(RegisteredCandidateError::Enumeration(
                        CandidateError::Owner(ResponsePreparationError::Invocation(
                            InvocationError::Handler(rejection)
                        ))
                    ))
                );
                assert_eq!(handler.calls.get(), usize::from(reject));
            },
        );
    }
}

#[test]
fn preparation_budget_rejects_before_any_source_handler_runs() {
    let current = waiting(true);
    let handler = Handler {
        pins: current.pins(),
        calls: Cell::new(0),
        reject: false,
    };
    with_pipeline(
        &current,
        &current,
        &handler,
        &[],
        false,
        |registry, context, selector| {
            let current_basis = current.basis();
            let template = command(true);
            let limits = RegisteredCandidateLimits {
                maximum_rules_preparations: 2,
                ..budgets()
            };
            assert_eq!(
                enumerate_registered_responses(
                    context,
                    registry,
                    RegisteredCandidateRequest {
                        template: &template,
                        current: CandidateContext {
                            basis: &current_basis,
                            pins: current.pins()
                        },
                        member: member(3),
                        operation: operation(),
                        selector
                    },
                    limits
                )
                .err(),
                Some(RegisteredCandidateError::Capacity)
            );
            assert_eq!(handler.calls.get(), 0);
        },
    );
}

#[test]
fn member_and_operation_are_bound_independently_of_query_template() {
    let current = waiting(true);
    let handler = Handler {
        pins: current.pins(),
        calls: Cell::new(0),
        reject: false,
    };
    for different_member in [false, true] {
        with_pipeline(
            &current,
            &current,
            &handler,
            &[],
            false,
            |registry, context, selector| {
                let current_basis = current.basis();
                let template = command(true);
                let trusted_member = if different_member {
                    member(5)
                } else {
                    member(3)
                };
                let trusted_operation = if different_member {
                    operation()
                } else {
                    OperationId::from_bytes(&[30; 16]).unwrap()
                };
                let error = enumerate_registered_responses(
                    context,
                    registry,
                    RegisteredCandidateRequest {
                        template: &template,
                        current: CandidateContext {
                            basis: &current_basis,
                            pins: current.pins(),
                        },
                        member: trusted_member,
                        operation: trusted_operation,
                        selector,
                    },
                    budgets(),
                )
                .err();
                assert_eq!(
                    error,
                    Some(if different_member {
                        RegisteredCandidateError::MemberMismatch
                    } else {
                        RegisteredCandidateError::OperationMismatch
                    })
                );
            },
        );
    }
    assert_eq!(handler.calls.get(), 0);
}

#[test]
fn no_participant_responses_return_empty_receipt_without_inner_calls() {
    for reaction in [false, true] {
        let mut state = waiting(reaction).state().clone();
        let remaining = match &mut state.pending[0].next {
            PendingInput::Choice { remaining } | PendingInput::Reaction { remaining } => remaining,
            _ => unreachable!(),
        };
        remaining.retain(|response| response.participant != member(3));
        let current = checkpoint(state).unwrap();
        let before = current.clone();
        let handler = Handler {
            pins: current.pins(),
            calls: Cell::new(0),
            reject: false,
        };
        with_pipeline(
            &current,
            &current,
            &handler,
            &[],
            false,
            |registry, context, selector| {
                let current_basis = current.basis();
                let template = command(reaction);
                let mut limits = budgets();
                limits.maximum_rules_preparations = 0;
                limits.candidates.max_candidates = 0;
                limits.candidates.max_candidate_bytes = 0;
                let admitted = enumerate_registered_responses(
                    context,
                    registry,
                    RegisteredCandidateRequest {
                        template: &template,
                        current: CandidateContext {
                            basis: &current_basis,
                            pins: current.pins(),
                        },
                        member: member(3),
                        operation: operation(),
                        selector,
                    },
                    limits,
                )
                .unwrap();
                assert!(admitted.offers().is_empty());
                assert_eq!(*admitted.basis(), current.basis());
                assert!(std::ptr::eq(admitted.pins(), current.pins()));
                assert_eq!(handler.calls.get(), 0);
            },
        );
        assert_eq!(current, before);
    }
}

// Publishes the exact registered response source independently of the current inventory.
// A revoked response can therefore reach the immutable inventory guard while its window
// source remains admitted, rather than failing because no source was registered at all.
fn with_response_source<H: RulesCommandHandler, R>(
    current: &Checkpoint,
    handler: &H,
    source: &RuleReference,
    inventory_rules: &[RuleReference],
    call: impl FnOnce(
        &DispatchRegistry<'_, PreconditionedCommandHandler<'_, H>>,
        CurrentRuleContext<'_>,
        &RevisionLabel,
    ) -> R,
) -> R {
    let contents = [content()];
    let resources = resource_constraints();
    let sources = std::slice::from_ref(source);
    let context = || CurrentRuleContext {
        checkpoint: current,
        basis: current.basis(),
        pins: current.pins(),
        inventory: ReferenceInventory {
            rules: inventory_rules,
            content: &contents,
            resources: &resources,
            assets: &[],
        },
        command_limits: CommandLimits {
            maximum_records: 128,
            maximum_text_bytes: 128,
            maximum_retained_bytes: 1024 * 1024,
        },
    };
    let wrapped = PreconditionedCommandHandler::new(
        handler,
        source,
        context(),
        RulePreconditions {
            prepared: current,
            sources,
            dependencies: &[],
        },
        PreconditionLimits {
            maximum_dependencies: 16,
            maximum_comparisons: 8 * 1024 * 1024,
            maximum_checkpoint_bytes: 1024 * 1024,
        },
    );
    let selector = label("fixture-compiled-handler");
    let registrations = [HandlerRegistration::new(&selector, source, &wrapped)];
    let entries = [CatalogEntry::new(source, b"synthetic-clause-fixture")];
    let catalog = CatalogSnapshot::from_published(
        &current.pins().rules.catalog,
        current.pins(),
        b"synthetic-clause-fixture",
        &entries,
        CatalogLimits {
            max_complete_bytes: 1024,
            max_entries: 8,
            max_item_bytes: 1024,
            max_total_item_bytes: 1024,
        },
    )
    .unwrap();
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 8).unwrap();
    call(&registry, context(), &selector)
}

#[test]
fn revoked_response_source_with_admitted_window_refuses_without_inner_calls() {
    for reaction in [false, true] {
        let admitted = rule();
        let mut revoked = rule();
        revoked.clause = label("revoked-response-clause");
        let mut state = waiting(reaction).state().clone();
        let remaining = match &mut state.pending[0].next {
            PendingInput::Choice { remaining } | PendingInput::Reaction { remaining } => remaining,
            _ => unreachable!(),
        };
        remaining[0].source = revoked.clone();
        let current = checkpoint_with_rules(state, &[admitted.clone(), revoked.clone()]).unwrap();
        let before = current.clone();
        let handler = Handler {
            pins: current.pins(),
            calls: Cell::new(0),
            reject: false,
        };
        with_response_source(
            &current,
            &handler,
            &revoked,
            std::slice::from_ref(&admitted),
            |registry, context, selector| {
                assert!(
                    context
                        .inventory
                        .rules
                        .contains(&current.state().pending[0].window.source)
                );
                assert!(!context.inventory.rules.contains(&revoked));
                let current_basis = current.basis();
                let template = command(reaction);
                let error = enumerate_registered_responses(
                    context,
                    registry,
                    RegisteredCandidateRequest {
                        template: &template,
                        current: CandidateContext {
                            basis: &current_basis,
                            pins: current.pins(),
                        },
                        member: member(3),
                        operation: operation(),
                        selector,
                    },
                    budgets(),
                )
                .err();
                assert_eq!(
                    error,
                    Some(RegisteredCandidateError::Enumeration(
                        CandidateError::Owner(ResponsePreparationError::Invocation(
                            InvocationError::Handler(PreconditionedRejection::Precondition(
                                PreconditionError::InvalidReference
                            ))
                        ))
                    ))
                );
                assert_eq!(handler.calls.get(), 0);
            },
        );
        assert_eq!(current, before);
    }
}

#[test]
fn previously_accepted_operation_refuses_without_handler_or_history_change() {
    for reaction in [false, true] {
        let mut state = waiting(reaction).state().clone();
        state.decisions.push(AcceptedDecision {
            operation: operation(),
            revision: basis().revision,
            facts: vec![],
            draws: vec![],
            effects: vec![],
            source_policy: label("fixture-handler"),
            semantic_output: None,
        });
        let current = checkpoint(state).unwrap();
        let before = current.clone();
        let handler = Handler {
            pins: current.pins(),
            calls: Cell::new(0),
            reject: false,
        };
        with_pipeline(
            &current,
            &current,
            &handler,
            &[],
            false,
            |registry, context, selector| {
                let current_basis = current.basis();
                let template = command(reaction);
                let error = enumerate_registered_responses(
                    context,
                    registry,
                    RegisteredCandidateRequest {
                        template: &template,
                        current: CandidateContext {
                            basis: &current_basis,
                            pins: current.pins(),
                        },
                        member: member(3),
                        operation: operation(),
                        selector,
                    },
                    budgets(),
                )
                .err();
                assert_eq!(
                    error,
                    Some(RegisteredCandidateError::Enumeration(
                        CandidateError::Owner(ResponsePreparationError::Invocation(
                            InvocationError::AlreadyAccepted
                        ))
                    ))
                );
                assert_eq!(handler.calls.get(), 0);
            },
        );
        assert_eq!(current.state().decisions, before.state().decisions);
        assert_eq!(current.state().draws, before.state().draws);
        assert_eq!(current, before);
    }
}

#[derive(Debug, Eq, PartialEq)]
enum PreviewRefusal {
    ActualDrawsRequired,
    Structural(CheckpointError),
}

// A source fixture requires outcomes only for the last offered option. The earlier options
// stage normally, so refusal must discard their entire prospective receipt. This is a typed
// source boundary probe; it does not define when any real game choice should need a draw.
struct DrawRequiredHandler<'a> {
    inner: Handler<'a>,
    previews: Cell<usize>,
}
impl RulesCommandHandler for DrawRequiredHandler<'_> {
    type Rejection = PreviewRefusal;
    fn pins(&self) -> &CheckpointPins {
        self.inner.pins
    }
    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        assert!(input.supplied_draws.is_empty());
        self.previews.set(self.previews.get() + 1);
        let GameInput::Game(command) = input.command else {
            return Err(PreviewRefusal::Structural(
                CheckpointError::InvalidReference,
            ));
        };
        let option = match &command.command {
            GameCommand::SelectChoice { option, .. }
            | GameCommand::SelectReaction { option, .. } => option,
            _ => {
                return Err(PreviewRefusal::Structural(
                    CheckpointError::InvalidReference,
                ));
            }
        };
        if *option == label("option-c") {
            return Err(PreviewRefusal::ActualDrawsRequired);
        }
        self.inner
            .stage(input, current)
            .map_err(PreviewRefusal::Structural)
    }
}

#[test]
fn outcome_required_choice_and_reaction_refuse_whole_zero_draw_preview() {
    for reaction in [false, true] {
        let current = waiting(reaction);
        let before = current.clone();
        let handler = DrawRequiredHandler {
            inner: Handler {
                pins: current.pins(),
                calls: Cell::new(0),
                reject: false,
            },
            previews: Cell::new(0),
        };
        with_response_source(
            &current,
            &handler,
            &rule(),
            &[rule()],
            |registry, context, selector| {
                let current_basis = current.basis();
                let template = command(reaction);
                let error = enumerate_registered_responses(
                    context,
                    registry,
                    RegisteredCandidateRequest {
                        template: &template,
                        current: CandidateContext {
                            basis: &current_basis,
                            pins: current.pins(),
                        },
                        member: member(3),
                        operation: operation(),
                        selector,
                    },
                    budgets(),
                )
                .err();
                assert_eq!(
                    error,
                    Some(RegisteredCandidateError::Enumeration(
                        CandidateError::Owner(ResponsePreparationError::Invocation(
                            InvocationError::Handler(PreconditionedRejection::Handler(
                                PreviewRefusal::ActualDrawsRequired
                            ))
                        ))
                    ))
                );
                assert_eq!(handler.previews.get(), 3);
                assert_eq!(handler.inner.calls.get(), 2);
            },
        );
        assert_eq!(current, before);
        assert_eq!(current.state().draws, before.state().draws);
        assert_eq!(current.state().decisions, before.state().decisions);
    }
}

#[test]
fn duplicate_canonical_response_identity_refuses_before_any_preparation() {
    let mut state = waiting(true).state().clone();
    let PendingInput::Reaction { remaining } = &mut state.pending[0].next else {
        unreachable!()
    };
    remaining[2].offer = remaining[0].offer.clone();
    // Canonical validation admits these individually valid records; the candidate boundary
    // additionally refuses the repeated participant/offer identity despite different options.
    let current = checkpoint(state).unwrap();
    let before = current.clone();
    let handler = Handler {
        pins: current.pins(),
        calls: Cell::new(0),
        reject: false,
    };
    with_pipeline(
        &current,
        &current,
        &handler,
        &[],
        false,
        |registry, context, selector| {
            let current_basis = current.basis();
            let template = command(true);
            let error = enumerate_registered_responses(
                context,
                registry,
                RegisteredCandidateRequest {
                    template: &template,
                    current: CandidateContext {
                        basis: &current_basis,
                        pins: current.pins(),
                    },
                    member: member(3),
                    operation: operation(),
                    selector,
                },
                budgets(),
            )
            .err();
            assert_eq!(
                error,
                Some(RegisteredCandidateError::Enumeration(
                    CandidateError::Duplicate {
                        first: 0,
                        duplicate: 2
                    }
                ))
            );
            assert_eq!(handler.calls.get(), 0);
        },
    );
    assert_eq!(current, before);
}

fn canonical_response_bytes(response: &OfferedResponse) -> usize {
    // The candidate byte contract accounts for the exact retained canonical record allocation,
    // including owned label buffers and reserved option slots rather than serialized lengths.
    size_of::<OfferedResponse>()
        + response.options.capacity() * size_of::<RevisionLabel>()
        + [
            &response.offer,
            &response.source.catalog,
            &response.source.source,
            &response.source.entry,
            &response.source.clause,
        ]
        .into_iter()
        .chain(&response.options)
        .map(RevisionLabel::retained_heap_bytes)
        .sum::<usize>()
}

#[test]
fn registered_limits_admit_exact_requirements_and_refuse_one_below() {
    for reaction in [false, true] {
        let current = waiting(reaction);
        let before = current.clone();
        let responses = match &current.state().pending[0].next {
            PendingInput::Choice { remaining } | PendingInput::Reaction { remaining } => remaining,
            _ => unreachable!(),
        };
        let candidate_bytes =
            canonical_response_bytes(&responses[0]) + canonical_response_bytes(&responses[2]);
        let mut selected = command(reaction);
        match &mut selected.command {
            GameCommand::SelectChoice { offer, option, .. }
            | GameCommand::SelectReaction { offer, option, .. } => {
                *offer = responses[0].offer.clone();
                *option = responses[0].options[0].clone();
            }
            _ => unreachable!(),
        }
        let input = GameInput::Game(selected);
        // Measure the actual fixture's staged checkpoint with an explicit no-outcome envelope.
        // The measurement is detached; enumeration receives a fresh handler/counter below.
        let measuring = Handler {
            pins: current.pins(),
            calls: Cell::new(0),
            reject: false,
        };
        let staged_bytes = measuring
            .stage(
                RulesCommandInput {
                    command: &input,
                    supplied_draws: &[],
                },
                &current,
            )
            .unwrap()
            .retained_bytes()
            .unwrap();
        let exact = RegisteredCandidateLimits {
            candidates: CandidateLimits {
                max_offers: 3,
                max_identity_comparisons: 3,
                max_candidates: 2,
                max_candidate_bytes: candidate_bytes,
            },
            maximum_checkpoint_bytes: current.retained_bytes().unwrap(),
            maximum_inventory_records: 3,
            maximum_rules_preparations: 3,
            maximum_staged_bytes: staged_bytes,
        };
        for boundary in 0..8 {
            for one_below in [false, true] {
                let handler = Handler {
                    pins: current.pins(),
                    calls: Cell::new(0),
                    reject: false,
                };
                let mut limits = exact;
                if one_below {
                    match boundary {
                        0 => limits.maximum_checkpoint_bytes -= 1,
                        1 => limits.maximum_inventory_records -= 1,
                        2 => limits.maximum_rules_preparations -= 1,
                        3 => limits.maximum_staged_bytes -= 1,
                        4 => limits.candidates.max_offers -= 1,
                        5 => limits.candidates.max_identity_comparisons -= 1,
                        6 => limits.candidates.max_candidates -= 1,
                        _ => limits.candidates.max_candidate_bytes -= 1,
                    }
                }
                with_pipeline(
                    &current,
                    &current,
                    &handler,
                    &[],
                    false,
                    |registry, context, selector| {
                        let current_basis = current.basis();
                        let template = command(reaction);
                        let result = enumerate_registered_responses(
                            context,
                            registry,
                            RegisteredCandidateRequest {
                                template: &template,
                                current: CandidateContext {
                                    basis: &current_basis,
                                    pins: current.pins(),
                                },
                                member: member(3),
                                operation: operation(),
                                selector,
                            },
                            limits,
                        );
                        if one_below {
                            let expected = match boundary {
                                0..=2 => RegisteredCandidateError::Capacity,
                                3 => RegisteredCandidateError::Enumeration(CandidateError::Owner(
                                    ResponsePreparationError::Invocation(InvocationError::Capacity),
                                )),
                                4 => RegisteredCandidateError::Enumeration(
                                    CandidateError::OfferCapacity {
                                        required: 3,
                                        limit: 2,
                                    },
                                ),
                                5 => RegisteredCandidateError::Enumeration(
                                    CandidateError::ComparisonCapacity {
                                        required: 3,
                                        limit: 2,
                                    },
                                ),
                                6 => RegisteredCandidateError::Enumeration(
                                    CandidateError::CandidateCapacity { limit: 1 },
                                ),
                                _ => RegisteredCandidateError::Enumeration(
                                    CandidateError::ByteCapacity {
                                        limit: candidate_bytes - 1,
                                    },
                                ),
                            };
                            assert_eq!(
                                result.err(),
                                Some(expected),
                                "boundary={boundary}, reaction={reaction}"
                            );
                            assert_eq!(
                                handler.calls.get(),
                                match boundary {
                                    3 => 1,
                                    6..=7 => 3,
                                    _ => 0,
                                }
                            );
                        } else {
                            let admitted = result.unwrap();
                            assert_eq!(admitted.offers().len(), 2);
                            assert!(std::ptr::eq(admitted.offers()[0], &responses[0]));
                            assert!(std::ptr::eq(admitted.offers()[1], &responses[2]));
                            assert_eq!(handler.calls.get(), 3);
                        }
                    },
                );
                assert_eq!(current, before);
            }
        }
    }
}
