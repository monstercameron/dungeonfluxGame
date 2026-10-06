#[path = "support/fixture_model.rs"]
pub mod fixture_model;
#[path = "support/relationship_staging.rs"]
pub mod support;

use df_engine::obligation_fulfillment::FulfillmentError;
use df_engine::relationship_staging::*;
use df_interaction::reactions::*;
use df_model::checkpoint::*;
use df_rules::{RulesCommandHandler, RulesCommandInput, stage_handler};
use fixture_model as model;
use support::*;

#[test]
fn registered_real_completion_applies_only_exact_direction_and_retains_all_mechanical_siblings() {
    let fixture = Fixture::new();
    let owner = SourceOwner::new(&fixture);
    let current = fixture.current();
    let original = current.clone();
    let input = fixture.input(&current);
    let mut base = stage_handler(
        &fixture.inner(&owner, &current),
        &fixture.pins,
        RulesCommandInput {
            command: &input,
            supplied_draws: &[],
        },
        &current,
        1024 * 1024,
    )
    .unwrap()
    .state()
    .clone();
    let candidate = registered(&fixture, &owner, &current, &input).unwrap();
    base.relationships[0].state = model::label("receptive");
    assert_eq!(candidate.state(), &base);
    assert_eq!(
        candidate.state().relationships[1],
        current.state().relationships[1]
    );
    assert_eq!(candidate.pins(), current.pins());
    assert_eq!(
        candidate.basis().revision,
        current.basis().revision.next_sequence().unwrap()
    );
    assert_eq!(candidate.state().facts.last().unwrap().id, fact(50));
    assert_eq!(
        candidate.state().facts.len(),
        current.state().facts.len() + 1
    );
    assert_eq!(current.state().draws.len(), 1);
    assert_eq!(candidate.state().draws, current.state().draws);
    assert_eq!(candidate.state().decisions[0].draws, [0]);
    assert_eq!(
        candidate.state().decisions.last().unwrap().source_policy,
        fixture.registration.policy
    );
    assert_eq!(current, original);
}

#[test]
fn ignorance_beliefs_memory_and_wrong_witness_return_typed_no_reaction_without_social_change() {
    for missing in ["known", "witness", "wrong-observer", "wrong-event"] {
        let fixture = Fixture::new();
        let owner = SourceOwner::new(&fixture);
        let mut state = fixture.state();
        let expected = if missing == "known" {
            state.continuity.npcs[0].known_facts.clear();
            state.beliefs.push(AttributedClaim {
                id: record(70),
                holder: model::entity(5),
                subject: model::entity(4),
                claim: "Private claimed perception".to_owned(),
                evidence: vec![fact(30)],
                audience: AudienceScope::Host,
                source: model::content(),
            });
            state.continuity.npcs[0].beliefs.push(record(70));
            state.memories.push(MemoryEpisode {
                id: record(71),
                holder: model::entity(5),
                source_facts: vec![fact(30)],
                retained_text: "Private memory claiming witness".to_owned(),
                audience: AudienceScope::Host,
                source_revision: model::basis().revision,
            });
            NoReactionReason::NotKnown
        } else {
            match missing {
                "witness" => state.continuity.witnesses.clear(),
                "wrong-observer" => state.continuity.witnesses[0].observer = model::entity(4),
                "wrong-event" => {
                    let mut fact_record = state.facts[0].clone();
                    fact_record.id = fact(32);
                    fact_record.ordinal = 2;
                    state.facts.push(fact_record);
                    state.decisions[0].facts.push(fact(32));
                    state.continuity.witnesses[0].fact = fact(32);
                }
                _ => unreachable!(),
            }
            NoReactionReason::NotWitnessed
        };
        let current = fixture.checkpoint(state);
        let original = current.clone();
        let result = stage(
            &fixture,
            &owner,
            &current,
            &fixture.input(&current),
            &[fixture.request(&current)],
            limits(),
        )
        .unwrap();
        assert_eq!(result.reactions, [ReactionOutcome::NoReaction(expected)]);
        assert_eq!(
            result.checkpoint.state().relationships,
            current.state().relationships
        );
        assert_eq!(result.checkpoint.state().beliefs, current.state().beliefs);
        assert_eq!(result.checkpoint.state().memories, current.state().memories);
        assert_eq!(
            result.checkpoint.state().continuity,
            current.state().continuity
        );
        assert_eq!(current, original);
    }
}

