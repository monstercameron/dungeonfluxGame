#[path = "support/fixture_model.rs"]
pub mod fixture_model;
#[path = "support/witness_staging.rs"]
mod support;

use df_engine::command_entry::CommandRejection;
use df_engine::witness_staging::*;
use df_knowledge::witness::{WitnessEligibility, WitnessError, WitnessRoute};
use df_model::checkpoint::*;
use df_rules::{InvocationError, RulesCommandHandler, RulesCommandInput};
use fixture_model as fixture;
use support::*;

fn stage(
    handler: &WitnessGrantHandler<'_, SourceOwner>,
    current: &Checkpoint,
    input: &GameInput,
) -> Result<Checkpoint, WitnessStagingError<SourceRefusal>> {
    handler.stage(
        RulesCommandInput {
            command: input,
            supplied_draws: &[],
        },
        current,
    )
}

#[test]
fn registered_witness_batch_preserves_private_causes_and_every_sibling_record() {
    let fixture = Fixture::new();
    let current = fixture.current();
    let original = current.clone();
    let source = SourceOwner::new(&fixture, &current);
    let next = registered(&fixture, &source, &current, &fixture.input(current.basis())).unwrap();
    assert_preserved(&current, &next);
    assert_eq!(current, original);
    assert_eq!(source.calls.get(), 1);
    assert_eq!(
        next.basis().revision,
        current.basis().revision.next_sequence().unwrap()
    );
    let decision = next.state().decisions.last().unwrap();
    assert_eq!(decision.operation, operation(50));
    assert_eq!(decision.source_policy, fixture.registration.decision_policy);
    assert!(decision.facts.is_empty() && decision.draws.is_empty() && decision.effects.is_empty());
}

#[test]
fn existing_grants_and_repeated_witnesses_are_deduplicated_after_current_admission() {
    let fixture = Fixture::new();
    let mut state = fixture.state();
    state.knowledge = expected_grants();
    let current = fixture.checkpoint(fixture::basis(), state);
    let source = SourceOwner::new(&fixture, &current);
    let next = registered(&fixture, &source, &current, &fixture.input(current.basis())).unwrap();
    assert_preserved(&current, &next);
    assert_eq!(source.calls.get(), 1);
    source.refusal.set(Some(SourceRefusal::Withdrawn));
    assert!(registered(&fixture, &source, &current, &fixture.input(current.basis())).is_err());
    assert_eq!(current.state().knowledge, expected_grants());
}

#[test]
fn source_withdrawal_and_changed_registration_refuse_before_proposal() {
    let fixture = Fixture::new();
    let current = fixture.current();
    let source = SourceOwner::new(&fixture, &current);
    let eligible = fixture.eligible();
    let input = fixture.input(current.basis());
    source.refusal.set(Some(SourceRefusal::Withdrawn));
    assert_eq!(
        registered(&fixture, &source, &current, &input),
        Err(CommandRejection::Invocation(InvocationError::Handler(
            WitnessStagingError::Source(SourceRefusal::Withdrawn)
        )))
    );
    source.refusal.set(None);
    {
        let mut registration = fixture.registration.clone();
        registration.decision_policy = fixture::label("unadmitted-policy");
        let mut handler = fixture.handler(&source, current.basis(), Some(&eligible));
        handler.registration = &registration;
        assert_eq!(
            stage(&handler, &current, &input),
            Err(WitnessStagingError::Source(SourceRefusal::Binding))
        );
    }
    {
        let mut registration = fixture.registration.clone();
        registration.witness_policy = content("not-in-inventory");
        let mut handler = fixture.handler(&source, current.basis(), Some(&eligible));
        handler.registration = &registration;
        assert_eq!(
            stage(&handler, &current, &input),
            Err(WitnessStagingError::InvalidRegistration)
        );
    }
    assert_eq!(current, fixture.current());
}

