//! Borrowed canonical facts filtered by the current observer audience.
use df_model::checkpoint::{
    AudienceScope, Basis, Checkpoint, CheckpointError, CheckpointPins, GameFact,
};
use df_types::MemberId;

/// Supplied by the authenticated projection owner, never a client role flag.
/// A member must still occur in the current checkpoint membership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObserverScope {
    Shared,
    Member(MemberId),
}

/// Finite work and borrowed-output limits, including current membership lookup.
/// Zero permits no work or output in that dimension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PerceptionLimits {
    pub maximum_scan_records: usize,
    pub maximum_member_comparisons: usize,
    pub maximum_selected_facts: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PerceptionError {
    Checkpoint(CheckpointError),
    ObserverUnavailable,
    ScanCapacity,
    ComparisonCapacity,
    ResultCapacity,
    AllocationCapacity,
}

/// Facts retain checkpoint order and identity; no truth or disclosure is created.
/// This selection is valid only for this immutable current checkpoint. Its owner
/// must revalidate authorization before later publication or commit. Debug output
/// deliberately omits private fact payloads and observer identities.
pub struct PerceivedFacts<'a> {
    checkpoint: &'a Checkpoint,
    facts: Vec<&'a GameFact>,
}

impl std::fmt::Debug for PerceivedFacts<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PerceivedFacts")
            .field("count", &self.facts.len())
            .finish_non_exhaustive()
    }
}

impl<'a> PerceivedFacts<'a> {
    pub fn basis(&self) -> Basis {
        self.checkpoint.basis()
    }

    pub fn pins(&self) -> &'a CheckpointPins {
        self.checkpoint.pins()
    }

    pub fn facts(&self) -> &[&'a GameFact] {
        &self.facts
    }
}

fn consume(remaining: &mut usize, error: PerceptionError) -> Result<(), PerceptionError> {
    *remaining = remaining.checked_sub(1).ok_or(error)?;
    Ok(())
}

/// Select only Shared facts for a shared observer, and Shared plus explicit
/// Members inclusion for an exact current member. Host facts are always omitted.
/// Knowledge grants, rumors, claims and retained text cannot widen this scope.
/// The trusted owner supplies an exact current basis and admitted pins. Every
/// failure returns no partial selection. Work scans each fact once, and each
/// membership/audience comparison consumes the explicit comparison budget.
pub fn perceive<'a>(
    current: &'a Checkpoint,
    expected: Basis,
    admitted: &CheckpointPins,
    observer: ObserverScope,
    limits: PerceptionLimits,
) -> Result<PerceivedFacts<'a>, PerceptionError> {
    current
        .validate_resume(expected, admitted)
        .map_err(PerceptionError::Checkpoint)?;
    let state = current.state();
    let mut scans = limits.maximum_scan_records;
    let mut comparisons = limits.maximum_member_comparisons;
    if let ObserverScope::Member(member) = observer {
        let mut found = false;
        for link in &state.members {
            consume(&mut scans, PerceptionError::ScanCapacity)?;
            consume(&mut comparisons, PerceptionError::ComparisonCapacity)?;
            if link.member == member {
                found = true;
                break;
            }
        }
        if !found {
            return Err(PerceptionError::ObserverUnavailable);
        }
    }
    // Admit the full fact scan before allocating or selecting anything.
    if state.facts.len() > scans {
        return Err(PerceptionError::ScanCapacity);
    }
    let mut facts = Vec::new();
    facts
        .try_reserve_exact(state.facts.len().min(limits.maximum_selected_facts))
        .map_err(|_| PerceptionError::AllocationCapacity)?;
    for fact in &state.facts {
        let permitted = match (&fact.audience, observer) {
            (AudienceScope::Shared, _) => true,
            (AudienceScope::Members(members), ObserverScope::Member(member)) => {
                let mut included = false;
                for recipient in members {
                    consume(&mut comparisons, PerceptionError::ComparisonCapacity)?;
                    if *recipient == member {
                        included = true;
                        break;
                    }
                }
                included
            }
            (AudienceScope::Members(_), ObserverScope::Shared) | (AudienceScope::Host, _) => false,
        };
        if permitted {
            if facts.len() == limits.maximum_selected_facts {
                return Err(PerceptionError::ResultCapacity);
            }
            facts.push(fact);
        }
    }
    Ok(PerceivedFacts {
        checkpoint: current,
        facts,
    })
}