#[test]
fn forged_actor_and_revoked_native_admission_fail_before_reaction_entry_access() {
    for failure in [
        "actor",
        "member",
        "owner",
        "withdrawn",
        "control",
        "command",
    ] {
        let fixture = Fixture::new();
        let owner = SourceOwner::new(&fixture);
        let mut state = fixture.state();
        if failure == "owner" {
            state.members.push(MembershipLink {
                member: model::member(9),
                character: Some(model::entity(4)),
            });
            state.characters[0].owner = model::member(9);
        }
        let current = fixture.checkpoint(state);
        let mut input = fixture.input(&current);
        let GameInput::Game(command) = &mut input else {
            unreachable!()
        };
        match failure {
            "actor" => {
                let GameCommand::ProposeAction { actor, .. } = &mut command.command else {
                    unreachable!()
                };
                *actor = model::entity(5);
            }
            "member" => command.member = model::member(9),
            "withdrawn" => owner.withdrawn.set(true),
            "control" => owner.controlled.set(false),
            "command" => {
                let GameCommand::ProposeAction { targets, .. } = &mut command.command else {
                    unreachable!()
                };
                targets.clear();
            }
            _ => {}
        }
        let original = current.clone();
        assert!(
            stage(
                &fixture,
                &owner,
                &current,
                &input,
                &[fixture.request(&current)],
                limits()
            )
            .is_err()
        );
        assert_eq!(owner.entry_calls.get(), 0);
        assert_eq!(current, original);
    }
}

#[test]
fn newly_staged_completion_fact_is_never_treated_as_a_committed_perception() {
    let fixture = Fixture::new();
    let owner = SourceOwner::new(&fixture);
    let current = fixture.current();
    let original = current.clone();
    let mut request = fixture.request(&current);
    request.event = fact(50);
    assert!(matches!(
        stage(
            &fixture,
            &owner,
            &current,
            &fixture.input(&current),
            &[request],
            limits()
        ),
        Err(RelationshipStagingError::Reaction(
            ReactionError::UnknownEvent
        ))
    ));
    assert_eq!(current, original);
    assert_eq!(owner.command_calls.get(), 1);
}

#[test]
fn duplicate_or_contradictory_authored_entries_and_duplicate_directions_refuse_whole_pass() {
    for contradiction in [false, true] {
        let mut fixture = Fixture::new();
        let mut entry = fixture.entries[0].clone();
        if contradiction {
            entry.to_state = model::label("suspicious");
        }
        fixture.entries.push(entry);
        let owner = SourceOwner::new(&fixture);
        let current = fixture.current();
        assert!(matches!(
            stage(
                &fixture,
                &owner,
                &current,
                &fixture.input(&current),
                &[fixture.request(&current)],
                limits()
            ),
            Err(RelationshipStagingError::Reaction(
                ReactionError::AmbiguousPolicy
            ))
        ));
        fixture.entries.reverse();
        let owner = SourceOwner::new(&fixture);
        assert!(matches!(
            stage(
                &fixture,
                &owner,
                &current,
                &fixture.input(&current),
                &[fixture.request(&current)],
                limits()
            ),
            Err(RelationshipStagingError::Reaction(
                ReactionError::AmbiguousPolicy
            ))
        ));
    }
    let fixture = Fixture::new();
    let owner = SourceOwner::new(&fixture);
    let current = fixture.current();
    let request = fixture.request(&current);
    assert!(matches!(
        stage(
            &fixture,
            &owner,
            &current,
            &fixture.input(&current),
            &[request, request],
            limits()
        ),
        Err(RelationshipStagingError::DuplicateDirection)
    ));
    let mut state = fixture.state();
    state.relationships.push(state.relationships[0].clone());
    let duplicate = fixture.checkpoint(state);
    assert!(matches!(
        stage(
            &fixture,
            &owner,
            &duplicate,
            &fixture.input(&duplicate),
            &[fixture.request(&duplicate)],
            limits()
        ),
        Err(RelationshipStagingError::DuplicateDirection)
    ));
}

