//! Pure categorical reactions to an NPC's recorded perception of a committed event.
use std::mem::size_of;

use df_model::checkpoint::{
    AcceptedDecision, Basis, Checkpoint, ContentPins, ContentReference, EntityId, FactId,
    FactValue, GameFact, RecordId, ReferenceInventory, Relationship,
};
use df_types::{OperationId, RevisionLabel, SessionRevision};

/// One source-authored categorical transition, never a mechanical bonus or action grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactionEntry {
    pub source: ContentReference,
    pub event: ContentReference,
    pub personality: ContentReference,
    pub motivation: ContentReference,
    pub relationship_policy: ContentReference,
    pub from_state: RevisionLabel,
    pub to_state: RevisionLabel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReactionSourceRefusal {
    NotAdmitted,
    AccessDenied,
    UnsupportedPolicy,
    StaleAdmission,
}

/// Native source ownership supplies current source/rights/eligibility admission.
/// Implementations read an already bounded immutable admission view, perform no I/O,
/// and validate the exact entry, basis and content pins. Inventory membership alone
/// does not authorize a transition or establish that its categorical labels are legal.
pub trait ReactionSourceOwner {
    fn validate_entry(
        &self,
        basis: Basis,
        pins: &ContentPins,
        entry: &ReactionEntry,
    ) -> Result<(), ReactionSourceRefusal>;
}

/// Explicit caller-admitted bounds. Work units count examined records and nested
/// reference slots, including worst-case inventory and policy matching scans.
/// Byte bounds charge inline representation and retained label capacities before clones.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReactionLimits {
    pub maximum_entries: usize,
    pub maximum_policy_bytes: usize,
    pub maximum_work: usize,
    pub maximum_proposal_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReactionLimit {
    Entries,
    PolicyBytes,
    Work,
    ProposalBytes,
}

/// Source-validated borrowed policy bound to one exact current checkpoint basis.
/// Construct from trusted native admission, then recheck that admission during react.
/// This local request view is not a persistent content catalog or rights authority.
pub struct ReactionPolicy<'a> {
    basis: Basis,
    pins: &'a ContentPins,
    entries: &'a [ReactionEntry],
    admitted: &'a [ContentReference],
    source_owner: &'a dyn ReactionSourceOwner,
    limits: ReactionLimits,
}

impl<'a> ReactionPolicy<'a> {
    pub fn new(
        checkpoint: &'a Checkpoint,
        entries: &'a [ReactionEntry],
        inventory: ReferenceInventory<'a>,
        source_owner: &'a dyn ReactionSourceOwner,
        limits: ReactionLimits,
    ) -> Result<Self, ReactionError> {
        if entries.len() > limits.maximum_entries {
            return Err(ReactionError::LimitExceeded(ReactionLimit::Entries));
        }
        let policy = Self {
            basis: checkpoint.basis(),
            pins: &checkpoint.pins().content,
            entries,
            admitted: inventory.content,
            source_owner,
            limits,
        };
        let mut work = WorkBudget(limits.maximum_work);
        policy.validate(&mut work)?;
        Ok(policy)
    }

    fn validate(&self, work: &mut WorkBudget) -> Result<(), ReactionError> {
        let mut bytes = self
            .entries
            .len()
            .checked_mul(size_of::<ReactionEntry>())
            .ok_or(ReactionError::LimitExceeded(ReactionLimit::PolicyBytes))?;
        for entry in self.entries {
            work.charge(1)?;
            bytes = bytes
                .checked_add(
                    entry_heap_bytes(entry)
                        .ok_or(ReactionError::LimitExceeded(ReactionLimit::PolicyBytes))?,
                )
                .ok_or(ReactionError::LimitExceeded(ReactionLimit::PolicyBytes))?;
            if bytes > self.limits.maximum_policy_bytes {
                return Err(ReactionError::LimitExceeded(ReactionLimit::PolicyBytes));
            }
            for reference in [
                &entry.source,
                &entry.event,
                &entry.personality,
                &entry.motivation,
                &entry.relationship_policy,
            ] {
                self.validate_reference(reference, work)?;
            }
            self.source_owner
                .validate_entry(self.basis, self.pins, entry)
                .map_err(ReactionError::SourceRefused)?;
        }
        Ok(())
    }

