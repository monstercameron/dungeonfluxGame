//! Source-admitted speech plans and listener projection. No provider or commit authority.
use std::fmt;

pub use df_knowledge::perception::{ClaimPerceptionLimits, ObserverScope, PerceptionLimits};
use df_knowledge::perception::{PerceptionError, perceive, perceive_claims};
use df_model::checkpoint::{
    Basis, Checkpoint, CheckpointError, CheckpointPins, ContentReference, EntityId, FactId,
    RecordId,
};
use df_types::{LocaleTag, RevisionLabel};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeechIntent {
    Claim,
    ApprovedDeceit,
    FalseBelief,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredUncertainty {
    NoneDeclared,
    Explicit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotKind {
    Outcome,
    Name,
    Number,
}

/// Source-reviewed spans in the canonical claim text, never caller-provided slot values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroundedSlot {
    pub kind: SlotKind,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedClaim {
    pub claim: RecordId,
    pub intent: SpeechIntent,
    pub uncertainty: DeclaredUncertainty,
    pub slots: Vec<GroundedSlot>,
}

/// Server-side candidate IDs are not public expression input.
pub struct SpeechProposal {
    pub source: ContentReference,
    pub claims: Vec<PlannedClaim>,
}

impl fmt::Debug for SpeechProposal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpeechProposal").finish_non_exhaustive()
    }
}

/// Explicit source/access/output identities. Equality never grants those rights.
#[derive(Clone, Eq, PartialEq)]
pub struct SpeechVersions {
    pub access: RevisionLabel,
    pub contract: RevisionLabel,
    pub profile: RevisionLabel,
    pub locale: LocaleTag,
    pub provider: RevisionLabel,
    pub model: RevisionLabel,
    pub format: RevisionLabel,
    pub quote: RevisionLabel,
}

impl fmt::Debug for SpeechVersions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpeechVersions").finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeechSourceError {
    AccessDenied,
    Unsupported,
    Stale,
}

/// The trusted compiled/native source owner admits intent, attribution, deceit and slot semantics.
/// No client flag, claim text or catalog pin alone implements this authority.
pub trait SpeechSourceOwner {
    fn validate(
        &self,
        basis: Basis,
        pins: &CheckpointPins,
        versions: &SpeechVersions,
        proposal: &SpeechProposal,
    ) -> Result<(), SpeechSourceError>;
}

#[derive(Clone, Copy, Debug)]
pub struct SpeechLimits {
    pub maximum_plan_claims: usize,
    pub maximum_slots: usize,
    pub maximum_claim_bytes: usize,
    /// Sum of selected claim text, evidence IDs and grounded slot text.
    /// Identities, source metadata and request framing are excluded here.
    /// The expression adapter's RequestLimits bounds the complete wire payload.
    pub maximum_selected_text_and_evidence_bytes: usize,
    pub maximum_comparisons: usize,
    pub facts: PerceptionLimits,
    pub claims: ClaimPerceptionLimits,
}

/// Observer and current authority come from the authenticated projection owner.
pub struct SpeechObservation<'a> {
    pub checkpoint: &'a Checkpoint,
    pub basis: Basis,
    pub pins: &'a CheckpointPins,
    pub observer: ObserverScope,
    pub versions: &'a SpeechVersions,
    pub source_owner: Option<&'a dyn SpeechSourceOwner>,
    pub limits: SpeechLimits,
}

#[derive(Debug, Eq, PartialEq)]
pub enum SpeechError {
    Checkpoint(CheckpointError),
    Perception(PerceptionError),
    SourceUnavailable,
    Source(SpeechSourceError),
    Capacity,
    Allocation,
    DuplicateClaim,
    UnknownClaim,
    SourceMismatch,
    InvalidSlot,
    StalePlan,
    ContextChanged,
}

/// An immutable proposed plan. This is not a persisted Model record or an accepted decision.
pub struct SpeechActPlan {
    basis: Basis,
    pins: CheckpointPins,
    versions: SpeechVersions,
    proposal: SpeechProposal,
}

impl fmt::Debug for SpeechActPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpeechActPlan").finish_non_exhaustive()
    }
}

fn spend(remaining: &mut usize, amount: usize) -> Result<(), SpeechError> {
    *remaining = remaining.checked_sub(amount).ok_or(SpeechError::Capacity)?;
    Ok(())
}

