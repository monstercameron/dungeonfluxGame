//! Structural command admission over the canonical model records. No mechanics or I/O.
use crate::checkpoint::{
    Basis, Checkpoint, CommandInput, ContentReference, CreationPhase, EntityId, GameCommand,
    GameInput, HostCommand, PendingInput, PendingResolution, ReferenceInventory, ResolutionId,
    WindowId,
};
use df_types::{MemberId, RevisionLabel, SessionRevision};

/// Caller-selected admission bounds; retained bytes include unused owned allocation capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandLimits {
    pub maximum_records: usize,
    pub maximum_text_bytes: usize,
    pub maximum_retained_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandError {
    InternalInput,
    PresentationReport,
    WrongSession,
    WrongRun,
    StaleRevision,
    UnknownMember,
    InvalidReference,
    DuplicateSelection,
    StaleWindow,
    WrongPendingKind,
    UnofferedResponse,
    InvalidDraft,
    Capacity,
}

/// Validates a new command against an immutable current checkpoint and trusted references.
///
/// This is structural admission, not authentication or authorization. The session must first
/// authenticate/bind the sender, authorize host actions, and return a prior operation receipt
/// before checking a retry against current offers. Rules still revalidate legality and actor
/// control. An observed older sequence is allowed when its exact pending window/offer survives;
/// a retired epoch or future revision is refused. No state, fact, draw or effect is produced.
/// Presentation reports use their separate authorized delivery boundary; native completions and
/// timers cannot pass this command entry point. DTO codecs belong to the API owner.
///
/// Unknown references, stale windows, mismatched input kinds and exhausted bounds return typed
/// refusals without mutating input or checkpoint. Caller-selected limits have no default.
pub fn validate_client_command(
    input: &GameInput,
    checkpoint: &Checkpoint,
    inventory: ReferenceInventory<'_>,
    limits: CommandLimits,
) -> Result<(), CommandError> {
    match input {
        GameInput::Job(_) | GameInput::Timer(_) => return Err(CommandError::InternalInput),
        GameInput::Presentation(_) => return Err(CommandError::PresentationReport),
        GameInput::Game(_) | GameInput::Host(_) => {}
    }
    if limits.maximum_records == 0
        || limits.maximum_text_bytes == 0
        || limits.maximum_retained_bytes == 0
        || input.retained_bytes().ok_or(CommandError::Capacity)? > limits.maximum_retained_bytes
    {
        return Err(CommandError::Capacity);
    }
    let admitted_content = |reference: &ContentReference| {
        if reference.package == checkpoint.pins().content.package
            && inventory.content.contains(reference)
        {
            Ok(())
        } else {
            Err(CommandError::InvalidReference)
        }
    };
    match input {
        GameInput::Game(command) => {
            validate_basis(command.basis, checkpoint.basis())?;
            validate_revision(command.observed_revision, checkpoint.basis().revision)?;
            validate_member(command.member, checkpoint)?;
            validate_game(command, checkpoint, &inventory, limits, admitted_content)
        }
        GameInput::Host(command) => {
            validate_basis(command.basis, checkpoint.basis())?;
            validate_member(command.host, checkpoint)?;
            match &command.command {
                HostCommand::ResolveRuling {
                    resolution,
                    window,
                    offer,
                    option,
                } => {
                    let pending = pending(checkpoint, *resolution, *window)?;
                    match &pending.next {
                        PendingInput::Ruling { permitted, .. } => {
                            validate_offer(permitted, command.host, offer, option)
                        }
                        _ => Err(CommandError::WrongPendingKind),
                    }
                }
                HostCommand::SetPause { policy, .. } => admitted_content(policy),
                HostCommand::RequestCheckpoint => Ok(()),
            }
        }
        GameInput::Job(_) | GameInput::Timer(_) | GameInput::Presentation(_) => {
            Err(CommandError::InternalInput)
        }
    }
}