#[cfg(not(target_arch = "wasm32"))]
mod session {
    use super::*;
    use df_observe::OperationContext;
    use df_session::inbox::{ActorInput, AdmissionSequence, Reducer};
    use df_session::submission::*;
    use df_types::{OperationId, SessionId};
    use std::cell::RefCell;
    use std::rc::Rc;

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
    // Controlled durable-port fixture; physical PostgreSQL/fence qualification is separate.
    struct Database {
        checkpoint: Checkpoint,
        ledger: Vec<(Scope, DecisionReceipt)>,
        failure: Failure,
        commits: usize,
        decisions: usize,
        publications: usize,
        refuse_reload: bool,
        command_stages: usize,
        entry_checks: usize,
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
        fixture: Fixture,
        source: SourceOwner,
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
            let result = registered(&self.fixture, &self.source, current, input)
                .map_err(|_| RepositoryError::InvalidCandidate);
            let mut db = self.database.borrow_mut();
            db.command_stages = self.source.command_calls.get();
            db.entry_checks = self.source.entry_calls.get();
            result
        }
        fn validate_recovery(&mut self, checkpoint: &Checkpoint) -> Result<(), RepositoryError> {
            checkpoint
                .validate_resume(checkpoint.basis(), &self.fixture.pins)
                .map(|_| ())
                .map_err(|_| RepositoryError::InvalidCandidate)
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
            assert_eq!(
                checkpoint.state().relationships[0].state,
                model::label("receptive")
            );
            assert_eq!(
                checkpoint.state().relationships[1].state,
                model::label("independent-reverse")
            );
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
        setup_fixture(Fixture::new(), failure)
    }
    fn setup_fixture(fixture: Fixture, failure: Failure) -> (Owner, Rc<RefCell<Database>>, Scope) {
        let current = fixture.current();
        let scope = Scope {
            input: fixture.input(&current),
        };
        let source = SourceOwner::new(&fixture);
        let database = Rc::new(RefCell::new(Database {
            checkpoint: current.clone(),
            ledger: vec![],
            failure,
            commits: 0,
            decisions: 0,
            publications: 0,
            refuse_reload: false,
            command_stages: 0,
            entry_checks: 0,
        }));
        let owner = DurableOwner::new(
            Repository {
                database: Rc::clone(&database),
            },
            Engine {
                fixture,
                source,
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
                build: "relationship-fixture".to_owned(),
            },
            scope.clone(),
            scope.input.clone(),
        );
        owner.reduce(AdmissionSequence(1), input);
        wait.try_recv().unwrap()
    }

    #[test]
    fn admitted_registered_reaction_commits_only_after_ack_and_exact_retry_returns_original_receipt()
     {
        let (mut owner, database, scope) = setup(Failure::None);
        let original = owner.checkpoint().clone();
        let first = submit(&mut owner, &scope);
        assert!(matches!(first, SubmissionOutcome::Confirmed(_)));
        assert_eq!(submit(&mut owner, &scope), first);
        let db = database.borrow();
        assert_eq!(owner.checkpoint(), &db.checkpoint);
        assert_eq!(
            db.checkpoint.state().relationships[0].state,
            model::label("receptive")
        );
        assert_eq!(
            db.checkpoint.state().relationships[1],
            original.state().relationships[1]
        );
        assert_eq!(
            db.checkpoint.state().facts.len(),
            original.state().facts.len() + 1
        );
        assert_eq!(original.state().draws.len(), 1);
        assert_eq!(db.checkpoint.state().draws, original.state().draws);
        assert_eq!(db.checkpoint.state().decisions[0].draws, [0]);
        assert_eq!(
            (db.decisions, db.commits, db.publications, db.ledger.len()),
            (1, 1, 1, 1)
        );
        let mut conflict = scope.clone();
        let GameInput::Game(command) = &mut conflict.input else {
            unreachable!()
        };
        command.observed_revision = model::revision(2, 7);
        drop(db);
        assert_eq!(
            submit(&mut owner, &conflict),
            SubmissionOutcome::OperationConflict
        );
    }

