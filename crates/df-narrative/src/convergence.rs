//! Source-admitted convergence and persisted intervention units over canonical checkpoints.
use df_content::narrative::{
    NarrativeBudgetPolicy as ContentBudgetPolicy, NarrativeBudgetRule as ContentBudgetRule,
};
use df_model::checkpoint::{
    AudienceScope, Basis, Checkpoint, CheckpointError, CheckpointLimits, CheckpointPins,
    ContentReference, FactId, FactValue, GameFact, NpcState, ReferenceInventory, WorldEntity,
};
use df_types::{MemberId, RevisionLabel};

type NarrativeBudgetPolicy = ContentBudgetPolicy<ContentReference>;
type NarrativeBudgetRule = ContentBudgetRule<ContentReference>;

use crate::{
    BeatSelectionError, BeatSelectionLimits, CheckpointBeatRequest, stage_checkpoint_beat_selection,
};

/// A canonical committed event receipt, not a proposed new fact or an arbitrary unit delta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NarrativeBudgetChange {
    StrongIntervention(FactId),
    ApprovedFreePlay(FactId),
}

pub struct NarrativeBudgetRequest<'a> {
    pub expected_basis: Basis,
    pub admitted_pins: &'a CheckpointPins,
    pub policy: &'a NarrativeBudgetPolicy,
    pub expected_policy: &'a NarrativeBudgetPolicy,
    pub source_policy: &'a RevisionLabel,
    pub change: NarrativeBudgetChange,
    pub inventory: ReferenceInventory<'a>,
    pub checkpoint_limits: CheckpointLimits,
}

/// Traversal/record capacity remains separate from the persisted policy-unit balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NarrativeBudgetLimits {
    pub records: usize,
    pub work: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NarrativeBudgetOutcome {
    Applied { before: u64, after: u64 },
    AlreadyConsumed,
}

