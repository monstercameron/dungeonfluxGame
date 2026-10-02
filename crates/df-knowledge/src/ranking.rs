use std::cmp::Ordering;

/// Explicit admission and selected-output limits chosen by the owning caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankingLimits {
    pub candidates: usize,
    pub selected: usize,
    pub bytes: usize,
    pub tokens: usize,
}

/// Selected-candidate costs in the caller's canonical projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateCost {
    pub bytes: usize,
    pub tokens: usize,
}

/// The owning authority supplies canonical records and current policy.
///
/// Batch validation must recheck source, index, observer and access generations.
/// Authorization runs before cost or comparison. Comparisons must be pure,
/// deterministic and form a total order; Greater means the left candidate is
/// preferred. Invalid semantic scores must return a typed owner error.
pub trait RankingOwner {
    type Request;
    type Basis;
    type Batch;
    type Candidate;
    type Error;

    fn candidates<'a>(&self, batch: &'a Self::Batch) -> &'a [Self::Candidate];

    fn validate_batch(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
        batch: &Self::Batch,
    ) -> Result<(), Self::Error>;

    fn authorize_candidate(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
        candidate: &Self::Candidate,
    ) -> Result<bool, Self::Error>;

    fn validate_candidate(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
        candidate: &Self::Candidate,
    ) -> Result<(), Self::Error>;

    fn candidate_cost(
        &self,
        request: &Self::Request,
        candidate: &Self::Candidate,
    ) -> Result<CandidateCost, Self::Error>;

    fn compare_candidates(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
        left: &Self::Candidate,
        right: &Self::Candidate,
    ) -> Result<Ordering, Self::Error>;
}

#[derive(Debug, Eq, PartialEq)]
pub enum RankingError<E> {
    CandidateCapacity,
    ByteCapacity,
    TokenCapacity,
    AllocationCapacity,
    Owner(E),
}

/// Borrowed selected canonical records and the exact current basis.
///
/// This value does not authorize later commit or disclosure. The session owner
/// must recheck its current source/access/basis before either operation.
pub struct RankedCandidates<'a, B, C> {
    basis: &'a B,
    candidates: Vec<&'a C>,
}

impl<'a, B, C> RankedCandidates<'a, B, C> {
    pub fn basis(&self) -> &'a B {
        self.basis
    }

    pub fn candidates(&self) -> &[&'a C] {
        &self.candidates
    }
}

/// Ranking result using the owning caller's canonical record and error types.
pub type RankingResult<'a, O> = Result<
    RankedCandidates<'a, <O as RankingOwner>::Basis, <O as RankingOwner>::Candidate>,
    RankingError<<O as RankingOwner>::Error>,
>;

/// Reauthorize and rank a bounded batch without cloning or changing its basis.
///
/// Equal policy comparisons retain input order. A failed admission, owner check
/// or selected prefix budget returns no successful partial result. Insertion
/// ordering performs at most n*(n-1)/2 comparisons over authorized candidates;
/// the caller must select an admission cap within its decision work budget.
pub fn rank_candidates<'a, O: RankingOwner>(
    owner: &O,
    request: &O::Request,
    basis: &'a O::Basis,
    batch: &'a O::Batch,
    limits: RankingLimits,
) -> RankingResult<'a, O> {
    let candidates = owner.candidates(batch);
    if candidates.len() > limits.candidates {
        return Err(RankingError::CandidateCapacity);
    }
    owner
        .validate_batch(request, basis, batch)
        .map_err(RankingError::Owner)?;
    let mut selected: Vec<&O::Candidate> = Vec::new();
    selected
        .try_reserve_exact(candidates.len())
        .map_err(|_| RankingError::AllocationCapacity)?;
    for candidate in candidates {
        if !owner
            .authorize_candidate(request, basis, candidate)
            .map_err(RankingError::Owner)?
        {
            continue;
        }
        owner
            .validate_candidate(request, basis, candidate)
            .map_err(RankingError::Owner)?;
        let mut position = selected.len();
        for (index, previous) in selected.iter().enumerate() {
            if owner
                .compare_candidates(request, basis, candidate, previous)
                .map_err(RankingError::Owner)?
                == Ordering::Greater
            {
                position = index;
                break;
            }
        }
        selected.insert(position, candidate);
    }
    selected.truncate(limits.selected);
    let mut bytes = 0_usize;
    let mut tokens = 0_usize;
    for candidate in &selected {
        let cost = owner
            .candidate_cost(request, candidate)
            .map_err(RankingError::Owner)?;
        bytes = bytes
            .checked_add(cost.bytes)
            .filter(|value| *value <= limits.bytes)
            .ok_or(RankingError::ByteCapacity)?;
        tokens = tokens
            .checked_add(cost.tokens)
            .filter(|value| *value <= limits.tokens)
            .ok_or(RankingError::TokenCapacity)?;
    }
    Ok(RankedCandidates {
        basis,
        candidates: selected,
    })
}
