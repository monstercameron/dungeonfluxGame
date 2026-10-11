//! Bounded Design qualification of the existing compound-head boundary, not a continuation runtime.
#![cfg(not(target_arch = "wasm32"))]

#[path = "support/pending_fixtures.rs"]
mod pending_fixtures;
#[path = "support/plan_staging.rs"]
mod plan_fixtures;

use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::command_entry::{CommandEntryContext, CommandEntryLimits, CommandRejection};
use df_engine::pending_resumption::ResumeError;
use df_engine::plan_staging::{PlanHeadAdmission, PlanHeadError, stage_plan_head};
use df_engine::semantic_candidate::SemanticResponseError;
use df_intent::candidate::{CandidateError, CandidateOwner};
use df_intent::plan::PlanError;
use df_model::checkpoint::*;
use df_model::commands::CommandError;
use df_rules::preconditions::{
    CurrentRuleContext, PreconditionError, PreconditionLimits, PreconditionedCommandHandler,
    PreconditionedRejection, RuleDependency, RulePreconditions,
};
use df_rules::{
    DispatchError, DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandInput,
};
use df_session::submission::*;
use pending_fixtures::*;
use plan_fixtures::durable::{
    Database, Failure, Outcome, Repository, Scope, setup, submit, submit_scope,
};
use plan_fixtures::{BYTES, admission, handler, invoke, operation, rebuild, two_steps};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// Retain the exact bounded decision/scenario fixture in the compiler's source closure.
const _: &str = include_str!("fixtures/compound_step_continuation_contract.json");

type Refusal = PlanHeadError<FixtureRejection>;
type Owner = DurableOwner<Repository, CurrentHead, Publication>;

#[derive(Clone, Copy, Default)]
enum FenceChange {
    #[default]
    None,
    Generation,
    Cancel,
}

#[derive(Default)]
struct CurrentAdmission {
    missing_selector: bool,
    source_withdrawn: bool,
    fence: FenceChange,
    stale_pins: Option<CheckpointPins>,
}

// Native fixture ports reuse the canonical repository and explicitly submit each head.
// No automatic tail producer, persistence schema, rule reducer or production rights are issued.
struct CurrentHead {
    database: Rc<RefCell<Database>>,
    admission: Rc<RefCell<CurrentAdmission>>,
}

impl SessionEngine<Scope> for CurrentHead {
    fn decide(
        &mut self,
        current: &Checkpoint,
        scope: &Scope,
        input: &GameInput,
    ) -> Result<Checkpoint, RepositoryError> {
        let (steps, prepared, outcome) = {
            let mut db = self.database.borrow_mut();
            db.engines += 1;
            db.observed_bases.push(current.basis());
            (
                db.ephemeral_steps.clone(),
                db.prepared.clone().unwrap_or_else(|| current.clone()),
                db.outcome,
            )
        };
        if steps.first() != Some(input) {
            return Err(RepositoryError::InputBinding);
        }
        let mut source_handler = handler(current, input, &[]);
        let mut state = source_handler.candidate.state().clone();
        match outcome {
            Outcome::Preserve => {}
            Outcome::SpendResource => state.resources[0].value -= 1,
            Outcome::CloseWindow => state.pending.clear(),
        }
        source_handler.candidate = rebuild(&source_handler.candidate, state);
        let controls = self.admission.borrow();
        let mut trusted = admission(current, input);
        trusted.member = scope.principal;
        trusted.operation = scope.operation();
        if controls.missing_selector {
            trusted.selector = label("source-selector-withdrawn");
        }
        if let Some(pins) = &controls.stale_pins {
            trusted.pins = pins.clone();
        }
        match controls.fence {
            FenceChange::None => {}
            FenceChange::Generation => trusted.fence.current_generation += 1,
            FenceChange::Cancel => trusted.fence.cancel_before_admission = true,
        }
        let staged = if controls.source_withdrawn {
            without_current_source(&steps, current, &source_handler)
        } else {
            invoke(&steps, &[], current, &prepared, &source_handler, trusted)
        };
        let mut db = self.database.borrow_mut();
        db.handlers += source_handler.calls.get();
        match staged {
            Ok(staged) => {
                assert_eq!(
                    source_handler.command_pointer.get(),
                    staged.first() as *const GameInput
                );
                db.returned_tail = staged.requires_revalidation().to_vec();
                Ok(staged.into_parts().0)
            }
            Err(error) => {
                db.refusals.push(error);
                Err(RepositoryError::InvalidCandidate)
            }
        }
    }