#[test]
fn host_foreign_private_and_missing_accepted_cause_fail_the_entire_tail() {
    let fixture = Fixture::new();
    for case in ["host", "foreign", "cause", "time", "units"] {
        let mut state = fixture.state();
        match case {
            "cause" => {
                state.decisions.remove(1);
            }
            "time" => state.continuity.witnesses[1].perceived_at.ticks += 1,
            "units" => state.continuity.witnesses[1].perceived_at.ticks_per_second += 1,
            _ => {}
        }
        let current = fixture.checkpoint(fixture::basis(), state);
        let source = SourceOwner::new(&fixture, &current);
        let mut eligible = fixture.eligible();
        if case == "host" {
            eligible[1].witness = record(42);
        }
        if case == "foreign" {
            eligible[1].witness = record(43);
        }
        let error = match case {
            "cause" => WitnessError::MissingAcceptedCause,
            "time" | "units" => WitnessError::InvalidWitnessTime,
            _ => WitnessError::AudienceDenied,
        };
        assert_eq!(
            stage(
                &fixture.handler(&source, current.basis(), Some(&eligible)),
                &current,
                &fixture.input(current.basis())
            ),
            Err(WitnessStagingError::Witness(error))
        );
        assert!(current.state().knowledge.is_empty());
        assert_eq!(
            current.state().decisions.len(),
            if case == "cause" { 3 } else { 4 }
        );
    }
}

#[test]
fn relationship_needs_current_direction_policy_state_and_independent_route_admission() {
    let fixture = Fixture::new();
    for case in ["missing", "direction", "policy", "state"] {
        let mut state = fixture.state();
        match case {
            "missing" => state.relationships.clear(),
            "direction" => {
                state.relationships[0].subject = fixture::entity(6);
                state.relationships[0].object = fixture::entity(4);
            }
            "policy" => state.relationships[0].policy = content("other-witness-policy"),
            _ => state.relationships[0].state = fixture::label("no-contact"),
        }
        let current = fixture.checkpoint(fixture::basis(), state);
        let source = SourceOwner::new(&fixture, &current);
        assert_eq!(
            registered(&fixture, &source, &current, &fixture.input(current.basis())),
            Err(CommandRejection::Invocation(InvocationError::Handler(
                WitnessStagingError::Witness(WitnessError::IneligibleRoute)
            )))
        );
        assert!(current.state().knowledge.is_empty());
    }
    let current = fixture.current();
    let source = SourceOwner::new(&fixture, &current);
    let unsupported = fixture::label("invented-contact-threshold");
    let mut eligible = fixture.eligible();
    eligible[3].route = WitnessRoute::Relationship {
        permitted_state: &unsupported,
    };
    assert_eq!(
        stage(
            &fixture.handler(&source, current.basis(), Some(&eligible)),
            &current,
            &fixture.input(current.basis())
        ),
        Err(WitnessStagingError::Source(SourceRefusal::Eligibility))
    );
}

#[test]
fn captured_source_eligibility_rechecks_member_character_and_exact_witness_snapshot() {
    let fixture = Fixture::new();
    let prepared = fixture.current();
    let source = SourceOwner::new(&fixture, &prepared);
    for case in ["member", "revoked", "witness", "source"] {
        let mut state = prepared.state().clone();
        match case {
            "member" => state.members[1].character = None,
            "revoked" => {
                state.members.remove(1);
                state.facts[3].audience = AudienceScope::Shared;
            }
            "witness" => state.continuity.witnesses[1].perceived_at.ticks -= 1,
            _ => state.continuity.witnesses[1].source = content("other-witness-policy"),
        }
        let current = fixture.checkpoint(prepared.basis(), state);
        assert_eq!(
            registered(&fixture, &source, &current, &fixture.input(current.basis())),
            Err(CommandRejection::Invocation(InvocationError::Handler(
                WitnessStagingError::Source(SourceRefusal::Eligibility)
            )))
        );
        assert!(current.state().knowledge.is_empty());
    }
}

#[test]
fn native_route_mismatch_is_refused_even_for_an_existing_or_duplicate_grant() {
    let fixture = Fixture::new();
    let mut state = fixture.state();
    state.knowledge = expected_grants();
    let current = fixture.checkpoint(fixture::basis(), state);
    let source = SourceOwner::new(&fixture, &current);
    let mut eligible = fixture.eligible();
    eligible[3].route = WitnessRoute::Hearing;
    assert_eq!(
        stage(
            &fixture.handler(&source, current.basis(), Some(&eligible)),
            &current,
            &fixture.input(current.basis())
        ),
        Err(WitnessStagingError::Source(SourceRefusal::Eligibility))
    );
    assert_eq!(current.state().knowledge, expected_grants());
}

