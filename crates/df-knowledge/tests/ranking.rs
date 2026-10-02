use std::cell::Cell;
use std::cmp::Ordering;

use df_knowledge::ranking::{
    CandidateCost, RankingError, RankingLimits, RankingOwner, rank_candidates,
};
use df_types::{MemberId, OperationId, RecoveryEpoch, SessionRevision};

#[derive(Debug, Eq, PartialEq)]
enum PolicyError {
    StaleBasis,
    StaleSource,
    StaleAccess,
    StaleIndex,
    NonfiniteScore,
    InvalidCost,
    InvalidComparison,
}

struct Request {
    observer: MemberId,
    revision: SessionRevision,
    access_generation: u64,
    source_generation: u64,
    index_generation: u64,
}

struct Basis {
    revision: SessionRevision,
    access_generation: u64,
    source_generation: u64,
}

struct Episode {
    id: OperationId,
    observer: MemberId,
    relevance: f64,
    salience: u64,
    game_time: u64,
    bytes: usize,
    tokens: usize,
}

struct Batch {
    index_generation: u64,
    episodes: Vec<Episode>,
}

#[derive(Default)]
struct Policy {
    authorized: Cell<usize>,
    validated: Cell<usize>,
    compared: Cell<usize>,
    costed: Cell<usize>,
    refuse_cost: bool,
    refuse_comparison: bool,
}

impl RankingOwner for Policy {
    type Request = Request;
    type Basis = Basis;
    type Batch = Batch;
    type Candidate = Episode;
    type Error = PolicyError;

    fn candidates<'a>(&self, batch: &'a Batch) -> &'a [Episode] {
        &batch.episodes
    }

    fn validate_batch(
        &self,
        request: &Request,
        basis: &Basis,
        batch: &Batch,
    ) -> Result<(), PolicyError> {
        if request.revision != basis.revision {
            return Err(PolicyError::StaleBasis);
        }
        if request.source_generation != basis.source_generation {
            return Err(PolicyError::StaleSource);
        }
        if request.access_generation != basis.access_generation {
            return Err(PolicyError::StaleAccess);
        }
        if request.index_generation != batch.index_generation {
            return Err(PolicyError::StaleIndex);
        }
        Ok(())
    }

    fn authorize_candidate(
        &self,
        request: &Request,
        _: &Basis,
        candidate: &Episode,
    ) -> Result<bool, PolicyError> {
        self.authorized.set(self.authorized.get() + 1);
        Ok(request.observer == candidate.observer)
    }

    fn validate_candidate(
        &self,
        request: &Request,
        _: &Basis,
        candidate: &Episode,
    ) -> Result<(), PolicyError> {
        assert_eq!(request.observer, candidate.observer);
        self.validated.set(self.validated.get() + 1);
        if !candidate.relevance.is_finite() {
            return Err(PolicyError::NonfiniteScore);
        }
        Ok(())
    }

    fn candidate_cost(
        &self,
        request: &Request,
        candidate: &Episode,
    ) -> Result<CandidateCost, PolicyError> {
        assert_eq!(request.observer, candidate.observer);
        self.costed.set(self.costed.get() + 1);
        if self.refuse_cost {
            return Err(PolicyError::InvalidCost);
        }
        Ok(CandidateCost {
            bytes: candidate.bytes,
            tokens: candidate.tokens,
        })
    }

    fn compare_candidates(
        &self,
        request: &Request,
        _: &Basis,
        left: &Episode,
        right: &Episode,
    ) -> Result<Ordering, PolicyError> {
        assert_eq!(request.observer, left.observer);
        assert_eq!(request.observer, right.observer);
        self.compared.set(self.compared.get() + 1);
        if self.refuse_comparison {
            return Err(PolicyError::InvalidComparison);
        }
        let relevance = left
            .relevance
            .partial_cmp(&right.relevance)
            .ok_or(PolicyError::NonfiniteScore)?;
        Ok(relevance
            .then(left.salience.cmp(&right.salience))
            .then(left.game_time.cmp(&right.game_time)))
    }
}

