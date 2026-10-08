//! Pure evidence admission for a recap or a speculative trailer.
//! Shot, caption, asset and publication policy belongs to presentation/session owners.

use df_knowledge::perception::{
    ClaimPerceptionLimits, ObserverScope, PerceptionError, PerceptionLimits, perceive,
    perceive_claims,
};
use df_model::checkpoint::{
    AcceptedDecision, AudienceScope, BookendKind, BookendSpec, Checkpoint, CheckpointError,
    CheckpointPins, ContentReference, FactId, FactSelection, GameFact,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BookendEvidenceError {
    Checkpoint(CheckpointError),
    Perception(PerceptionError),
    Audience,
    Range,
    Capacity,
    Evidence,
    Thread,
}

/// Borrowed server admission for one exact bookend evidence selection.
/// Thread admission is supplied by the authenticated content/audience owner.
pub struct BookendEvidenceRequest<'a> {
    pub spec: &'a BookendSpec,
    pub selected: &'a FactSelection,
    pub admitted_pins: &'a CheckpointPins,
    pub observer: ObserverScope,
    pub admitted_threads: &'a [ContentReference],
    pub selected_thread: Option<&'a ContentReference>,
}

/// Explicit perception and local comparison bounds, with no production defaults.
#[derive(Clone, Copy)]
pub struct BookendEvidenceLimits {
    pub facts: PerceptionLimits,
    pub claims: ClaimPerceptionLimits,
    pub maximum_work: usize,
}

struct Work(usize);

impl Work {
    fn charge(&mut self) -> Result<(), BookendEvidenceError> {
        self.0 = self
            .0
            .checked_sub(1)
            .ok_or(BookendEvidenceError::Capacity)?;
        Ok(())
    }
}

fn accepted_in_range(
    fact: &GameFact,
    decisions: &[AcceptedDecision],
    spec: &BookendSpec,
    work: &mut Work,
) -> Result<bool, BookendEvidenceError> {
    if fact.revision < spec.from || fact.revision > spec.through {
        return Ok(false);
    }
    for decision in decisions {
        work.charge()?;
        if decision.operation == fact.operation && decision.revision == fact.revision {
            return Ok(decision.facts.get(fact.ordinal as usize) == Some(&fact.id));
        }
    }
    Ok(false)
}

fn selected_fact<'a>(
    id: FactId,
    permitted: &[&'a GameFact],
    work: &mut Work,
) -> Result<Option<(usize, &'a GameFact)>, BookendEvidenceError> {
    for (position, fact) in permitted.iter().enumerate() {
        work.charge()?;
        if fact.id == id {
            return Ok(Some((position, fact)));
        }
    }
    Ok(None)
}

