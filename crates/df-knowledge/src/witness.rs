//! Source-admitted witness eligibility becomes an uncommitted knowledge proposal.
use df_model::checkpoint::{
    AudienceScope, Basis, Checkpoint, CheckpointError, CheckpointPins, ContentReference,
    KnowledgeGrant, RecordId,
};
use df_types::{MemberId, RevisionLabel};

/// The native source owner has already checked sight, hearing or actual contact.
/// These labels never compute perception, authorize disclosure or infer contact
/// from a relationship. A relationship state is an exact source-admitted value,
/// not a score threshold invented by this crate.
#[derive(Clone, Copy)]
pub enum WitnessRoute<'a> {
    Sight,
    Hearing,
    Relationship { permitted_state: &'a RevisionLabel },
}

/// One source-admitted eligibility, bound to an existing canonical witness.
/// Client flags, dialogue, rumors and beliefs cannot supply this authority.
#[derive(Clone, Copy)]
pub struct WitnessEligibility<'a> {
    pub witness: RecordId,
    pub recipient: MemberId,
    pub route: WitnessRoute<'a>,
}

/// Explicit finite work and grant-buffer limits. Zero permits no work/output in
/// that dimension. Scan/comparison limits include nested fact/audience IDs and
/// deduplication. One record comparison checks a fixed bounded record predicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WitnessLimits {
    pub maximum_candidates: usize,
    pub maximum_scan_records: usize,
    pub maximum_record_comparisons: usize,
    pub maximum_grants: usize,
    pub maximum_grant_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WitnessError {
    Checkpoint(CheckpointError),
    CandidateCapacity,
    ScanCapacity,
    ComparisonCapacity,
    GrantCapacity,
    GrantByteCapacity,
    AllocationCapacity,
    ObserverUnavailable,
    UnknownWitness,
    UnknownFact,
    SourceMismatch,
    InvalidWitnessTime,
    MissingAcceptedCause,
    AudienceDenied,
    IneligibleRoute,
}

/// Uncommitted grants in first eligible input order, with exact current basis and
/// pins. The checkpoint is immutable; the session owner must revalidate current
/// membership, source admission and eligibility before committing this proposal.
/// No facts, witnesses, relationships, beliefs, reputation or text are changed.
pub struct WitnessProposal<'a> {
    current: &'a Checkpoint,
    grants: Vec<KnowledgeGrant>,
}

impl std::fmt::Debug for WitnessProposal<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WitnessProposal")
            .field("grant_count", &self.grants.len())
            .finish_non_exhaustive()
    }
}

impl WitnessProposal<'_> {
    pub fn basis(&self) -> Basis {
        self.current.basis()
    }

    pub fn pins(&self) -> &CheckpointPins {
        self.current.pins()
    }

    pub fn grants(&self) -> &[KnowledgeGrant] {
        &self.grants
    }
}

struct Work {
    scans: usize,
    comparisons: usize,
}

impl Work {
    fn visit(&mut self) -> Result<(), WitnessError> {
        self.scans = self
            .scans
            .checked_sub(1)
            .ok_or(WitnessError::ScanCapacity)?;
        self.comparisons = self
            .comparisons
            .checked_sub(1)
            .ok_or(WitnessError::ComparisonCapacity)?;
        Ok(())
    }
}

// All canonical lookups and deduplication share the same finite work budget.
fn find<'a, T>(
    records: &'a [T],
    work: &mut Work,
    matches: impl Fn(&T) -> bool,
) -> Result<Option<&'a T>, WitnessError> {
    for record in records {
        work.visit()?;
        if matches(record) {
            return Ok(Some(record));
        }
    }
    Ok(None)
}