fn fixture() -> (Request, Basis, Batch) {
    let observer = MemberId::from_bytes(&[1; 16]).unwrap();
    let revision = SessionRevision::new(RecoveryEpoch::new(3).unwrap(), 7);
    let request = Request {
        observer,
        revision,
        access_generation: 4,
        source_generation: 9,
        index_generation: 2,
    };
    let basis = Basis {
        revision,
        access_generation: 4,
        source_generation: 9,
    };
    let episodes = (1..=4)
        .map(|index| Episode {
            id: OperationId::from_bytes(&[index; 16]).unwrap(),
            observer,
            relevance: 0.5,
            salience: 2,
            game_time: 10,
            bytes: 3,
            tokens: 2,
        })
        .collect();
    (
        request,
        basis,
        Batch {
            index_generation: 2,
            episodes,
        },
    )
}

fn limits() -> RankingLimits {
    RankingLimits {
        candidates: 4,
        selected: 4,
        bytes: 12,
        tokens: 8,
    }
}

#[test]
fn semantic_ties_preserve_exact_canonical_basis_and_candidate_references() {
    let (request, basis, batch) = fixture();
    let result = rank_candidates(&Policy::default(), &request, &basis, &batch, limits()).unwrap();
    assert!(std::ptr::eq(result.basis(), &basis));
    for (actual, original) in result.candidates().iter().zip(&batch.episodes) {
        assert!(std::ptr::eq(*actual, original));
        assert_eq!(actual.id, original.id);
    }
    assert_eq!(basis.revision, request.revision);
}

#[test]
fn current_owner_priority_orders_relevance_salience_and_recency() {
    let (request, basis, mut batch) = fixture();
    batch.episodes[0].game_time = 20;
    batch.episodes[1].salience = 3;
    batch.episodes[2].relevance = 0.75;
    let result = rank_candidates(&Policy::default(), &request, &basis, &batch, limits()).unwrap();
    let ids: Vec<_> = result
        .candidates()
        .iter()
        .map(|episode| episode.id)
        .collect();
    assert_eq!(
        ids,
        [
            batch.episodes[2].id,
            batch.episodes[1].id,
            batch.episodes[0].id,
            batch.episodes[3].id
        ]
    );
}

#[test]
fn secret_variants_never_reach_validation_costs_or_comparison() {
    let (request, basis, mut batch) = fixture();
    batch.episodes[1].observer = MemberId::from_bytes(&[9; 16]).unwrap();
    batch.episodes[1].relevance = f64::NAN;
    batch.episodes[1].bytes = usize::MAX;
    let first_policy = Policy::default();
    let first = rank_candidates(&first_policy, &request, &basis, &batch, limits()).unwrap();
    let first_ids: Vec<_> = first
        .candidates()
        .iter()
        .map(|episode| episode.id)
        .collect();
    drop(first);
    batch.episodes[1].relevance = f64::INFINITY;
    batch.episodes[1].tokens = usize::MAX;
    let second_policy = Policy::default();
    let second = rank_candidates(&second_policy, &request, &basis, &batch, limits()).unwrap();
    let second_ids: Vec<_> = second
        .candidates()
        .iter()
        .map(|episode| episode.id)
        .collect();
    assert_eq!(first_ids, second_ids);
    assert_eq!(first_policy.validated.get(), 3);
    assert_eq!(first_policy.costed.get(), 3);
    assert_eq!(first_policy.compared.get(), second_policy.compared.get());
}

#[test]
fn stale_basis_access_source_and_index_refuse_before_candidate_inspection() {
    for field in 0..4 {
        let (mut request, basis, batch) = fixture();
        let expected = match field {
            0 => {
                request.revision = request.revision.next_sequence().unwrap();
                PolicyError::StaleBasis
            }
            1 => {
                request.access_generation += 1;
                PolicyError::StaleAccess
            }
            2 => {
                request.source_generation += 1;
                PolicyError::StaleSource
            }
            _ => {
                request.index_generation += 1;
                PolicyError::StaleIndex
            }
        };
        let policy = Policy::default();
        assert!(
            matches!(rank_candidates(&policy, &request, &basis, &batch, limits()), Err(RankingError::Owner(error)) if error == expected)
        );
        assert_eq!(policy.authorized.get(), 0);
        assert_eq!(policy.compared.get(), 0);
        assert_eq!(policy.costed.get(), 0);
    }
}

#[test]
fn candidate_admission_refuses_before_owner_callbacks() {
    let (request, basis, batch) = fixture();
    let policy = Policy::default();
    let cap = RankingLimits {
        candidates: 3,
        ..limits()
    };
    assert!(matches!(
        rank_candidates(&policy, &request, &basis, &batch, cap),
        Err(RankingError::CandidateCapacity)
    ));
    assert_eq!(policy.authorized.get(), 0);
}