/// Selects exact committed recap evidence or one currently admitted open thread.
///
/// The caller supplies an authenticated observer, current pins, and threads already
/// authorized for that observer. This function verifies those threads remain open;
/// it cannot infer their disclosure policy from a bare content reference. This
/// boundary currently admits Shared and one-member audiences.
/// A recap returns canonical fact/claim IDs and `None`. A trailer returns an empty
/// fact/claim selection and its authorized thread. Neither result contains a
/// predicted outcome or changes the checkpoint. The owner must revalidate the
/// basis, audience and source before presentation or durable publication.
pub fn select_bookend_evidence(
    current: &Checkpoint,
    request: BookendEvidenceRequest<'_>,
    limits: BookendEvidenceLimits,
) -> Result<(FactSelection, Option<ContentReference>), BookendEvidenceError> {
    let BookendEvidenceRequest {
        spec,
        selected: requested,
        admitted_pins,
        observer,
        admitted_threads,
        selected_thread,
    } = request;
    current
        .validate_resume(spec.basis, admitted_pins)
        .map_err(BookendEvidenceError::Checkpoint)?;
    if spec.from > spec.through || spec.through > spec.basis.revision {
        return Err(BookendEvidenceError::Range);
    }
    let audience_matches = match (&spec.audience, observer) {
        (AudienceScope::Shared, ObserverScope::Shared) => true,
        (AudienceScope::Members(members), ObserverScope::Member(member)) => {
            members.as_slice() == [member]
        }
        _ => false,
    };
    if !audience_matches || requested.audience != spec.audience {
        return Err(BookendEvidenceError::Audience);
    }
    let mut work = Work(limits.maximum_work);
    match spec.kind {
        BookendKind::SpeculativeTrailer => {
            if !requested.facts.is_empty() || !requested.attributed_claims.is_empty() {
                return Err(BookendEvidenceError::Evidence);
            }
            perceive(current, spec.basis, admitted_pins, observer, limits.facts)
                .map_err(BookendEvidenceError::Perception)?;
            let thread = selected_thread.ok_or(BookendEvidenceError::Thread)?;
            let mut admitted = false;
            for candidate in admitted_threads {
                work.charge()?;
                if candidate == thread {
                    admitted = true;
                    break;
                }
            }
            if !admitted {
                return Err(BookendEvidenceError::Thread);
            }
            for open in &current.state().narrative.open_threads {
                work.charge()?;
                if open == thread {
                    return Ok((
                        FactSelection {
                            facts: Vec::new(),
                            attributed_claims: Vec::new(),
                            audience: spec.audience.clone(),
                        },
                        Some(thread.clone()),
                    ));
                }
            }
            Err(BookendEvidenceError::Thread)
        }
        BookendKind::Recap => {
            if selected_thread.is_some() || !admitted_threads.is_empty() {
                return Err(BookendEvidenceError::Thread);
            }
            if requested.facts.len() > limits.facts.maximum_selected_facts
                || requested.attributed_claims.len() > limits.claims.maximum_selected_claims
            {
                return Err(BookendEvidenceError::Capacity);
            }
            let perceived = perceive(current, spec.basis, admitted_pins, observer, limits.facts)
                .map_err(BookendEvidenceError::Perception)?;
            let claims =
                perceive_claims(current, spec.basis, admitted_pins, observer, limits.claims)
                    .map_err(BookendEvidenceError::Perception)?;
            let mut facts = Vec::new();
            facts
                .try_reserve_exact(requested.facts.len())
                .map_err(|_| BookendEvidenceError::Capacity)?;
            let mut previous_position = None;
            for id in &requested.facts {
                let (position, fact) = selected_fact(*id, perceived.facts(), &mut work)?
                    .ok_or(BookendEvidenceError::Evidence)?;
                if previous_position.is_some_and(|previous| position <= previous) {
                    return Err(BookendEvidenceError::Evidence);
                }
                if !accepted_in_range(fact, &current.state().decisions, spec, &mut work)? {
                    return Err(BookendEvidenceError::Evidence);
                }
                previous_position = Some(position);
                facts.push(*id);
            }
            let mut attributed_claims = Vec::new();
            attributed_claims
                .try_reserve_exact(requested.attributed_claims.len())
                .map_err(|_| BookendEvidenceError::Capacity)?;
            for id in &requested.attributed_claims {
                for earlier in &attributed_claims {
                    work.charge()?;
                    if earlier == id {
                        return Err(BookendEvidenceError::Evidence);
                    }
                }
                let mut selected = None;
                for claim in claims.claims() {
                    work.charge()?;
                    if claim.id == *id {
                        selected = Some(claim);
                        break;
                    }
                }
                let claim = selected.ok_or(BookendEvidenceError::Evidence)?;
                if claim.evidence.is_empty() {
                    return Err(BookendEvidenceError::Evidence);
                }
                for evidence in &claim.evidence {
                    let (_, fact) = selected_fact(*evidence, perceived.facts(), &mut work)?
                        .ok_or(BookendEvidenceError::Evidence)?;
                    if !accepted_in_range(fact, &current.state().decisions, spec, &mut work)? {
                        return Err(BookendEvidenceError::Evidence);
                    }
                }
                attributed_claims.push(*id);
            }
            Ok((
                FactSelection {
                    facts,
                    attributed_claims,
                    audience: spec.audience.clone(),
                },
                None,
            ))
        }
    }
}
