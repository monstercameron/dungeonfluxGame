use super::*;
use df_observe::OperationContext;
use df_session::inbox::{ActorInput, AdmissionSequence, Reducer};
use df_session::submission::*;
use df_types::{OperationId, SessionId};
use std::cell::RefCell;
use std::rc::Rc;

// Controlled transaction/publication ports drive the real DurableOwner and registered
// native journey. They model acknowledgement states, not physical PostgreSQL authority.
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
    fn capture_uncertainty_key(&self, maximum: usize) -> Result<Self, RepositoryError> {
        if self
            .input
            .retained_bytes()
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
            .is_none_or(|bytes| bytes > maximum)
        {
            return Err(RepositoryError::Capacity);
        }
        Ok(self.clone())
    }
    fn session(&self) -> SessionId {
        let GameInput::Game(command) = &self.input else {
            panic!("command")
        };
        command.basis.session
    }
    fn operation(&self) -> OperationId {
        let GameInput::Game(command) = &self.input else {
            panic!("command")
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
    decisions: usize,
    commits: usize,
    publications: usize,
    samples: usize,
    refuse_reload: bool,
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
            .find(|(prior, _)| prior.operation() == scope.operation())
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
            .find(|decision| decision.operation == scope.operation())
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
        let db = self.database.borrow();
        if db.refuse_reload {
            return Err(RepositoryError::Unavailable);
        }
        Ok(db.checkpoint.clone())
    }
}
struct Engine {
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
        stage_with_supplier(current, input, &mut |_| {
            self.database.borrow_mut().samples += 1;
            panic!("story alternatives cannot request a draw")
        })
    }
    fn validate_recovery(&mut self, checkpoint: &Checkpoint) -> Result<(), RepositoryError> {
        super::super::super::phase(checkpoint)?;
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
        super::super::super::phase(checkpoint).unwrap();
        db.publications += 1;
        Ok(())
    }
    fn wake_committed_intents(&mut self, _: &Scope) -> Result<(), DeliveryError> {
        Ok(())
    }
}
type Owner = DurableOwner<Repository, Engine, Publication>;
fn setup(failure: Failure, entry: &str) -> (Owner, Rc<RefCell<Database>>, Scope) {
    let current = if entry == "begin-story" {
        prepared_story()
    } else {
        opening_story()
    };
    let scope = Scope {
        input: input(&current, first(), 50, entry, vec![]),
    };
    let database = Rc::new(RefCell::new(Database {
        checkpoint: current.clone(),
        ledger: vec![],
        failure,
        decisions: 0,
        commits: 0,
        publications: 0,
        samples: 0,
        refuse_reload: false,
    }));
    let owner = DurableOwner::new(
        Repository {
            database: Rc::clone(&database),
        },
        Engine {
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
fn submit(owner: &mut Owner, scope: &Scope, deliver: bool) -> Option<SubmissionOutcome> {
    let (input, wait) = OwnedInput::new(
        OperationContext {
            trace_parent: String::new(),
            build: "native-story-selection-consumer".to_owned(),
        },
        scope.clone(),
        scope.input.clone(),
    );
    if !deliver {
        drop(wait);
        owner.reduce(AdmissionSequence(1), input);
        return None;
    }
    owner.reduce(AdmissionSequence(1), input);
    Some(wait.try_recv().unwrap())
}
fn assert_exact_committed_change(before: &Checkpoint, after: &Checkpoint, scope: &Scope) {
    let expected = run(before, &scope.input);
    assert_eq!(after, &expected);
    assert_eq!(
        &after.state().facts[..before.state().facts.len()],
        before.state().facts
    );
    assert_eq!(
        &after.state().decisions[..before.state().decisions.len()],
        before.state().decisions
    );
    assert_eq!(after.state().draws, before.state().draws);
    let operation = scope.operation();
    assert_eq!(
        after
            .state()
            .decisions
            .iter()
            .filter(|decision| decision.operation == operation)
            .count(),
        1
    );
    let terminal = after.state().facts.last().unwrap();
    assert_eq!(
        after.state().narrative.accepted_facts.last(),
        Some(&terminal.id)
    );
    assert_eq!(
        after.state().narrative.open_threads,
        [model::content(PACKET_THREAD).unwrap()]
    );
}

#[test]
fn native_story_session_ack_retry_and_cancelled_wait_commit_each_alternative_once() {
    for entry in ["begin-story", "ask-courier", "escort-courier"] {
        for deliver in [false, true] {
            let (mut owner, database, scope) = setup(Failure::None, entry);
            let original = owner.checkpoint().clone();
            let first = submit(&mut owner, &scope, deliver);
            let retry = submit(&mut owner, &scope, true).unwrap();
            assert!(matches!(retry, SubmissionOutcome::Confirmed(_)));
            if deliver {
                assert_eq!(Some(retry), first);
            }
            let db = database.borrow();
            assert_eq!(owner.checkpoint(), &db.checkpoint);
            assert_exact_committed_change(&original, &db.checkpoint, &scope);
            assert_eq!(
                (
                    db.decisions,
                    db.commits,
                    db.publications,
                    db.samples,
                    db.ledger.len()
                ),
                (1, 1, 1, 0, 1)
            );
            let mut conflict = scope.clone();
            let GameInput::Game(command) = &mut conflict.input else {
                panic!("command")
            };
            command.observed_revision = command.observed_revision.next_sequence().unwrap();
            drop(db);
            assert_eq!(
                submit(&mut owner, &conflict, true),
                Some(SubmissionOutcome::OperationConflict)
            );
            assert_eq!(database.borrow().decisions, 1);
        }
    }
}

#[test]
fn native_story_session_rollback_retains_all_state_then_safe_retry_commits_once() {
    for entry in ["begin-story", "ask-courier", "escort-courier"] {
        let (mut owner, database, scope) = setup(Failure::BeforeCommit, entry);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope, true),
            Some(SubmissionOutcome::Refused(RepositoryError::Unavailable))
        );
        assert_eq!(owner.checkpoint(), &original);
        assert_eq!(database.borrow().checkpoint, original);
        assert_eq!(database.borrow().publications, 0);
        database.borrow_mut().failure = Failure::None;
        assert!(matches!(
            submit(&mut owner, &scope, true),
            Some(SubmissionOutcome::Confirmed(_))
        ));
        let db = database.borrow();
        assert_eq!(owner.checkpoint(), &db.checkpoint);
        assert_exact_committed_change(&original, &db.checkpoint, &scope);
        assert_eq!(
            (
                db.decisions,
                db.commits,
                db.publications,
                db.samples,
                db.ledger.len()
            ),
            (2, 2, 1, 0, 1)
        );
    }
}

#[test]
fn native_story_session_lost_ack_fences_other_input_until_exact_receipt_and_validated_reload() {
    for entry in ["begin-story", "ask-courier", "escort-courier"] {
        let (mut owner, database, scope) = setup(Failure::LostAck, entry);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope, true),
            Some(SubmissionOutcome::LookupRequired)
        );
        assert!(owner.has_uncertain_operation());
        assert_eq!(owner.checkpoint(), &original);
        assert_exact_committed_change(&original, &database.borrow().checkpoint, &scope);
        let mut other = scope.clone();
        let GameInput::Game(command) = &mut other.input else {
            panic!("command")
        };
        command.operation = OperationId::from_bytes(&[51; 16]).unwrap();
        assert_eq!(
            submit(&mut owner, &other, true),
            Some(SubmissionOutcome::LookupRequired)
        );
        database.borrow_mut().refuse_reload = true;
        let receipt = submit(&mut owner, &scope, true).unwrap();
        assert!(matches!(receipt, SubmissionOutcome::Confirmed(_)));
        assert!(owner.has_uncertain_operation());
        assert_eq!(owner.checkpoint(), &original);
        database.borrow_mut().refuse_reload = false;
        assert_eq!(submit(&mut owner, &scope, true), Some(receipt));
        assert!(owner.is_current());
        let db = database.borrow();
        assert_eq!(owner.checkpoint(), &db.checkpoint);
        assert_exact_committed_change(&original, &db.checkpoint, &scope);
        assert_eq!(
            (
                db.decisions,
                db.commits,
                db.publications,
                db.samples,
                db.ledger.len()
            ),
            (1, 1, 0, 0, 1)
        );
    }
}