    fn validate_recovery(&mut self, current: &Checkpoint) -> Result<(), RepositoryError> {
        current
            .validate_resume(current.basis(), &pins())
            .map(|_| ())
            .map_err(|_| RepositoryError::InvalidCandidate)
    }
}

// A known selector remains registered for another clause, never for the current pending source.
// Only source inventory/registration data changes; all admission/staging remains canonical.
fn without_current_source<'a>(
    steps: &'a [GameInput],
    current: &Checkpoint,
    handler: &plan_fixtures::Handler,
) -> Result<df_engine::plan_staging::StagedPlanHead<'a>, Refusal> {
    let source = rule();
    let mut other = source.clone();
    other.clause = label("different-source-clause");
    let rules = [source.clone()];
    let contents = [content()];
    let resources = resource_constraints();
    let inventory = || ReferenceInventory {
        rules: &rules,
        content: &contents,
        resources: &resources,
        assets: &[],
    };
    let dependencies = [RuleDependency::PendingResolutions];
    let wrapper = PreconditionedCommandHandler::new(
        handler,
        &other,
        CurrentRuleContext {
            checkpoint: current,
            basis: current.basis(),
            pins: current.pins(),
            inventory: inventory(),
            command_limits: command_limits(),
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
    let entries = [CatalogEntry::new(&other, b"synthetic-other-clause")];
    let catalog = CatalogSnapshot::from_published(
        &current.pins().rules.catalog,
        current.pins(),
        b"synthetic-current-publication",
        &entries,
        CatalogLimits {
            max_complete_bytes: 128,
            max_entries: 4,
            max_item_bytes: 64,
            max_total_item_bytes: 128,
        },
    )
    .unwrap();
    let selector = label("fixture-continuation-position-1");
    let registrations = [HandlerRegistration::new(&selector, &other, &wrapper)];
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 1).unwrap();
    let trusted = admission(current, &steps[0]);
    stage_plan_head(
        steps,
        &[],
        current,
        CommandEntryContext {
            current_basis: current.basis(),
            admitted_pins: current.pins(),
            inventory: inventory(),
            limits: CommandEntryLimits {
                command: command_limits(),
                maximum_staged_bytes: BYTES,
            },
        },
        PlanHeadAdmission {
            owner: CandidateOwner {
                basis: current.basis(),
                pins: current.pins(),
                member: trusted.member,
                operation: trusted.operation,
            },
            fence: trusted.fence,
            limits: df_engine::pending_resumption::ResumeLimits {
                maximum_checkpoint_bytes: BYTES,
            },
            plan_limits: trusted.plan,
        },
        &registry,
        &selector,
    )
}

struct Publication(Rc<RefCell<Database>>);
impl PublicationOwner<Scope> for Publication {
    fn publish_committed(
        &mut self,
        scope: &Scope,
        checkpoint: &Checkpoint,
    ) -> Result<(), DeliveryError> {
        let mut db = self.0.borrow_mut();
        assert_eq!(&db.checkpoint, checkpoint);
        assert!(db.ledger.iter().any(|(key, _)| key == scope));
        db.publications += 1;
        Ok(())
    }
    fn wake_committed_intents(&mut self, _: &Scope) -> Result<(), DeliveryError> {
        Ok(())
    }
}

