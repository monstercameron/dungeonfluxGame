//! Executable boundaries for partial progress over the existing bounded plan admission API.
//!
//! These tests exercise immutable inputs and canonical checkpoints. They do not implement or
//! claim session commits, cancellation receipts, persisted plans, or production integration.

#[path = "../src/partial_progress.rs"]
mod partial_progress;

use df_intent::plan::PlanError;
use df_model::checkpoint::{Basis, GameInput};
use partial_progress::{
    checkpoint_after_one_accepted_step, checkpoint_after_two_accepted_steps, operation, response,
    validate, validate_with_owner,
};

#[test]
fn abandoning_a_proposal_keeps_prior_accepted_facts_and_decisions_unchanged() {
    let current = checkpoint_after_one_accepted_step();
    let before = current.clone();
    let steps = [
        response(7, "fixture-option-1"),
        response(8, "fixture-option-1"),
    ];
    let steps_before = steps.clone();

    {
        let admission = validate(&steps, &current, 7).expect("first step is currently offered");
        assert_eq!(admission.first().operation, operation(7));
        assert_eq!(admission.requires_revalidation(), &steps[1..]);
    }

    assert_eq!(current, before);
    assert_eq!(steps, steps_before);
    assert_eq!(current.state().decisions.len(), 1);
    assert_eq!(current.state().decisions[0].operation, operation(6));
    assert_eq!(current.state().facts.len(), 1);
    assert_eq!(current.state().facts[0].operation, operation(6));
}

#[test]
fn the_unaccepted_tail_is_readmitted_against_the_checkpoint_after_progress() {
    let initial = checkpoint_after_one_accepted_step();
    let steps = [
        response(7, "fixture-option-1"),
        response(8, "fixture-option-1"),
    ];
    let admission = validate(&steps, &initial, 7).expect("first step is currently offered");
    let tail = admission.requires_revalidation();

    // This canonical checkpoint models the session owner having committed step 7.
    let later = checkpoint_after_two_accepted_steps();
    let before = later.clone();
    let resumed = validate(tail, &later, 8).expect("tail remains offered at the new basis");

    assert_eq!(resumed.first().operation, operation(8));
    assert!(resumed.requires_revalidation().is_empty());
    assert_eq!(resumed.basis(), later.basis());
    assert_eq!(later, before);
    assert_eq!(later.state().decisions.len(), 2);
    assert_eq!(later.state().decisions[0].operation, operation(6));
    assert_eq!(later.state().decisions[1].operation, operation(7));
    assert_eq!(later.state().facts.len(), 2);
    assert_eq!(later.state().facts[0].operation, operation(6));
    assert_eq!(later.state().facts[1].operation, operation(7));
}

#[test]
fn a_previously_accepted_operation_requires_receipt_lookup_before_admission() {
    let current = checkpoint_after_one_accepted_step();
    let before = current.clone();
    let replay = [response(6, "fixture-option-1")];

    assert_eq!(
        validate(&replay, &current, 6).unwrap_err(),
        PlanError::LookupRequired {
            operation: operation(6)
        }
    );
    assert_eq!(current, before);
}

#[test]
fn a_stale_native_basis_refuses_without_changing_accepted_progress() {
    let current = checkpoint_after_one_accepted_step();
    let before = current.clone();
    let steps = [response(7, "fixture-option-1")];
    let stale = Basis {
        revision: current
            .basis()
            .revision
            .next_sequence()
            .expect("fixture revision advances"),
        ..current.basis()
    };
    let mut supplied_owner = partial_progress::owner(&current, 7);
    supplied_owner.basis = stale;

    assert!(matches!(
        validate_with_owner(&steps, &current, supplied_owner),
        Err(PlanError::Candidate(_))
    ));
    assert_eq!(current, before);
    assert_eq!(current.state().decisions[0].operation, operation(6));
    assert_eq!(current.state().facts[0].operation, operation(6));
}

#[test]
fn tail_requests_are_borrowed_exactly_and_are_not_admitted_early() {
    let current = checkpoint_after_one_accepted_step();
    let steps = [
        response(7, "fixture-option-1"),
        response(8, "fixture-option-1"),
    ];
    let before: Vec<GameInput> = steps.clone().into();
    let admission = validate(&steps, &current, 7).expect("first step is currently offered");

    assert!(std::ptr::eq(
        admission.requires_revalidation().as_ptr(),
        steps[1..].as_ptr()
    ));
    assert_eq!(admission.requires_revalidation(), &steps[1..]);
    assert_eq!(steps.to_vec(), before);
}
