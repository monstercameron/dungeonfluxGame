#[path = "support/fixture_model.rs"]
pub mod fixture_model;

use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::command_entry::{
    CommandEntryContext, CommandEntryLimits, CommandRejection, decide_registered_command,
};
use df_engine::obligation_fulfillment::*;
use df_interaction::debts::{DebtStatus, DebtTransitionRefusal};
use df_model::checkpoint::*;
use df_model::commands::CommandLimits;
use df_rules::{
    DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandHandler, RulesCommandInput,
};
use df_types::OperationId;
use fixture_model as fixture;
use std::cell::Cell;

fn content(entry: &str) -> ContentReference {
    ContentReference {
        package: fixture::content().package,
        entry: fixture::label(entry),
    }
}
fn record(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}
fn fact(value: u8) -> FactId {
    FactId::from_bytes(&[value; 16]).unwrap()
}
fn operation(value: u8) -> OperationId {
    OperationId::from_bytes(&[value; 16]).unwrap()
}

struct Fixture {
    registration: FulfillmentRegistration,
    rules: Vec<RuleReference>,
    content: Vec<ContentReference>,
    resources: Vec<ResourceConstraint>,
    pins: CheckpointPins,
}
impl Fixture {
    fn new() -> Self {
        Self {
            registration: FulfillmentRegistration {
                obligation_definition: content("explicit-agreement"),
                cause_definition: content("accepted-delivery"),
                completion_definition: content("agreement-completed"),
                source: fixture::rule(),
                policy: fixture::label("compiled-completion-policy"),
            },
            rules: vec![fixture::rule()],
            content: vec![
                fixture::content(),
                content("explicit-agreement"),
                content("accepted-delivery"),
                content("agreement-completed"),
                content("other-admitted-event"),
            ],
            resources: fixture::resource_constraints(),
            pins: fixture::pins(),
        }
    }
    fn inventory(&self) -> ReferenceInventory<'_> {
        ReferenceInventory {
            rules: &self.rules,
            content: &self.content,
            resources: &self.resources,
            assets: &[],
        }
    }
    fn state(&self) -> GameState {
        let mut state = fixture::state();
        let mut beneficiary = state.entities[0].clone();
        beneficiary.id = fixture::entity(5);
        state.entities.push(beneficiary);
        state.obligations = vec![
            Obligation {
                id: record(40),
                obligor: fixture::entity(4),
                beneficiary: fixture::entity(5),
                definition: self.registration.obligation_definition.clone(),
                due: Some(state.logical_time),
                fulfilled: false,
            },
            Obligation {
                id: record(41),
                obligor: fixture::entity(5),
                beneficiary: fixture::entity(4),
                definition: self.registration.obligation_definition.clone(),
                due: None,
                fulfilled: false,
            },
        ];
        state.facts.push(GameFact {
            id: fact(30),
            revision: fixture::basis().revision,
            operation: operation(30),
            ordinal: 0,
            cause: None,
            audience: AudienceScope::Members(vec![fixture::member(3)]),
            value: FactValue::ContentEvent {
                definition: self.registration.cause_definition.clone(),
                subjects: vec![fixture::entity(4), fixture::entity(5)],
            },
        });
        state.decisions.push(AcceptedDecision {
            operation: operation(30),
            revision: fixture::basis().revision,
            facts: vec![fact(30)],
            draws: vec![],
            effects: vec![],
            source_policy: fixture::label("original-accepted-cause"),
            semantic_output: None,
        });
        state
    }
    fn checkpoint(&self, basis: Basis, state: GameState) -> Checkpoint {
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis,
            self.pins.clone(),
            state,
            self.inventory(),
            fixture::limits(),
        )
        .unwrap()
    }
    fn current(&self) -> Checkpoint {
        self.checkpoint(fixture::basis(), self.state())
    }
    fn input(&self, basis: Basis, id: OperationId) -> GameInput {
        GameInput::Game(CommandInput {
            basis,
            observed_revision: basis.revision,
            operation: id,
            member: fixture::member(3),
            command: GameCommand::ProposeAction {
                actor: fixture::entity(4),
                action: self.registration.obligation_definition.clone(),
                targets: vec![fixture::entity(5)],
                choices: vec![],
            },
        })
    }
    fn handler<'a>(
        &'a self,
        owner: &'a SourceOwner,
        basis: Basis,
    ) -> ObligationFulfillmentHandler<'a, SourceOwner> {
        ObligationFulfillmentHandler {
            owner,
            current_basis: basis,
            admitted_pins: &self.pins,
            inventory: self.inventory(),
            registration: &self.registration,
            obligation: record(40),
            cause: AcceptedFulfillmentCause {
                fact: fact(30),
                operation: operation(30),
                revision: fixture::basis().revision,
            },
            completion_fact: fact(50),
            limits: FulfillmentLimits {
                maximum_checkpoint_bytes: 1024 * 1024,
                maximum_pass_bytes: 4 * 1024 * 1024,
                maximum_inventory_records: 32,
                checkpoint: fixture::limits(),
            },
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum SourceRefusal {
    Withdrawn,
    Binding,
    ActorControl,
}

struct SourceOwner {
    expected: FulfillmentRegistration,
    pins: CheckpointPins,
    withdrawn: Cell<bool>,
    actor_control: Cell<bool>,
    calls: Cell<usize>,
}
impl SourceOwner {
    fn new(fixture: &Fixture) -> Self {
        Self {
            expected: fixture.registration.clone(),
            pins: fixture.pins.clone(),
            withdrawn: Cell::new(false),
            actor_control: Cell::new(true),
            calls: Cell::new(0),
        }
    }
}
impl FulfillmentSourceOwner for SourceOwner {
    type Refusal = SourceRefusal;
    fn admit(
        &self,
        current: &Checkpoint,
        registration: &FulfillmentRegistration,
        command: &CommandInput,
        obligation: &Obligation,
        _: &GameFact,
    ) -> Result<(), Self::Refusal> {
        self.calls.set(self.calls.get() + 1);
        if self.withdrawn.get() {
            return Err(SourceRefusal::Withdrawn);
        }
        if current.pins() != &self.pins || registration != &self.expected {
            return Err(SourceRefusal::Binding);
        }
        if !self.actor_control.get()
            || !current.state().members.iter().any(|link| {
                link.member == command.member && link.character == Some(obligation.obligor)
            })
        {
            return Err(SourceRefusal::ActorControl);
        }
        Ok(())
    }
}

fn registered(
    fixture: &Fixture,
    owner: &SourceOwner,
    current: &Checkpoint,
    input: &GameInput,
) -> Result<Checkpoint, CommandRejection<FulfillmentError<SourceRefusal>>> {
    let handler = fixture.handler(owner, current.basis());
    registered_handler(fixture, current, input, &handler)
}
fn registered_handler(
    fixture: &Fixture,
    current: &Checkpoint,
    input: &GameInput,
    handler: &ObligationFulfillmentHandler<'_, SourceOwner>,
) -> Result<Checkpoint, CommandRejection<FulfillmentError<SourceRefusal>>> {
    let selector = fixture::label("compiled-obligation-completion");
    let entries = [CatalogEntry::new(
        &fixture.registration.source,
        b"exact synthetic source",
    )];
    let catalog = CatalogSnapshot::from_published(
        &fixture.pins.rules.catalog,
        &fixture.pins,
        b"complete synthetic catalog",
        &entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 1,
            max_item_bytes: 64,
            max_total_item_bytes: 64,
        },
    )
    .unwrap();
    let registrations = [HandlerRegistration::new(
        &selector,
        &fixture.registration.source,
        handler,
    )];
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 1).unwrap();
    decide_registered_command(
        RulesCommandInput {
            command: input,
            supplied_draws: &[],
        },
        current,
        CommandEntryContext {
            current_basis: handler.current_basis,
            admitted_pins: handler.admitted_pins,
            inventory: fixture.inventory(),
            limits: CommandEntryLimits {
                command: CommandLimits {
                    maximum_records: 100,
                    maximum_text_bytes: 256,
                    maximum_retained_bytes: 1024 * 1024,
                },
                maximum_staged_bytes: 1024 * 1024,
            },
        },
        &registry,
        &selector,
        &fixture.registration.source,
    )
}

