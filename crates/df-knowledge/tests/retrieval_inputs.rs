#[path = "../src/retrieval_inputs.rs"]
mod retrieval_inputs;

use df_knowledge::ranking::{RankingError, RankingLimits, rank_candidates, retrieve_ranked};
use retrieval_inputs::{
    BatchCoverage, Event, FixtureOwner, KnowledgeBasis, MemoryCandidate, MemoryCandidateBatch,
    ObserverId, OwnerError, QueryPolicy, RankFacts, RetrievalRequest,
};

const OBSERVER: ObserverId = ObserverId(7);
const OTHER_OBSERVER: ObserverId = ObserverId(9);

fn request() -> RetrievalRequest {
    RetrievalRequest {
        observer: OBSERVER,
        access_generation: 3,
    }
}

fn basis() -> KnowledgeBasis {
    KnowledgeBasis {
        observer: OBSERVER,
        source_generation: 11,
        index_generation: 5,
        access_generation: 3,
    }
}

fn candidate(
    reference: u64,
    visible_to: ObserverId,
    rank: RankFacts,
    bytes: usize,
    tokens: usize,
) -> MemoryCandidate {
    MemoryCandidate {
        reference,
        visible_to,
        source_generation: 11,
        access_generation: 3,
        rank,
        cost: df_knowledge::ranking::CandidateCost { bytes, tokens },
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

fn limits() -> RankingLimits {
    RankingLimits {
        candidates: 4,
        selected: 2,
        bytes: 20,
        tokens: 8,
    }
}

#[test]
fn retrieval_reauthorizes_around_read_and_returns_only_bounded_visible_candidates() {
    let owner = FixtureOwner::new(batch(vec![
        candidate(
            1,
            OBSERVER,
            RankFacts {
                relevance: 4,
                salience: 8,
                recency: 9,
            },
            6,
            2,
        ),
        candidate(
            2,
            OBSERVER,
            RankFacts {
                relevance: 9,
                salience: 4,
                recency: 3,
            },
            10,
            4,
        ),
        candidate(
            3,
            OBSERVER,
            RankFacts {
                relevance: 9,
                salience: 4,
                recency: 3,
            },
            10,
            4,
        ),
        candidate(
            4,
            OTHER_OBSERVER,
            RankFacts {
                relevance: u16::MAX,
                salience: u16::MAX,
                recency: u16::MAX,
            },
            1,
            1,
        ),
    ]));

    let retrieval_request = request();
    let current_basis = basis();
    let selected = retrieve_ranked(&owner, &retrieval_request, &current_basis, limits())
        .expect("authorized fixture retrieval should succeed");
    let references: Vec<_> = selected
        .candidates()
        .iter()
        .map(|candidate| candidate.reference)
        .collect();

    assert_eq!(selected.basis(), &current_basis);
    assert_eq!(references, [2, 3]);
    let events = owner.events();
    assert_eq!(
        &events[..3],
        &[
            Event::AuthorizeQuery,
            Event::ReadBatch,
            Event::AuthorizeQuery
        ]
    );
    assert_eq!(events.last(), Some(&Event::AuthorizeReturn(2)));
    assert!(events.contains(&Event::AuthorizeCandidate(4)));
    assert!(!events.contains(&Event::ValidateCandidate(4)));
    assert!(!events.contains(&Event::CandidateCost(4)));
}

#[test]
fn unauthorized_query_is_rejected_before_candidate_read() {
    let owner = FixtureOwner::with_query_policy(
        batch(vec![candidate(
            1,
            OBSERVER,
            RankFacts {
                relevance: 1,
                salience: 1,
                recency: 1,
            },
            1,
            1,
        )]),
        QueryPolicy::DenyAtCall(1),
    );

    let retrieval_request = request();
    let current_basis = basis();
    let result = retrieve_ranked(&owner, &retrieval_request, &current_basis, limits());

    assert!(matches!(
        result,
        Err(RankingError::Owner(OwnerError::AccessDenied))
    ));
    assert_eq!(owner.events(), [Event::AuthorizeQuery]);
}

#[test]
fn access_revocation_after_read_returns_no_selected_candidates() {
    let owner = FixtureOwner::with_query_policy(
        batch(vec![candidate(
            1,
            OBSERVER,
            RankFacts {
                relevance: 1,
                salience: 1,
                recency: 1,
            },
            1,
            1,
        )]),
        QueryPolicy::DenyAtCall(2),
    );

    let retrieval_request = request();
    let current_basis = basis();
    let result = retrieve_ranked(&owner, &retrieval_request, &current_basis, limits());

    assert!(matches!(
        result,
        Err(RankingError::Owner(OwnerError::AccessDenied))
    ));
    let events = owner.events();
    assert_eq!(
        &events[..3],
        &[
            Event::AuthorizeQuery,
            Event::ReadBatch,
            Event::AuthorizeQuery
        ]
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::AuthorizeReturn(_)))
    );
}

#[test]
fn stale_or_incomplete_batch_is_rejected_as_a_typed_owner_error() {
    let mut stale = batch(vec![candidate(
        1,
        OBSERVER,
        RankFacts {
            relevance: 1,
            salience: 1,
            recency: 1,
        },
        1,
        1,
    )]);
    stale.source_generation += 1;
    let owner = FixtureOwner::new(stale);

    let retrieval_request = request();
    let current_basis = basis();
    let result = retrieve_ranked(&owner, &retrieval_request, &current_basis, limits());

    assert!(matches!(
        result,
        Err(RankingError::Owner(OwnerError::StaleBasis))
    ));
    assert_eq!(owner.events().last(), Some(&Event::AuthorizeReturn(0)));

    let mut incomplete = batch(Vec::new());
    incomplete.coverage = BatchCoverage::Incomplete;
    let incomplete_owner = FixtureOwner::new(incomplete);
    let current_basis = basis();
    let result = retrieve_ranked(
        &incomplete_owner,
        &retrieval_request,
        &current_basis,
        limits(),
    );

    assert!(matches!(
        result,
        Err(RankingError::Owner(OwnerError::IncompleteBatch))
    ));
}

#[test]
fn ranking_rejects_admission_and_selected_projection_budget_overages() {
    let owner = FixtureOwner::new(batch(vec![
        candidate(
            1,
            OBSERVER,
            RankFacts {
                relevance: 2,
                salience: 2,
                recency: 2,
            },
            6,
            2,
        ),
        candidate(
            2,
            OBSERVER,
            RankFacts {
                relevance: 1,
                salience: 1,
                recency: 1,
            },
            6,
            2,
        ),
    ]));
    let admission_limits = RankingLimits {
        candidates: 1,
        ..limits()
    };

    assert!(matches!(
        rank_candidates(
            &owner,
            &request(),
            &basis(),
            owner.batch(),
            admission_limits
        ),
        Err(RankingError::CandidateCapacity)
    ));

    let projection_limits = RankingLimits {
        bytes: 5,
        ..limits()
    };
    assert!(matches!(
        rank_candidates(
            &owner,
            &request(),
            &basis(),
            owner.batch(),
            projection_limits
        ),
        Err(RankingError::ByteCapacity)
    ));
}
