#[path = "support/candidate_fixture.rs"]
mod candidate_fixture;
#[path = "../src/text_question_contract.rs"]
mod text_question_contract;

use candidate_fixture::*;
use df_intent::dialogue::{DialogueError, DialogueLimits};
use df_model::checkpoint::{Checkpoint, CheckpointError};
use df_model::intent::{DiscourseContext, InputFinality, IntentDisposition, RawInputRef};
use df_types::{BuildIdentity, OperationId, RunId, SessionId};
use text_question_contract::{TextQuestionRequest, classify_text_request};

fn input(current: &Checkpoint, finality: InputFinality) -> RawInputRef {
    RawInputRef {
        input: OperationId::from_bytes(&[30; 16]).unwrap(),
        member: member(3),
        basis: current.basis(),
        pins: current.pins().clone(),
        finality,
    }
}

fn current() -> Checkpoint {
    let mut initial = state();
    initial.facts.push(fact(7, 0));
    initial.pending.push(pending());
    checkpoint(initial).unwrap()
}

fn classify(
    current: &Checkpoint,
    input: &RawInputRef,
    text: &str,
    context: DiscourseContext,
    maximum_text_bytes: usize,
) -> Result<IntentDisposition, DialogueError> {
    classify_text_request(
        TextQuestionRequest::Dialogue {
            text,
            context,
            limits: DialogueLimits { maximum_text_bytes },
        },
        input,
        current,
        member(3),
    )
}

#[test]
fn declared_joke_and_inquiry_never_spend_resources_even_with_action_wording() {
    let current = current();
    let before = current.clone();
    for (context, text, expected) in [
        (
            DiscourseContext::Question,
            "Can I attack fixture-option-1 and spend a spell slot?",
            IntentDisposition::Question,
        ),
        (
            DiscourseContext::Joke,
            "I cast every spell at the moon. Just kidding.",
            IntentDisposition::Joke,
        ),
        (
            DiscourseContext::Question,
            "What would happen if I used all my arrows?",
            IntentDisposition::Question,
        ),
        (
            DiscourseContext::Joke,
            "Attack now! That was a joke.",
            IntentDisposition::Joke,
        ),
    ] {
        for finality in [InputFinality::Partial, InputFinality::Final] {
            let input = input(&current, finality);
            assert_eq!(classify(&current, &input, text, context, 256), Ok(expected));
            assert_eq!(current.state().resources, before.state().resources);
            assert_eq!(current.state().pending, before.state().pending);
            assert_eq!(current, before);
        }
    }
}

#[test]
fn all_declared_non_action_contexts_and_ambiguous_words_stay_non_action() {
    let current = current();
    let before = current.clone();
    for (context, expected) in [
        (DiscourseContext::Social, IntentDisposition::Social),
        (DiscourseContext::PlanOnly, IntentDisposition::PlanOnly),
        (DiscourseContext::Meta, IntentDisposition::Meta),
        (DiscourseContext::Unspecified, IntentDisposition::Clarify),
        (DiscourseContext::Clarify, IntentDisposition::Clarify),
    ] {
        for finality in [InputFinality::Partial, InputFinality::Final] {
            assert_eq!(
                classify(
                    &current,
                    &input(&current, finality),
                    "Attack fixture-option-1 now",
                    context,
                    256
                ),
                Ok(expected)
            );
            assert_ne!(expected, IntentDisposition::Action);
            assert_eq!(current, before);
        }
    }
}

#[test]
fn current_explicit_final_confirmation_is_only_a_proposal_guard() {
    let current = current();
    let before = current.clone();
    let input = input(&current, InputFinality::Final);
    for _ in 0..2 {
        assert_eq!(
            classify_text_request(
                TextQuestionRequest::ExplicitFinalConfirmation,
                &input,
                &current,
                member(3)
            ),
            Ok(IntentDisposition::Action)
        );
        assert_eq!(current.state().resources, before.state().resources);
        assert_eq!(current.state().draws, before.state().draws);
        assert_eq!(current.state().decisions, before.state().decisions);
        assert_eq!(current, before);
    }
}

#[test]
fn partial_confirmation_refuses_without_changing_prior_pending_spend() {
    let current = current();
    let before = current.clone();
    let input = input(&current, InputFinality::Partial);
    assert_eq!(
        classify_text_request(
            TextQuestionRequest::ExplicitFinalConfirmation,
            &input,
            &current,
            member(3)
        ),
        Err(DialogueError::PartialInput)
    );
    assert_eq!(
        current.state().pending[0].spent,
        before.state().pending[0].spent
    );
    assert_eq!(current, before);
}