#[test]
fn registered_completion_retains_exact_cause_terms_parties_and_every_unrelated_field() {
    let fixture = Fixture::new();
    let owner = SourceOwner::new(&fixture);
    let current = fixture.current();
    let original = current.clone();
    let candidate = registered(
        &fixture,
        &owner,
        &current,
        &fixture.input(current.basis(), operation(50)),
    )
    .unwrap();
    let mut expected = current.state().clone();
    expected.obligations[0].fulfilled = true;
    let completion = candidate.state().facts.last().unwrap();
    assert_eq!(completion.cause, Some(fact(30)));
    assert_eq!(completion.operation, operation(50));
    assert_eq!(completion.revision, candidate.basis().revision);
    assert_eq!(completion.ordinal, 0);
    assert_eq!(completion.audience, current.state().facts[0].audience);
    assert_eq!(
        completion.value,
        FactValue::ContentEvent {
            definition: fixture.registration.completion_definition.clone(),
            subjects: vec![fixture::entity(4), fixture::entity(5)],
        }
    );
    let decision = candidate.state().decisions.last().unwrap();
    assert_eq!(decision.source_policy, fixture.registration.policy);
    assert_eq!(decision.facts, [fact(50)]);
    assert_eq!(decision.operation, operation(50));
    assert_eq!(decision.revision, candidate.basis().revision);
    assert!(decision.draws.is_empty() && decision.effects.is_empty());
    expected.facts.push(completion.clone());
    expected.decisions.push(decision.clone());
    assert_eq!(candidate.state(), &expected);
    assert_eq!(candidate.pins(), current.pins());
    assert_eq!(
        candidate.basis().revision,
        current.basis().revision.next_sequence().unwrap()
    );
    assert_eq!(current, original);
    assert_eq!(owner.calls.get(), 1);
}

