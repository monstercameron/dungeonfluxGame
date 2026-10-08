//! Source-backed Design examples for the existing retrieval owner boundary.
//! Native summary/index publication and accepted-job lifetime remain prospective
//! decisions in fixtures/summary_index_lifetimes.json, not implemented executors.

#[path = "../src/retrieval_inputs.rs"]
mod retrieval_inputs;

use std::{cell::Cell, cmp::Ordering};

use df_knowledge::ranking::{
    CandidateCost, RankingError, RankingLimits, RankingOwner, RetrievalOwner, rank_candidates,
    retrieve_ranked,
};
use retrieval_inputs::{
    BatchCoverage, Event, FixtureOwner, KnowledgeBasis, MemoryCandidate, MemoryCandidateBatch,
    ObserverId, OwnerError, QueryPolicy, RankFacts, RetrievalRequest,
};

const OBSERVER: ObserverId = ObserverId(7);

fn basis() -> KnowledgeBasis {
    KnowledgeBasis {
        observer: OBSERVER,
        source_generation: 11,
        index_generation: 5,
        access_generation: 3,
    }
}

fn request() -> RetrievalRequest {
    RetrievalRequest {
        observer: OBSERVER,
        access_generation: 3,
    }
}

fn limits() -> RankingLimits {
    RankingLimits {
        candidates: 4,
        selected: 2,
        bytes: 8,
        tokens: 4,
    }
}

fn candidate(reference: u64, visible_to: ObserverId) -> MemoryCandidate {
    MemoryCandidate {
        reference,
        visible_to,
        source_generation: 11,
        access_generation: 3,
        rank: RankFacts {
            relevance: 2,
            salience: 3,
            recency: 4,
        },
        cost: CandidateCost {
            bytes: 4,
            tokens: 2,
        },
    }
}

fn batch(candidates: Vec<MemoryCandidate>) -> MemoryCandidateBatch {
    MemoryCandidateBatch {
        source_generation: 11,
        index_generation: 5,
        access_generation: 3,
        coverage: BatchCoverage::Complete,
        candidates,
    }
}

#[derive(Clone, Copy)]
enum GenerationChange {
    Source,
    Index,
    Access,
}

impl GenerationChange {
    fn advance(self, current: &mut KnowledgeBasis) {
        match self {
            Self::Source => current.source_generation += 1,
            Self::Index => current.index_generation += 1,
            Self::Access => current.access_generation += 1,
        }
    }

    fn error(self) -> OwnerError {
        match self {
            Self::Source | Self::Index => OwnerError::StaleBasis,
            Self::Access => OwnerError::AccessDenied,
        }
    }
}

#[derive(Clone, Copy)]
enum ChangeAt {
    Read,
    Return,
}

// Only the existing owner port is substituted. These controlled observations
// challenge retrieve_ranked's call ordering; they do not model index publication.
struct CurrentOwner {
    inner: FixtureOwner,
    current: Cell<KnowledgeBasis>,
    change: GenerationChange,
    change_at: ChangeAt,
    returned_count: Cell<Option<usize>>,
}

impl CurrentOwner {
    fn new(batch: MemoryCandidateBatch, change: GenerationChange, change_at: ChangeAt) -> Self {
        Self {
            inner: FixtureOwner::new(batch),
            current: Cell::new(basis()),
            change,
            change_at,
            returned_count: Cell::new(None),
        }
    }

    fn advance(&self) {
        let mut current = self.current.get();
        self.change.advance(&mut current);
        self.current.set(current);
    }

    fn reauthorize(&self, supplied: &KnowledgeBasis) -> Result<(), OwnerError> {
        let current = self.current.get();
        if supplied.observer != current.observer
            || supplied.access_generation != current.access_generation
        {
            return Err(OwnerError::AccessDenied);
        }
        if supplied.source_generation != current.source_generation
            || supplied.index_generation != current.index_generation
        {
            return Err(OwnerError::StaleBasis);
        }
        Ok(())
    }
}

impl RankingOwner for CurrentOwner {
    type Request = RetrievalRequest;
    type Basis = KnowledgeBasis;
    type Batch = MemoryCandidateBatch;
    type Candidate = MemoryCandidate;
    type Error = OwnerError;

    fn candidates<'a>(&self, batch: &'a Self::Batch) -> &'a [Self::Candidate] {
        self.inner.candidates(batch)
    }

    fn validate_batch(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
        batch: &Self::Batch,
    ) -> Result<(), Self::Error> {
        self.inner.validate_batch(request, basis, batch)
    }

    fn authorize_candidate(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
        candidate: &Self::Candidate,
    ) -> Result<bool, Self::Error> {
        self.inner.authorize_candidate(request, basis, candidate)
    }

    fn validate_candidate(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
        candidate: &Self::Candidate,
    ) -> Result<(), Self::Error> {
        self.inner.validate_candidate(request, basis, candidate)
    }

    fn candidate_cost(
        &self,
        request: &Self::Request,
        candidate: &Self::Candidate,
    ) -> Result<CandidateCost, Self::Error> {
        self.inner.candidate_cost(request, candidate)
    }

    fn compare_candidates(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
        left: &Self::Candidate,
        right: &Self::Candidate,
    ) -> Result<Ordering, Self::Error> {
        self.inner.compare_candidates(request, basis, left, right)
    }
}

impl RetrievalOwner for CurrentOwner {
    fn authorize_query(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
    ) -> Result<(), Self::Error> {
        RetrievalOwner::authorize_query(&self.inner, request, basis)?;
        self.reauthorize(basis)
    }

