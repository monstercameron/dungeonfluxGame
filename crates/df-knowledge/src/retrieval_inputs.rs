//! Test-only fixtures for the bounded native retrieval contract.
//!
//! This module is mounted by `tests/retrieval_inputs.rs`; it is not exported by
//! the crate and does not define production storage or authorization types.

use std::{cell::RefCell, cmp::Ordering};

use df_knowledge::ranking::{CandidateCost, RankingLimits, RankingOwner, RetrievalOwner};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ObserverId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RetrievalRequest {
    pub observer: ObserverId,
    pub access_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct KnowledgeBasis {
    pub observer: ObserverId,
    pub source_generation: u64,
    pub index_generation: u64,
    pub access_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RankFacts {
    pub relevance: u16,
    pub salience: u16,
    pub recency: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MemoryCandidate {
    pub reference: u64,
    pub visible_to: ObserverId,
    pub source_generation: u64,
    pub access_generation: u64,
    pub rank: RankFacts,
    pub cost: CandidateCost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BatchCoverage {
    Complete,
    Incomplete,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct MemoryCandidateBatch {
    pub source_generation: u64,
    pub index_generation: u64,
    pub access_generation: u64,
    pub coverage: BatchCoverage,
    pub candidates: Vec<MemoryCandidate>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OwnerError {
    AccessDenied,
    StaleBasis,
    IncompleteBatch,
    Capacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Event {
    AuthorizeQuery,
    ReadBatch,
    ValidateBatch,
    AuthorizeCandidate(u64),
    ValidateCandidate(u64),
    CompareCandidates(u64, u64),
    CandidateCost(u64),
    AuthorizeReturn(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QueryPolicy {
    Allow,
    DenyAtCall(usize),
}

pub(super) struct FixtureOwner {
    batch: MemoryCandidateBatch,
    query_policy: QueryPolicy,
    query_calls: RefCell<usize>,
    events: RefCell<Vec<Event>>,
}

impl FixtureOwner {
    pub(super) fn new(batch: MemoryCandidateBatch) -> Self {
        Self {
            batch,
            query_policy: QueryPolicy::Allow,
            query_calls: RefCell::new(0),
            events: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn with_query_policy(
        batch: MemoryCandidateBatch,
        query_policy: QueryPolicy,
    ) -> Self {
        Self {
            batch,
            query_policy,
            query_calls: RefCell::new(0),
            events: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn batch(&self) -> &MemoryCandidateBatch {
        &self.batch
    }

    pub(super) fn events(&self) -> Vec<Event> {
        self.events.borrow().clone()
    }

    fn authorize_query(
        &self,
        request: &RetrievalRequest,
        basis: &KnowledgeBasis,
    ) -> Result<(), OwnerError> {
        self.events.borrow_mut().push(Event::AuthorizeQuery);
        let mut query_calls = self.query_calls.borrow_mut();
        *query_calls += 1;
        if request.observer != basis.observer
            || request.access_generation != basis.access_generation
            || matches!(self.query_policy, QueryPolicy::DenyAtCall(call) if call == *query_calls)
        {
            return Err(OwnerError::AccessDenied);
        }
        Ok(())
    }
}

impl RankingOwner for FixtureOwner {
    type Request = RetrievalRequest;
    type Basis = KnowledgeBasis;
    type Batch = MemoryCandidateBatch;
    type Candidate = MemoryCandidate;
    type Error = OwnerError;

    fn candidates<'a>(&self, batch: &'a Self::Batch) -> &'a [Self::Candidate] {
        &batch.candidates
    }

    fn validate_batch(
        &self,
        _request: &Self::Request,
        basis: &Self::Basis,
        batch: &Self::Batch,
    ) -> Result<(), Self::Error> {
        self.events.borrow_mut().push(Event::ValidateBatch);
        if batch.coverage != BatchCoverage::Complete {
            return Err(OwnerError::IncompleteBatch);
        }
        if batch.source_generation != basis.source_generation
            || batch.index_generation != basis.index_generation
            || batch.access_generation != basis.access_generation
        {
            return Err(OwnerError::StaleBasis);
        }
        Ok(())
    }

    fn authorize_candidate(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
        candidate: &Self::Candidate,
    ) -> Result<bool, Self::Error> {
        self.events
            .borrow_mut()
            .push(Event::AuthorizeCandidate(candidate.reference));
        Ok(candidate.visible_to == request.observer
            && candidate.access_generation == basis.access_generation)
    }

    fn validate_candidate(
        &self,
        _request: &Self::Request,
        basis: &Self::Basis,
        candidate: &Self::Candidate,
    ) -> Result<(), Self::Error> {
        self.events
            .borrow_mut()
            .push(Event::ValidateCandidate(candidate.reference));
        if candidate.source_generation != basis.source_generation {
            return Err(OwnerError::StaleBasis);
        }
        Ok(())
    }

    fn candidate_cost(
        &self,
        _request: &Self::Request,
        candidate: &Self::Candidate,
    ) -> Result<CandidateCost, Self::Error> {
        self.events
            .borrow_mut()
            .push(Event::CandidateCost(candidate.reference));
        Ok(candidate.cost)
    }

    fn compare_candidates(
        &self,
        _request: &Self::Request,
        _basis: &Self::Basis,
        left: &Self::Candidate,
        right: &Self::Candidate,
    ) -> Result<Ordering, Self::Error> {
        self.events
            .borrow_mut()
            .push(Event::CompareCandidates(left.reference, right.reference));
        Ok(
            (left.rank.relevance, left.rank.salience, left.rank.recency).cmp(&(
                right.rank.relevance,
                right.rank.salience,
                right.rank.recency,
            )),
        )
    }
}

impl RetrievalOwner for FixtureOwner {
    fn authorize_query(
        &self,
        request: &Self::Request,
        basis: &Self::Basis,
    ) -> Result<(), Self::Error> {
        FixtureOwner::authorize_query(self, request, basis)
    }

    fn read_batch<'a>(
        &'a self,
        _request: &Self::Request,
        _basis: &Self::Basis,
        limits: RankingLimits,
    ) -> Result<&'a Self::Batch, Self::Error> {
        self.events.borrow_mut().push(Event::ReadBatch);
        if self.batch.candidates.len() > limits.candidates {
            return Err(OwnerError::Capacity);
        }
        Ok(&self.batch)
    }

    fn authorize_return(
        &self,
        _request: &Self::Request,
        _basis: &Self::Basis,
        selected: &[&Self::Candidate],
    ) -> Result<(), Self::Error> {
        self.events
            .borrow_mut()
            .push(Event::AuthorizeReturn(selected.len()));
        Ok(())
    }
}