#[test]
fn uncommitted_unrelated_or_wrong_subject_causes_never_produce_a_candidate() {
    for case in ["uncommitted", "definition", "subjects", "missing"] {
        let fixture = Fixture::new();
        let owner = SourceOwner::new(&fixture);
        let mut state = fixture.state();
        let expected = match case {
            "uncommitted" => {
                state.decisions.clear();
                FulfillmentError::MissingAcceptedCause
            }
            "definition" => {
                state.facts[0].value = FactValue::ContentEvent {
                    definition: content("other-admitted-event"),
                    subjects: vec![fixture::entity(4), fixture::entity(5)],
                };
                FulfillmentError::WrongCause
            }
            "subjects" => {
                state.facts[0].value = FactValue::ContentEvent {
                    definition: fixture.registration.cause_definition.clone(),
                    subjects: vec![fixture::entity(5), fixture::entity(4)],
                };
                FulfillmentError::WrongSubjects
            }
            "missing" => {
                state.facts.clear();
                state.decisions.clear();
                FulfillmentError::MissingAcceptedCause
            }
            _ => unreachable!(),
        };
        let current = fixture.checkpoint(fixture::basis(), state);
        let saved = current.clone();
        assert_eq!(
            registered(
                &fixture,
                &owner,
                &current,
                &fixture.input(current.basis(), operation(50))
            ),
            Err(CommandRejection::Invocation(InvocationError::Handler(
                expected
            ))),
        );
        assert_eq!(current, saved);
        assert_eq!(owner.calls.get(), 0);
    }
}

#[test]
fn current_exact_cause_identity_registration_and_capacity_are_enforced() {
    let fixture = Fixture::new();
    let owner = SourceOwner::new(&fixture);
    let current = fixture.current();
    let input = fixture.input(current.basis(), operation(50));
    for case in [
        "cause-operation",
        "cause-revision",
        "duplicate",
        "obligation",
        "capacity",
    ] {
        let mut handler = fixture.handler(&owner, current.basis());
        let expected = match case {
            "cause-operation" => {
                handler.cause.operation = operation(31);
                FulfillmentError::MissingAcceptedCause
            }
            "cause-revision" => {
                handler.cause.revision = fixture::revision(2, 7);
                FulfillmentError::MissingAcceptedCause
            }
            "duplicate" => {
                handler.completion_fact = fact(30);
                FulfillmentError::DuplicateFact
            }
            "obligation" => {
                handler.obligation = record(99);
                FulfillmentError::UnknownObligation
            }
            "capacity" => {
                handler.limits.maximum_pass_bytes = 1;
                FulfillmentError::Capacity
            }
            _ => unreachable!(),
        };
        assert_eq!(
            registered_handler(&fixture, &current, &input, &handler),
            Err(CommandRejection::Invocation(InvocationError::Handler(
                expected
            ))),
        );
    }
    let mut changed = fixture.registration.clone();
    changed.completion_definition = content("other-admitted-event");
    let mut handler = fixture.handler(&owner, current.basis());
    handler.registration = &changed;
    assert_eq!(
        registered_handler(&fixture, &current, &input, &handler),
        Err(CommandRejection::Invocation(InvocationError::Handler(
            FulfillmentError::Source(SourceRefusal::Binding),
        ))),
    );
    let mut unadmitted = fixture.registration.clone();
    unadmitted.completion_definition = content("unadmitted-completion");
    handler.registration = &unadmitted;
    assert_eq!(
        registered_handler(&fixture, &current, &input, &handler),
        Err(CommandRejection::Invocation(InvocationError::Handler(
            FulfillmentError::InvalidRegistration,
        ))),
    );
    assert_eq!(current, fixture.current());
}