    fn read_batch<'a>(
        &'a self,
        request: &Self::Request,
        basis: &Self::Basis,
        limits: RankingLimits,
    ) -> Result<&'a Self::Batch, Self::Error> {
        let result = self.inner.read_batch(request, basis, limits);
        if matches!(self.change_at, ChangeAt::Read) {
            self.advance();
        }
        result
    }

    fn authorize_return(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
        selected: &[&Self::Candidate],
    ) -> Result<(), Self::Error> {
        self.inner.authorize_return(request, basis, selected)?;
        self.returned_count.set(Some(selected.len()));
        if matches!(self.change_at, ChangeAt::Return) {
            self.advance();
        }
        self.reauthorize(basis)
    }
}

#[test]
fn each_stale_batch_generation_refuses_before_candidate_inspection() {
    for change in [
        GenerationChange::Source,
        GenerationChange::Index,
        GenerationChange::Access,
    ] {
        let owner = FixtureOwner::new(batch(vec![candidate(1, OBSERVER)]));
        let mut current = basis();
        change.advance(&mut current);
        let current_request = RetrievalRequest {
            access_generation: current.access_generation,
            ..request()
        };
        let result = retrieve_ranked(&owner, &current_request, &current, limits());

        assert!(matches!(
            result,
            Err(RankingError::Owner(OwnerError::StaleBasis))
        ));
        assert_eq!(
            owner.events(),
            [
                Event::AuthorizeQuery,
                Event::ReadBatch,
                Event::AuthorizeQuery,
                Event::ValidateBatch,
                Event::AuthorizeReturn(0)
            ]
        );
    }
}

#[test]
fn borrowed_selection_does_not_authorize_a_new_source_or_access_generation() {
    let owner = FixtureOwner::new(batch(vec![candidate(1, OBSERVER)]));
    let old = basis();
    let selected = rank_candidates(&owner, &request(), &old, owner.batch(), limits()).unwrap();
    assert_eq!(selected.candidates()[0].reference, 1);

    for change in [
        GenerationChange::Source,
        GenerationChange::Index,
        GenerationChange::Access,
    ] {
        let mut current = old;
        change.advance(&mut current);
        let current_request = RetrievalRequest {
            access_generation: current.access_generation,
            ..request()
        };
        assert!(matches!(
            retrieve_ranked(&owner, &current_request, &current, limits()),
            Err(RankingError::Owner(OwnerError::StaleBasis))
        ));
        assert_eq!(selected.basis(), &old);
    }
}

#[test]
fn current_generation_changes_during_read_refuse_before_ranking() {
    for change in [
        GenerationChange::Source,
        GenerationChange::Index,
        GenerationChange::Access,
    ] {
        let owner = CurrentOwner::new(batch(vec![candidate(1, OBSERVER)]), change, ChangeAt::Read);
        let current = basis();
        let result = retrieve_ranked(&owner, &request(), &current, limits());

        assert!(matches!(result, Err(RankingError::Owner(error)) if error == change.error()));
        assert_eq!(owner.returned_count.get(), None);
        assert_eq!(
            owner.inner.events(),
            [
                Event::AuthorizeQuery,
                Event::ReadBatch,
                Event::AuthorizeQuery
            ]
        );
    }
}

#[test]
fn return_reauthorization_supersedes_both_selected_success_and_ranking_error() {
    for change in [
        GenerationChange::Source,
        GenerationChange::Index,
        GenerationChange::Access,
    ] {
        for incomplete in [false, true] {
            let mut supplied = batch(vec![candidate(1, OBSERVER)]);
            if incomplete {
                supplied.coverage = BatchCoverage::Incomplete;
            }
            let owner = CurrentOwner::new(supplied, change, ChangeAt::Return);
            let current = basis();
            let result = retrieve_ranked(&owner, &request(), &current, limits());

            assert!(matches!(result, Err(RankingError::Owner(error)) if error == change.error()));
            assert_eq!(owner.returned_count.get(), Some(usize::from(!incomplete)));
        }
    }
}

#[test]
fn denied_query_reads_nothing_even_when_a_stale_index_contains_private_material() {
    let mut supplied = batch(vec![candidate(99, ObserverId(9))]);
    supplied.index_generation = 0;
    let owner = FixtureOwner::with_query_policy(supplied, QueryPolicy::DenyAtCall(1));
    let current = basis();
    let result = retrieve_ranked(&owner, &request(), &current, limits());

    assert!(matches!(
        result,
        Err(RankingError::Owner(OwnerError::AccessDenied))
    ));
    assert_eq!(owner.events(), [Event::AuthorizeQuery]);
}

#[test]
fn hidden_stale_material_cannot_change_bounded_current_visible_selection() {
    for hidden_count in 0..=2 {
        let mut supplied = vec![candidate(1, OBSERVER), candidate(2, OBSERVER)];
        for index in 0..hidden_count {
            let mut hidden = candidate(90 + index, ObserverId(9));
            hidden.source_generation = 0;
            hidden.cost.bytes = usize::MAX;
            hidden.rank.relevance = u16::MAX;
            supplied.push(hidden);
        }
        let owner = FixtureOwner::new(batch(supplied));
        let current = basis();
        let selected = retrieve_ranked(&owner, &request(), &current, limits()).unwrap();
        let visible: Vec<_> = selected
            .candidates()
            .iter()
            .map(|record| record.reference)
            .collect();

        assert_eq!(visible, [1, 2]);
        assert_eq!(selected.basis(), &current);
        for event in owner.events() {
            match event {
                Event::ValidateCandidate(id) | Event::CandidateCost(id) => assert!(id < 90),
                Event::CompareCandidates(left, right) => assert!(left < 90 && right < 90),
                _ => {}
            }
        }
    }
}