    #[test]
    fn failed_commit_preserves_social_and_mechanical_state_and_safe_retry_applies_once() {
        let (mut owner, database, scope) = setup(Failure::BeforeCommit);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::Refused(RepositoryError::Unavailable)
        );
        assert_eq!(owner.checkpoint(), &original);
        assert_eq!(database.borrow().checkpoint, original);
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
        assert_eq!(
            db.checkpoint.state().relationships[0].state,
            model::label("receptive")
        );
    }

    #[test]
    fn lost_ack_and_failed_reload_fence_other_input_until_exact_receipt_and_validated_snapshot() {
        let (mut owner, database, scope) = setup(Failure::LostAck);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::LookupRequired
        );
        assert!(owner.has_uncertain_operation());
        assert_eq!(owner.checkpoint(), &original);
        assert_eq!(
            database.borrow().checkpoint.state().relationships[0].state,
            model::label("receptive")
        );
        let mut other = scope.clone();
        let GameInput::Game(command) = &mut other.input else {
            unreachable!()
        };
        command.operation = operation(51);
        assert_eq!(
            submit(&mut owner, &other),
            SubmissionOutcome::LookupRequired
        );
        database.borrow_mut().refuse_reload = true;
        assert!(matches!(
            submit(&mut owner, &scope),
            SubmissionOutcome::Confirmed(_)
        ));
        assert!(owner.has_uncertain_operation());
        assert_eq!(owner.checkpoint(), &original);
        database.borrow_mut().refuse_reload = false;
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
    }

    #[test]
    fn unknown_absent_commit_never_reexecutes_reaction_even_on_not_recorded_exact_lookup() {
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
        assert!(owner.has_uncertain_operation());
        let db = database.borrow();
        assert_eq!(owner.checkpoint(), &original);
        assert_eq!(db.checkpoint, original);
        assert_eq!(
            (db.decisions, db.commits, db.publications, db.ledger.len()),
            (1, 1, 0, 0)
        );
    }

    #[test]
    fn admitted_second_reaction_failure_never_commits_or_publishes_first_reaction_or_mechanical_siblings()
     {
        let mut fixture = Fixture::new();
        fixture.reaction_tail = true;
        let (mut owner, database, scope) = setup_fixture(fixture, Failure::None);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::Refused(RepositoryError::InvalidCandidate)
        );
        let db = database.borrow();
        // One policy construction, a successful first react, then second react's policy recheck.
        assert_eq!((db.command_stages, db.entry_checks), (1, 3));
        assert_eq!(
            (db.decisions, db.commits, db.publications, db.ledger.len()),
            (1, 0, 0, 0)
        );
        assert_eq!(owner.checkpoint(), &original);
        assert_eq!(db.checkpoint, original);
        assert_eq!(db.checkpoint.state().draws.len(), 1);
        assert_eq!(
            db.checkpoint.state().relationships[0].state,
            model::label("reserved")
        );
        assert_eq!(
            db.checkpoint.state().relationships[2].state,
            model::label("reserved")
        );
        assert!(!db.checkpoint.state().obligations[0].fulfilled);
    }
}