#[test]
fn global_fact_belief_memory_and_summary_do_not_replace_a_native_witness_producer() {
    let fixture = Fixture::new();
    let mut state = fixture.state();
    state.beliefs.push(AttributedClaim {
        id: record(80),
        holder: fixture::entity(4),
        subject: fixture::entity(6),
        claim: "Everyone saw the private event".to_owned(),
        evidence: vec![fact(32)],
        audience: AudienceScope::Host,
        source: fixture.registration.witness_policy.clone(),
    });
    state.memories.push(MemoryEpisode {
        id: record(81),
        holder: fixture::entity(4),
        source_facts: vec![fact(32)],
        retained_text: "Shared recollection".to_owned(),
        audience: AudienceScope::Host,
        source_revision: fixture::basis().revision,
    });
    state.continuity.summaries.push(MemorySummary {
        id: record(82),
        episodes: vec![record(81)],
        derived_claims: vec![record(80)],
        source_digest: fixture.pins.content.content_digest,
        source_revision: fixture::basis().revision,
        summarizer: fixture::label("fixture-summary"),
        model: fixture::label("fixture-model"),
        policy: fixture.registration.witness_policy.clone(),
        audience: AudienceScope::Shared,
        text: "Everyone knows the secret".to_owned(),
        incomplete: false,
    });
    let current = fixture.checkpoint(fixture::basis(), state);
    let source = SourceOwner::new(&fixture, &current);
    let input = fixture.input(current.basis());
    assert_eq!(
        stage(
            &fixture.handler(&source, current.basis(), None),
            &current,
            &input
        ),
        Err(WitnessStagingError::MissingProducer)
    );
    let next = stage(
        &fixture.handler(&source, current.basis(), Some(&[])),
        &current,
        &input,
    )
    .unwrap();
    let mut expected = current.state().clone();
    expected
        .decisions
        .push(next.state().decisions.last().unwrap().clone());
    assert_eq!(next.state(), &expected);
    assert!(next.state().knowledge.is_empty());
    let unknown = [WitnessEligibility {
        witness: record(99),
        recipient: fixture::member(3),
        route: WitnessRoute::Sight,
    }];
    assert!(
        stage(
            &fixture.handler(&source, current.basis(), Some(&unknown)),
            &current,
            &input
        )
        .is_err()
    );
}

#[test]
fn current_basis_pins_and_unavailable_source_refuse_before_source_work() {
    let fixture = Fixture::new();
    let current = fixture.current();
    let source = SourceOwner::new(&fixture, &current);
    let eligible = fixture.eligible();
    let input = fixture.input(current.basis());
    let mut handler = fixture.handler(&source, current.basis(), Some(&eligible));
    handler.current_basis.revision = fixture::revision(3, 8);
    assert_eq!(
        stage(&handler, &current, &input),
        Err(WitnessStagingError::Snapshot(CheckpointError::StaleBasis))
    );
    handler.current_basis = current.basis();
    let mut pins = fixture.pins.clone();
    pins.rules.handler = fixture::label("unadmitted-handler");
    handler.admitted_pins = &pins;
    assert_eq!(
        stage(&handler, &current, &input),
        Err(WitnessStagingError::Snapshot(
            CheckpointError::RulesMismatch
        ))
    );
    let mut state = fixture.state();
    state
        .continuity
        .recovery
        .unavailable_sources
        .push(fixture::label("withdrawn-source"));
    let unavailable = fixture.checkpoint(current.basis(), state);
    let handler = fixture.handler(&source, current.basis(), Some(&eligible));
    assert_eq!(
        stage(&handler, &unavailable, &input),
        Err(WitnessStagingError::Snapshot(
            CheckpointError::UnavailableCheckpoint
        ))
    );
    assert_eq!(source.calls.get(), 0);
}

