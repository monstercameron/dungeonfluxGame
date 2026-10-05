//! Admission of typed suggestions against current source-backed pending responses.
//! This boundary returns proposals only; it cannot interpret text or authorize execution.

use df_model::checkpoint::{
    Basis, Checkpoint, CheckpointError, CheckpointPins, CommandInput, GameCommand, GameInput,
    PendingInput, ReferenceInventory,
};
use df_model::commands::{CommandError, CommandLimits, validate_client_command};
use df_rules::current_responses::{
    ResponseError, ResponsePreparationError, prepare_current_response, validate_current_submission,
};
use df_rules::preconditions::{CurrentRuleContext, PreconditionedCommandHandler};
use df_rules::{DispatchRegistry, RulesCommandHandler, RulesCommandInput};
use df_types::{MemberId, OperationId, RevisionLabel};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateError {
    UnsupportedVariant,
    Snapshot(CheckpointError),
    MemberMismatch,
    OperationMismatch,
    Command(CommandError),
}

/// Native admission context captured independently of classifier output and client observations.
/// This borrowed context grants no authentication or source rights by itself.
pub struct CandidateOwner<'a> {
    pub basis: Basis,
    pub pins: &'a CheckpointPins,
    pub member: MemberId,
    pub operation: OperationId,
}

/// Admission refusals remain distinct from source dispatch, dependency and handler refusals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CandidatePreparationError<R> {
    Candidate(CandidateError),
    Rules(ResponsePreparationError<R>),
}

/// Passes an admitted suggestion through the same compiled rules preparation used on submit.
///
/// The registry binds the exact pending source to a compiled handler with immutable source-
/// reviewed dependencies. An admission success alone is insufficient to produce this result.
/// Unknown handlers, changed dependencies or handler refusal return their typed rules outcome.
/// The returned canonical checkpoint is staged only; this function never commits or publishes it.
/// Classifier text, host commands and general actions cannot bypass the candidate variant gate.
/// The native owner supplies actual outcomes independently of classifier output. This adapter
/// validates only the canonical command and forwards the same borrowed draw slice to the rules
/// guard; it cannot generate outcomes, substitute empty draws or consume durable dice.
pub fn prepare_semantic_candidate<H: RulesCommandHandler>(
    candidate: RulesCommandInput<'_>,
    context: CurrentRuleContext<'_>,
    owner: CandidateOwner<'_>,
    registry: &DispatchRegistry<'_, PreconditionedCommandHandler<'_, H>>,
    selector: &RevisionLabel,
    maximum_staged_bytes: usize,
) -> Result<Checkpoint, CandidatePreparationError<H::Rejection>> {
    validate_semantic_candidate(
        candidate.command,
        context.checkpoint,
        owner,
        ReferenceInventory {
            rules: context.inventory.rules,
            content: context.inventory.content,
            resources: context.inventory.resources,
            assets: context.inventory.assets,
        },
        context.command_limits,
    )
    .map_err(CandidatePreparationError::Candidate)?;
    prepare_current_response(candidate, context, registry, selector, maximum_staged_bytes)
        .map_err(CandidatePreparationError::Rules)
}