/// Propose grants only for actual canonical witnesses and accepted canonical
/// fact provenance under the exact current basis/pins. Sight/hearing bind the
/// recipient's current character to the witness observer. Indirect transmission
/// also requires the actual directional relationship, exact admitted policy and
/// supplied permitted state. The supplied eligibility remains the native source
/// owner's responsibility; this function is not a perception/contact producer.
///
/// Shared permits a current recipient; Members requires exact inclusion; Host is
/// refused. No witness, existing grant or claim widens this audience. Each grant
/// retains the accepted witnessed fact as both fact and source. Existing and
/// repeated recipient+fact grants are no-ops after validation. Any refusal or
/// overflow returns no partial successful proposal and never mutates current.
pub fn propose_witness_grants<'a>(
    current: &'a Checkpoint,
    expected: Basis,
    admitted: &CheckpointPins,
    policy: &ContentReference,
    eligible: &[WitnessEligibility<'_>],
    limits: WitnessLimits,
) -> Result<WitnessProposal<'a>, WitnessError> {
    current
        .validate_resume(expected, admitted)
        .map_err(WitnessError::Checkpoint)?;
    if eligible.len() > limits.maximum_candidates {
        return Err(WitnessError::CandidateCapacity);
    }
    let state = current.state();
    let mut work = Work {
        scans: limits.maximum_scan_records,
        comparisons: limits.maximum_record_comparisons,
    };
    let mut grants: Vec<KnowledgeGrant> = Vec::new();
    for candidate in eligible {
        let member = find(&state.members, &mut work, |link| {
            link.member == candidate.recipient
        })?
        .ok_or(WitnessError::ObserverUnavailable)?;
        let recipient = member.character.ok_or(WitnessError::ObserverUnavailable)?;
        let witness = find(&state.continuity.witnesses, &mut work, |record| {
            record.id == candidate.witness
        })?
        .ok_or(WitnessError::UnknownWitness)?;
        let fact = find(&state.facts, &mut work, |fact| fact.id == witness.fact)?
            .ok_or(WitnessError::UnknownFact)?;
        let permitted = match &fact.audience {
            AudienceScope::Shared => true,
            AudienceScope::Members(members) => {
                find(members, &mut work, |member| *member == candidate.recipient)?.is_some()
            }
            AudienceScope::Host => false,
        };
        if !permitted {
            return Err(WitnessError::AudienceDenied);
        }
        if witness.source != *policy {
            return Err(WitnessError::SourceMismatch);
        }
        if witness.perceived_at.ticks_per_second != state.logical_time.ticks_per_second
            || witness.perceived_at.ticks > state.logical_time.ticks
        {
            return Err(WitnessError::InvalidWitnessTime);
        }
        let decision = find(&state.decisions, &mut work, |decision| {
            decision.operation == fact.operation && decision.revision == fact.revision
        })?
        .ok_or(WitnessError::MissingAcceptedCause)?;
        if find(&decision.facts, &mut work, |id| *id == fact.id)?.is_none() {
            return Err(WitnessError::MissingAcceptedCause);
        }
        match candidate.route {
            WitnessRoute::Sight | WitnessRoute::Hearing if witness.observer != recipient => {
                return Err(WitnessError::IneligibleRoute);
            }
            WitnessRoute::Relationship { permitted_state } => {
                if find(&state.relationships, &mut work, |relationship| {
                    relationship.subject == witness.observer
                        && relationship.object == recipient
                        && relationship.policy == *policy
                        && relationship.state == *permitted_state
                })?
                .is_none()
                {
                    return Err(WitnessError::IneligibleRoute);
                }
            }
            WitnessRoute::Sight | WitnessRoute::Hearing => {}
        }
        let already_known =
            |grant: &KnowledgeGrant| grant.observer == candidate.recipient && grant.fact == fact.id;
        if find(&state.knowledge, &mut work, already_known)?.is_some()
            || find(&grants, &mut work, already_known)?.is_some()
        {
            continue;
        }
        if grants.len() == limits.maximum_grants {
            return Err(WitnessError::GrantCapacity);
        }
        let required = grants
            .len()
            .checked_add(1)
            .and_then(|count| count.checked_mul(std::mem::size_of::<KnowledgeGrant>()))
            .ok_or(WitnessError::GrantByteCapacity)?;
        if required > limits.maximum_grant_bytes {
            return Err(WitnessError::GrantByteCapacity);
        }
        grants
            .try_reserve_exact(1)
            .map_err(|_| WitnessError::AllocationCapacity)?;
        let retained = grants
            .capacity()
            .checked_mul(std::mem::size_of::<KnowledgeGrant>())
            .ok_or(WitnessError::GrantByteCapacity)?;
        if retained > limits.maximum_grant_bytes {
            return Err(WitnessError::GrantByteCapacity);
        }
        grants.push(KnowledgeGrant {
            observer: candidate.recipient,
            fact: fact.id,
            source: fact.id,
        });
    }
    Ok(WitnessProposal { current, grants })
}
