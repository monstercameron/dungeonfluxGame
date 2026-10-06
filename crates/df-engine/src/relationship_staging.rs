//! Recomputed categorical reactions attached to an actual source-selected command transition.
//! The session alone commits the complete checkpoint; private proposals never become facts.

use df_interaction::reactions::{
    ReactionEntry, ReactionError, ReactionLimits, ReactionOutcome, ReactionPolicy, ReactionRequest,
    ReactionSourceOwner, react,
};
use df_model::checkpoint::{
    Basis, Checkpoint, CheckpointError, CheckpointLimits, CheckpointPins, CommandInput,
    GameCommand, GameInput, ReferenceInventory,
};
use df_rules::{InvocationError, RulesCommandHandler, RulesCommandInput, stage_handler};

/// Native admission for this exact command and social consequence pass, distinct from entry
/// validation. Implementations must check current source rights, controlled actor, permitted
/// requests and complete authored entries under the full checkpoint pins. They perform no I/O.
/// Inventory membership, client selections and a ReactionSourceOwner alone grant no control.
pub trait RelationshipApplicationOwner: ReactionSourceOwner {
    type Refusal;

    fn admit_application(
        &self,
        current: &Checkpoint,
        command: &CommandInput,
        requests: &[ReactionRequest],
        entries: &[ReactionEntry],
    ) -> Result<(), Self::Refusal>;
}

/// Count and retained-capacity bounds for this detached pass. maximum_work reserves the
/// reaction budgets and local actor/directional scans; native owner/inner handler work and
/// canonical validation retain their own bounded contracts and are not charged as reaction work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationshipStagingLimits {
    pub maximum_reactions: usize,
    pub maximum_relationships: usize,
    pub maximum_work: usize,
    pub maximum_inventory_records: usize,
    pub maximum_checkpoint_bytes: usize,
    pub maximum_pass_bytes: usize,
    pub checkpoint: CheckpointLimits,
    pub reaction: ReactionLimits,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RelationshipStagingError<CommandRefusal, SourceRefusal> {
    Capacity,
    Snapshot(CheckpointError),
    UnsupportedInput,
    StaleCommand,
    PendingResolution,
    WrongActor,
    DuplicateDirection,
    Source(SourceRefusal),
    Command(InvocationError<CommandRefusal>),
    Reaction(ReactionError),
    RelationshipConflict,
    InvalidCandidate(CheckpointError),
}

/// Private selected checkpoint and exact recomputed outcomes for the native caller. NoReaction
/// remains typed and does not invalidate the source-admitted base command. Evidence is transient:
/// the current categorical Relationship has no persisted event-consumption/provenance field.
/// Snapshot reload and exact operation receipts are supported; event-only reconstruction and
/// deduplication across distinct operations require the owning model's separate schema gate.
pub struct RelationshipTransition {
    pub checkpoint: Checkpoint,
    pub reactions: Vec<ReactionOutcome>,
}

/// Compose with a real source-selected command handler and invoke through decide_registered_command.
/// Requests and entries are native supplied, never client authored. Reactions use the unchanged
/// current checkpoint: this pass cannot claim that a newly staged event was already committed.
/// The base command must retain existing relationships. Every replacement is rechecked against
/// its original; conflicting siblings or a last reaction/constructor failure discard the pass.
pub struct RelationshipReactionHandler<'a, Handler, Owner> {
    pub command_handler: &'a Handler,
    pub owner: &'a Owner,
    pub current_basis: Basis,
    pub admitted_pins: &'a CheckpointPins,
    pub inventory: ReferenceInventory<'a>,
    pub requests: &'a [ReactionRequest],
    pub entries: &'a [ReactionEntry],
    pub limits: RelationshipStagingLimits,
}

