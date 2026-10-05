//! Audience-safe evidence selection for already admitted canonical threat relationships.
use df_knowledge::perception::{ObserverScope, PerceptionError, PerceptionLimits, perceive};
use df_model::checkpoint::{
    Basis, Checkpoint, CheckpointError, CheckpointPins, ContentReference, FactId, GameFact,
    ThreatClock,
};
use std::mem::size_of;

/// The source owner has admitted this relationship, not merely matching names or IDs.
/// A borrowed expected clock is private server admission data, never a disclosure grant.
pub struct AdmittedThreatEvidence<'a> {
    pub expected: &'a ThreatClock,
    pub evidence: &'a [FactId],
}

/// Native source and audience admission for one immutable checkpoint.
/// Possession of these fields does not authenticate the caller or authorize publication.
pub struct ThreatRelevanceRequest<'a> {
    pub expected_basis: Basis,
    pub admitted_pins: &'a CheckpointPins,
    pub observer: ObserverScope,
    pub policy: &'a ContentReference,
    pub expected_policy: &'a ContentReference,
    pub admitted_content: &'a [ContentReference],
    pub mappings: Option<&'a [AdmittedThreatEvidence<'a>]>,
}

/// Explicit fixture/owner bounds, not calibrated production values.
/// Scratch flags are bounded by the canonical perception result count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreatRelevanceLimits {
    pub maximum_checkpoint_bytes: usize,
    pub maximum_candidates: usize,
    pub maximum_evidence_records: usize,
    pub maximum_clock_records: usize,
    pub maximum_content_records: usize,
    pub maximum_reference_bytes: usize,
    pub maximum_work: usize,
    pub maximum_output_bytes: usize,
    pub perception: PerceptionLimits,
}

/// Internal refusal classes only; never a public cue or audience error projection.
/// Admission errors intentionally contain no clock identity, fields or mismatch details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreatRelevanceError {
    NotAdmitted,
    Checkpoint(CheckpointError),
    Perception(PerceptionError),
    InvalidAdmission,
    InputCapacity,
    WorkCapacity,
    OutputCapacity,
    AllocationCapacity,
}

/// Server-only selection without clock fields or grouping metadata.
/// Basis, pins, policy and byte accounting are owner admission metadata, not audience cue fields.
/// Only `facts()` is eligible for downstream authorized projection; this is not a public DTO.
/// Current authorization and source admission must be rechecked before later publication.
pub struct ThreatRelevanceSelection<'a> {
    checkpoint: &'a Checkpoint,
    policy: &'a ContentReference,
    facts: Vec<&'a GameFact>,
    accounted_output_bytes: usize,
}

impl std::fmt::Debug for ThreatRelevanceSelection<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ThreatRelevanceSelection")
            .field("facts", &self.facts.len())
            .finish_non_exhaustive()
    }
}

impl<'a> ThreatRelevanceSelection<'a> {
    pub fn basis(&self) -> Basis {
        self.checkpoint.basis()
    }

    pub fn pins(&self) -> &'a CheckpointPins {
        self.checkpoint.pins()
    }

    /// The exact admitted selector policy, not a clock definition or score model.
    pub fn policy(&self) -> &'a ContentReference {
        self.policy
    }

    pub fn facts(&self) -> &[&'a GameFact] {
        &self.facts
    }

    /// Counts this result's inline size and retained reference-vector capacity.
    /// Borrowed checkpoint payloads and the separately bounded perception pass are excluded.
    pub fn accounted_output_bytes(&self) -> usize {
        self.accounted_output_bytes
    }
}

struct Work {
    remaining: usize,
}

impl Work {
    fn charge(&mut self, amount: usize) -> Result<(), ThreatRelevanceError> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or(ThreatRelevanceError::WorkCapacity)?;
        Ok(())
    }
}

fn admitted_reference(
    reference: &ContentReference,
    content: &[ContentReference],
    work: &mut Work,
) -> Result<bool, ThreatRelevanceError> {
    for admitted in content {
        work.charge(1)?;
        if admitted == reference {
            return Ok(true);
        }
    }
    Ok(false)
}