fn validate_game(
    input: &CommandInput,
    checkpoint: &Checkpoint,
    inventory: &ReferenceInventory<'_>,
    limits: CommandLimits,
    content: impl Fn(&ContentReference) -> Result<(), CommandError>,
) -> Result<(), CommandError> {
    let entity = |id: EntityId| {
        if checkpoint
            .state()
            .entities
            .iter()
            .any(|record| record.id == id)
        {
            Ok(())
        } else {
            Err(CommandError::InvalidReference)
        }
    };
    match &input.command {
        GameCommand::SelectChoice {
            resolution,
            window,
            offer,
            option,
        }
        | GameCommand::SelectReaction {
            resolution,
            window,
            offer,
            option,
        } => {
            let pending = pending(checkpoint, *resolution, *window)?;
            let remaining = match (&input.command, &pending.next) {
                (GameCommand::SelectChoice { .. }, PendingInput::Choice { remaining })
                | (GameCommand::SelectReaction { .. }, PendingInput::Reaction { remaining }) => {
                    remaining
                }
                _ => return Err(CommandError::WrongPendingKind),
            };
            validate_offer(remaining, input.member, offer, option)
        }
        GameCommand::SubmitRoll { resolution, window } => {
            let pending = pending(checkpoint, *resolution, *window)?;
            match &pending.next {
                PendingInput::Roll { participant, .. } if *participant == input.member => Ok(()),
                PendingInput::Roll { .. } => Err(CommandError::UnofferedResponse),
                _ => Err(CommandError::WrongPendingKind),
            }
        }
        GameCommand::ProposeAction {
            actor,
            action,
            targets,
            choices,
        } => {
            entity(*actor)?;
            content(action)?;
            count(targets.len(), choices.len(), limits.maximum_records)?;
            for (index, target) in targets.iter().enumerate() {
                entity(*target)?;
                if targets.iter().take(index).any(|earlier| earlier == target) {
                    return Err(CommandError::DuplicateSelection);
                }
            }
            for (index, (offer, _)) in choices.iter().enumerate() {
                if choices
                    .iter()
                    .take(index)
                    .any(|(earlier, _)| earlier == offer)
                {
                    return Err(CommandError::DuplicateSelection);
                }
            }
            Ok(())
        }
        GameCommand::SubmitCharacterDraft { draft } => {
            if draft.member != input.member
                || draft.phase == CreationPhase::Accepted
                || checkpoint
                    .state()
                    .continuity
                    .creation
                    .iter()
                    .any(|current| {
                        current.entity == draft.entity && current.phase == CreationPhase::Accepted
                    })
            {
                return Err(CommandError::InvalidDraft);
            }
            entity(draft.entity)?;
            count(
                draft.classes.len(),
                draft.choices.len(),
                limits.maximum_records,
            )?;
            for reference in draft
                .ancestry
                .iter()
                .chain(draft.background.iter())
                .chain(draft.classes.iter())
            {
                content(reference)?;
            }
            for (index, choice) in draft.choices.iter().enumerate() {
                if choice.participant != input.member
                    || choice.source.catalog != checkpoint.pins().rules.catalog
                    || !inventory.rules.contains(&choice.source)
                {
                    return Err(CommandError::InvalidDraft);
                }
                if draft
                    .choices
                    .iter()
                    .take(index)
                    .any(|earlier| earlier.offer == choice.offer)
                {
                    return Err(CommandError::DuplicateSelection);
                }
            }
            Ok(())
        }
        GameCommand::Speak {
            speaker,
            text,
            conversation,
        } => {
            entity(*speaker)?;
            if text.len() > limits.maximum_text_bytes {
                return Err(CommandError::Capacity);
            }
            if let Some(id) = conversation {
                let conversation = checkpoint
                    .state()
                    .conversations
                    .iter()
                    .find(|record| record.id == *id)
                    .ok_or(CommandError::InvalidReference)?;
                if !conversation.participants.contains(speaker) {
                    return Err(CommandError::InvalidReference);
                }
            }
            Ok(())
        }
    }
}

fn validate_basis(basis: Basis, current: Basis) -> Result<(), CommandError> {
    if basis.session != current.session {
        return Err(CommandError::WrongSession);
    }
    if basis.run != current.run {
        return Err(CommandError::WrongRun);
    }
    validate_revision(basis.revision, current.revision)
}

fn validate_revision(
    revision: SessionRevision,
    current: SessionRevision,
) -> Result<(), CommandError> {
    if revision.epoch() != current.epoch() || revision > current {
        Err(CommandError::StaleRevision)
    } else {
        Ok(())
    }
}

fn validate_member(member: MemberId, checkpoint: &Checkpoint) -> Result<(), CommandError> {
    if checkpoint
        .state()
        .members
        .iter()
        .any(|record| record.member == member)
    {
        Ok(())
    } else {
        Err(CommandError::UnknownMember)
    }
}

fn pending(
    checkpoint: &Checkpoint,
    resolution: ResolutionId,
    window: WindowId,
) -> Result<&PendingResolution, CommandError> {
    checkpoint
        .state()
        .pending
        .iter()
        .find(|record| record.id == resolution && record.window.id == window)
        .ok_or(CommandError::StaleWindow)
}

fn validate_offer(
    offered: &[crate::checkpoint::OfferedResponse],
    member: MemberId,
    offer: &RevisionLabel,
    option: &RevisionLabel,
) -> Result<(), CommandError> {
    if offered.iter().any(|record| {
        record.participant == member && record.offer == *offer && record.options.contains(option)
    }) {
        Ok(())
    } else {
        Err(CommandError::UnofferedResponse)
    }
}

fn count(left: usize, right: usize, maximum: usize) -> Result<(), CommandError> {
    if left.checked_add(right).ok_or(CommandError::Capacity)? > maximum {
        Err(CommandError::Capacity)
    } else {
        Ok(())
    }
}
