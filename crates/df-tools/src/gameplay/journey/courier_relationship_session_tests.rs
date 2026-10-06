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
        stage_with_supplier(current, input, &mut |sides| {
            assert_eq!(sides, 20);
            self.database.borrow_mut().samples += 1;
            Ok(10)
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
        assert_eq!(reaction_facts(checkpoint).len(), 1);
        assert_eq!(
            relation(checkpoint, entity(ENTITIES[0]).unwrap())
                .state
                .as_str(),
            ESCORT_SUPPORTED
        );
        assert_eq!(
            relation(checkpoint, entity(ENTITIES[1]).unwrap())
                .state
                .as_str(),
            UNFAMILIAR
        );
        db.publications += 1;
        Ok(())
    }
    fn wake_committed_intents(&mut self, _: &Scope) -> Result<(), DeliveryError> {
        Ok(())
    }
}
type Owner = DurableOwner<Repository, Engine, Publication>;
fn setup(failure: Failure) -> (Owner, Rc<RefCell<Database>>, Scope) {
    let current = escorted();
    let scope = Scope {
        input: defend_input(&current, 0, 7),
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
            build: "courier-relationship-consumer".to_owned(),
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
fn assert_exact_committed_change(before: &Checkpoint, after: &Checkpoint) {
    assert_eq!(reaction_facts(after).len(), 1);
    assert_eq!(
        &after.state().facts[..before.state().facts.len()],
        before.state().facts
    );
    assert_eq!(
        &after.state().decisions[..before.state().decisions.len()],
        before.state().decisions
    );
    assert_eq!(
        relation(after, entity(ENTITIES[0]).unwrap()).state.as_str(),
        ESCORT_SUPPORTED
    );
    assert_eq!(
        relation(after, entity(ENTITIES[1]).unwrap()),
        relation(before, entity(ENTITIES[1]).unwrap())
    );
    assert_eq!(after.state().continuity, before.state().continuity);
    assert_eq!(after.state().knowledge, before.state().knowledge);
    assert_eq!(after.state().draws.len(), before.state().draws.len() + 3);
}

#[test]
fn courier_relationship_session_ack_exact_retry_and_cancelled_receipt_apply_one_consequence() {
    for deliver in [false, true] {
        let (mut owner, database, scope) = setup(Failure::None);
        let original = owner.checkpoint().clone();
        let first = submit(&mut owner, &scope, deliver);
        let retry = submit(&mut owner, &scope, true).unwrap();
        assert!(matches!(retry, SubmissionOutcome::Confirmed(_)));
        if deliver {
            assert_eq!(Some(retry), first);
        }
        let db = database.borrow();
        assert_eq!(owner.checkpoint(), &db.checkpoint);
        assert_exact_committed_change(&original, &db.checkpoint);
        assert_eq!(
            (
                db.decisions,
                db.commits,
                db.publications,
                db.samples,
                db.ledger.len()
            ),
            (1, 1, 1, 3, 1)
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

#[test]
fn courier_relationship_session_commit_rollback_retains_canonical_state_and_safe_retry_applies_once()
 {
    let (mut owner, database, scope) = setup(Failure::BeforeCommit);
    let original = owner.checkpoint().clone();
    assert_eq!(
        submit(&mut owner, &scope, true),
        Some(SubmissionOutcome::Refused(RepositoryError::Unavailable))
    );
    assert_eq!(owner.checkpoint(), &original);
    assert_eq!(database.borrow().checkpoint, original);
    assert_eq!(database.borrow().publications, 0);
    assert!(reaction_facts(owner.checkpoint()).is_empty());
    database.borrow_mut().failure = Failure::None;
    assert!(matches!(
        submit(&mut owner, &scope, true),
        Some(SubmissionOutcome::Confirmed(_))
    ));
    let db = database.borrow();
    assert_eq!(owner.checkpoint(), &db.checkpoint);
    assert_exact_committed_change(&original, &db.checkpoint);
    assert_eq!(
        (
            db.decisions,
            db.commits,
            db.publications,
            db.samples,
            db.ledger.len()
        ),
        (2, 2, 1, 6, 1)
    );
}

#[test]
fn courier_relationship_session_lost_ack_fences_other_input_until_exact_receipt_and_validated_reload()
 {
    let (mut owner, database, scope) = setup(Failure::LostAck);
    let original = owner.checkpoint().clone();
    assert_eq!(
        submit(&mut owner, &scope, true),
        Some(SubmissionOutcome::LookupRequired)
    );
    assert!(owner.has_uncertain_operation());
    assert_eq!(owner.checkpoint(), &original);
    assert_exact_committed_change(&original, &database.borrow().checkpoint);
    let mut other = scope.clone();
    let GameInput::Game(command) = &mut other.input else {
        panic!("command")
    };
    command.operation = OperationId::from_bytes(&[8; 16]).unwrap();
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
    assert_exact_committed_change(&original, &db.checkpoint);
    assert_eq!(
        (
            db.decisions,
            db.commits,
            db.publications,
            db.samples,
            db.ledger.len()
        ),
        (1, 1, 0, 3, 1)
    );
}

#[test]
fn courier_relationship_session_unknown_not_recorded_never_reexecutes_or_publishes_reaction() {
    let (mut owner, database, scope) = setup(Failure::UnknownNotCommitted);
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
    assert!(reaction_facts(&db.checkpoint).is_empty());
    assert_eq!(
        (
            db.decisions,
            db.commits,
            db.publications,
            db.samples,
            db.ledger.len()
        ),
        (1, 1, 0, 3, 0)
    );
}