fn evidence_index(
    id: FactId,
    facts: &[&GameFact],
    work: &mut Work,
) -> Result<Option<usize>, ThreatRelevanceError> {
    for (index, fact) in facts.iter().enumerate() {
        work.charge(1)?;
        if fact.id == id {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

fn reference_bytes(reference: &ContentReference) -> Option<usize> {
    reference
        .package
        .retained_heap_bytes()
        .checked_add(reference.entry.retained_heap_bytes())
}

/// Selects evidence for source-owner-admitted threat relationships without inventing relevance.
///
/// The real Knowledge perception boundary filters the actual current checkpoint for the trusted
/// observer. An empty mapping or any evidence absent from that permitted result omits the whole
/// mapping before inspecting its clock. Unknown and private evidence therefore behave identically.
/// Eligible mappings require the exact current expected clock and an admitted content definition.
/// No numeric urgency, clock-based ordering, inferred relationship or new disclosure is computed.
///
/// Output is the deduplicated union of eligible evidence in canonical fact order, with no clock
/// IDs/definitions/progress/capacity, hidden counts or group boundaries. Duplicate mappings and
/// evidence cannot change that union. Refusal returns no partial selection; the checkpoint, time,
/// facts and knowledge remain unchanged. Errors and admission data remain internal. This function
/// neither commits a moment nor projects a cue, and source admission is not authentication.
pub fn select_threat_relevance<'a>(
    current: &'a Checkpoint,
    request: ThreatRelevanceRequest<'a>,
    limits: ThreatRelevanceLimits,
) -> Result<ThreatRelevanceSelection<'a>, ThreatRelevanceError> {
    let mappings = request.mappings.ok_or(ThreatRelevanceError::NotAdmitted)?;
    current
        .validate_resume(request.expected_basis, request.admitted_pins)
        .map_err(ThreatRelevanceError::Checkpoint)?;
    if current
        .retained_bytes()
        .is_none_or(|bytes| bytes > limits.maximum_checkpoint_bytes)
        || mappings.len() > limits.maximum_candidates
        || current.state().threats.len() > limits.maximum_clock_records
        || request.admitted_content.len() > limits.maximum_content_records
    {
        return Err(ThreatRelevanceError::InputCapacity);
    }
    let mut evidence_records = 0usize;
    for mapping in mappings {
        evidence_records = evidence_records
            .checked_add(mapping.evidence.len())
            .filter(|count| *count <= limits.maximum_evidence_records)
            .ok_or(ThreatRelevanceError::InputCapacity)?;
    }
    for reference in request
        .admitted_content
        .iter()
        .chain([request.policy, request.expected_policy])
        .chain(mappings.iter().map(|mapping| &mapping.expected.definition))
    {
        if reference_bytes(reference).is_none_or(|bytes| bytes > limits.maximum_reference_bytes) {
            return Err(ThreatRelevanceError::InputCapacity);
        }
    }
    let mut work = Work {
        remaining: limits.maximum_work,
    };
    work.charge(mappings.len())?;
    if request.policy != request.expected_policy
        || request.policy.package != current.pins().content.package
        || !admitted_reference(request.policy, request.admitted_content, &mut work)?
    {
        return Err(ThreatRelevanceError::InvalidAdmission);
    }
    let perceived = perceive(
        current,
        request.expected_basis,
        request.admitted_pins,
        request.observer,
        limits.perception,
    )
    .map_err(ThreatRelevanceError::Perception)?;
    work.charge(perceived.facts().len())?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(perceived.facts().len())
        .map_err(|_| ThreatRelevanceError::AllocationCapacity)?;
    selected.resize(perceived.facts().len(), false);
    for mapping in mappings {
        if mapping.evidence.is_empty() {
            continue;
        }
        let mut permitted = true;
        for id in mapping.evidence {
            if evidence_index(*id, perceived.facts(), &mut work)?.is_none() {
                permitted = false;
                break;
            }
        }
        if !permitted {
            continue;
        }
        let mut found = None;
        for clock in &current.state().threats {
            work.charge(1)?;
            if clock.id == mapping.expected.id {
                found = Some(clock);
                break;
            }
        }
        if found != Some(mapping.expected)
            || mapping.expected.definition.package != current.pins().content.package
            || !admitted_reference(
                &mapping.expected.definition,
                request.admitted_content,
                &mut work,
            )?
        {
            return Err(ThreatRelevanceError::InvalidAdmission);
        }
        for id in mapping.evidence {
            let index = evidence_index(*id, perceived.facts(), &mut work)?
                .ok_or(ThreatRelevanceError::InvalidAdmission)?;
            let flag = selected
                .get_mut(index)
                .ok_or(ThreatRelevanceError::InvalidAdmission)?;
            *flag = true;
        }
    }
    work.charge(selected.len())?;
    let count = selected.iter().filter(|flag| **flag).count();
    let minimum_bytes = count
        .checked_mul(size_of::<&GameFact>())
        .and_then(|bytes| bytes.checked_add(size_of::<ThreatRelevanceSelection<'_>>()))
        .ok_or(ThreatRelevanceError::OutputCapacity)?;
    if minimum_bytes > limits.maximum_output_bytes {
        return Err(ThreatRelevanceError::OutputCapacity);
    }
    let mut facts = Vec::new();
    facts
        .try_reserve_exact(count)
        .map_err(|_| ThreatRelevanceError::AllocationCapacity)?;
    let bytes = facts
        .capacity()
        .checked_mul(size_of::<&GameFact>())
        .and_then(|bytes| bytes.checked_add(size_of::<ThreatRelevanceSelection<'_>>()))
        .ok_or(ThreatRelevanceError::OutputCapacity)?;
    if bytes > limits.maximum_output_bytes {
        return Err(ThreatRelevanceError::OutputCapacity);
    }
    for (fact, selected) in perceived.facts().iter().zip(selected) {
        work.charge(1)?;
        if selected {
            facts.push(*fact);
        }
    }
    Ok(ThreatRelevanceSelection {
        checkpoint: current,
        policy: request.policy,
        facts,
        accounted_output_bytes: bytes,
    })
}
