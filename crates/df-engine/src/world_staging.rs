//! Source-owned environmental proposals through the registered rules/session boundary.
//! Causes must already be committed. This adapter creates no facts, mechanics or game time.

use df_model::checkpoint::{
    AcceptedDecision, Basis, Checkpoint, CheckpointError, CheckpointLimits, CheckpointPins,
    CommandInput, ContentReference, FactId, GameCommand, GameInput, ReferenceInventory,
    RuleReference,
};
use df_rules::{RulesCommandHandler, RulesCommandInput};
use df_types::RevisionLabel;
use df_world::{
    AdmittedEnvironmentalChange, EnvironmentalDeltaError, EnvironmentalDeltaLimits,
    stage_environmental_delta,
};

/// Compiled native source mapping; possession of these references is not authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentalRegistration {
    pub action: ContentReference,
    pub environment: ContentReference,
    pub source: RuleReference,
    pub policy: RevisionLabel,
}

/// Independently authorize the complete current registration, source rights and actor control.
/// Implementations are pure and bounded. They inspect every proposed replacement and its
/// retained accepted causes, including the original decision policy and permitted audience.
/// Joined membership, an inventory reference, or a client action never grants source authority.
/// Geometry-byte/spatial legality, when required by the source, remains this owner's prerequisite.
pub trait EnvironmentalSourceOwner {
    type Refusal;

    fn admit(
        &self,
        current: &Checkpoint,
        registration: &EnvironmentalRegistration,
        command: &CommandInput,
        changes: &[AdmittedEnvironmentalChange<'_>],
    ) -> Result<(), Self::Refusal>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldStagingLimits {
    pub maximum_checkpoint_bytes: usize,
    pub maximum_pass_bytes: usize,
    pub maximum_inventory_records: usize,
    pub checkpoint: CheckpointLimits,
    pub world: EnvironmentalDeltaLimits,
}

#[derive(Debug, Eq, PartialEq)]
pub enum WorldStagingError<R> {
    Capacity,
    Snapshot(CheckpointError),
    UnsupportedInput,
    StaleCommand,
    PendingResolution,
    AlreadyAccepted,
    MissingProducer,
    InvalidRegistration,
    WrongCommand,
    Source(R),
    World(EnvironmentalDeltaError),
    RevisionExhausted,
    InvalidCandidate(CheckpointError),
}

/// Trusted native request captured independently of client input. Register this handler behind
/// `PreconditionedCommandHandler` with the source-reviewed prerequisites, then invoke
/// `decide_registered_command` inside `SessionEngine`. Only the session owner commits/publishes.
/// `None` is a missing producer; `Some([])` is an explicitly source-admitted no-change request.
pub struct EnvironmentalChangeHandler<'a, Owner> {
    pub owner: &'a Owner,
    pub current_basis: Basis,
    pub admitted_pins: &'a CheckpointPins,
    pub inventory: ReferenceInventory<'a>,
    pub registration: &'a EnvironmentalRegistration,
    pub changes: Option<&'a [AdmittedEnvironmentalChange<'a>]>,
    pub limits: WorldStagingLimits,
}

impl<Owner: EnvironmentalSourceOwner> RulesCommandHandler
    for EnvironmentalChangeHandler<'_, Owner>
{
    type Rejection = WorldStagingError<Owner::Refusal>;

    fn pins(&self) -> &CheckpointPins {
        self.admitted_pins
    }

    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        self.check_capacity(current, input.command)?;
        current
            .validate_resume(self.current_basis, self.admitted_pins)
            .map_err(WorldStagingError::Snapshot)?;
        let GameInput::Game(command) = input.command else {
            return Err(WorldStagingError::UnsupportedInput);
        };
        if !input.supplied_draws.is_empty() {
            return Err(WorldStagingError::UnsupportedInput);
        }
        if command.basis != self.current_basis
            || command.observed_revision != self.current_basis.revision
        {
            return Err(WorldStagingError::StaleCommand);
        }
        if !current.state().pending.is_empty() {
            return Err(WorldStagingError::PendingResolution);
        }
        if current
            .state()
            .decisions
            .iter()
            .any(|decision| decision.operation == command.operation)
        {
            return Err(WorldStagingError::AlreadyAccepted);
        }
        let changes = self.changes.ok_or(WorldStagingError::MissingProducer)?;
        let registration = self.registration;
        if registration.source.catalog != current.pins().rules.catalog
            || !self.inventory.rules.contains(&registration.source)
            || [&registration.action, &registration.environment]
                .into_iter()
                .any(|reference| {
                    reference.package != current.pins().content.package
                        || !self.inventory.content.contains(reference)
                })
            || changes
                .iter()
                .any(|change| change.replacement.definition != registration.environment)
        {
            return Err(WorldStagingError::InvalidRegistration);
        }
        let GameCommand::ProposeAction {
            actor,
            action,
            targets,
            choices,
        } = &command.command
        else {
            return Err(WorldStagingError::WrongCommand);
        };
        if action != &registration.action
            || !choices.is_empty()
            || targets.len() != changes.len()
            || targets
                .iter()
                .zip(changes)
                .any(|(target, change)| *target != change.expected.location)
            || !current
                .state()
                .members
                .iter()
                .any(|link| link.member == command.member && link.character == Some(*actor))
            || !current
                .state()
                .entities
                .iter()
                .any(|entity| entity.id == *actor)
        {
            return Err(WorldStagingError::WrongCommand);
        }
        // Source authority is checked independently before any detached proposal/candidate.
        self.owner
            .admit(current, registration, command, changes)
            .map_err(WorldStagingError::Source)?;
        let proposal = stage_environmental_delta(
            current,
            self.admitted_pins,
            self.current_basis,
            Some(changes),
            self.limits.world,
        )
        .map_err(WorldStagingError::World)?;
        let mut next = self.current_basis;
        next.revision = next
            .revision
            .next_sequence()
            .map_err(|_| WorldStagingError::RevisionExhausted)?;
        let mut state = current.state().clone();
        for replacement in proposal.replacements {
            let record = state
                .continuity
                .environment
                .iter_mut()
                .find(|record| record.location == replacement.location)
                .ok_or(WorldStagingError::World(
                    EnvironmentalDeltaError::UnknownEnvironment,
                ))?;
            *record = replacement;
        }
        state
            .decisions
            .try_reserve_exact(1)
            .map_err(|_| WorldStagingError::Capacity)?;
        // The new operation owns application of the proposal, not the original cause facts.
        // Keeping its fact list empty preserves the immutable declaration of each prior cause.
        state.decisions.push(AcceptedDecision {
            operation: command.operation,
            revision: next.revision,
            facts: vec![],
            draws: vec![],
            effects: vec![],
            source_policy: registration.policy.clone(),
            semantic_output: None,
        });
        Checkpoint::new(
            current.schema(),
            next,
            current.pins().clone(),
            state,
            ReferenceInventory {
                rules: self.inventory.rules,
                content: self.inventory.content,
                resources: self.inventory.resources,
                assets: self.inventory.assets,
            },
            self.limits.checkpoint,
        )
        .map_err(WorldStagingError::InvalidCandidate)
    }
}

