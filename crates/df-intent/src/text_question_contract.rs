//! Executable test-mounted decision for text questions, jokes and explicit confirmation.
//! This adapter reuses the canonical intent guards and cannot stage game changes.
//! Native ticket/offer authorization and durable rules/session execution remain separate.
use df_intent::dialogue::{DialogueError, DialogueLimits, classify_dialogue, confirm_final_input};
use df_model::checkpoint::Checkpoint;
use df_model::intent::{DiscourseContext, IntentDisposition, RawInputRef};
use df_types::MemberId;

/// The native owner chooses the route; raw wording cannot request confirmation.
pub(crate) enum TextQuestionRequest<'a> {
    Dialogue {
        text: &'a str,
        context: DiscourseContext,
        limits: DialogueLimits,
    },
    ExplicitFinalConfirmation,
}

/// Returns only the existing disposition. Even Action is a proposal guard, not a
/// command, target, roll, resource spend, fact, committed receipt or permission.
pub(crate) fn classify_text_request(
    request: TextQuestionRequest<'_>,
    input: &RawInputRef,
    current: &Checkpoint,
    authenticated_member: MemberId,
) -> Result<IntentDisposition, DialogueError> {
    match request {
        TextQuestionRequest::Dialogue {
            text,
            context,
            limits,
        } => classify_dialogue(text, context, input, current, authenticated_member, limits),
        TextQuestionRequest::ExplicitFinalConfirmation => {
            confirm_final_input(input, current, authenticated_member)
        }
    }
}
