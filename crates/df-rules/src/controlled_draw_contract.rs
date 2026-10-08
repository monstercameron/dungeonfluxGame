//! Executable controlled-roll contract over the canonical response boundary.
//! Model/client commands identify a pending roll; only the trusted session owner supplies
//! actual outcomes separately. This pure test-mounted adapter does not generate randomness,
//! authenticate the supplier, commit draws, or qualify synthetic source mechanics.
use df_model::checkpoint::{Checkpoint, CommandInput, GameCommand, GameInput};
use df_rules::current_responses::{
    ResponseError, ResponsePreparationError, prepare_current_response,
};
use df_rules::preconditions::{CurrentRuleContext, PreconditionedCommandHandler};
use df_rules::{DispatchRegistry, RulesCommandHandler, RulesCommandInput};
use df_types::RevisionLabel;

/// Accepts only the canonical roll request and forwards the untouched draw envelope through
/// current admission, exact source selection, dependency revalidation and draw accounting.
/// A returned checkpoint is detached; durable consumption and retry receipts belong to the owner.
pub fn prepare_controlled_roll<H: RulesCommandHandler>(
    input: RulesCommandInput<'_>,
    context: CurrentRuleContext<'_>,
    registry: &DispatchRegistry<'_, PreconditionedCommandHandler<'_, H>>,
    selector: &RevisionLabel,
    maximum_staged_bytes: usize,
) -> Result<Checkpoint, ResponsePreparationError<H::Rejection>> {
    if !matches!(
        input.command,
        GameInput::Game(CommandInput {
            command: GameCommand::SubmitRoll {
                resolution: _,
                window: _
            },
            ..
        })
    ) {
        return Err(ResponsePreparationError::Response(
            ResponseError::UnsupportedCommand,
        ));
    }
    prepare_current_response(input, context, registry, selector, maximum_staged_bytes)
}