#[test]
fn selected_prefix_obeys_item_byte_and_token_caps_exactly() {
    let (request, basis, batch) = fixture();
    let cap = RankingLimits {
        selected: 2,
        bytes: 6,
        tokens: 4,
        ..limits()
    };
    let result = rank_candidates(&Policy::default(), &request, &basis, &batch, cap).unwrap();
    assert_eq!(result.candidates().len(), 2);
    assert_eq!(result.candidates()[0].id, batch.episodes[0].id);
    let byte_cap = RankingLimits { bytes: 5, ..cap };
    assert!(matches!(
        rank_candidates(&Policy::default(), &request, &basis, &batch, byte_cap),
        Err(RankingError::ByteCapacity)
    ));
    let token_cap = RankingLimits { tokens: 3, ..cap };
    assert!(matches!(
        rank_candidates(&Policy::default(), &request, &basis, &batch, token_cap),
        Err(RankingError::TokenCapacity)
    ));
}

#[test]
fn cumulative_cost_overflow_is_typed_and_returns_no_partial_result() {
    let (request, basis, mut batch) = fixture();
    batch.episodes[0].bytes = usize::MAX;
    let cap = RankingLimits {
        bytes: usize::MAX,
        ..limits()
    };
    assert!(matches!(
        rank_candidates(&Policy::default(), &request, &basis, &batch, cap),
        Err(RankingError::ByteCapacity)
    ));
    batch.episodes[0].bytes = 3;
    batch.episodes[0].tokens = usize::MAX;
    let cap = RankingLimits {
        tokens: usize::MAX,
        ..limits()
    };
    assert!(matches!(
        rank_candidates(&Policy::default(), &request, &basis, &batch, cap),
        Err(RankingError::TokenCapacity)
    ));
}

#[test]
fn singleton_and_multiple_nonfinite_scores_refuse_without_defaults() {
    for score in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let (request, basis, mut batch) = fixture();
        batch.episodes.truncate(1);
        batch.episodes[0].relevance = score;
        assert!(matches!(
            rank_candidates(&Policy::default(), &request, &basis, &batch, limits()),
            Err(RankingError::Owner(PolicyError::NonfiniteScore))
        ));
    }
    let (request, basis, mut batch) = fixture();
    batch.episodes[2].relevance = f64::NAN;
    assert!(matches!(
        rank_candidates(&Policy::default(), &request, &basis, &batch, limits()),
        Err(RankingError::Owner(PolicyError::NonfiniteScore))
    ));
}

#[test]
fn cost_refusal_empty_batch_and_zero_selection_remain_explicit() {
    let (request, basis, mut batch) = fixture();
    let policy = Policy {
        refuse_cost: true,
        ..Policy::default()
    };
    assert!(matches!(
        rank_candidates(&policy, &request, &basis, &batch, limits()),
        Err(RankingError::Owner(PolicyError::InvalidCost))
    ));
    let no_selection = RankingLimits {
        selected: 0,
        bytes: 0,
        tokens: 0,
        ..limits()
    };
    let result = rank_candidates(&policy, &request, &basis, &batch, no_selection).unwrap();
    assert!(result.candidates().is_empty());
    assert!(std::ptr::eq(result.basis(), &basis));
    batch.episodes.clear();
    let zero = RankingLimits {
        candidates: 0,
        selected: 0,
        bytes: 0,
        tokens: 0,
    };
    let result = rank_candidates(&policy, &request, &basis, &batch, zero).unwrap();
    assert!(result.candidates().is_empty());
}

#[test]
fn comparison_policy_refusal_preserves_original_batch_and_basis() {
    let (request, basis, batch) = fixture();
    let policy = Policy {
        refuse_comparison: true,
        ..Policy::default()
    };
    let before: Vec<_> = batch.episodes.iter().map(|episode| episode.id).collect();
    assert!(matches!(
        rank_candidates(&policy, &request, &basis, &batch, limits()),
        Err(RankingError::Owner(PolicyError::InvalidComparison))
    ));
    assert_eq!(policy.compared.get(), 1);
    let after: Vec<_> = batch.episodes.iter().map(|episode| episode.id).collect();
    assert_eq!(before, after);
    assert_eq!(basis.revision, request.revision);
}