#[test]
fn withdrawn_source_actor_control_terminal_state_and_stale_bindings_refuse_atomically() {
    let fixture = Fixture::new();
    let owner = SourceOwner::new(&fixture);
    let current = fixture.current();
    let input = fixture.input(current.basis(), operation(50));
    owner.withdrawn.set(true);
    assert_eq!(
        registered(&fixture, &owner, &current, &input),
        Err(CommandRejection::Invocation(InvocationError::Handler(
            FulfillmentError::Source(SourceRefusal::Withdrawn),
        )))
    );
    owner.withdrawn.set(false);
    owner.actor_control.set(false);
    assert_eq!(
        registered(&fixture, &owner, &current, &input),
        Err(CommandRejection::Invocation(InvocationError::Handler(
            FulfillmentError::Source(SourceRefusal::ActorControl),
        )))
    );
    owner.actor_control.set(true);
    let mut state = fixture.state();
    state.obligations[0].fulfilled = true;
    let terminal = fixture.checkpoint(current.basis(), state);
    assert_eq!(
        registered(&fixture, &owner, &terminal, &input),
        Err(CommandRejection::Invocation(InvocationError::Handler(
            FulfillmentError::Lifecycle(DebtTransitionRefusal::AlreadyTerminal(
                DebtStatus::Completed
            )),
        )))
    );
    let mut handler = fixture.handler(&owner, current.basis());
    handler.current_basis.revision = fixture::revision(2, 7);
    assert_eq!(
        registered_handler(&fixture, &current, &input, &handler),
        Err(CommandRejection::Checkpoint(CheckpointError::StaleBasis))
    );
    let mut stale_pins = fixture.pins.clone();
    stale_pins.content.content_digest = ContentDigest([99; 32]);
    handler.current_basis = current.basis();
    handler.admitted_pins = &stale_pins;
    assert_eq!(
        registered_handler(&fixture, &current, &input, &handler),
        Err(CommandRejection::Checkpoint(
            CheckpointError::ContentMismatch
        ))
    );
    let mut stale_input = fixture.input(current.basis(), operation(50));
    let GameInput::Game(command) = &mut stale_input else {
        unreachable!()
    };
    command.basis.revision = fixture::revision(2, 7);
    command.observed_revision = fixture::revision(2, 7);
    assert_eq!(
        registered(&fixture, &owner, &current, &stale_input),
        Err(CommandRejection::Invocation(InvocationError::Handler(
            FulfillmentError::StaleCommand
        )))
    );
    assert_eq!(current, fixture.current());
}

#[cfg(not(target_arch = "wasm32"))]
mod session {
    use super::*;
    use df_observe::OperationContext;
    use df_session::inbox::{ActorInput, AdmissionSequence, Reducer};
    use df_session::submission::*;
    use df_types::SessionId;
    use std::cell::RefCell;
    use std::rc::Rc;