#[test]
fn last_reaction_source_and_constructor_failure_discard_all_tentative_mechanics_and_relationships()
{
    for failure in [
        "tail",
        "source",
        "constructor",
        "budget",
        "work",
        "proposal",
    ] {
        let mut fixture = Fixture::new();
        fixture.reaction_tail = failure == "tail";
        let owner = SourceOwner::new(&fixture);
        let current = fixture.current();
        let original = current.clone();
        let mut requests = vec![fixture.request(&current)];
        let mut bounds = limits();
        match failure {
            "tail" => {
                requests = fixture.requests(&current);
            }
            "source" => owner.withdraw_after_entry.set(Some(2)),
            "constructor" => bounds.checkpoint.maximum_records = 1,
            "budget" => bounds.maximum_pass_bytes = 1,
            "work" => bounds.maximum_work = 1,
            "proposal" => bounds.reaction.maximum_proposal_bytes = 1,
            _ => unreachable!(),
        }
        if failure == "tail" {
            let first = stage(
                &fixture,
                &owner,
                &current,
                &fixture.input(&current),
                &requests[..1],
                bounds,
            )
            .unwrap();
            assert!(matches!(&first.reactions[0], ReactionOutcome::Proposed(_)));
            assert_eq!(
                first.checkpoint.state().relationships[0].state,
                model::label("receptive")
            );
            assert_eq!(
                first.checkpoint.state().relationships[2],
                current.state().relationships[2]
            );
            assert_eq!(first.checkpoint.state().draws, current.state().draws);
            let input = fixture.input(&current);
            let sibling = stage_handler(
                &fixture.inner(&owner, &current),
                &fixture.pins,
                RulesCommandInput {
                    command: &input,
                    supplied_draws: &[],
                },
                &current,
                1024 * 1024,
            )
            .unwrap();
            let sibling_original = sibling.clone();
            let entries_before = owner.entry_calls.get();
            let commands_before = owner.command_calls.get();
            assert!(matches!(
                stage(&fixture, &owner, &current, &input, &requests, bounds),
                Err(RelationshipStagingError::Reaction(
                    ReactionError::UnknownEvent
                ))
            ));
            assert_eq!(owner.entry_calls.get() - entries_before, 3);
            assert_eq!(owner.command_calls.get() - commands_before, 1);
            assert_eq!(sibling, sibling_original);
            assert_eq!(sibling.state().relationships, current.state().relationships);
            assert_eq!(sibling.state().draws, current.state().draws);
        } else {
            assert!(
                stage(
                    &fixture,
                    &owner,
                    &current,
                    &fixture.input(&current),
                    &requests,
                    bounds
                )
                .is_err()
            );
        }
        assert_eq!(current, original);
    }
}

#[test]
fn stale_request_full_pins_and_unavailable_checkpoint_refuse_without_policy_access() {
    let fixture = Fixture::new();
    let owner = SourceOwner::new(&fixture);
    let current = fixture.current();
    let mut request = fixture.request(&current);
    request.expected_basis.revision = model::revision(2, 7);
    assert!(matches!(
        stage(
            &fixture,
            &owner,
            &current,
            &fixture.input(&current),
            &[request],
            limits()
        ),
        Err(RelationshipStagingError::StaleCommand)
    ));
    let mut state = fixture.state();
    state
        .continuity
        .recovery
        .unavailable_sources
        .push(model::label("withdrawn-source"));
    let unavailable = fixture.checkpoint(state);
    assert!(matches!(
        stage(
            &fixture,
            &owner,
            &unavailable,
            &fixture.input(&unavailable),
            &[fixture.request(&unavailable)],
            limits()
        ),
        Err(RelationshipStagingError::Snapshot(
            CheckpointError::UnavailableCheckpoint
        ))
    ));
    let mut other_pins = fixture.pins.clone();
    other_pins.content.content_digest = ContentDigest([99; 32]);
    let changed = Checkpoint::new(
        CHECKPOINT_SCHEMA,
        current.basis(),
        other_pins,
        current.state().clone(),
        fixture.inventory(),
        model::limits(),
    )
    .unwrap();
    assert!(matches!(
        stage(
            &fixture,
            &owner,
            &changed,
            &fixture.input(&changed),
            &[fixture.request(&changed)],
            limits()
        ),
        Err(RelationshipStagingError::Snapshot(
            CheckpointError::ContentMismatch
        ))
    ));
    assert_eq!(owner.entry_calls.get(), 0);
}

