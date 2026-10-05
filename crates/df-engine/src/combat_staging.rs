//! Exact current member-controlled tactical responses enter the registered engine boundary.
use crate::command_entry::CommandEntryContext;
use crate::pending_resumption::{ResumeError, ResumeFence, ResumeLimits, stage_pending_response};
use df_combat::candidates::AdmittedCandidates;
use df_model::checkpoint::{Checkpoint, GameCommand, GameInput, OfferedResponse, PendingInput};
use df_rules::preconditions::PreconditionedCommandHandler;
use df_rules::{DispatchRegistry, RulesCommandHandler, RulesCommandInput};
use df_types::{MemberId, OperationId, RevisionLabel};

/// Current native admission, independent of tactical suggestions and client labels.
/// The receipt borrows exact canonical offers, not detached or reconstructed responses.
pub struct TacticalResponseAdmission<'a> {
    pub candidates: &'a AdmittedCandidates<'a, OfferedResponse>,
    pub member: MemberId,
    pub operation: OperationId,
    pub fence: ResumeFence,
    pub limits: ResumeLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TacticalResponseError<R> {
    Capacity,
    UnsupportedCommand,
    MemberMismatch,
    OperationMismatch,
    StaleBasis,
    StalePins,
    UnadmittedResponse,
    Resume(ResumeError<R>),
}

/// Stage an exact selected registered Choice/Reaction through the existing legal command path.
///
/// The caller composes Combat's registered enumeration and tactical selection, then supplies the
/// selected input and its current receipt. A preview checkpoint is never applied here. Exact
/// canonical response identity is rechecked; equal detached payloads are insufficient. The
/// current source registry, full source-owned dependencies and native generation/cancellation
/// fence are checked again by the pending engine entry before the handler runs.
///
/// Native member/operation admission must come from the authenticated session owner independently
/// of a suggestion. These fields and canonical attribution confer no autonomous NPC control.
/// General actions, rolls, host inputs and native completions remain outside this bounded entry.
/// Source rights, actor control and approved utility policy remain their existing owners' duties;
/// this adapter neither chooses a policy nor grants authority from a receipt.
///
/// The original native draw envelope is forwarded unchanged. A successful checkpoint is detached;
/// scoped operation lookup must precede this call, and durable acceptance/publication remain with
/// Session. Rejected/stale/unknown work cannot commit a preview or generate replacement outcomes.
pub fn stage_tactical_response<H: RulesCommandHandler>(
    input: RulesCommandInput<'_>,
    current: &Checkpoint,
    context: CommandEntryContext<'_>,
    admission: TacticalResponseAdmission<'_>,
    registry: &DispatchRegistry<'_, PreconditionedCommandHandler<'_, H>>,
    selector: &RevisionLabel,
) -> Result<Checkpoint, TacticalResponseError<H::Rejection>> {
    if admission.limits.maximum_checkpoint_bytes == 0
        || current
            .retained_bytes()
            .is_none_or(|bytes| bytes > admission.limits.maximum_checkpoint_bytes)
        || admission.candidates.offers().len() > context.limits.command.maximum_records
        || input
            .command
            .retained_bytes()
            .is_none_or(|bytes| bytes > context.limits.command.maximum_retained_bytes)
    {
        return Err(TacticalResponseError::Capacity);
    }
    let GameInput::Game(command) = input.command else {
        return Err(TacticalResponseError::UnsupportedCommand);
    };
    if command.member != admission.member {
        return Err(TacticalResponseError::MemberMismatch);
    }
    if command.operation != admission.operation {
        return Err(TacticalResponseError::OperationMismatch);
    }
    if admission.candidates.basis() != &current.basis()
        || context.current_basis != current.basis()
        || command.basis != current.basis()
        || command.observed_revision != current.basis().revision
    {
        return Err(TacticalResponseError::StaleBasis);
    }
    if admission.candidates.pins() != current.pins() || context.admitted_pins != current.pins() {
        return Err(TacticalResponseError::StalePins);
    }
    let (resolution, window, offer, option, reaction) = match &command.command {
        GameCommand::SelectChoice {
            resolution,
            window,
            offer,
            option,
        } => (resolution, window, offer, option, false),
        GameCommand::SelectReaction {
            resolution,
            window,
            offer,
            option,
        } => (resolution, window, offer, option, true),
        _ => return Err(TacticalResponseError::UnsupportedCommand),
    };
    let pending = current
        .state()
        .pending
        .iter()
        .find(|pending| pending.id == *resolution && pending.window.id == *window);
    let responses = match pending.map(|pending| &pending.next) {
        Some(PendingInput::Choice { remaining }) if !reaction => remaining,
        Some(PendingInput::Reaction { remaining }) if reaction => remaining,
        _ => return Err(TacticalResponseError::UnadmittedResponse),
    };
    let exact = admission.candidates.offers().iter().any(|candidate| {
        responses
            .iter()
            .any(|response| std::ptr::eq(*candidate, response))
            && candidate.participant == admission.member
            && &candidate.offer == offer
            && candidate.options.contains(option)
    });
    if !exact {
        return Err(TacticalResponseError::UnadmittedResponse);
    }
    stage_pending_response(
        input,
        current,
        context,
        admission.fence,
        admission.limits,
        registry,
        selector,
    )
    .map_err(TacticalResponseError::Resume)
}