    fn validate_reference(
        &self,
        reference: &ContentReference,
        work: &mut WorkBudget,
    ) -> Result<(), ReactionError> {
        work.charge(self.admitted.len())?;
        if reference.package != self.pins.package || !self.admitted.contains(reference) {
            return Err(ReactionError::SourceUnavailable);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReactionRequest {
    pub expected_basis: Basis,
    pub npc: EntityId,
    pub target: EntityId,
    pub event: FactId,
    pub witness: RecordId,
}

/// Safe classifications contain no private social state, claim text or evidence IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReactionError {
    StaleBasis,
    ContentMismatch,
    SourceUnavailable,
    SourceRefused(ReactionSourceRefusal),
    LimitExceeded(ReactionLimit),
    UnknownNpc,
    UnknownEvent,
    UnacceptedEvent,
    UnknownRelationship,
    AmbiguousRelationship,
    InvalidPerceptionTime,
    AmbiguousPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoReactionReason {
    NotKnown,
    NotWitnessed,
    UnsupportedEvent,
    UnrelatedTarget,
    NoMatchingPolicy,
    UnchangedState,
}

/// Private staged replacement and canonical causal evidence, for the engine/session.
/// This is never listener-safe expression, a prompt, disclosure, fact, or committed delta.
#[derive(Clone, Eq, PartialEq)]
pub struct ReactionProposal {
    pub expected_basis: Basis,
    pub content_pins: ContentPins,
    pub original: Relationship,
    pub proposed: Relationship,
    pub policy_entry: ReactionEntry,
    pub event: FactId,
    pub cause: Option<FactId>,
    pub witness: RecordId,
    pub accepted_operation: OperationId,
    pub accepted_revision: SessionRevision,
}

impl std::fmt::Debug for ReactionProposal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReactionProposal")
            .field("expected_basis", &self.expected_basis)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReactionOutcome {
    Proposed(Box<ReactionProposal>),
    NoReaction(NoReactionReason),
}

/// Propose at most one exact authored directional categorical state change.
/// A committed ContentEvent must be known by the actual NPC and have its exact
/// recorded witness. Beliefs, memories, dialogue and global fact presence cannot
/// replace these prerequisites. Duplicate matching entries refuse in every order.
/// No state, truth, debt, mechanical action, knowledge, source or access is granted.
/// Engine selection and current source/basis revalidation at sole session commit
/// remain necessary; repeated proposal equality is not committed idempotency.
pub fn react(
    checkpoint: &Checkpoint,
    request: ReactionRequest,
    policy: &ReactionPolicy<'_>,
) -> Result<ReactionOutcome, ReactionError> {
    if checkpoint.basis() != request.expected_basis || checkpoint.basis() != policy.basis {
        return Err(ReactionError::StaleBasis);
    }
    if checkpoint.pins().content != *policy.pins {
        return Err(ReactionError::ContentMismatch);
    }
    let mut work = WorkBudget(policy.limits.maximum_work);
    policy.validate(&mut work)?;
    let state = checkpoint.state();
    work.charge(state.continuity.npcs.len())?;
    let npc = state
        .continuity
        .npcs
        .iter()
        .find(|npc| npc.entity == request.npc)
        .ok_or(ReactionError::UnknownNpc)?;
    work.charge(state.facts.len())?;
    let event = state
        .facts
        .iter()
        .find(|fact| fact.id == request.event)
        .ok_or(ReactionError::UnknownEvent)?;
    let accepted = accepted_decision(checkpoint, event, &mut work)?;
    if let Some(cause) = event.cause {
        work.charge(state.facts.len())?;
        let cause = state
            .facts
            .iter()
            .find(|fact| fact.id == cause)
            .ok_or(ReactionError::UnacceptedEvent)?;
        accepted_decision(checkpoint, cause, &mut work)?;
    }
    work.charge(npc.known_facts.len())?;
    if !npc.known_facts.contains(&event.id) {
        return Ok(ReactionOutcome::NoReaction(NoReactionReason::NotKnown));
    }
    work.charge(state.continuity.witnesses.len())?;
    let Some(witness) = state.continuity.witnesses.iter().find(|witness| {
        witness.id == request.witness && witness.observer == npc.entity && witness.fact == event.id
    }) else {
        return Ok(ReactionOutcome::NoReaction(NoReactionReason::NotWitnessed));
    };
    policy.validate_reference(&witness.source, &mut work)?;
    if witness.perceived_at.ticks_per_second != state.logical_time.ticks_per_second
        || witness.perceived_at.ticks > state.logical_time.ticks
    {
        return Err(ReactionError::InvalidPerceptionTime);
    }
    let FactValue::ContentEvent {
        definition,
        subjects,
    } = &event.value
    else {
        return Ok(ReactionOutcome::NoReaction(
            NoReactionReason::UnsupportedEvent,
        ));
    };
    work.charge(subjects.len())?;
    if !subjects.contains(&request.target) {
        return Ok(ReactionOutcome::NoReaction(
            NoReactionReason::UnrelatedTarget,
        ));
    }
    work.charge(state.relationships.len())?;
    let mut relationships = state.relationships.iter().filter(|relationship| {
        relationship.subject == npc.entity && relationship.object == request.target
    });
    let original = relationships
        .next()
        .ok_or(ReactionError::UnknownRelationship)?;
    if relationships.next().is_some() {
        return Err(ReactionError::AmbiguousRelationship);
    }
    work.charge(policy.entries.len())?;
    work.charge(
        policy
            .entries
            .len()
            .checked_mul(npc.motivations.len())
            .ok_or(ReactionError::LimitExceeded(ReactionLimit::Work))?,
    )?;
    let mut matches = policy.entries.iter().filter(|entry| {
        entry.event == *definition
            && entry.personality == npc.personality
            && npc.motivations.contains(&entry.motivation)
            && entry.relationship_policy == original.policy
            && entry.from_state == original.state
    });
    let Some(entry) = matches.next() else {
        return Ok(ReactionOutcome::NoReaction(
            NoReactionReason::NoMatchingPolicy,
        ));
    };
    if matches.next().is_some() {
        return Err(ReactionError::AmbiguousPolicy);
    }
    if entry.to_state == original.state {
        return Ok(ReactionOutcome::NoReaction(
            NoReactionReason::UnchangedState,
        ));
    }
    let bytes = proposal_bytes(original, entry, policy.pins)
        .ok_or(ReactionError::LimitExceeded(ReactionLimit::ProposalBytes))?;
    if bytes > policy.limits.maximum_proposal_bytes {
        return Err(ReactionError::LimitExceeded(ReactionLimit::ProposalBytes));
    }
    let mut proposed = original.clone();
    proposed.state = entry.to_state.clone();
    Ok(ReactionOutcome::Proposed(Box::new(ReactionProposal {
        expected_basis: request.expected_basis,
        content_pins: policy.pins.clone(),
        original: original.clone(),
        proposed,
        policy_entry: entry.clone(),
        event: event.id,
        cause: event.cause,
        witness: witness.id,
        accepted_operation: accepted.operation,
        accepted_revision: accepted.revision,
    })))
}

struct WorkBudget(usize);

impl WorkBudget {
    fn charge(&mut self, units: usize) -> Result<(), ReactionError> {
        self.0 = self
            .0
            .checked_sub(units)
            .ok_or(ReactionError::LimitExceeded(ReactionLimit::Work))?;
        Ok(())
    }
}

fn accepted_decision<'a>(
    checkpoint: &'a Checkpoint,
    fact: &GameFact,
    work: &mut WorkBudget,
) -> Result<&'a AcceptedDecision, ReactionError> {
    work.charge(checkpoint.state().decisions.len())?;
    for decision in &checkpoint.state().decisions {
        work.charge(decision.facts.len())?;
    }
    checkpoint
        .state()
        .decisions
        .iter()
        .find(|decision| {
            decision.operation == fact.operation
                && decision.revision == fact.revision
                && decision.facts.get(fact.ordinal as usize) == Some(&fact.id)
        })
        .ok_or(ReactionError::UnacceptedEvent)
}

fn content_heap_bytes(reference: &ContentReference) -> Option<usize> {
    reference
        .package
        .retained_heap_bytes()
        .checked_add(reference.entry.retained_heap_bytes())
}

fn entry_heap_bytes(entry: &ReactionEntry) -> Option<usize> {
    [
        &entry.source,
        &entry.event,
        &entry.personality,
        &entry.motivation,
        &entry.relationship_policy,
    ]
    .iter()
    .try_fold(0usize, |bytes, reference| {
        bytes.checked_add(content_heap_bytes(reference)?)
    })?
    .checked_add(entry.from_state.retained_heap_bytes())?
    .checked_add(entry.to_state.retained_heap_bytes())
}

fn proposal_bytes(
    original: &Relationship,
    entry: &ReactionEntry,
    pins: &ContentPins,
) -> Option<usize> {
    size_of::<ReactionOutcome>()
        .checked_add(size_of::<ReactionProposal>())?
        .checked_add(pins.content.retained_heap_bytes())?
        .checked_add(pins.package.retained_heap_bytes())?
        .checked_add(content_heap_bytes(&original.policy)?.checked_mul(2)?)?
        .checked_add(original.state.retained_heap_bytes())?
        .checked_add(entry.to_state.retained_heap_bytes())?
        .checked_add(entry_heap_bytes(entry)?)
}