type Controlled = (
    Owner,
    Rc<RefCell<Database>>,
    Rc<RefCell<CurrentAdmission>>,
    Vec<GameInput>,
);
fn controlled(failure: Failure) -> Controlled {
    let (prior, database, steps) = setup(failure);
    let current = prior.checkpoint().clone();
    let repository = prior.into_repository(); // This controlled repository owns no external driver.
    let controls = Rc::new(RefCell::new(CurrentAdmission::default()));
    let owner = DurableOwner::new(
        repository,
        CurrentHead {
            database: Rc::clone(&database),
            admission: Rc::clone(&controls),
        },
        Publication(Rc::clone(&database)),
        current,
        64 * 1024,
    )
    .unwrap();
    (owner, database, controls, steps)
}

fn submit_current(owner: &mut Owner, input: &GameInput) -> SubmissionOutcome {
    submit_current_scope(
        owner,
        Scope {
            input: input.clone(),
            principal: member(3),
        },
    )
}
fn submit_current_scope(owner: &mut Owner, scope: Scope) -> SubmissionOutcome {
    use df_observe::OperationContext;
    use df_session::inbox::{AdmissionSequence, bounded_inbox};
    let (input, receipt) = OwnedInput::new(
        OperationContext {
            trace_parent: String::new(),
            build: "F37-current-head-contract".to_owned(),
        },
        scope.clone(),
        scope.input.clone(),
    );
    let (inbox, actor) = bounded_inbox();
    assert!(inbox.try_submit(input).is_ok());
    inbox.stop().unwrap();
    let drained = actor.run(owner).unwrap();
    assert_eq!(drained.reduced_inputs, 1);
    assert_eq!(drained.last_sequence, Some(AdmissionSequence(1)));
    receipt.try_recv().unwrap()
}

fn next_head(db: &Rc<RefCell<Database>>, steps: &[GameInput]) {
    let mut db = db.borrow_mut();
    assert_eq!(db.returned_tail, steps[1..]);
    db.ephemeral_steps = db.returned_tail.clone();
}
fn assert_prefix(
    owner: &Owner,
    db: &Rc<RefCell<Database>>,
    prefix: &Checkpoint,
    receipt: &SubmissionOutcome,
) {
    assert_eq!(owner.checkpoint(), prefix);
    let db = db.borrow();
    assert_eq!(&db.checkpoint, prefix);
    assert_eq!(db.checkpoint.state(), prefix.state());
    assert_eq!(
        (db.handlers, db.commits, db.publications, db.ledger.len()),
        (1, 1, 1, 1)
    );
    let SubmissionOutcome::Confirmed(receipt) = receipt else {
        panic!("confirmed fixture prefix")
    };
    assert_eq!(&db.ledger[0].1, receipt);
}
fn dispatch(error: DispatchError) -> Refusal {
    PlanHeadError::Semantic(SemanticResponseError::Resume(ResumeError::Command(
        CommandRejection::Invocation(InvocationError::Dispatch(error)),
    )))
}

#[test]
fn separately_accepted_heads_revalidate_fresh_source_and_checkpoint() {
    let (mut owner, db, _, steps) = controlled(Failure::None);
    let before = owner.checkpoint().clone();
    let first = submit_current(&mut owner, &steps[0]);
    assert!(matches!(first, SubmissionOutcome::Confirmed(_)));
    let prefix = owner.checkpoint().clone();
    next_head(&db, &steps);
    assert!(matches!(
        submit_current(&mut owner, &steps[1]),
        SubmissionOutcome::Confirmed(_)
    ));
    let final_state = owner.checkpoint().clone();
    assert_eq!(submit_current(&mut owner, &steps[0]), first);
    assert_eq!(owner.checkpoint(), &final_state);
    let db = db.borrow();
    assert_eq!(db.observed_bases, [before.basis(), prefix.basis()]);
    assert_eq!(
        (db.engines, db.handlers, db.commits, db.publications),
        (2, 2, 2, 2)
    );
    assert!(
        final_state
            .state()
            .decisions
            .starts_with(&prefix.state().decisions)
    );
    assert_eq!(final_state.state().decisions.len(), 2);
    assert_eq!(final_state.state().draws, prefix.state().draws);
    assert!(db.returned_tail.is_empty());
}