    // Controlled transaction/ledger fixture. Actual PostgreSQL qualification belongs to Root.
    #[derive(Clone, Eq, PartialEq)]
    struct Scope {
        input: GameInput,
    }
    impl ActorInput for Scope {
        fn retained_heap_bytes(&self) -> Option<usize> {
            self.input.retained_heap_bytes()
        }
    }
    impl OperationScope for Scope {
        type UncertaintyKey = Scope;
        fn capture_uncertainty_key(&self, maximum: usize) -> Result<Scope, RepositoryError> {
            if self
                .input
                .retained_bytes()
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Scope>()))
                .is_none_or(|bytes| bytes > maximum)
            {
                return Err(RepositoryError::Capacity);
            }
            Ok(self.clone())
        }
        fn session(&self) -> SessionId {
            let GameInput::Game(command) = &self.input else {
                unreachable!()
            };
            command.basis.session
        }
        fn operation(&self) -> OperationId {
            let GameInput::Game(command) = &self.input else {
                unreachable!()
            };
            command.operation
        }
        fn validate_input(&self, input: &GameInput) -> Result<(), RepositoryError> {
            if &self.input == input {
                Ok(())
            } else {
                Err(RepositoryError::InputBinding)
            }
        }
        fn is_lookup_only(&self) -> bool {
            false
        }
    }
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum Failure {
        None,
        BeforeCommit,
        LostAck,
        UnknownNotCommitted,
    }
    struct Database {
        checkpoint: Checkpoint,
        ledger: Vec<(Scope, DecisionReceipt)>,
        failure: Failure,
        commits: usize,
        decisions: usize,
        publications: usize,
    }
    struct Repository {
        database: Rc<RefCell<Database>>,
    }
    impl SessionRepository for Repository {
        type Scope = Scope;
        fn lookup_operation(
            &mut self,
            scope: &Scope,
            _: &OperationContext,
        ) -> Result<OperationLookup, RepositoryError> {
            let db = self.database.borrow();
            if let Some((prior, receipt)) = db
                .ledger
                .iter()
                .find(|(s, _)| s.operation() == scope.operation())
            {
                return Ok(if prior == scope {
                    OperationLookup::Committed(receipt.clone())
                } else {
                    OperationLookup::Conflict
                });
            }
            Ok(OperationLookup::NotRecorded)
        }
        fn commit_decision(
            &mut self,
            scope: &Scope,
            candidate: &Checkpoint,
            expected: Basis,
            _: &OperationContext,
        ) -> Result<CommitOutcome, RepositoryError> {
            let mut db = self.database.borrow_mut();
            db.commits += 1;
            if expected != db.checkpoint.basis() {
                return Err(RepositoryError::RevisionConflict);
            }
            if db.failure == Failure::BeforeCommit {
                return Err(RepositoryError::Unavailable);
            }
            if db.failure == Failure::UnknownNotCommitted {
                return Ok(CommitOutcome::Indeterminate);
            }
            let decision = candidate
                .state()
                .decisions
                .iter()
                .find(|d| d.operation == scope.operation())
                .unwrap()
                .clone();
            let receipt = DecisionReceipt::new(candidate.basis(), decision, 64 * 1024)?;
            db.checkpoint = candidate.clone();
            db.ledger.push((scope.clone(), receipt.clone()));
            if db.failure == Failure::LostAck {
                Ok(CommitOutcome::Indeterminate)
            } else {
                Ok(CommitOutcome::Confirmed(receipt))
            }
        }
        fn load_current(
            &mut self,
            _: &Scope,
            _: &OperationContext,
        ) -> Result<Checkpoint, RepositoryError> {
            Ok(self.database.borrow().checkpoint.clone())
        }
    }
    struct Engine {
        fixture: Fixture,
        owner: SourceOwner,
        database: Rc<RefCell<Database>>,
    }
    impl SessionEngine<Scope> for Engine {
        fn decide(
            &mut self,
            current: &Checkpoint,
            _: &Scope,
            input: &GameInput,
        ) -> Result<Checkpoint, RepositoryError> {
            self.database.borrow_mut().decisions += 1;
            registered(&self.fixture, &self.owner, current, input)
                .map_err(|_| RepositoryError::InvalidCandidate)
        }
        fn validate_recovery(&mut self, current: &Checkpoint) -> Result<(), RepositoryError> {
            current
                .validate_resume(current.basis(), &self.fixture.pins)
                .map_err(|_| RepositoryError::InvalidCandidate)?;
            Ok(())
        }
    }
    struct Publication {
        database: Rc<RefCell<Database>>,
    }
    impl PublicationOwner<Scope> for Publication {
        fn publish_committed(
            &mut self,
            _: &Scope,
            checkpoint: &Checkpoint,
        ) -> Result<(), DeliveryError> {
            let mut db = self.database.borrow_mut();
            assert_eq!(checkpoint, &db.checkpoint);
            assert!(checkpoint.state().obligations[0].fulfilled);
            db.publications += 1;
            Ok(())
        }
        fn wake_committed_intents(&mut self, _: &Scope) -> Result<(), DeliveryError> {
            Ok(())
        }
    }
    type Owner = DurableOwner<Repository, Engine, Publication>;
    fn setup(failure: Failure) -> (Owner, Rc<RefCell<Database>>, Scope) {
        let fixture = Fixture::new();
        let current = fixture.current();
        let scope = Scope {
            input: fixture.input(current.basis(), operation(50)),
        };
        let source = SourceOwner::new(&fixture);
        let database = Rc::new(RefCell::new(Database {
            checkpoint: current.clone(),
            ledger: vec![],
            failure,
            commits: 0,
            decisions: 0,
            publications: 0,
        }));
        let owner = DurableOwner::new(
            Repository {
                database: Rc::clone(&database),
            },
            Engine {
                fixture,
                owner: source,
                database: Rc::clone(&database),
            },
            Publication {
                database: Rc::clone(&database),
            },
            current,
            64 * 1024,
        )
        .unwrap();
        (owner, database, scope)
    }
    fn submit(owner: &mut Owner, scope: &Scope) -> SubmissionOutcome {
        let (input, wait) = OwnedInput::new(
            OperationContext {
                trace_parent: String::new(),
                build: "obligation-fixture".to_owned(),
            },
            scope.clone(),
            scope.input.clone(),
        );
        owner.reduce(AdmissionSequence(1), input);
        wait.try_recv().unwrap()
    }
    #[test]
    fn normal_submit_commits_whole_completion_and_exact_retry_uses_original_receipt() {
        let (mut owner, database, scope) = setup(Failure::None);
        let first = submit(&mut owner, &scope);
        assert!(matches!(first, SubmissionOutcome::Confirmed(_)));
        assert_eq!(submit(&mut owner, &scope), first);
        let db = database.borrow();
        assert_eq!(owner.checkpoint(), &db.checkpoint);
        assert!(db.checkpoint.state().obligations[0].fulfilled);
        assert!(!db.checkpoint.state().obligations[1].fulfilled);
        assert_eq!(db.checkpoint.state().facts.len(), 2);
        assert_eq!(db.checkpoint.state().decisions.len(), 2);
        assert_eq!(
            (db.decisions, db.commits, db.publications, db.ledger.len()),
            (1, 1, 1, 1)
        );
    }
    #[test]
    fn known_failed_commit_retains_cause_prefix_and_allows_safe_exact_retry() {
        let (mut owner, database, scope) = setup(Failure::BeforeCommit);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::Refused(RepositoryError::Unavailable)
        );
        assert_eq!(owner.checkpoint(), &original);
        assert_eq!(database.borrow().checkpoint, original);
        assert!(database.borrow().ledger.is_empty());
        assert_eq!(database.borrow().publications, 0);
        database.borrow_mut().failure = Failure::None;
        assert!(matches!(
            submit(&mut owner, &scope),
            SubmissionOutcome::Confirmed(_)
        ));
        let db = database.borrow();
        assert_eq!(
            (db.decisions, db.commits, db.publications, db.ledger.len()),
            (2, 2, 1, 1)
        );
        assert!(
            db.checkpoint
                .state()
                .facts
                .starts_with(&original.state().facts)
        );
        assert!(
            db.checkpoint
                .state()
                .decisions
                .starts_with(&original.state().decisions)
        );
    }
    #[test]
    fn lost_commit_ack_recovers_exact_identity_without_duplicate_fulfillment_or_publication() {
        let (mut owner, database, scope) = setup(Failure::LostAck);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::LookupRequired
        );
        assert!(owner.has_uncertain_operation());
        assert_eq!(owner.checkpoint(), &original);
        assert!(database.borrow().checkpoint.state().obligations[0].fulfilled);
        assert_eq!(database.borrow().publications, 0);
        let mut other = scope.clone();
        let GameInput::Game(command) = &mut other.input else {
            unreachable!()
        };
        command.operation = operation(51);
        assert_eq!(
            submit(&mut owner, &other),
            SubmissionOutcome::LookupRequired
        );
        assert!(matches!(
            submit(&mut owner, &scope),
            SubmissionOutcome::Confirmed(_)
        ));
        assert!(owner.is_current());
        let db = database.borrow();
        assert_eq!(owner.checkpoint(), &db.checkpoint);
        assert_eq!(
            (db.decisions, db.commits, db.publications, db.ledger.len()),
            (1, 1, 0, 1)
        );
        assert_eq!(db.checkpoint.state().facts.len(), 2);
    }
    #[test]
    fn unknown_absent_commit_never_replays_even_when_original_ledger_lookup_is_not_recorded() {
        let (mut owner, database, scope) = setup(Failure::UnknownNotCommitted);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::LookupRequired
        );
        database.borrow_mut().failure = Failure::None;
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::LookupRequired
        );
        let mut conflict = scope.clone();
        let GameInput::Game(command) = &mut conflict.input else {
            unreachable!()
        };
        command.command = GameCommand::Speak {
            speaker: fixture::entity(4),
            text: "different exact input".to_owned(),
            conversation: None,
        };
        assert_eq!(
            submit(&mut owner, &conflict),
            SubmissionOutcome::LookupRequired
        );
        assert!(owner.has_uncertain_operation());
        let db = database.borrow();
        assert_eq!(owner.checkpoint(), &original);
        assert_eq!(db.checkpoint, original);
        assert_eq!(
            (db.decisions, db.commits, db.publications, db.ledger.len()),
            (1, 1, 0, 0)
        );
    }
}