fn validate_source(
    plan: &SpeechActPlan,
    current: &SpeechObservation<'_>,
) -> Result<(), SpeechError> {
    current
        .checkpoint
        .validate_resume(current.basis, current.pins)
        .map_err(SpeechError::Checkpoint)?;
    if plan.basis != current.basis
        || &plan.pins != current.pins
        || &plan.versions != current.versions
    {
        return Err(SpeechError::StalePlan);
    }
    current
        .source_owner
        .ok_or(SpeechError::SourceUnavailable)?
        .validate(
            current.basis,
            current.pins,
            current.versions,
            &plan.proposal,
        )
        .map_err(SpeechError::Source)
}

pub fn admit_speech(
    proposal: SpeechProposal,
    current: &SpeechObservation<'_>,
) -> Result<SpeechActPlan, SpeechError> {
    current
        .checkpoint
        .validate_resume(current.basis, current.pins)
        .map_err(SpeechError::Checkpoint)?;
    let limits = current.limits;
    if proposal.claims.len() > limits.maximum_plan_claims {
        return Err(SpeechError::Capacity);
    }
    let mut work = limits.maximum_comparisons;
    let mut slots = limits.maximum_slots;
    for (index, item) in proposal.claims.iter().enumerate() {
        for prior in &proposal.claims[..index] {
            spend(&mut work, 1)?;
            if prior.claim == item.claim {
                return Err(SpeechError::DuplicateClaim);
            }
        }
        let mut found = None;
        for claim in &current.checkpoint.state().beliefs {
            spend(&mut work, 1)?;
            if claim.id == item.claim {
                found = Some(claim);
                break;
            }
        }
        let claim = found.ok_or(SpeechError::UnknownClaim)?;
        if claim.source != proposal.source {
            return Err(SpeechError::SourceMismatch);
        }
        if claim.claim.len() > limits.maximum_claim_bytes {
            return Err(SpeechError::Capacity);
        }
        let mut previous_end = 0;
        for slot in &item.slots {
            spend(&mut slots, 1)?;
            if slot.start < previous_end
                || slot.start >= slot.end
                || claim.claim.get(slot.start..slot.end).is_none()
            {
                return Err(SpeechError::InvalidSlot);
            }
            previous_end = slot.end;
        }
    }
    current
        .source_owner
        .ok_or(SpeechError::SourceUnavailable)?
        .validate(current.basis, current.pins, current.versions, &proposal)
        .map_err(SpeechError::Source)?;
    Ok(SpeechActPlan {
        basis: current.basis,
        pins: current.pins.clone(),
        versions: current.versions.clone(),
        proposal,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionSlot<'a> {
    pub kind: SlotKind,
    pub value: &'a str,
}

/// All fields are derived from an audience-permitted canonical claim, not private reasoning.
#[derive(Clone, Eq, PartialEq)]
pub struct ExpressionClaim<'a> {
    pub id: RecordId,
    pub holder: EntityId,
    pub subject: EntityId,
    pub text: &'a str,
    pub evidence: &'a [FactId],
    pub source: &'a ContentReference,
    pub intent: SpeechIntent,
    pub uncertainty: DeclaredUncertainty,
    pub slots: Vec<ExpressionSlot<'a>>,
}

impl fmt::Debug for ExpressionClaim<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpressionClaim").finish_non_exhaustive()
    }
}

/// Internal request identity. Native egress must not serialize this private binding as prompt data.
#[derive(Clone, Eq, PartialEq)]
pub struct ExpressionBasis {
    basis: Basis,
    pins: CheckpointPins,
    observer: ObserverScope,
    versions: SpeechVersions,
    source: ContentReference,
    claims: Vec<RecordId>,
}

impl fmt::Debug for ExpressionBasis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpressionBasis").finish_non_exhaustive()
    }
}

/// Owned public metadata actually mapped by the expression adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionMetadata<'a> {
    pub schema: u16,
    pub locale: &'a str,
    pub permitted_claims: usize,
}

/// Constructed only through current Knowledge projection. No public raw-context constructor.
pub struct ExpressionContext<'a> {
    plan: &'a SpeechActPlan,
    observer: ObserverScope,
    clauses: Vec<ExpressionClaim<'a>>,
    identity: ExpressionBasis,
}

impl fmt::Debug for ExpressionContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpressionContext").finish_non_exhaustive()
    }
}