#[test]
fn native_story_session_unknown_not_recorded_never_reexecutes_or_publishes() {
    for entry in ["begin-story", "ask-courier", "escort-courier"] {
        let (mut owner, database, scope) = setup(Failure::UnknownNotCommitted, entry);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope, true),
            Some(SubmissionOutcome::LookupRequired)
        );
        database.borrow_mut().failure = Failure::None;
        assert_eq!(
            submit(&mut owner, &scope, true),
            Some(SubmissionOutcome::LookupRequired)
        );
        assert!(owner.has_uncertain_operation());
        let db = database.borrow();
        assert_eq!(owner.checkpoint(), &original);
        assert_eq!(db.checkpoint, original);
        assert_eq!(
            (
                db.decisions,
                db.commits,
                db.publications,
                db.samples,
                db.ledger.len()
            ),
            (1, 1, 0, 0, 0)
        );
    }
}

#[test]
fn native_story_session_missing_canonical_creation_cause_refuses_before_commit_or_publication() {
    let (mut owner, database, _) = setup(Failure::None, "begin-story");
    let mut state = owner.checkpoint().state().clone();
    let create = model::content("create-character").unwrap();
    let operation = state.facts.iter().find(|fact|
        matches!(&fact.value, FactValue::ContentEvent { definition, .. } if *definition == create)
    ).unwrap().operation;
    state
        .decisions
        .retain(|decision| decision.operation != operation);
    let missing = model::checkpoint(owner.checkpoint().basis(), state).unwrap();
    let scope = Scope {
        input: input(&missing, first(), 52, "begin-story", vec![]),
    };
    database.borrow_mut().checkpoint = missing.clone();
    owner = DurableOwner::new(
        Repository {
            database: Rc::clone(&database),
        },
        Engine {
            database: Rc::clone(&database),
        },
        Publication {
            database: Rc::clone(&database),
        },
        missing.clone(),
        64 * 1024,
    )
    .unwrap();
    assert_eq!(
        submit(&mut owner, &scope, true),
        Some(SubmissionOutcome::Refused(
            RepositoryError::InvalidCandidate
        ))
    );
    assert_eq!(owner.checkpoint(), &missing);
    let db = database.borrow();
    assert_eq!(db.checkpoint, missing);
    assert_eq!(
        (
            db.decisions,
            db.commits,
            db.publications,
            db.samples,
            db.ledger.len()
        ),
        (1, 0, 0, 0, 0)
    );
}
