//! Pure command admission, exact source-handler selection and immutable canonical staging.
//! Session authentication, operation lookup and durable commit remain caller-owned.
use df_model::checkpoint::{
    Basis, Checkpoint, CheckpointError, CheckpointPins, GameInput, ReferenceInventory,
    RuleReference,
};
use df_model::commands::{CommandError, CommandLimits, validate_client_command};
use df_rules::{DispatchRegistry, InvocationError, stage_handler};
pub use df_rules::{RulesCommandHandler, RulesCommandInput};
use df_types::RevisionLabel;

/// Explicit bounds; the native session freezes production values under its load/device gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandEntryLimits {
    pub command: CommandLimits,
    pub maximum_staged_bytes: usize,
}

/// Trusted current owner/source context; possession is not authentication or source qualification.
pub struct CommandEntryContext<'a> {
    pub current_basis: Basis,
    pub admitted_pins: &'a CheckpointPins,
    pub inventory: ReferenceInventory<'a>,
    pub limits: CommandEntryLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandRejection<R> {
    Checkpoint(CheckpointError),
    Structural(CommandError),
    Invocation(InvocationError<R>),
}

fn admit_command<R>(
    input: &GameInput,
    checkpoint: &Checkpoint,
    context: CommandEntryContext<'_>,
) -> Result<(), CommandRejection<R>> {
    checkpoint
        .validate_resume(context.current_basis, context.admitted_pins)
        .map_err(CommandRejection::Checkpoint)?;
    validate_client_command(input, checkpoint, context.inventory, context.limits.command)
        .map_err(CommandRejection::Structural)
}

/// Stage through an already-selected source-qualified handler without applying a checkpoint.
/// The session must authenticate/authorize ingress and lookup prior operation receipts first.
/// Canonical structural rejection never invokes the handler; typed mechanics/unsupported and
/// source/candidate failures retain their classes. The shared rules staging guard validates exact
/// next revision, same session/run/pins, matching AcceptedDecision and owned checkpoint capacity.
/// Actual draws are supplied explicitly through the rules owner's borrowed command envelope;
/// they are forwarded unchanged and never inferred or drawn by this entry. Sources/time/policy
/// remain caller-owned. No I/O, hidden draw, logging SDK, publication or commit occurs here.
pub fn decide_command<H: RulesCommandHandler>(
    input: RulesCommandInput<'_>,
    checkpoint: &Checkpoint,
    current_basis: Basis,
    admitted_pins: &CheckpointPins,
    inventory: ReferenceInventory<'_>,
    limits: CommandEntryLimits,
    handler: &H,
) -> Result<Checkpoint, CommandRejection<H::Rejection>> {
    admit_command(
        input.command,
        checkpoint,
        CommandEntryContext {
            current_basis,
            admitted_pins,
            inventory,
            limits,
        },
    )?;
    stage_handler(
        handler,
        admitted_pins,
        input,
        checkpoint,
        limits.maximum_staged_bytes,
    )
    .map_err(CommandRejection::Invocation)
}

/// Validate a command, select its exact registered source handler, and return a staged checkpoint.
/// The admitted command-to-selector/source mapping comes from the source owner, never from
/// imported executable bytes or an inferred action label. Missing selectors/clauses are typed
/// gaps and cannot invoke a fallback. The native session alone commits the returned checkpoint.
/// Older same-epoch observed offers retain the canonical model validator's current-state policy.
pub fn decide_registered_command<H: RulesCommandHandler>(
    input: RulesCommandInput<'_>,
    checkpoint: &Checkpoint,
    context: CommandEntryContext<'_>,
    registry: &DispatchRegistry<'_, H>,
    selector: &RevisionLabel,
    source: &RuleReference,
) -> Result<Checkpoint, CommandRejection<H::Rejection>> {
    let pins = context.admitted_pins;
    let maximum_bytes = context.limits.maximum_staged_bytes;
    admit_command(input.command, checkpoint, context)?;
    registry
        .stage(pins, selector, source, input, checkpoint, maximum_bytes)
        .map_err(CommandRejection::Invocation)
}