#[derive(Debug)]
pub struct NarrativeBudgetProposal {
    pub checkpoint: Checkpoint,
    pub outcome: NarrativeBudgetOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NarrativeBudgetError {
    Binding(CheckpointError),
    Capacity,
    Policy,
    Source,
    UnsupportedEvent,
    InvalidBalance,
    InsufficientBudget,
    InvalidCandidate(CheckpointError),
}

struct Work(usize);
impl Work {
    fn charge(&mut self) -> Result<(), NarrativeBudgetError> {
        self.0 = self
            .0
            .checked_sub(1)
            .ok_or(NarrativeBudgetError::Capacity)?;
        Ok(())
    }
    fn contains<T: PartialEq>(
        &mut self,
        values: &[T],
        value: &T,
    ) -> Result<bool, NarrativeBudgetError> {
        for candidate in values {
            self.charge()?;
            if candidate == value {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Composes a source-classified budget receipt with an already staged authoritative decision.
/// The new terminal event must be the next operation's exact narrative-consumption receipt.
/// Its existing fence is retained, so the native thread ledger is never given an unrelated
/// historical budget receipt. Session's operation fence commits this whole candidate once.
/// A committed/replayed event cannot enter this path: the basis and immutable history must
/// prove exactly one fresh accepted decision, and its balance must still equal `before`.
pub fn stage_candidate_narrative_budget(
    before: &Checkpoint,
    candidate: &Checkpoint,
    request: NarrativeBudgetRequest<'_>,
    limits: NarrativeBudgetLimits,
) -> Result<NarrativeBudgetProposal, NarrativeBudgetError> {
    before
        .validate_resume(request.expected_basis, request.admitted_pins)
        .map_err(NarrativeBudgetError::Binding)?;
    let mut next = before.basis();
    next.revision = next
        .revision
        .next_sequence()
        .map_err(|_| NarrativeBudgetError::Source)?;
    candidate
        .validate_resume(next, request.admitted_pins)
        .map_err(NarrativeBudgetError::Binding)?;
    let old = before.state();
    let staged = candidate.state();
    if before.schema() != candidate.schema()
        || before
            .retained_bytes()
            .is_none_or(|n| n > request.checkpoint_limits.maximum_retained_bytes)
        || candidate
            .retained_bytes()
            .is_none_or(|n| n > request.checkpoint_limits.maximum_retained_bytes)
        || [
            old.facts.len(),
            old.decisions.len(),
            old.narrative.accepted_facts.len(),
            staged.facts.len(),
            staged.decisions.len(),
            staged.narrative.accepted_facts.len(),
        ]
        .into_iter()
        .any(|n| n > limits.records)
    {
        return Err(NarrativeBudgetError::Capacity);
    }
    let id = match request.change {
        NarrativeBudgetChange::StrongIntervention(id)
        | NarrativeBudgetChange::ApprovedFreePlay(id) => id,
    };
    if old.decisions.len().checked_add(1) != Some(staged.decisions.len())
        || !staged.decisions.starts_with(&old.decisions)
        || staged.facts.len() <= old.facts.len()
        || !staged.facts.starts_with(&old.facts)
        || !staged.draws.starts_with(&old.draws)
        || old.narrative.accepted_facts.len().checked_add(1)
            != Some(staged.narrative.accepted_facts.len())
        || !staged
            .narrative
            .accepted_facts
            .starts_with(&old.narrative.accepted_facts)
        || staged.narrative.accepted_facts.last() != Some(&id)
        || staged.narrative.remaining_budget != old.narrative.remaining_budget
    {
        return Err(NarrativeBudgetError::Source);
    }
    let mut work = Work(limits.work);
    let event = accepted(candidate, id, request.source_policy, &mut work)?;
    let decision = staged
        .decisions
        .last()
        .ok_or(NarrativeBudgetError::Source)?;
    if event.revision != next.revision
        || event.operation != decision.operation
        || staged.facts.last().map(|fact| fact.id) != Some(id)
        || decision.facts.last() != Some(&id)
    {
        return Err(NarrativeBudgetError::Source);
    }
    // Only the detached staging view temporarily removes the fresh receipt. The ordinary
    // budget validator restores that identical final ledger while applying the unit delta.
    let mut state = staged.clone();
    if state.narrative.accepted_facts.pop() != Some(id) {
        return Err(NarrativeBudgetError::Source);
    }
    let pending = Checkpoint::new(
        candidate.schema(),
        candidate.basis(),
        candidate.pins().clone(),
        state,
        request.inventory,
        request.checkpoint_limits,
    )
    .map_err(NarrativeBudgetError::InvalidCandidate)?;
    stage_checkpoint_narrative_budget(
        &pending,
        NarrativeBudgetRequest {
            expected_basis: next,
            ..request
        },
        NarrativeBudgetLimits {
            records: limits.records,
            work: work.0,
        },
    )
}

fn accepted<'a>(
    current: &'a Checkpoint,
    id: FactId,
    source_policy: &RevisionLabel,
    work: &mut Work,
) -> Result<&'a GameFact, NarrativeBudgetError> {
    let mut found = None;
    for fact in &current.state().facts {
        work.charge()?;
        if fact.id == id {
            found = Some(fact);
            break;
        }
    }
    let fact = found.ok_or(NarrativeBudgetError::Source)?;
    for decision in &current.state().decisions {
        work.charge()?;
        if decision.operation == fact.operation && decision.revision == fact.revision {
            if decision.source_policy != *source_policy
                || decision.facts.get(fact.ordinal as usize) != Some(&fact.id)
            {
                return Err(NarrativeBudgetError::Source);
            }
            return Ok(fact);
        }
    }
    Err(NarrativeBudgetError::Source)
}

fn validate_policy(
    current: &Checkpoint,
    policy: &NarrativeBudgetPolicy,
    expected: &NarrativeBudgetPolicy,
    inventory: ReferenceInventory<'_>,
    limits: NarrativeBudgetLimits,
    work: &mut Work,
) -> Result<(), NarrativeBudgetError> {
    if [
        policy.strong_events.len(),
        policy.free_play_events.len(),
        expected.strong_events.len(),
        expected.free_play_events.len(),
    ]
    .into_iter()
    .any(|count| count > limits.records)
    {
        return Err(NarrativeBudgetError::Capacity);
    }
    if policy != expected || policy.maximum == 0 {
        return Err(NarrativeBudgetError::Policy);
    }
    let rules = policy.strong_events.iter().chain(&policy.free_play_events);
    for reference in
        std::iter::once(&policy.definition).chain(rules.clone().map(|rule| &rule.event))
    {
        work.charge()?;
        if reference.package != current.pins().content.package
            || !work.contains(inventory.content, reference)?
        {
            return Err(NarrativeBudgetError::Policy);
        }
    }
    for (position, rule) in rules.clone().enumerate() {
        work.charge()?;
        if rule.units == 0 || rule.units > policy.maximum {
            return Err(NarrativeBudgetError::Policy);
        }
        for earlier in rules.clone().take(position) {
            work.charge()?;
            if earlier.event == rule.event {
                return Err(NarrativeBudgetError::Policy);
            }
        }
    }
    if current.state().narrative.remaining_budget > policy.maximum {
        return Err(NarrativeBudgetError::InvalidBalance);
    }
    Ok(())
}

fn rule<'a>(
    rules: &'a [NarrativeBudgetRule],
    event: &ContentReference,
    work: &mut Work,
) -> Result<&'a NarrativeBudgetRule, NarrativeBudgetError> {
    for rule in rules {
        work.charge()?;
        if rule.event == *event {
            return Ok(rule);
        }
    }
    Err(NarrativeBudgetError::UnsupportedEvent)
}

/// Stages one source-approved budget receipt. Session commits the returned checkpoint atomically.
///
/// `accepted_facts` is the canonical narrative-consumption fence. A receipt consumed by another
/// narrative pass cannot also mint credit or spend twice. Approved free play is an exact admitted
/// content event, never elapsed time, rest completion, disconnect or an untrusted activity label.
/// Replenishment retains the actual capped balance; it never modifies World time or resources.
pub fn stage_checkpoint_narrative_budget(
    current: &Checkpoint,
    request: NarrativeBudgetRequest<'_>,
    limits: NarrativeBudgetLimits,
) -> Result<NarrativeBudgetProposal, NarrativeBudgetError> {
    current
        .validate_resume(request.expected_basis, request.admitted_pins)
        .map_err(NarrativeBudgetError::Binding)?;
    let state = current.state();
    if current
        .retained_bytes()
        .is_none_or(|bytes| bytes > request.checkpoint_limits.maximum_retained_bytes)
        || [
            state.facts.len(),
            state.decisions.len(),
            state.narrative.accepted_facts.len(),
            request.inventory.content.len(),
        ]
        .into_iter()
        .any(|count| count > limits.records)
    {
        return Err(NarrativeBudgetError::Capacity);
    }
    let mut work = Work(limits.work);
    validate_policy(
        current,
        request.policy,
        request.expected_policy,
        request.inventory,
        limits,
        &mut work,
    )?;
    let (id, rules, spending) = match request.change {
        NarrativeBudgetChange::StrongIntervention(id) => (id, &request.policy.strong_events, true),
        NarrativeBudgetChange::ApprovedFreePlay(id) => {
            (id, &request.policy.free_play_events, false)
        }
    };
    let fact = accepted(current, id, request.source_policy, &mut work)?;
    let FactValue::ContentEvent { definition, .. } = &fact.value else {
        return Err(NarrativeBudgetError::Source);
    };
    let admitted = rule(rules, definition, &mut work)?;
    if work.contains(&state.narrative.accepted_facts, &id)? {
        return Ok(NarrativeBudgetProposal {
            checkpoint: current.clone(),
            outcome: NarrativeBudgetOutcome::AlreadyConsumed,
        });
    }
    if state.narrative.accepted_facts.len() >= limits.records {
        return Err(NarrativeBudgetError::Capacity);
    }
    let before = state.narrative.remaining_budget;
    let after = if spending {
        before
            .checked_sub(admitted.units)
            .ok_or(NarrativeBudgetError::InsufficientBudget)?
    } else {
        // Both operands are bounded by maximum, including u64::MAX: no overflowing addition.
        before + admitted.units.min(request.policy.maximum - before)
    };
    let mut candidate = state.clone();
    candidate.narrative.remaining_budget = after;
    candidate.narrative.accepted_facts.push(id);
    let checkpoint = Checkpoint::new(
        current.schema(),
        current.basis(),
        current.pins().clone(),
        candidate,
        request.inventory,
        request.checkpoint_limits,
    )
    .map_err(NarrativeBudgetError::InvalidCandidate)?;
    Ok(NarrativeBudgetProposal {
        checkpoint,
        outcome: NarrativeBudgetOutcome::Applied { before, after },
    })
}

/// Optional delivery is already admitted by the native/Interaction owner. Narrative rechecks
/// exact current NPC identity, place, motivation, knowledge and witnessed chronology before use.
/// Omission means no NPC disclosure proposal, not permission to fabricate an NPC's knowledge.
pub struct AdmittedConvergenceDelivery<'a> {
    pub npc: &'a NpcState,
    pub world: &'a WorldEntity,
    pub motivation: &'a ContentReference,
    pub disclosed: &'a [FactId],
}

