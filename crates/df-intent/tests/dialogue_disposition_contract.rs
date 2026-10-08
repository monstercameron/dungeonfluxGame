use df_intent::dialogue::{DialogueError, DialogueLimits, classify_dialogue, confirm_final_input};
use df_model::intent::{DiscourseContext, InputFinality, IntentDisposition, RawInputRef};
use df_types::OperationId;

#[path = "support/candidate_fixture.rs"]
mod candidate_fixture;
use candidate_fixture::*;

fn reference() -> RawInputRef {
    RawInputRef {
        input: OperationId::from_bytes(&[30; 16]).unwrap(),
        member: member(3),
        basis: basis(),
        pins: pins(),
        finality: InputFinality::Final,
    }
}

#[test]
fn raw_dialogue_and_action_wording_never_propose_mechanics() {
    let mut initial = state();
    initial.facts.push(fact(7, 0));
    initial.pending.push(pending());
    let current = checkpoint(initial).unwrap();
    let original = current.clone();
    let cases = [
        (
            DiscourseContext::Question,
            "Can I attack the courier?",
            IntentDisposition::Question,
        ),
        (
            DiscourseContext::Social,
            "Hello, courier.",
            IntentDisposition::Social,
        ),
        (
            DiscourseContext::PlanOnly,
            "If I attack then run away...",
            IntentDisposition::PlanOnly,
        ),
        (
            DiscourseContext::Meta,
            "Pause; how do the controls work?",
            IntentDisposition::Meta,
        ),
        (
            DiscourseContext::Joke,
            "I stab the moon. Just kidding.",
            IntentDisposition::Joke,
        ),
        (
            DiscourseContext::Unspecified,
            "Attack fixture-option-1 now",
            IntentDisposition::Clarify,
        ),
        (
            DiscourseContext::Clarify,
            "Do that to him",
            IntentDisposition::Clarify,
        ),
    ];
    for (context, text, expected) in cases {
        for finality in [InputFinality::Partial, InputFinality::Final] {
            let mut input = reference();
            input.finality = finality;
            assert_eq!(
                classify_dialogue(
                    text,
                    context,
                    &input,
                    &current,
                    member(3),
                    DialogueLimits {
                        maximum_text_bytes: 4096
                    }
                ),
                Ok(expected)
            );
            assert_ne!(expected, IntentDisposition::Action);
            assert_eq!(current, original);
        }
    }
}

#[test]
fn separate_confirmation_requires_final_current_source_and_member() {
    let current = checkpoint(state()).unwrap();
    let mut input = reference();
    assert_eq!(
        confirm_final_input(&input, &current, member(3)),
        Ok(IntentDisposition::Action)
    );
    input.finality = InputFinality::Partial;
    assert_eq!(
        confirm_final_input(&input, &current, member(3)),
        Err(DialogueError::PartialInput)
    );
    input = reference();
    input.basis.revision = revision(2, 7);
    assert!(matches!(
        confirm_final_input(&input, &current, member(3)),
        Err(DialogueError::Snapshot(_))
    ));
    input = reference();
    input.pins.rules.handler = label("changed-handler-1");
    assert!(matches!(
        confirm_final_input(&input, &current, member(3)),
        Err(DialogueError::Snapshot(_))
    ));
    input = reference();
    assert_eq!(
        confirm_final_input(&input, &current, member(4)),
        Err(DialogueError::MemberMismatch)
    );
}

#[test]
fn bounded_invalid_raw_input_returns_refusal_without_state() {
    let current = checkpoint(state()).unwrap();
    let input = reference();
    assert_eq!(
        classify_dialogue(
            " ",
            DiscourseContext::Unspecified,
            &input,
            &current,
            member(3),
            DialogueLimits {
                maximum_text_bytes: 8
            }
        ),
        Err(DialogueError::EmptyInput)
    );
    assert_eq!(
        classify_dialogue(
            "attack now",
            DiscourseContext::Unspecified,
            &input,
            &current,
            member(3),
            DialogueLimits {
                maximum_text_bytes: 8
            }
        ),
        Err(DialogueError::Capacity)
    );
    assert_eq!(
        classify_dialogue(
            "hello",
            DiscourseContext::Joke,
            &input,
            &current,
            member(3),
            DialogueLimits {
                maximum_text_bytes: 0
            }
        ),
        Err(DialogueError::Capacity)
    );
}
