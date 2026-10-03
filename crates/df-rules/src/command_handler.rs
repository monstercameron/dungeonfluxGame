//! One compiled handler contract and source-pinned invocation boundary.
use crate::dispatch::{DispatchError, DispatchRegistry};
use df_model::checkpoint::{Checkpoint, CheckpointError, CheckpointPins, GameInput, RuleReference};
use df_types::{RevisionError, RevisionLabel};

/// Compiled source-qualified mechanics supplied by the rules owner.
///
/// Staging borrows canonical input/state and returns a detached canonical checkpoint. It must
/// use supplied time/dice/policy only, perform no I/O and grant no commit authority. Unsupported
/// mechanics return the implementation's typed rejection. Pins alone never qualify a source.
pub trait RulesCommandHandler {
    type Rejection;

    fn pins(&self) -> &CheckpointPins;
    fn stage(
        &self,
        input: &GameInput,
        checkpoint: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection>;
}

/// A missing source binding and a rejected mechanic remain distinct typed refusals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvocationError<Rejection> {
    Dispatch(DispatchError),
    UnsupportedInput,
    InputBasisMismatch,
    AlreadyAccepted,
    CurrentPinsMismatch,
    CurrentCheckpoint(CheckpointError),
    HandlerRulesMismatch,
    HandlerContentMismatch,
    HandlerBuildMismatch,
    Revision(RevisionError),
    Handler(Rejection),
    CandidateBasisMismatch,
    CandidateRulesMismatch,
    CandidateContentMismatch,
    CandidateBuildMismatch,
    CandidateCheckpoint(CheckpointError),
    MissingAcceptedDecision,
    Capacity,
}

/// Invokes one compiled handler and verifies its canonical output binding without applying it.
/// Structural command admission, source selection and dependency legality belong to their
/// respective callers. Reuse this same boundary after registry selection for offer and submit.
pub fn stage_handler<Handler: RulesCommandHandler>(
    handler: &Handler,
    expected_pins: &CheckpointPins,
    input: &GameInput,
    current: &Checkpoint,
    maximum_candidate_bytes: usize,
) -> Result<Checkpoint, InvocationError<Handler::Rejection>> {
    if current.pins() != expected_pins {
        return Err(InvocationError::CurrentPinsMismatch);
    }
    current
        .validate_resume(current.basis(), expected_pins)
        .map_err(InvocationError::CurrentCheckpoint)?;
    let (input_basis, operation) = match input {
        GameInput::Game(command) => (command.basis, command.operation),
        GameInput::Host(command) => (command.basis, command.operation),
        _ => return Err(InvocationError::UnsupportedInput),
    };
    let current_basis = current.basis();
    if input_basis.session != current_basis.session
        || input_basis.run != current_basis.run
        || input_basis.revision.epoch() != current_basis.revision.epoch()
        || input_basis.revision > current_basis.revision
    {
        return Err(InvocationError::InputBasisMismatch);
    }
    if current
        .state()
        .decisions
        .iter()
        .any(|decision| decision.operation == operation)
    {
        return Err(InvocationError::AlreadyAccepted);
    }
    if maximum_candidate_bytes == 0 {
        return Err(InvocationError::Capacity);
    }
    let handler_pins = handler.pins();
    if handler_pins.rules != expected_pins.rules {
        return Err(InvocationError::HandlerRulesMismatch);
    }
    if handler_pins.content != expected_pins.content {
        return Err(InvocationError::HandlerContentMismatch);
    }
    if handler_pins.build != expected_pins.build {
        return Err(InvocationError::HandlerBuildMismatch);
    }
    let mut next_basis = current.basis();
    next_basis.revision = next_basis
        .revision
        .next_sequence()
        .map_err(InvocationError::Revision)?;
    let candidate = handler
        .stage(input, current)
        .map_err(InvocationError::Handler)?;
    if candidate.basis() != next_basis {
        return Err(InvocationError::CandidateBasisMismatch);
    }
    if candidate.pins().rules != expected_pins.rules {
        return Err(InvocationError::CandidateRulesMismatch);
    }
    if candidate.pins().content != expected_pins.content {
        return Err(InvocationError::CandidateContentMismatch);
    }
    if candidate.pins().build != expected_pins.build {
        return Err(InvocationError::CandidateBuildMismatch);
    }
    candidate
        .validate_resume(next_basis, expected_pins)
        .map_err(InvocationError::CandidateCheckpoint)?;
    if !candidate
        .state()
        .decisions
        .iter()
        .any(|decision| decision.operation == operation && decision.revision == next_basis.revision)
    {
        return Err(InvocationError::MissingAcceptedDecision);
    }
    if candidate
        .retained_bytes()
        .filter(|bytes| *bytes <= maximum_candidate_bytes)
        .is_none()
    {
        return Err(InvocationError::Capacity);
    }
    Ok(candidate)
}

impl<Handler: RulesCommandHandler> DispatchRegistry<'_, Handler> {
    /// Selects the exact pinned compiled handler, then runs the shared staging guard.
    /// Selection failure never invokes any registered handler. The source owner supplies the
    /// command-to-selector/clause mapping; imported catalog bytes cannot supply executable code.
    pub fn stage(
        &self,
        expected_pins: &CheckpointPins,
        selector: &RevisionLabel,
        source: &RuleReference,
        input: &GameInput,
        current: &Checkpoint,
        maximum_candidate_bytes: usize,
    ) -> Result<Checkpoint, InvocationError<Handler::Rejection>> {
        let handler = self
            .select(expected_pins, selector, source)
            .map_err(InvocationError::Dispatch)?;
        stage_handler(
            handler,
            expected_pins,
            input,
            current,
            maximum_candidate_bytes,
        )
    }
}