pub struct CheckpointConvergenceRequest<'a> {
    pub beat: CheckpointBeatRequest<'a>,
    pub budget: &'a NarrativeBudgetPolicy,
    pub expected_budget: &'a NarrativeBudgetPolicy,
    /// Required for a selection classified strong by its pinned policy. This receipt must be
    /// one of that alternative's actual accepted causes, with that exact source event.
    pub strong_receipt: Option<FactId>,
    pub delivery: Option<AdmittedConvergenceDelivery<'a>>,
}

#[derive(Clone, Copy)]
pub struct ConvergenceLimits {
    pub beats: BeatSelectionLimits,
    pub budget: NarrativeBudgetLimits,
    pub maximum_evidence_bytes: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ConvergenceError {
    Beat(BeatSelectionError),
    Budget(NarrativeBudgetError),
    MissingInterventionReceipt,
    PreviouslyConsumed,
    Delivery,
    Capacity,
}

/// All entries are exact source references. There are no generated facts, score defaults, forced
/// actions or new mechanical outcomes. The native owner retains this evidence with its decision.
#[derive(Debug)]
pub struct CheckpointConvergenceProposal {
    pub checkpoint: Checkpoint,
    pub chosen: ContentReference,
    pub relevant_alternatives: Vec<ContentReference>,
    pub causes: Vec<FactId>,
    pub budget_policy: ContentReference,
}

fn evidence_bytes(proposal: &CheckpointConvergenceProposal) -> Option<usize> {
    // The detached checkpoint payload has its own CheckpointLimits byte bound.
    let mut bytes = std::mem::size_of::<CheckpointConvergenceProposal>()
        .checked_add(
            proposal
                .relevant_alternatives
                .capacity()
                .checked_mul(std::mem::size_of::<ContentReference>())?,
        )?
        .checked_add(
            proposal
                .causes
                .capacity()
                .checked_mul(std::mem::size_of::<FactId>())?,
        )?;
    for reference in std::iter::once(&proposal.chosen)
        .chain(std::iter::once(&proposal.budget_policy))
        .chain(&proposal.relevant_alternatives)
    {
        bytes = bytes
            .checked_add(reference.package.retained_heap_bytes())?
            .checked_add(reference.entry.retained_heap_bytes())?;
    }
    Some(bytes)
}

fn permitted(
    audience: &AudienceScope,
    member: MemberId,
    work: &mut Work,
) -> Result<bool, ConvergenceError> {
    match audience {
        AudienceScope::Shared => Ok(true),
        AudienceScope::Members(members) => work
            .contains(members, &member)
            .map_err(ConvergenceError::Budget),
        AudienceScope::Host => Ok(false),
    }
}

fn delivery(
    current: &Checkpoint,
    admitted: AdmittedConvergenceDelivery<'_>,
    member: MemberId,
    source_policy: &RevisionLabel,
    limits: NarrativeBudgetLimits,
    work: &mut Work,
) -> Result<(), ConvergenceError> {
    let state = current.state();
    if admitted.npc.entity != admitted.world.id
        || admitted.disclosed.is_empty()
        || admitted.world.location.is_none()
        || [
            state.members.len(),
            state.continuity.npcs.len(),
            state.entities.len(),
            state.continuity.witnesses.len(),
            admitted.disclosed.len(),
            admitted.npc.motivations.len(),
            admitted.npc.known_facts.len(),
        ]
        .into_iter()
        .any(|count| count > limits.records)
        || !work
            .contains(&state.continuity.npcs, admitted.npc)
            .map_err(ConvergenceError::Budget)?
        || !work
            .contains(&state.entities, admitted.world)
            .map_err(ConvergenceError::Budget)?
        || !work
            .contains(&admitted.npc.motivations, admitted.motivation)
            .map_err(ConvergenceError::Budget)?
    {
        return Err(ConvergenceError::Delivery);
    }
    let mut co_located = false;
    for link in &state.members {
        work.charge().map_err(ConvergenceError::Budget)?;
        if link.member == member {
            for world in &state.entities {
                work.charge().map_err(ConvergenceError::Budget)?;
                if Some(world.id) == link.character && world.location == admitted.world.location {
                    co_located = true;
                    break;
                }
            }
            break;
        }
    }
    if !co_located {
        return Err(ConvergenceError::Delivery);
    }
    for id in admitted.disclosed {
        let fact = accepted(current, *id, source_policy, work).map_err(ConvergenceError::Budget)?;
        if !work
            .contains(&admitted.npc.known_facts, id)
            .map_err(ConvergenceError::Budget)?
            || !permitted(&fact.audience, member, work)?
        {
            return Err(ConvergenceError::Delivery);
        }
        let mut witnessed = false;
        for witness in &state.continuity.witnesses {
            work.charge().map_err(ConvergenceError::Budget)?;
            if witness.observer == admitted.npc.entity
                && witness.fact == *id
                && witness.perceived_at.ticks_per_second == state.logical_time.ticks_per_second
                && witness.perceived_at.ticks <= state.logical_time.ticks
            {
                witnessed = true;
                break;
            }
        }
        if !witnessed {
            return Err(ConvergenceError::Delivery);
        }
    }
    Ok(())
}

/// Validates every retained relevant alternative against the same immutable causal checkpoint,
/// then stages only the explicit selection through the canonical beat validator. Unselected or
/// refused alternatives never run, spend units, emit facts or erase a player's intent.
/// Strong selections require and consume their exact policy-classified event receipt in this
/// same candidate; free-play credit uses the separate receipt pass above. Earned routes cannot
/// be mislabeled strong merely to turn work limits into an intervention balance.
pub fn stage_checkpoint_convergence(
    current: &Checkpoint,
    request: CheckpointConvergenceRequest<'_>,
    limits: ConvergenceLimits,
) -> Result<CheckpointConvergenceProposal, ConvergenceError> {
    let mut work = Work(limits.budget.work);
    if current.state().facts.len() > limits.budget.records
        || current.state().decisions.len() > limits.budget.records
        || request.beat.inventory.content.len() > limits.budget.records
    {
        return Err(ConvergenceError::Capacity);
    }
    validate_policy(
        current,
        request.budget,
        request.expected_budget,
        request.beat.inventory,
        limits.budget,
        &mut work,
    )
    .map_err(ConvergenceError::Budget)?;
    if let Some(admitted) = request.delivery {
        delivery(
            current,
            admitted,
            request.beat.recipient,
            request.beat.policy,
            limits.budget,
            &mut work,
        )?;
    }
    let mut bytes = std::mem::size_of::<CheckpointConvergenceProposal>()
        .checked_add(request.budget.definition.package.retained_heap_bytes())
        .ok_or(ConvergenceError::Capacity)?
        .checked_add(request.budget.definition.entry.retained_heap_bytes())
        .ok_or(ConvergenceError::Capacity)?;
    if request.beat.alternatives.len() > limits.beats.alternatives {
        return Err(ConvergenceError::Capacity);
    }
    let mut relevant = Vec::new();
    let mut causes = Vec::new();
    for alternative in request.beat.alternatives {
        if relevant.contains(alternative.selection)
            || alternative.causes.len() > limits.beats.records
        {
            return Err(ConvergenceError::Capacity);
        }
        bytes = bytes
            .checked_add(std::mem::size_of::<ContentReference>())
            .and_then(|count| {
                count.checked_add(alternative.selection.package.retained_heap_bytes())
            })
            .and_then(|count| count.checked_add(alternative.selection.entry.retained_heap_bytes()))
            .ok_or(ConvergenceError::Capacity)?;
        if bytes > limits.maximum_evidence_bytes {
            return Err(ConvergenceError::Capacity);
        }
        for cause in alternative.causes {
            accepted(current, cause.fact, request.beat.policy, &mut work)
                .map_err(ConvergenceError::Budget)?;
        }
        // Reuse the canonical validator for *each* retained alternative: causal references,
        // current beat/thread prerequisites, accepted facts, source audience and source pins.
        stage_checkpoint_beat_selection(
            current,
            CheckpointBeatRequest {
                expected_basis: request.beat.expected_basis,
                admitted_pins: request.beat.admitted_pins,
                policy: request.beat.policy,
                expected_policy: request.beat.expected_policy,
                recipient: request.beat.recipient,
                selection: alternative.selection,
                alternatives: request.beat.alternatives,
                inventory: request.beat.inventory,
                checkpoint_limits: request.beat.checkpoint_limits,
            },
            limits.beats,
        )
        .map_err(ConvergenceError::Beat)?;
        relevant.push(alternative.selection.clone());
        if alternative.selection == request.beat.selection {
            bytes = bytes
                .checked_add(alternative.selection.package.retained_heap_bytes())
                .and_then(|count| {
                    count.checked_add(alternative.selection.entry.retained_heap_bytes())
                })
                .ok_or(ConvergenceError::Capacity)?;
            for cause in alternative.causes {
                bytes = bytes
                    .checked_add(std::mem::size_of::<FactId>())
                    .ok_or(ConvergenceError::Capacity)?;
                if bytes > limits.maximum_evidence_bytes {
                    return Err(ConvergenceError::Capacity);
                }
                causes.push(cause.fact);
            }
        }
    }
    let strong = request
        .budget
        .strong_events
        .iter()
        .any(|rule| &rule.event == request.beat.selection);
    let budget_candidate = if strong {
        let id = request
            .strong_receipt
            .ok_or(ConvergenceError::MissingInterventionReceipt)?;
        let selected = request
            .beat
            .alternatives
            .iter()
            .find(|alternative| alternative.selection == request.beat.selection)
            .ok_or(ConvergenceError::MissingInterventionReceipt)?;
        if !selected
            .causes
            .iter()
            .any(|cause| cause.fact == id && cause.event == request.beat.selection)
        {
            return Err(ConvergenceError::MissingInterventionReceipt);
        }
        let proposed = stage_checkpoint_narrative_budget(
            current,
            NarrativeBudgetRequest {
                expected_basis: request.beat.expected_basis,
                admitted_pins: request.beat.admitted_pins,
                policy: request.budget,
                expected_policy: request.expected_budget,
                source_policy: request.beat.policy,
                change: NarrativeBudgetChange::StrongIntervention(id),
                inventory: request.beat.inventory,
                checkpoint_limits: request.beat.checkpoint_limits,
            },
            limits.budget,
        )
        .map_err(ConvergenceError::Budget)?;
        if proposed.outcome == NarrativeBudgetOutcome::AlreadyConsumed {
            return Err(ConvergenceError::PreviouslyConsumed);
        }
        Some(proposed.checkpoint)
    } else {
        if request.strong_receipt.is_some() {
            return Err(ConvergenceError::MissingInterventionReceipt);
        }
        None
    };
    // All relevant references passed the canonical validator before their bytes were cloned.
    let chosen = relevant
        .iter()
        .find(|reference| request.beat.selection == *reference)
        .ok_or(ConvergenceError::Capacity)?
        .clone();
    let checkpoint = stage_checkpoint_beat_selection(
        budget_candidate.as_ref().unwrap_or(current),
        request.beat,
        limits.beats,
    )
    .map_err(ConvergenceError::Beat)?;
    let proposed = CheckpointConvergenceProposal {
        checkpoint,
        chosen,
        relevant_alternatives: relevant,
        causes,
        budget_policy: request.budget.definition.clone(),
    };
    if evidence_bytes(&proposed).is_none_or(|bytes| bytes > limits.maximum_evidence_bytes) {
        return Err(ConvergenceError::Capacity);
    }
    Ok(proposed)
}
