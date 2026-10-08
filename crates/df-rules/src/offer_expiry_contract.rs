//! Executable offer-expiry contract mounted by its dedicated qualification test.
//! Session authentication, receipt lookup and committing the candidate remain owner gates.
use df_model::checkpoint::{Checkpoint, CommandInput, GameInput};
use df_rules::current_responses::{
    ResponsePreparationError, offered_current_commands, prepare_current_response,
    validate_current_submission,
};
use df_rules::preconditions::{CurrentRuleContext, PreconditionedCommandHandler};
use df_rules::{DispatchRegistry, RulesCommandHandler, RulesCommandInput};
use df_types::RevisionLabel;

/// Enumerate only source-prepared current options; stale template labels cannot create offers.
pub(crate) fn enumerate<H: RulesCommandHandler>(
    template: &CommandInput,
    context: CurrentRuleContext<'_>,
    registry: &DispatchRegistry<'_, PreconditionedCommandHandler<'_, H>>,
    selector: &RevisionLabel,
    maximum_staged_bytes: usize,
) -> Result<Vec<GameInput>, ResponsePreparationError<H::Rejection>> {
    offered_current_commands(template, context, registry, selector, maximum_staged_bytes)
}

/// Validate the current choice/reaction window and its exact source-owned target prerequisites.
/// An older same-epoch observation is context, rather than an unconditional revision lock.
/// Expiry is an authoritative change to pending state or a handler-declared time dependency;
/// this boundary does not decide expiry from a browser countdown or invent a clock.
pub(crate) fn submit<H: RulesCommandHandler>(
    input: RulesCommandInput<'_>,
    context: CurrentRuleContext<'_>,
    registry: &DispatchRegistry<'_, PreconditionedCommandHandler<'_, H>>,
    selector: &RevisionLabel,
    maximum_staged_bytes: usize,
) -> Result<Checkpoint, ResponsePreparationError<H::Rejection>> {
    validate_current_submission(
        input.command,
        context.checkpoint,
        context.basis,
        context.pins,
        df_model::checkpoint::ReferenceInventory {
            rules: context.inventory.rules,
            content: context.inventory.content,
            resources: context.inventory.resources,
            assets: context.inventory.assets,
        },
        context.command_limits,
    )
    .map_err(ResponsePreparationError::Response)?;
    prepare_current_response(input, context, registry, selector, maximum_staged_bytes)
}