#[test]
fn work_output_candidate_and_owned_pass_bounds_never_escape_a_partial_batch() {
    let fixture = Fixture::new();
    let current = fixture.current();
    let source = SourceOwner::new(&fixture, &current);
    let eligible = fixture.eligible();
    let input = fixture.input(current.basis());
    for case in [
        "scan",
        "comparison",
        "grants",
        "grant-bytes",
        "candidate",
        "pass",
        "inventory",
    ] {
        let mut handler = fixture.handler(&source, current.basis(), Some(&eligible));
        let error = match case {
            "scan" => {
                handler.limits.witness.maximum_scan_records = 1;
                WitnessStagingError::Witness(WitnessError::ScanCapacity)
            }
            "comparison" => {
                handler.limits.witness.maximum_record_comparisons = 1;
                WitnessStagingError::Witness(WitnessError::ComparisonCapacity)
            }
            "grants" => {
                handler.limits.witness.maximum_grants = 1;
                WitnessStagingError::Witness(WitnessError::GrantCapacity)
            }
            "grant-bytes" => {
                handler.limits.witness.maximum_grant_bytes = 1;
                WitnessStagingError::Witness(WitnessError::GrantByteCapacity)
            }
            "candidate" => {
                handler.limits.witness.maximum_candidates = 1;
                WitnessStagingError::Capacity
            }
            "pass" => {
                handler.limits.maximum_pass_bytes = 1;
                WitnessStagingError::Capacity
            }
            _ => {
                handler.limits.maximum_inventory_records = 1;
                WitnessStagingError::Capacity
            }
        };
        assert_eq!(stage(&handler, &current, &input), Err(error));
        assert_eq!(current, fixture.current());
    }
    let mut handler = fixture.handler(&source, current.basis(), Some(&eligible));
    handler.limits.checkpoint.maximum_records = 1;
    assert_eq!(
        stage(&handler, &current, &input),
        Err(WitnessStagingError::InvalidCandidate(
            CheckpointError::Capacity
        ))
    );
    assert!(current.state().knowledge.is_empty());
}

#[test]
fn changed_client_action_actor_and_stale_observation_do_not_admit_native_witnesses() {
    let fixture = Fixture::new();
    let current = fixture.current();
    let source = SourceOwner::new(&fixture, &current);
    let eligible = fixture.eligible();
    let handler = fixture.handler(&source, current.basis(), Some(&eligible));
    for case in ["actor", "action", "choices", "observed"] {
        let mut input = fixture.input(current.basis());
        let GameInput::Game(command) = &mut input else {
            unreachable!()
        };
        if case == "observed" {
            command.observed_revision = fixture::revision(2, 7);
        }
        let GameCommand::ProposeAction {
            actor,
            action,
            choices,
            ..
        } = &mut command.command
        else {
            unreachable!()
        };
        match case {
            "actor" => *actor = fixture::entity(6),
            "action" => *action = fixture::content(),
            "choices" => choices.push((
                fixture::label("observation-offer"),
                fixture::label("client-says-i-saw-it"),
            )),
            _ => {}
        }
        let error = match case {
            "choices" => WitnessStagingError::Source(SourceRefusal::ActorControl),
            "observed" => WitnessStagingError::StaleCommand,
            _ => WitnessStagingError::WrongCommand,
        };
        assert_eq!(stage(&handler, &current, &input), Err(error));
        assert!(current.state().knowledge.is_empty());
    }
}

