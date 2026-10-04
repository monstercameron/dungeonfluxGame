//! Canonical selected-movement binding before the existing rules handler executes.
//! Exact path mechanics and complete source-reviewed dependencies remain handler-owned.

use crate::movement::SelectedMovement;
use df_model::checkpoint::{
    Checkpoint, CheckpointPins, ContentReference, GameCommand, GameInput, RuleReference,
};
use df_rules::{RulesCommandHandler, RulesCommandInput};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MovementBindingError<R> {
    PreparedBasisMismatch,
    PinsMismatch,
    SourceMismatch,
    CommandMismatch,
    Handler(R),
}

/// Registrable wrapper around one actual source-bound compiled rules handler.
///
/// The inner handler must be reviewed for this exact selected path/action mapping;
/// canonical ProposeAction currently has no path/position payload. This wrapper
/// preserves the selected path as borrowed evidence, never invents that payload.
/// Wrap it in Rules I04's PreconditionedCommandHandler with the immutable complete
/// source-declared Entity/Encounter/Environment/resource/effect/timing dependencies,
/// then invoke Rules I01 stage_handler or the same registry through Engine I01.
/// This adapter checks selection-to-command binding only; the actual I04 guard owns
/// canonical prepared/current dependency comparisons. No state is mutated here.
/// The original borrowed command/draw envelope is forwarded without replacing draws.
pub struct BoundMovementHandler<'a, Path, Handler> {
    pub handler: &'a Handler,
    pub selected: &'a SelectedMovement<'a, Path>,
    pub prepared: &'a Checkpoint,
    /// Exact clause/action mapping supplied by the reviewed inner handler owner.
    /// Its I01 registration and I04 wrapper must bind this same clause.
    pub source: &'a RuleReference,
    pub action: &'a ContentReference,
}

impl<Path, Handler: RulesCommandHandler> RulesCommandHandler
    for BoundMovementHandler<'_, Path, Handler>
{
    type Rejection = MovementBindingError<Handler::Rejection>;

    fn pins(&self) -> &CheckpointPins {
        self.handler.pins()
    }

    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        let selected = self.selected;
        if selected.basis != self.prepared.basis() {
            return Err(MovementBindingError::PreparedBasisMismatch);
        }
        if selected.pins != self.prepared.pins() || selected.pins != current.pins() {
            return Err(MovementBindingError::PinsMismatch);
        }
        if selected.facts.source != self.source {
            return Err(MovementBindingError::SourceMismatch);
        }
        let GameInput::Game(command) = input.command else {
            return Err(MovementBindingError::CommandMismatch);
        };
        let GameCommand::ProposeAction {
            actor,
            action,
            targets,
            ..
        } = &command.command
        else {
            return Err(MovementBindingError::CommandMismatch);
        };
        if *actor != selected.facts.actor
            || action != self.action
            || targets.as_slice() != [selected.facts.destination]
        {
            return Err(MovementBindingError::CommandMismatch);
        }
        self.handler
            .stage(input, current)
            .map_err(MovementBindingError::Handler)
    }
}
