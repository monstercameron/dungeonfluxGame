//! Conservative dialogue classification and final-input confirmation guards.
//! Raw language never produces an action, target, rule, die or canonical fact.
use df_model::checkpoint::{Checkpoint, CheckpointError};
use df_model::intent::{DiscourseContext, InputFinality, IntentDisposition, RawInputRef};
use df_types::MemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueLimits {
    pub maximum_text_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueError {
    Capacity,
    EmptyInput,
    MemberMismatch,
    PartialInput,
    Snapshot(CheckpointError),
}

fn validate_basis(
    input: &RawInputRef,
    current: &Checkpoint,
    authenticated_member: MemberId,
) -> Result<(), DialogueError> {
    if input.member != authenticated_member
        || !current
            .state()
            .members
            .iter()
            .any(|link| link.member == authenticated_member)
    {
        return Err(DialogueError::MemberMismatch);
    }
    current
        .validate_resume(input.basis, &input.pins)
        .map(|_| ())
        .map_err(DialogueError::Snapshot)
}

/// Classifies bounded raw input using declared non-action discourse context.
/// Untagged or ambiguous language conservatively requests clarification. Context
/// cannot select an action, including when text names a valid current offer.
/// Native authorization, input identity, finality and explicit choice remain owned
/// by the caller; a disposition is not a permission, staged outcome or commit.
pub fn classify_dialogue(
    text: &str,
    context: DiscourseContext,
    input: &RawInputRef,
    current: &Checkpoint,
    authenticated_member: MemberId,
    limits: DialogueLimits,
) -> Result<IntentDisposition, DialogueError> {
    validate_basis(input, current, authenticated_member)?;
    if limits.maximum_text_bytes == 0 || text.len() > limits.maximum_text_bytes {
        return Err(DialogueError::Capacity);
    }
    if text.trim().is_empty() {
        return Err(DialogueError::EmptyInput);
    }
    Ok(match context {
        DiscourseContext::Question => IntentDisposition::Question,
        DiscourseContext::Social => IntentDisposition::Social,
        DiscourseContext::PlanOnly => IntentDisposition::PlanOnly,
        DiscourseContext::Meta => IntentDisposition::Meta,
        DiscourseContext::Joke => IntentDisposition::Joke,
        DiscourseContext::Unspecified | DiscourseContext::Clarify => IntentDisposition::Clarify,
    })
}

/// Validates the input reference for a separate, explicit native choice.
/// The native consumer must additionally authenticate current authority, match
/// its retained ticket and exact current offer, then use the existing Rules and
/// durable Session path. This function never converts raw text into a command.
pub fn confirm_final_input(
    input: &RawInputRef,
    current: &Checkpoint,
    authenticated_member: MemberId,
) -> Result<IntentDisposition, DialogueError> {
    validate_basis(input, current, authenticated_member)?;
    if input.finality != InputFinality::Final {
        return Err(DialogueError::PartialInput);
    }
    Ok(IntentDisposition::Action)
}