#[test]
fn wrong_agreement_actor_target_and_unsupported_command_cannot_complete() {
    let fixture = Fixture::new();
    let owner = SourceOwner::new(&fixture);
    let current = fixture.current();
    let mut state = fixture.state();
    state.obligations[0].definition = content("other-admitted-event");
    let wrong_agreement = fixture.checkpoint(current.basis(), state);
    assert_eq!(
        registered(
            &fixture,
            &owner,
            &wrong_agreement,
            &fixture.input(wrong_agreement.basis(), operation(50)),
        ),
        Err(CommandRejection::Invocation(InvocationError::Handler(
            FulfillmentError::InvalidRegistration,
        )))
    );
    for case in ["actor", "action", "target", "speech"] {
        let mut input = fixture.input(current.basis(), operation(50));
        let GameInput::Game(command) = &mut input else {
            unreachable!()
        };
        match case {
            "actor" => {
                command.command = GameCommand::ProposeAction {
                    actor: fixture::entity(5),
                    action: fixture.registration.obligation_definition.clone(),
                    targets: vec![fixture::entity(5)],
                    choices: vec![],
                }
            }
            "action" => {
                command.command = GameCommand::ProposeAction {
                    actor: fixture::entity(4),
                    action: content("other-admitted-event"),
                    targets: vec![fixture::entity(5)],
                    choices: vec![],
                }
            }
            "target" => {
                command.command = GameCommand::ProposeAction {
                    actor: fixture::entity(4),
                    action: fixture.registration.obligation_definition.clone(),
                    targets: vec![fixture::entity(4)],
                    choices: vec![],
                }
            }
            "speech" => {
                command.command = GameCommand::Speak {
                    speaker: fixture::entity(4),
                    text: "I fulfilled the promise".to_owned(),
                    conversation: None,
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(
            registered(&fixture, &owner, &current, &input),
            Err(CommandRejection::Invocation(InvocationError::Handler(
                FulfillmentError::WrongCommand
            )))
        );
    }
    let handler = fixture.handler(&owner, current.basis());
    let host = GameInput::Host(HostInput {
        basis: current.basis(),
        operation: operation(50),
        host: fixture::member(3),
        command: HostCommand::RequestCheckpoint,
    });
    assert_eq!(
        handler.stage(
            RulesCommandInput {
                command: &host,
                supplied_draws: &[]
            },
            &current
        ),
        Err(FulfillmentError::UnsupportedInput)
    );
    assert_eq!(owner.calls.get(), 0);
    assert_eq!(current, fixture.current());
}