/// Admits only a choice, reaction or roll request already permitted by the current checkpoint.
///
/// The caller supplies the authenticated member and admitted operation independently of model
/// output. Sources must remain in the trusted reference inventory. The native owner's basis and
/// complete pins must match the current snapshot. An older same-epoch client observation remains
/// valid when its exact current offer survives; it never substitutes for the native current basis.
/// Existing operation receipt lookup remains a session responsibility before this admission.
/// Overlapping current option identities must agree on their exact source; otherwise the
/// suggestion is refused as a duplicate selection rather than selecting provenance by order.
///
/// The borrowed result preserves the supplied command without producing state, costs, dice or
/// facts. It does not grant actor control or confirm an ambiguous utterance. Session/engine owners
/// must enforce those permissions, input finality and rules revalidation before committing.
/// General actions, speech, character drafts, host commands and native inputs are unsupported:
/// the canonical semantic-disposition and source-legal action contracts are not defined yet.
/// Bounds cover retained candidate allocation and every collection scanned by this operation.
pub fn validate_semantic_candidate<'a>(
    candidate: &'a GameInput,
    current: &Checkpoint,
    owner: CandidateOwner<'_>,
    inventory: ReferenceInventory<'_>,
    limits: CommandLimits,
) -> Result<&'a CommandInput, CandidateError> {
    let GameInput::Game(input) = candidate else {
        return Err(CandidateError::UnsupportedVariant);
    };
    let (resolution, window) = match &input.command {
        GameCommand::SelectChoice {
            resolution, window, ..
        }
        | GameCommand::SelectReaction {
            resolution, window, ..
        }
        | GameCommand::SubmitRoll { resolution, window } => (*resolution, *window),
        GameCommand::ProposeAction { .. }
        | GameCommand::Speak { .. }
        | GameCommand::SubmitCharacterDraft { .. } => {
            return Err(CandidateError::UnsupportedVariant);
        }
    };
    current
        .validate_resume(owner.basis, owner.pins)
        .map_err(CandidateError::Snapshot)?;
    if input.member != owner.member {
        return Err(CandidateError::MemberMismatch);
    }
    if input.operation != owner.operation {
        return Err(CandidateError::OperationMismatch);
    }
    if limits.maximum_text_bytes == 0
        || limits.maximum_retained_bytes == 0
        || candidate
            .retained_bytes()
            .is_none_or(|bytes| bytes > limits.maximum_retained_bytes)
    {
        return Err(CandidateError::Command(CommandError::Capacity));
    }
    let mut records = current
        .state()
        .members
        .len()
        .checked_add(current.state().pending.len())
        .and_then(|count| count.checked_add(inventory.rules.len()))
        .ok_or(CandidateError::Command(CommandError::Capacity))?;
    if limits.maximum_records == 0 || records > limits.maximum_records {
        return Err(CandidateError::Command(CommandError::Capacity));
    }
    let pending = current
        .state()
        .pending
        .iter()
        .find(|pending| pending.id == resolution && pending.window.id == window)
        .ok_or(CandidateError::Command(CommandError::StaleWindow))?;
    let source = match (&input.command, &pending.next) {
        (GameCommand::SelectChoice { offer, option, .. }, PendingInput::Choice { remaining })
        | (
            GameCommand::SelectReaction { offer, option, .. },
            PendingInput::Reaction { remaining },
        ) => {
            records = records
                .checked_add(remaining.len())
                .ok_or(CandidateError::Command(CommandError::Capacity))?;
            if records > limits.maximum_records {
                return Err(CandidateError::Command(CommandError::Capacity));
            }
            for response in remaining {
                records = records
                    .checked_add(response.options.len())
                    .ok_or(CandidateError::Command(CommandError::Capacity))?;
                if records > limits.maximum_records {
                    return Err(CandidateError::Command(CommandError::Capacity));
                }
            }
            if offer.as_str().len() > limits.maximum_text_bytes
                || option.as_str().len() > limits.maximum_text_bytes
            {
                return Err(CandidateError::Command(CommandError::Capacity));
            }
            let mut matches = remaining.iter().filter(|response| {
                response.participant == owner.member
                    && response.offer == *offer
                    && response.options.contains(option)
            });
            let response = matches
                .next()
                .ok_or(CandidateError::Command(CommandError::UnofferedResponse))?;
            if matches.any(|other| other.source != response.source) {
                return Err(CandidateError::Command(CommandError::DuplicateSelection));
            }
            &response.source
        }
        (GameCommand::SubmitRoll { .. }, PendingInput::Roll { source, .. }) => source,
        _ => return Err(CandidateError::Command(CommandError::WrongPendingKind)),
    };
    if source.catalog != current.pins().rules.catalog
        || !inventory.rules.contains(source)
        || !inventory.rules.contains(&pending.window.source)
    {
        return Err(CandidateError::Command(CommandError::InvalidReference));
    }
    match &input.command {
        GameCommand::SelectChoice { .. } | GameCommand::SelectReaction { .. } => {
            validate_current_submission(
                candidate,
                current,
                owner.basis,
                owner.pins,
                inventory,
                limits,
            )
            .map_err(|error| match error {
                ResponseError::Snapshot(error) => CandidateError::Snapshot(error),
                ResponseError::Admission(error) => CandidateError::Command(error),
                ResponseError::UnsupportedCommand => CandidateError::UnsupportedVariant,
            })?;
        }
        GameCommand::SubmitRoll { .. } => {
            validate_client_command(candidate, current, inventory, limits)
                .map_err(CandidateError::Command)?;
        }
        _ => return Err(CandidateError::UnsupportedVariant),
    }
    Ok(input)
}
