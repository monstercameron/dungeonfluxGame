//! Read-only admission of exact caller-owned encounter participant identities.

/// Current world and disclosure status supplied by the owning authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticipantStatus {
    Available,
    Unknown,
    Unavailable,
    Undisclosed,
}

/// Supplies canonical actor and basis types without creating replacement game models.
///
/// The owner must validate the session/run, world, content, and disclosure-policy
/// basis. Status checks must use that current authority, perform no mutation or
/// I/O, and return deterministic results. Correlation identifiers grant no access.
pub trait ParticipantOwner {
    type Actor: Eq;
    type Basis;
    type Error;

    fn validate_basis(
        &self,
        proposal_basis: &Self::Basis,
        current_basis: &Self::Basis,
    ) -> Result<(), Self::Error>;

    fn participant_status(
        &self,
        current_basis: &Self::Basis,
        actor: &Self::Actor,
    ) -> Result<ParticipantStatus, Self::Error>;
}

/// Caller-admitted bounds; neither field has an unlimited default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParticipantLimits {
    pub max_participants: usize,
    pub max_identity_comparisons: usize,
}

/// Rejection contains positions and safe status classes, never actor identities.
#[derive(Debug, Eq, PartialEq)]
pub enum ParticipantError<OwnerError> {
    ParticipantCapacity {
        required: usize,
        limit: usize,
    },
    ComparisonCapacity {
        required: usize,
        limit: usize,
    },
    ComparisonCountOverflow,
    Duplicate {
        first: usize,
        duplicate: usize,
    },
    Inadmissible {
        index: usize,
        status: ParticipantStatus,
    },
    Owner(OwnerError),
}

/// Exact borrowed participants and basis admitted by the current owner.
///
/// This is a staged validation result, not a committed encounter or authority to
/// mutate world state. Revalidate when the owning world or disclosure basis changes.
pub struct ValidatedParticipants<'a, Actor, Basis> {
    actors: &'a [Actor],
    current_basis: &'a Basis,
}

impl<'a, Actor, Basis> ValidatedParticipants<'a, Actor, Basis> {
    pub fn actors(&self) -> &'a [Actor] {
        self.actors
    }

    pub fn current_basis(&self) -> &'a Basis {
        self.current_basis
    }
}

type ParticipantResult<'a, Owner> = Result<
    ValidatedParticipants<
        'a,
        <Owner as ParticipantOwner>::Actor,
        <Owner as ParticipantOwner>::Basis,
    >,
    ParticipantError<<Owner as ParticipantOwner>::Error>,
>;

/// Validate a bounded proposal without allocating, copying, or inventing actors.
///
/// Capacity is admitted before owner callbacks. Basis validation precedes identity
/// and status checks. Duplicate comparison work is bounded explicitly. A failed
/// check returns no partial successful result; accepted order is unchanged.
pub fn validate_participants<'a, Owner: ParticipantOwner>(
    owner: &Owner,
    proposal_basis: &Owner::Basis,
    current_basis: &'a Owner::Basis,
    actors: &'a [Owner::Actor],
    limits: ParticipantLimits,
) -> ParticipantResult<'a, Owner> {
    let count = actors.len();
    if count > limits.max_participants {
        return Err(ParticipantError::ParticipantCapacity {
            required: count,
            limit: limits.max_participants,
        });
    }
    let comparisons = comparison_count(count).ok_or(ParticipantError::ComparisonCountOverflow)?;
    if comparisons > limits.max_identity_comparisons {
        return Err(ParticipantError::ComparisonCapacity {
            required: comparisons,
            limit: limits.max_identity_comparisons,
        });
    }
    owner
        .validate_basis(proposal_basis, current_basis)
        .map_err(ParticipantError::Owner)?;
    for (duplicate, actor) in actors.iter().enumerate() {
        for (first, earlier) in actors.iter().take(duplicate).enumerate() {
            if actor == earlier {
                return Err(ParticipantError::Duplicate { first, duplicate });
            }
        }
    }
    for (index, actor) in actors.iter().enumerate() {
        let status = owner
            .participant_status(current_basis, actor)
            .map_err(ParticipantError::Owner)?;
        if status != ParticipantStatus::Available {
            return Err(ParticipantError::Inadmissible { index, status });
        }
    }
    Ok(ValidatedParticipants {
        actors,
        current_basis,
    })
}

fn comparison_count(count: usize) -> Option<usize> {
    if count < 2 {
        return Some(0);
    }
    if count.is_multiple_of(2) {
        (count / 2).checked_mul(count - 1)
    } else {
        count.checked_mul((count - 1) / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::comparison_count;

    #[test]
    fn comparison_count_is_exact_and_overflow_is_explicit() {
        assert_eq!(comparison_count(0), Some(0));
        assert_eq!(comparison_count(1), Some(0));
        assert_eq!(comparison_count(2), Some(1));
        assert_eq!(comparison_count(3), Some(3));
        assert_eq!(comparison_count(4), Some(6));
        assert_eq!(comparison_count(usize::MAX), None);
    }
}