#[test]
fn missing_later_selector_refuses_before_handler_and_preserves_committed_prefix() {
    let (mut owner, db, controls, steps) = controlled(Failure::None);
    let receipt = submit_current(&mut owner, &steps[0]);
    let prefix = owner.checkpoint().clone();
    next_head(&db, &steps);
    controls.borrow_mut().missing_selector = true;
    assert_eq!(
        submit_current(&mut owner, &steps[1]),
        SubmissionOutcome::Refused(RepositoryError::InvalidCandidate)
    );
    assert_prefix(&owner, &db, &prefix, &receipt);
    assert_eq!(
        db.borrow().refusals,
        [dispatch(DispatchError::UnknownHandler)]
    );
}

#[test]
fn source_membership_removed_at_next_head_cannot_execute() {
    let (mut owner, db, controls, steps) = controlled(Failure::None);
    let receipt = submit_current(&mut owner, &steps[0]);
    let prefix = owner.checkpoint().clone();
    next_head(&db, &steps);
    controls.borrow_mut().source_withdrawn = true;
    assert_eq!(
        submit_current(&mut owner, &steps[1]),
        SubmissionOutcome::Refused(RepositoryError::InvalidCandidate)
    );
    assert_prefix(&owner, &db, &prefix, &receipt);
    assert_eq!(
        db.borrow().refusals,
        [dispatch(DispatchError::UnsupportedSource)]
    );
}

#[test]
fn changed_native_source_pins_refuse_before_later_handler() {
    for case in 0..3 {
        let (mut owner, db, controls, steps) = controlled(Failure::None);
        let receipt = submit_current(&mut owner, &steps[0]);
        let prefix = owner.checkpoint().clone();
        next_head(&db, &steps);
        let mut stale = prefix.pins().clone();
        let error = match case {
            0 => {
                stale.rules.handler = label("old-handler");
                CheckpointError::RulesMismatch
            }
            1 => {
                stale.content.content = label("old-content");
                CheckpointError::ContentMismatch
            }
            _ => {
                stale.build = df_types::BuildIdentity::new(
                    Some("old-source"),
                    Some("old-native"),
                    Some("old-wasm"),
                    Some("old-config"),
                    Some("old-content"),
                )
                .unwrap();
                CheckpointError::BuildMismatch
            }
        };
        controls.borrow_mut().stale_pins = Some(stale);
        assert_eq!(
            submit_current(&mut owner, &steps[1]),
            SubmissionOutcome::Refused(RepositoryError::InvalidCandidate)
        );
        assert_prefix(&owner, &db, &prefix, &receipt);
        assert_eq!(
            db.borrow().refusals,
            [PlanHeadError::Plan(PlanError::Candidate(
                CandidateError::Snapshot(error)
            ))]
        );
    }
}

#[test]
fn stale_owner_generation_and_cancellation_stop_only_uncommitted_tail() {
    for (change, error) in [
        (FenceChange::Generation, ResumeError::StaleGeneration),
        (FenceChange::Cancel, ResumeError::CancelledBeforeAdmission),
    ] {
        let (mut owner, db, controls, steps) = controlled(Failure::None);
        let receipt = submit_current(&mut owner, &steps[0]);
        let prefix = owner.checkpoint().clone();
        next_head(&db, &steps);
        controls.borrow_mut().fence = change;
        assert_eq!(
            submit_current(&mut owner, &steps[1]),
            SubmissionOutcome::Refused(RepositoryError::InvalidCandidate)
        );
        assert_prefix(&owner, &db, &prefix, &receipt);
        assert_eq!(
            db.borrow().refusals,
            [PlanHeadError::Semantic(SemanticResponseError::Resume(
                error
            ))]
        );
    }
}