#[test]
fn stale_session_run_and_revision_refuse_dialogue_and_confirmation() {
    let current = current();
    let before = current.clone();
    for field in 0..4 {
        let mut input = input(&current, InputFinality::Final);
        let cause = match field {
            0 => {
                input.basis.session = SessionId::from_bytes(&[91; 16]).unwrap();
                CheckpointError::WrongSession
            }
            1 => {
                input.basis.run = RunId::from_bytes(&[92; 16]).unwrap();
                CheckpointError::WrongRun
            }
            2 => {
                input.basis.revision = revision(2, 7);
                CheckpointError::StaleBasis
            }
            _ => {
                input.basis.revision = revision(3, 0);
                CheckpointError::StaleBasis
            }
        };
        assert_eq!(
            classify(
                &current,
                &input,
                "Can I attack?",
                DiscourseContext::Question,
                256
            ),
            Err(DialogueError::Snapshot(cause))
        );
        assert_eq!(
            classify_text_request(
                TextQuestionRequest::ExplicitFinalConfirmation,
                &input,
                &current,
                member(3)
            ),
            Err(DialogueError::Snapshot(cause))
        );
        assert_eq!(current, before);
    }
}

#[test]
fn stale_source_content_and_build_pins_refuse_before_interpretation() {
    let current = current();
    let before = current.clone();
    for field in 0..3 {
        let mut input = input(&current, InputFinality::Final);
        let cause = match field {
            0 => {
                input.pins.rules.handler = label("changed-handler");
                CheckpointError::RulesMismatch
            }
            1 => {
                input.pins.content.package = label("changed-package");
                CheckpointError::ContentMismatch
            }
            _ => {
                input.pins.build = BuildIdentity::new(
                    Some("changed-source"),
                    Some("native"),
                    Some("wasm"),
                    Some("config"),
                    Some("content"),
                )
                .unwrap();
                CheckpointError::BuildMismatch
            }
        };
        assert_eq!(
            classify(&current, &input, "Just joking", DiscourseContext::Joke, 256),
            Err(DialogueError::Snapshot(cause))
        );
        assert_eq!(
            classify_text_request(
                TextQuestionRequest::ExplicitFinalConfirmation,
                &input,
                &current,
                member(3)
            ),
            Err(DialogueError::Snapshot(cause))
        );
        assert_eq!(current, before);
    }
}

#[test]
fn wrong_or_departed_member_cannot_classify_or_confirm() {
    let current = current();
    let before = current.clone();
    let mut forged = input(&current, InputFinality::Final);
    forged.member = member(99);
    assert_eq!(
        classify(
            &current,
            &forged,
            "How much does that cost?",
            DiscourseContext::Question,
            256
        ),
        Err(DialogueError::MemberMismatch)
    );
    assert_eq!(
        classify_text_request(
            TextQuestionRequest::ExplicitFinalConfirmation,
            &forged,
            &current,
            member(3)
        ),
        Err(DialogueError::MemberMismatch)
    );
    assert_eq!(
        classify_text_request(
            TextQuestionRequest::ExplicitFinalConfirmation,
            &forged,
            &current,
            member(99)
        ),
        Err(DialogueError::MemberMismatch)
    );
    assert_eq!(current, before);
}

#[test]
fn invalid_text_and_utf8_byte_bounds_refuse_without_resource_or_draft_mutation() {
    let current = current();
    let before = current.clone();
    let input = input(&current, InputFinality::Final);
    let draft = String::from("é?");
    let draft_before = draft.clone();
    assert_eq!(
        classify(&current, &input, &draft, DiscourseContext::Question, 3),
        Ok(IntentDisposition::Question)
    );
    for (text, limit, expected) in [
        (" \t\n", 16, DialogueError::EmptyInput),
        ("attack now", 0, DialogueError::Capacity),
        ("attack now", 9, DialogueError::Capacity),
        (draft.as_str(), 2, DialogueError::Capacity),
    ] {
        assert_eq!(
            classify(&current, &input, text, DiscourseContext::Question, limit),
            Err(expected)
        );
        assert_eq!(current, before);
    }
    assert_eq!(draft, draft_before);
}

#[test]
fn classification_does_not_satisfy_the_separate_confirmation_path() {
    let current = current();
    let before = current.clone();
    let input = input(&current, InputFinality::Partial);
    assert_eq!(
        classify(
            &current,
            &input,
            "I attack now",
            DiscourseContext::Unspecified,
            256
        ),
        Ok(IntentDisposition::Clarify)
    );
    assert_eq!(
        classify_text_request(
            TextQuestionRequest::ExplicitFinalConfirmation,
            &input,
            &current,
            member(3)
        ),
        Err(DialogueError::PartialInput)
    );
    assert_eq!(current, before);
}