#[test]
fn pending_rules_draws_and_reaccepted_operations_cannot_apply_a_witness_pass() {
    let fixture = Fixture::new();
    let current = fixture.current();
    let source = SourceOwner::new(&fixture, &current);
    let eligible = fixture.eligible();
    let handler = fixture.handler(&source, current.basis(), Some(&eligible));
    let input = fixture.input(current.basis());
    let resolution = ResolutionId::from_bytes(&[9; 16]).unwrap();
    let window = WindowId::from_bytes(&[10; 16]).unwrap();
    let draw = ActualDraw {
        operation: operation(50),
        ordinal: 0,
        resolution,
        window,
        sides: 20,
        value: 1,
        source: fixture::rule(),
    };
    assert_eq!(
        handler.stage(
            RulesCommandInput {
                command: &input,
                supplied_draws: &[draw]
            },
            &current
        ),
        Err(WitnessStagingError::UnsupportedInput)
    );
    let mut state = fixture.state();
    state.pending.push(PendingResolution {
        id: resolution,
        basis: current.basis(),
        continuation: fixture::label("existing-source-continuation"),
        window: ResolutionWindow {
            id: window,
            phase: TriggerPhase::BeforeDraw,
            causal_fact: fact(31),
            source: fixture::rule(),
            timer: None,
        },
        next: PendingInput::Choice {
            remaining: vec![OfferedResponse {
                participant: fixture::member(3),
                offer: fixture::label("existing-offer"),
                options: vec![fixture::label("existing-choice")],
                source: fixture::rule(),
            }],
        },
        choices: vec![],
        draw_ordinals: vec![],
        spent: vec![],
        rulings: vec![],
    });
    let pending = fixture.checkpoint(current.basis(), state);
    assert_eq!(
        stage(&handler, &pending, &input),
        Err(WitnessStagingError::PendingResolution)
    );
    assert_eq!(source.calls.get(), 0);
    assert_eq!(pending.state().pending.len(), 1);
    let next = registered(&fixture, &source, &current, &input).unwrap();
    let fresh_source = SourceOwner::new(&fixture, &next);
    let next_input = fixture.input(next.basis());
    assert_eq!(
        stage(
            &fixture.handler(&fresh_source, next.basis(), Some(&eligible)),
            &next,
            &next_input
        ),
        Err(WitnessStagingError::AlreadyAccepted)
    );
    assert_eq!(fresh_source.calls.get(), 0);
}

#[test]
fn exhausted_revision_refuses_without_mutating_original_witness_state() {
    let fixture = Fixture::new();
    let mut basis = fixture::basis();
    basis.revision = fixture::revision(2, u64::MAX);
    let current = fixture.checkpoint(basis, fixture.state());
    let source = SourceOwner::new(&fixture, &current);
    let eligible = fixture.eligible();
    assert_eq!(
        stage(
            &fixture.handler(&source, basis, Some(&eligible)),
            &current,
            &fixture.input(basis)
        ),
        Err(WitnessStagingError::RevisionExhausted)
    );
    assert!(current.state().knowledge.is_empty());
}

#[cfg(not(target_arch = "wasm32"))]
mod durable_session {
    use super::*;
    use df_observe::OperationContext;
    use df_session::submission::{RepositoryError, SubmissionOutcome};
    use support::durable::*;

    #[test]
    fn committed_witness_batch_acknowledges_then_publishes_once_and_retry_returns_same_receipt() {
        let (mut owner, db, scope) = setup(Failure::None);
        let original = owner.checkpoint().clone();
        let receipt = submit(&mut owner, &scope);
        assert!(matches!(receipt, SubmissionOutcome::Confirmed(_)));
        assert_eq!(submit(&mut owner, &scope), receipt);
        let db = db.borrow();
        assert_eq!(owner.checkpoint(), &db.checkpoint);
        assert_preserved(&original, &db.checkpoint);
        assert_eq!(
            (
                db.decisions,
                db.commits,
                db.publications,
                db.wakes,
                db.ledger.len()
            ),
            (1, 1, 1, 1, 1)
        );
    }