#[test]
fn changed_resources_or_response_window_revalidate_at_each_head() {
    for outcome in [Outcome::SpendResource, Outcome::CloseWindow] {
        let (mut owner, db, _, steps) = controlled(Failure::None);
        let before = owner.checkpoint().clone();
        db.borrow_mut().outcome = outcome;
        let receipt = submit_current(&mut owner, &steps[0]);
        let prefix = owner.checkpoint().clone();
        next_head(&db, &steps);
        {
            let mut db = db.borrow_mut();
            db.outcome = Outcome::Preserve;
            if outcome == Outcome::SpendResource {
                db.prepared = Some(before);
            }
        }
        assert_eq!(
            submit_current(&mut owner, &steps[1]),
            SubmissionOutcome::Refused(RepositoryError::InvalidCandidate)
        );
        assert_prefix(&owner, &db, &prefix, &receipt);
        let expected = if outcome == Outcome::SpendResource {
            PlanHeadError::Semantic(SemanticResponseError::Resume(ResumeError::Command(
                CommandRejection::Invocation(InvocationError::Handler(
                    PreconditionedRejection::Precondition(PreconditionError::StaleResource),
                )),
            )))
        } else {
            PlanHeadError::Plan(PlanError::Candidate(CandidateError::Command(
                CommandError::StaleWindow,
            )))
        };
        assert_eq!(db.borrow().refusals, [expected]);
    }
}

#[test]
fn native_roll_draws_are_bound_to_exact_current_head() {
    for kind in 0..3 {
        let current = waiting(kind);
        let steps = two_steps(kind);
        let draws = if kind == 2 {
            vec![native_roll_draw(0)]
        } else {
            vec![]
        };
        let inner = handler(&current, &steps[0], &draws);
        let staged = invoke(
            &steps,
            &draws,
            &current,
            &current,
            &inner,
            admission(&current, &steps[0]),
        )
        .unwrap();
        assert_eq!(inner.command_pointer.get(), &steps[0] as *const GameInput);
        assert_eq!(inner.draw_pointer.get(), draws.as_ptr());
        assert_eq!(staged.requires_revalidation(), &steps[1..]);
        assert_eq!(staged.checkpoint().state().draws, draws);
        let canonical = SuppliedHandler {
            candidate: inner.candidate.clone(),
            pins: current.pins().clone(),
            calls: Rc::new(Cell::new(0)),
            refuse: false,
            observed_draw_pointer: Rc::new(Cell::new(std::ptr::null())),
        };
        assert_eq!(
            pending_fixtures::invoke(
                RulesCommandInput {
                    command: &steps[0],
                    supplied_draws: &draws
                },
                &current,
                &current,
                &canonical,
                fence(),
                BYTES
            )
            .unwrap(),
            *staged.checkpoint()
        );
        assert_eq!(canonical.observed_draw_pointer.get(), draws.as_ptr());
        if kind == 2 {
            for case in 0..4 {
                let mut bad = draws.clone();
                let expected = match case {
                    0 => {
                        bad.clear();
                        InvocationError::RollInputMismatch
                    }
                    1 => {
                        bad[0].operation = df_types::OperationId::from_bytes(&[99; 16]).unwrap();
                        InvocationError::DrawOperationMismatch
                    }
                    2 => {
                        bad[0].value = 21;
                        InvocationError::InvalidDrawInput
                    }
                    _ => {
                        bad[0].window = WindowId::from_bytes(&[99; 16]).unwrap();
                        InvocationError::RollInputMismatch
                    }
                };
                assert_eq!(
                    invoke(
                        &steps,
                        &bad,
                        &current,
                        &current,
                        &inner,
                        admission(&current, &steps[0])
                    )
                    .err(),
                    Some(PlanHeadError::Semantic(SemanticResponseError::Resume(
                        ResumeError::Command(CommandRejection::Invocation(expected))
                    )))
                );
                assert_eq!(inner.calls.get(), 1);
            }
        }
    }
}