#[derive(Clone, Copy)]
enum SiblingChange {
    Relationship,
    Knowledge,
}
struct ChangedSibling<'a> {
    fixture: &'a Fixture,
    owner: &'a SourceOwner,
    change: SiblingChange,
}
impl RulesCommandHandler for ChangedSibling<'_> {
    type Rejection = FulfillmentError<SourceRefusal>;
    fn pins(&self) -> &CheckpointPins {
        &self.fixture.pins
    }
    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        let base = self
            .fixture
            .inner(self.owner, current)
            .stage(input, current)?;
        let mut state = base.state().clone();
        match self.change {
            SiblingChange::Relationship => {
                state.relationships[0].state = model::label("conflicting")
            }
            SiblingChange::Knowledge => state.continuity.npcs[0].known_facts.push(fact(30)),
        }
        Ok(Checkpoint::new(
            base.schema(),
            base.basis(),
            base.pins().clone(),
            state,
            self.fixture.inventory(),
            model::limits(),
        )
        .unwrap())
    }
}

#[test]
fn changed_sibling_relationship_refuses_and_newly_staged_knowledge_cannot_authorize_reaction() {
    let fixture = Fixture::new();
    let owner = SourceOwner::new(&fixture);
    for change in [SiblingChange::Relationship, SiblingChange::Knowledge] {
        let mut state = fixture.state();
        let knowledge = matches!(change, SiblingChange::Knowledge);
        if knowledge {
            state.continuity.npcs[0].known_facts.clear();
        }
        let current = fixture.checkpoint(state);
        let original = current.clone();
        let sibling = ChangedSibling {
            fixture: &fixture,
            owner: &owner,
            change,
        };
        let requests = [fixture.request(&current)];
        let handler = RelationshipReactionHandler {
            command_handler: &sibling,
            owner: &owner,
            current_basis: current.basis(),
            admitted_pins: &fixture.pins,
            inventory: fixture.inventory(),
            requests: &requests,
            entries: &fixture.entries,
            limits: limits(),
        };
        let input = fixture.input(&current);
        let result = handler.stage_with_outcomes(
            RulesCommandInput {
                command: &input,
                supplied_draws: &[],
            },
            &current,
        );
        if knowledge {
            let result = result.unwrap();
            assert_eq!(
                result.reactions,
                [ReactionOutcome::NoReaction(NoReactionReason::NotKnown)]
            );
            assert_eq!(
                result.checkpoint.state().relationships,
                current.state().relationships
            );
            assert_eq!(
                result.checkpoint.state().continuity.npcs[0].known_facts,
                [fact(30)]
            );
        } else {
            assert!(matches!(
                result,
                Err(RelationshipStagingError::RelationshipConflict)
            ));
        }
        assert_eq!(current, original);
    }
}

#[test]
fn exact_event_acceptance_and_prior_cause_acceptance_cannot_be_replaced_by_fact_presence() {
    for missing in ["event", "cause"] {
        let fixture = Fixture::new();
        let owner = SourceOwner::new(&fixture);
        let mut state = fixture.state();
        let mut event = state.facts[0].clone();
        event.id = fact(32);
        event.ordinal = 2;
        event.cause = Some(fact(30));
        if missing == "cause" {
            let mut cause = state.facts[0].clone();
            cause.id = fact(34);
            cause.operation = operation(33);
            cause.ordinal = 0;
            state.facts.push(cause);
            event.cause = Some(fact(34));
            state.decisions[0].facts.push(fact(32));
        }
        state.facts.push(event);
        state.continuity.npcs[0].known_facts.push(fact(32));
        state.continuity.witnesses[0].fact = fact(32);
        let current = fixture.checkpoint(state);
        let original = current.clone();
        let mut request = fixture.request(&current);
        request.event = fact(32);
        assert!(matches!(
            stage(
                &fixture,
                &owner,
                &current,
                &fixture.input(&current),
                &[request],
                limits()
            ),
            Err(RelationshipStagingError::Reaction(
                ReactionError::UnacceptedEvent
            ))
        ));
        assert_eq!(current, original);
    }
}