impl<'a> ExpressionContext<'a> {
    pub fn claims(&self) -> &[ExpressionClaim<'a>] {
        &self.clauses
    }
    pub fn basis(&self) -> Basis {
        self.plan.basis
    }
    pub fn semantic_basis(&self) -> &ExpressionBasis {
        &self.identity
    }
    pub fn metadata(&self) -> ExpressionMetadata<'_> {
        ExpressionMetadata {
            schema: 1,
            locale: self.plan.versions.locale.as_str(),
            permitted_claims: self.clauses.len(),
        }
    }

    /// Reproject every channel before later request/output consumption.
    pub fn validate_current(&self, current: &SpeechObservation<'_>) -> Result<(), SpeechError> {
        if self.observer != current.observer {
            return Err(SpeechError::ContextChanged);
        }
        let next = project_expression_context(self.plan, current)?;
        if next.identity != self.identity || next.clauses != self.clauses {
            return Err(SpeechError::ContextChanged);
        }
        Ok(())
    }
}

pub fn project_expression_context<'a>(
    plan: &'a SpeechActPlan,
    current: &SpeechObservation<'a>,
) -> Result<ExpressionContext<'a>, SpeechError> {
    validate_source(plan, current)?;
    if plan.proposal.claims.len() > current.limits.maximum_plan_claims {
        return Err(SpeechError::Capacity);
    }
    let mut remaining_slots = current.limits.maximum_slots;
    for item in &plan.proposal.claims {
        spend(&mut remaining_slots, item.slots.len())?;
    }
    // Both canonical selectors are real production Knowledge functions.
    let facts = perceive(
        current.checkpoint,
        current.basis,
        current.pins,
        current.observer,
        current.limits.facts,
    )
    .map_err(SpeechError::Perception)?;
    let claims = perceive_claims(
        current.checkpoint,
        current.basis,
        current.pins,
        current.observer,
        current.limits.claims,
    )
    .map_err(SpeechError::Perception)?;
    let mut clauses = Vec::new();
    clauses
        .try_reserve_exact(plan.proposal.claims.len())
        .map_err(|_| SpeechError::Allocation)?;
    let mut ids = Vec::new();
    ids.try_reserve_exact(plan.proposal.claims.len())
        .map_err(|_| SpeechError::Allocation)?;
    let mut work = current.limits.maximum_comparisons;
    let mut bytes = current.limits.maximum_selected_text_and_evidence_bytes;
    for item in &plan.proposal.claims {
        let mut visible = None;
        for claim in claims.claims() {
            spend(&mut work, 1)?;
            if claim.id == item.claim {
                visible = Some(*claim);
                break;
            }
        }
        let Some(claim) = visible else {
            continue;
        };
        if claim.claim.len() > current.limits.maximum_claim_bytes {
            return Err(SpeechError::Capacity);
        }
        // Knowledge already enforces evidence visibility; keep this real fact selection bound.
        for id in &claim.evidence {
            let mut permitted = false;
            for fact in facts.facts() {
                spend(&mut work, 1)?;
                if fact.id == *id {
                    permitted = true;
                    break;
                }
            }
            if !permitted {
                return Err(SpeechError::ContextChanged);
            }
        }
        spend(&mut bytes, claim.claim.len())?;
        spend(
            &mut bytes,
            claim
                .evidence
                .len()
                .checked_mul(16)
                .ok_or(SpeechError::Capacity)?,
        )?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(item.slots.len())
            .map_err(|_| SpeechError::Allocation)?;
        for slot in &item.slots {
            let value = claim
                .claim
                .get(slot.start..slot.end)
                .ok_or(SpeechError::InvalidSlot)?;
            spend(&mut bytes, value.len())?;
            slots.push(ExpressionSlot {
                kind: slot.kind,
                value,
            });
        }
        ids.push(claim.id);
        clauses.push(ExpressionClaim {
            id: claim.id,
            holder: claim.holder,
            subject: claim.subject,
            text: &claim.claim,
            evidence: &claim.evidence,
            source: &claim.source,
            intent: item.intent,
            uncertainty: item.uncertainty,
            slots,
        });
    }
    Ok(ExpressionContext {
        plan,
        observer: current.observer,
        clauses,
        identity: ExpressionBasis {
            basis: plan.basis,
            pins: plan.pins.clone(),
            observer: current.observer,
            versions: plan.versions.clone(),
            source: plan.proposal.source.clone(),
            claims: ids,
        },
    })
}