#[test]
fn known_failed_commit_leaves_no_prefix_or_tail_publication() {
    let (mut owner, db, _, steps) = controlled(Failure::BeforeCommit);
    let original = owner.checkpoint().clone();
    assert_eq!(
        submit_current(&mut owner, &steps[0]),
        SubmissionOutcome::Refused(RepositoryError::Unavailable)
    );
    assert_eq!(owner.checkpoint(), &original);
    assert_eq!(db.borrow().checkpoint, original);
    assert_eq!(db.borrow().publications, 0);
    assert!(db.borrow().ledger.is_empty());
    assert!(!owner.has_uncertain_operation());
    assert_eq!(db.borrow().engines, 1);
    db.borrow_mut().failure = Failure::None;
    let first = submit_current(&mut owner, &steps[0]);
    assert!(matches!(first, SubmissionOutcome::Confirmed(_)));
    assert_eq!(submit_current(&mut owner, &steps[0]), first);
    let db = db.borrow();
    assert_eq!(db.observed_bases, [original.basis(), original.basis()]);
    assert_eq!(
        (
            db.engines,
            db.handlers,
            db.commits,
            db.publications,
            db.ledger.len()
        ),
        (2, 2, 2, 1, 1)
    );
}

#[test]
fn lost_ack_fences_tail_until_exact_authorized_lookup_and_reload() {
    let (mut owner, db, _, steps) = controlled(Failure::LostAck);
    let original = owner.checkpoint().clone();
    assert_eq!(
        submit_current(&mut owner, &steps[0]),
        SubmissionOutcome::LookupRequired
    );
    assert!(owner.has_uncertain_operation());
    db.borrow_mut().ephemeral_steps = steps[1..].to_vec();
    assert_eq!(
        submit_current(&mut owner, &steps[1]),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(db.borrow().engines, 1);
    assert_eq!(owner.checkpoint(), &original);
    assert!(matches!(
        submit_current(&mut owner, &steps[0]),
        SubmissionOutcome::Confirmed(_)
    ));
    let prefix = owner.checkpoint().clone();
    assert_eq!(prefix, db.borrow().checkpoint);
    assert!(owner.is_current());
    assert!(!owner.has_uncertain_operation());
    assert_eq!(db.borrow().handlers, 1);
    db.borrow_mut().failure = Failure::None;
    assert!(matches!(
        submit_current(&mut owner, &steps[1]),
        SubmissionOutcome::Confirmed(_)
    ));
    let db = db.borrow();
    assert_eq!(db.observed_bases, [original.basis(), prefix.basis()]);
    assert_eq!(
        (
            db.engines,
            db.handlers,
            db.commits,
            db.publications,
            db.ledger.len()
        ),
        (2, 2, 2, 1, 2)
    );
    assert!(
        db.checkpoint
            .state()
            .decisions
            .starts_with(&prefix.state().decisions)
    );
}

#[test]
fn unknown_not_recorded_and_changed_lookup_identity_never_replay() {
    let (mut owner, db, _, steps) = controlled(Failure::UnknownNotCommitted);
    let original = owner.checkpoint().clone();
    assert_eq!(
        submit_current(&mut owner, &steps[0]),
        SubmissionOutcome::LookupRequired
    );
    db.borrow_mut().failure = Failure::None;
    assert_eq!(
        submit_current(&mut owner, &steps[0]),
        SubmissionOutcome::LookupRequired
    );
    db.borrow_mut().ephemeral_steps = steps[1..].to_vec();
    assert_eq!(
        submit_current(&mut owner, &steps[1]),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(
        submit_current_scope(
            &mut owner,
            Scope {
                input: steps[0].clone(),
                principal: member(99)
            }
        ),
        SubmissionOutcome::Refused(RepositoryError::Unauthorized)
    );
    assert_eq!(owner.checkpoint(), &original);
    assert_eq!(db.borrow().checkpoint, original);
    assert!(owner.has_uncertain_operation());
    assert_eq!(
        (
            db.borrow().engines,
            db.borrow().handlers,
            db.borrow().commits,
            db.borrow().publications
        ),
        (1, 1, 1, 0)
    );
    assert!(db.borrow().ledger.is_empty());

    // Existing repository exact-input lookup is also exercised independently of unknown state.
    let (mut known, db, steps) = setup(Failure::None);
    let accepted = submit(&mut known, &steps[0]);
    let prefix = known.checkpoint().clone();
    let mut changed = steps[0].clone();
    let GameInput::Game(command) = &mut changed else {
        unreachable!()
    };
    command.command = GameCommand::Speak {
        speaker: entity(4),
        text: "changed exact input".to_owned(),
        conversation: None,
    };
    assert_eq!(
        submit(&mut known, &changed),
        SubmissionOutcome::OperationConflict
    );
    assert_eq!(
        submit_scope(
            &mut known,
            Scope {
                input: steps[0].clone(),
                principal: member(99)
            }
        ),
        SubmissionOutcome::Refused(RepositoryError::Unauthorized)
    );
    assert_eq!(submit(&mut known, &steps[0]), accepted);
    assert_eq!(known.checkpoint(), &prefix);
    assert_eq!(db.borrow().handlers, 1);
}

#[test]
fn whole_list_bounds_and_unapproved_tail_are_preserved() {
    let current = waiting(0);
    let clean = two_steps(0);
    let inner = handler(&current, &clean[0], &[]);
    for case in 0..7 {
        let mut steps = clean.clone();
        let mut trusted = admission(&current, &steps[0]);
        let expected = match case {
            0 => {
                steps.clear();
                PlanError::EmptyPlan
            }
            1 => {
                trusted.plan.maximum_steps = 1;
                PlanError::Capacity
            }
            2 => {
                trusted.plan.maximum_total_input_bytes = 1;
                PlanError::Capacity
            }
            3 => {
                trusted.plan.maximum_comparisons = 0;
                PlanError::Capacity
            }
            4 => {
                steps[1] = steps[0].clone();
                PlanError::DuplicateOperation
            }
            5 => {
                let GameInput::Game(command) = &mut steps[1] else {
                    unreachable!()
                };
                command.member = member(99);
                PlanError::MemberMismatch { index: 1 }
            }
            _ => {
                let GameInput::Game(command) = &mut steps[1] else {
                    unreachable!()
                };
                command.basis.run = df_types::RunId::from_bytes(&[99; 16]).unwrap();
                PlanError::ScopeMismatch { index: 1 }
            }
        };
        assert_eq!(
            invoke(&steps, &[], &current, &current, &inner, trusted).err(),
            Some(PlanHeadError::Plan(expected))
        );
    }
    let unsupported = [response(3)];
    assert_eq!(
        invoke(
            &unsupported,
            &[],
            &current,
            &current,
            &inner,
            admission(&current, &clean[0])
        )
        .err(),
        Some(PlanHeadError::Plan(PlanError::UnsupportedStep { index: 0 }))
    );
    assert_eq!(inner.calls.get(), 0);
    let mut steps = clean;
    let GameInput::Game(command) = &mut steps[1] else {
        unreachable!()
    };
    let GameCommand::SelectChoice { option, .. } = &mut command.command else {
        unreachable!()
    };
    *option = label("unoffered-next-head");
    let staged = invoke(
        &steps,
        &[],
        &current,
        &current,
        &inner,
        admission(&current, &steps[0]),
    )
    .unwrap();
    assert!(std::ptr::eq(
        staged.requires_revalidation().as_ptr(),
        steps[1..].as_ptr()
    ));
    let next = staged.checkpoint();
    let next_handler = handler(next, &steps[1], &[]);
    assert_eq!(
        invoke(
            staged.requires_revalidation(),
            &[],
            next,
            next,
            &next_handler,
            admission(next, &steps[1])
        )
        .err(),
        Some(PlanHeadError::Plan(PlanError::Candidate(
            CandidateError::Command(CommandError::UnofferedResponse)
        )))
    );
    assert_eq!(next_handler.calls.get(), 0);
    assert_eq!(next.state().decisions.len(), 1);
    assert_eq!(operation(&steps[0]), next.state().decisions[0].operation);
}