impl<Handler, Owner> RelationshipReactionHandler<'_, Handler, Owner>
where
    Handler: RulesCommandHandler,
    Owner: RelationshipApplicationOwner,
{
    /// Return a fully validated next-revision checkpoint together with private typed outcomes.
    /// Admission and the actual command handler run before any policy validation or react call.
    /// Structural client-command validation and registry selector admission are supplied by
    /// decide_registered_command, which must wrap production use of this handler.
    pub fn stage_with_outcomes(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<RelationshipTransition, RelationshipStagingError<Handler::Rejection, Owner::Refusal>>
    {
        self.check_capacity(current, input.command)?;
        current
            .validate_resume(self.current_basis, self.admitted_pins)
            .map_err(RelationshipStagingError::Snapshot)?;
        let GameInput::Game(command) = input.command else {
            return Err(RelationshipStagingError::UnsupportedInput);
        };
        if command.basis != self.current_basis
            || command.observed_revision != self.current_basis.revision
            || self
                .requests
                .iter()
                .any(|request| request.expected_basis != self.current_basis)
        {
            return Err(RelationshipStagingError::StaleCommand);
        }
        if !current.state().pending.is_empty() {
            return Err(RelationshipStagingError::PendingResolution);
        }
        let actor = match &command.command {
            GameCommand::ProposeAction { actor, .. } => *actor,
            _ => return Err(RelationshipStagingError::UnsupportedInput),
        };
        if !current
            .state()
            .members
            .iter()
            .any(|link| link.member == command.member && link.character == Some(actor))
            || !current
                .state()
                .entities
                .iter()
                .any(|entity| entity.id == actor)
            || !current
                .state()
                .characters
                .iter()
                .any(|character| character.entity == actor && character.owner == command.member)
        {
            return Err(RelationshipStagingError::WrongActor);
        }
        self.owner
            .admit_application(current, command, self.requests, self.entries)
            .map_err(RelationshipStagingError::Source)?;
        let staged = stage_handler(
            self.command_handler,
            self.admitted_pins,
            input,
            current,
            self.limits.maximum_checkpoint_bytes,
        )
        .map_err(RelationshipStagingError::Command)?;
        if staged.state().relationships != current.state().relationships {
            return Err(RelationshipStagingError::RelationshipConflict);
        }
        for (index, relationship) in current.state().relationships.iter().enumerate() {
            if current
                .state()
                .relationships
                .iter()
                .take(index)
                .any(|prior| {
                    prior.subject == relationship.subject && prior.object == relationship.object
                })
            {
                return Err(RelationshipStagingError::DuplicateDirection);
            }
        }
        for (index, request) in self.requests.iter().enumerate() {
            if self
                .requests
                .iter()
                .take(index)
                .any(|prior| prior.npc == request.npc && prior.target == request.target)
            {
                return Err(RelationshipStagingError::DuplicateDirection);
            }
        }
        let policy = ReactionPolicy::new(
            current,
            self.entries,
            self.inventory(),
            self.owner,
            self.limits.reaction,
        )
        .map_err(RelationshipStagingError::Reaction)?;
        let mut reactions = Vec::new();
        reactions
            .try_reserve_exact(self.requests.len())
            .map_err(|_| RelationshipStagingError::Capacity)?;
        for request in self.requests {
            reactions.push(
                react(current, *request, &policy).map_err(RelationshipStagingError::Reaction)?,
            );
        }
        let mut state = staged.state().clone();
        for outcome in &reactions {
            if let ReactionOutcome::Proposed(proposal) = outcome {
                let record = state
                    .relationships
                    .iter_mut()
                    .find(|relationship| {
                        relationship.subject == proposal.original.subject
                            && relationship.object == proposal.original.object
                    })
                    .ok_or(RelationshipStagingError::RelationshipConflict)?;
                if *record != proposal.original {
                    return Err(RelationshipStagingError::RelationshipConflict);
                }
                *record = proposal.proposed.clone();
            }
        }
        let checkpoint = Checkpoint::new(
            staged.schema(),
            staged.basis(),
            staged.pins().clone(),
            state,
            self.inventory(),
            self.limits.checkpoint,
        )
        .map_err(RelationshipStagingError::InvalidCandidate)?;
        Ok(RelationshipTransition {
            checkpoint,
            reactions,
        })
    }

    fn inventory(&self) -> ReferenceInventory<'_> {
        ReferenceInventory {
            rules: self.inventory.rules,
            content: self.inventory.content,
            resources: self.inventory.resources,
            assets: self.inventory.assets,
        }
    }

    fn check_capacity(
        &self,
        current: &Checkpoint,
        input: &GameInput,
    ) -> Result<(), RelationshipStagingError<Handler::Rejection, Owner::Refusal>> {
        let limits = self.limits;
        let relationships = current.state().relationships.len();
        if limits.maximum_reactions == 0
            || limits.maximum_relationships == 0
            || limits.maximum_inventory_records == 0
            || limits.checkpoint.maximum_retained_bytes == 0
            || limits.checkpoint.maximum_retained_bytes > limits.maximum_checkpoint_bytes
            || self.requests.len() > limits.maximum_reactions
            || relationships > limits.maximum_relationships
            || self.entries.len() > limits.reaction.maximum_entries
        {
            return Err(RelationshipStagingError::Capacity);
        }
        let records = [
            self.inventory.rules.len(),
            self.inventory.content.len(),
            self.inventory.resources.len(),
            self.inventory.assets.len(),
        ]
        .into_iter()
        .try_fold(0usize, |count, next| count.checked_add(next))
        .ok_or(RelationshipStagingError::Capacity)?;
        // Reserve the complete policy scan and every react call, plus directional uniqueness.
        let work = self
            .requests
            .len()
            .checked_add(1)
            .and_then(|count| count.checked_mul(limits.reaction.maximum_work))
            .and_then(|work| work.checked_add(relationships.checked_mul(relationships)?))
            .and_then(|work| {
                work.checked_add(self.requests.len().checked_mul(self.requests.len())?)
            })
            .and_then(|work| work.checked_add(current.state().members.len()))
            .and_then(|work| work.checked_add(current.state().entities.len()))
            .and_then(|work| work.checked_add(current.state().characters.len()))
            .and_then(|work| work.checked_add(self.requests.len()))
            .ok_or(RelationshipStagingError::Capacity)?;
        let bytes = current
            .retained_bytes()
            .ok_or(RelationshipStagingError::Capacity)?;
        let pass = limits
            .maximum_checkpoint_bytes
            .checked_mul(3)
            .and_then(|scratch| scratch.checked_add(bytes))
            .and_then(|total| total.checked_add(input.retained_bytes()?))
            .and_then(|total| total.checked_add(limits.reaction.maximum_policy_bytes))
            .and_then(|total| {
                total.checked_add(
                    self.requests
                        .len()
                        .checked_mul(limits.reaction.maximum_proposal_bytes)?,
                )
            })
            .and_then(|total| {
                total.checked_add(
                    self.requests
                        .len()
                        .checked_mul(std::mem::size_of::<ReactionRequest>())?,
                )
            })
            .ok_or(RelationshipStagingError::Capacity)?;
        if records > limits.maximum_inventory_records
            || work > limits.maximum_work
            || bytes > limits.maximum_checkpoint_bytes
            || pass > limits.maximum_pass_bytes
        {
            return Err(RelationshipStagingError::Capacity);
        }
        Ok(())
    }
}

impl<Handler, Owner> RulesCommandHandler for RelationshipReactionHandler<'_, Handler, Owner>
where
    Handler: RulesCommandHandler,
    Owner: RelationshipApplicationOwner,
{
    type Rejection = RelationshipStagingError<Handler::Rejection, Owner::Refusal>;

    fn pins(&self) -> &CheckpointPins {
        self.admitted_pins
    }

    fn bound_source(&self) -> Option<&df_model::checkpoint::RuleReference> {
        self.command_handler.bound_source()
    }

    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        Ok(self.stage_with_outcomes(input, current)?.checkpoint)
    }
}