impl<Owner: EnvironmentalSourceOwner> EnvironmentalChangeHandler<'_, Owner> {
    fn check_capacity(
        &self,
        current: &Checkpoint,
        input: &GameInput,
    ) -> Result<(), WorldStagingError<Owner::Refusal>> {
        let limits = self.limits;
        let changes = self.changes.unwrap_or(&[]);
        if limits.maximum_checkpoint_bytes == 0
            || limits.maximum_inventory_records == 0
            || limits.checkpoint.maximum_retained_bytes == 0
            || limits.checkpoint.maximum_retained_bytes > limits.maximum_checkpoint_bytes
            || limits.world.input_records == 0
            || limits.world.input_records > 4096
            || limits.world.changes == 0
            || limits.world.changes > limits.world.input_records
            || limits.world.output_bytes == 0
            || changes.len() > limits.world.changes
        {
            return Err(WorldStagingError::Capacity);
        }
        let mut request_bytes = std::mem::size_of::<EnvironmentalRegistration>();
        let mut add = |bytes: usize| -> Result<(), WorldStagingError<Owner::Refusal>> {
            request_bytes = request_bytes
                .checked_add(bytes)
                .ok_or(WorldStagingError::Capacity)?;
            if request_bytes > limits.maximum_pass_bytes {
                return Err(WorldStagingError::Capacity);
            }
            Ok(())
        };
        for label in [
            &self.registration.action.package,
            &self.registration.action.entry,
            &self.registration.environment.package,
            &self.registration.environment.entry,
            &self.registration.source.catalog,
            &self.registration.source.source,
            &self.registration.source.entry,
            &self.registration.source.clause,
            &self.registration.policy,
        ] {
            add(label.retained_heap_bytes())?;
        }
        for change in changes {
            add(std::mem::size_of::<AdmittedEnvironmentalChange<'_>>())?;
            for environment in [change.expected, change.replacement] {
                if environment.change_facts.len() > limits.world.input_records {
                    return Err(WorldStagingError::Capacity);
                }
                add(std::mem::size_of_val(environment))?;
                add(environment.definition.package.retained_heap_bytes())?;
                add(environment.definition.entry.retained_heap_bytes())?;
                add(environment
                    .change_facts
                    .len()
                    .checked_mul(std::mem::size_of::<FactId>())
                    .ok_or(WorldStagingError::Capacity)?)?;
            }
            if change.geometry.source_facts.len() > limits.world.input_records {
                return Err(WorldStagingError::Capacity);
            }
            add(std::mem::size_of_val(change.geometry))?;
            for label in [
                &change.geometry.revision,
                &change.geometry.geometry.key,
                &change.geometry.canonical_pack,
            ] {
                add(label.retained_heap_bytes())?;
            }
            add(change
                .geometry
                .source_facts
                .len()
                .checked_mul(std::mem::size_of::<FactId>())
                .ok_or(WorldStagingError::Capacity)?)?;
        }
        let records = [
            self.inventory.rules.len(),
            self.inventory.content.len(),
            self.inventory.resources.len(),
            self.inventory.assets.len(),
        ]
        .into_iter()
        .try_fold(0usize, |count, next| count.checked_add(next))
        .ok_or(WorldStagingError::Capacity)?;
        let current_bytes = current
            .retained_bytes()
            .ok_or(WorldStagingError::Capacity)?;
        let total = limits
            .maximum_checkpoint_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(current_bytes))
            .and_then(|bytes| bytes.checked_add(limits.world.output_bytes))
            .and_then(|bytes| bytes.checked_add(request_bytes))
            .and_then(|bytes| bytes.checked_add(input.retained_bytes()?))
            .ok_or(WorldStagingError::Capacity)?;
        if records > limits.maximum_inventory_records
            || current_bytes > limits.maximum_checkpoint_bytes
            || total > limits.maximum_pass_bytes
        {
            return Err(WorldStagingError::Capacity);
        }
        Ok(())
    }
}