    #[test]
    fn rollback_keeps_every_record_and_known_safe_retry_commits_one_complete_witness_batch() {
        let (mut owner, db, scope) = setup(Failure::BeforeCommit);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::Refused(RepositoryError::Unavailable)
        );
        assert_eq!(owner.checkpoint(), &original);
        assert_eq!(db.borrow().checkpoint, original);
        assert!(db.borrow().ledger.is_empty());
        assert_eq!((db.borrow().publications, db.borrow().wakes), (0, 0));
        db.borrow_mut().failure = Failure::None;
        assert!(matches!(
            submit(&mut owner, &scope),
            SubmissionOutcome::Confirmed(_)
        ));
        assert_preserved(&original, &db.borrow().checkpoint);
        assert_eq!(
            (
                db.borrow().decisions,
                db.borrow().commits,
                db.borrow().ledger.len()
            ),
            (2, 2, 1)
        );
    }

    #[test]
    fn unknown_absent_commit_blocks_reexecution_other_operations_and_explicit_reload() {
        let (mut owner, db, scope) = setup(Failure::UnknownNotCommitted);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::LookupRequired
        );
        db.borrow_mut().failure = Failure::None;
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::LookupRequired
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
        assert_eq!(
            owner.reload_current(
                &scope,
                &OperationContext {
                    trace_parent: String::new(),
                    build: "witness-fixture".to_owned()
                }
            ),
            Err(RepositoryError::UnresolvedCommit)
        );
        assert!(owner.has_uncertain_operation());
        let db = db.borrow();
        assert_eq!(owner.checkpoint(), &original);
        assert_eq!(db.checkpoint, original);
        assert_eq!(
            (
                db.decisions,
                db.commits,
                db.publications,
                db.wakes,
                db.reloads
            ),
            (1, 1, 0, 0, 0)
        );
    }

    #[test]
    fn lost_ack_reloads_exact_committed_witnesses_without_restage_or_premature_publication() {
        let (mut owner, db, scope) = setup(Failure::LostAck);
        let original = owner.checkpoint().clone();
        assert_eq!(
            submit(&mut owner, &scope),
            SubmissionOutcome::LookupRequired
        );
        assert_eq!(owner.checkpoint(), &original);
        assert_preserved(&original, &db.borrow().checkpoint);
        assert_eq!((db.borrow().publications, db.borrow().wakes), (0, 0));
        let receipt = submit(&mut owner, &scope);
        assert!(matches!(receipt, SubmissionOutcome::Confirmed(_)));
        assert!(owner.is_current());
        assert_eq!(submit(&mut owner, &scope), receipt);
        let db = db.borrow();
        assert_eq!(owner.checkpoint(), &db.checkpoint);
        assert_eq!(
            (
                db.decisions,
                db.commits,
                db.publications,
                db.wakes,
                db.reloads
            ),
            (1, 1, 0, 0, 1)
        );
    }

    #[test]
    fn failed_publication_preserves_commit_and_same_operation_cannot_run_witnesses_again() {
        let (mut owner, db, scope) = setup(Failure::Publication);
        let original = owner.checkpoint().clone();
        let receipt = submit(&mut owner, &scope);
        assert!(matches!(receipt, SubmissionOutcome::Confirmed(_)));
        assert_eq!(submit(&mut owner, &scope), receipt);
        assert_preserved(&original, &db.borrow().checkpoint);
        assert_eq!(owner.checkpoint(), &db.borrow().checkpoint);
        assert_eq!(
            (
                db.borrow().decisions,
                db.borrow().commits,
                db.borrow().publications,
                db.borrow().ledger.len()
            ),
            (1, 1, 1, 1)
        );
    }

    #[test]
    fn failed_tail_and_source_refusal_never_commit_publish_or_wake_any_grant() {
        for failure in [Failure::FailedTail, Failure::SourceWithdrawn] {
            let (mut owner, db, scope) = setup(failure);
            let original = owner.checkpoint().clone();
            assert_eq!(
                submit(&mut owner, &scope),
                SubmissionOutcome::Refused(RepositoryError::InvalidCandidate)
            );
            let db = db.borrow();
            assert_eq!(owner.checkpoint(), &original);
            assert_eq!(db.checkpoint, original);
            assert_eq!(
                (
                    db.decisions,
                    db.commits,
                    db.publications,
                    db.wakes,
                    db.ledger.len()
                ),
                (1, 0, 0, 0, 0)
            );
        }
    }

    #[test]
    fn principal_binding_and_conflicting_operation_input_refuse_without_a_second_stage() {
        let (mut owner, db, scope) = setup(Failure::None);
        let mut forged = scope.clone();
        forged.principal = fixture::member(5);
        assert_eq!(
            submit(&mut owner, &forged),
            SubmissionOutcome::Refused(RepositoryError::Unauthorized)
        );
        assert_eq!((db.borrow().decisions, db.borrow().commits), (0, 0));
        assert!(matches!(
            submit(&mut owner, &scope),
            SubmissionOutcome::Confirmed(_)
        ));
        let mut conflict = scope.clone();
        let GameInput::Game(command) = &mut conflict.input else {
            unreachable!()
        };
        command.command = GameCommand::Speak {
            speaker: fixture::entity(4),
            text: "different exact operation input".to_owned(),
            conversation: None,
        };
        assert_eq!(
            submit(&mut owner, &conflict),
            SubmissionOutcome::OperationConflict
        );
        assert_eq!(
            (
                db.borrow().decisions,
                db.borrow().commits,
                db.borrow().ledger.len()
            ),
            (1, 1, 1)
        );
    }
}
