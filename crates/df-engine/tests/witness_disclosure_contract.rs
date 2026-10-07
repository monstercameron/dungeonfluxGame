#[path = "support/fixture_model.rs"]
pub mod fixture_model;
#[path = "support/witness_staging.rs"]
mod support;

use df_engine::witness_staging::WitnessStagingError;
use df_knowledge::perception::{
    ClaimPerceptionLimits, ObserverScope, PerceptionError, PerceptionLimits, perceive,
    perceive_claims,
};
use df_knowledge::witness::{
    WitnessEligibility, WitnessError, WitnessRoute, propose_witness_grants,
};
use df_model::checkpoint::*;
use df_rules::{RulesCommandHandler, RulesCommandInput};
use fixture_model as fixture;
use support::{Fixture, SourceOwner, SourceRefusal, fact, record, registered};

// Native consumer fixture, not a production visibility/contact or consent source.
fn claim_state(fixture: &Fixture) -> GameState {
    let mut state = fixture.state();
    state.beliefs = vec![
        AttributedClaim {
            id: record(70),
            holder: fixture::entity(4),
            subject: fixture::entity(6),
            claim: "synthetic private false belief".to_owned(),
            evidence: vec![fact(30)],
            audience: AudienceScope::Members(vec![fixture::member(3), fixture::member(5)]),
            source: fixture::content(),
        },
        AttributedClaim {
            id: record(71),
            holder: fixture::entity(4),
            subject: fixture::entity(6),
            claim: "synthetic public attributed claim".to_owned(),
            evidence: vec![fact(30)],
            audience: AudienceScope::Shared,
            source: fixture::content(),
        },
    ];
    state.continuity.npcs = vec![NpcState {
        entity: fixture::entity(4),
        role: fixture::content(),
        personality: fixture::content(),
        motivations: vec![],
        goals: vec![],
        needs: vec![],
        fears: vec![],
        known_facts: vec![],
        beliefs: vec![record(70), record(71)],
        secrets: vec![SecretPolicy {
            holder: fixture::entity(4),
            claims: vec![record(70)],
            policy: fixture::content(),
            permitted_audience: AudienceScope::Members(vec![fixture::member(3)]),
        }],
    }];
    state
}

fn claim_limits() -> ClaimPerceptionLimits {
    ClaimPerceptionLimits {
        maximum_scan_records: 512,
        maximum_record_comparisons: 512,
        maximum_selected_claims: 16,
    }
}

fn claim_ids(current: &Checkpoint, observer: ObserverScope) -> Vec<RecordId> {
    perceive_claims(
        current,
        current.basis(),
        current.pins(),
        observer,
        claim_limits(),
    )
    .unwrap()
    .claims()
    .iter()
    .map(|claim| claim.id)
    .collect()
}

fn fact_ids(current: &Checkpoint, observer: ObserverScope) -> Vec<FactId> {
    perceive(
        current,
        current.basis(),
        current.pins(),
        observer,
        PerceptionLimits {
            maximum_scan_records: 512,
            maximum_member_comparisons: 512,
            maximum_selected_facts: 16,
        },
    )
    .unwrap()
    .facts()
    .iter()
    .map(|fact| fact.id)
    .collect()
}

fn direct() -> WitnessEligibility<'static> {
    WitnessEligibility {
        witness: record(40),
        recipient: fixture::member(3),
        route: WitnessRoute::Sight,
    }
}

fn stage(
    fixture: &Fixture,
    owner: &SourceOwner,
    current: &Checkpoint,
    eligible: Option<&[WitnessEligibility<'_>]>,
) -> Result<Checkpoint, WitnessStagingError<SourceRefusal>> {
    let input = fixture.input(current.basis());
    fixture.handler(owner, current.basis(), eligible).stage(
        RulesCommandInput {
            command: &input,
            supplied_draws: &[],
        },
        current,
    )
}

#[test]
fn explicit_fact_audiences_separate_display_group_and_individual_member() {
    let fixture = Fixture::new();
    let mut state = fixture.state();
    state.facts[3].audience = AudienceScope::Members(vec![fixture::member(3), fixture::member(5)]);
    let current = fixture.checkpoint(fixture::basis(), state);

    assert_eq!(fact_ids(&current, ObserverScope::Shared), vec![fact(30)]);
    assert_eq!(
        fact_ids(&current, ObserverScope::Member(fixture::member(3))),
        vec![fact(30), fact(31), fact(33)]
    );
    assert_eq!(
        fact_ids(&current, ObserverScope::Member(fixture::member(5))),
        vec![fact(30), fact(33)]
    );
    assert_eq!(current.state().facts[2].audience, AudienceScope::Host);
}

#[test]
fn secret_owner_narrows_only_its_exact_claim_with_public_evidence_unchanged() {
    let fixture = Fixture::new();
    let current = fixture.checkpoint(fixture::basis(), claim_state(&fixture));
    let original = current.clone();

    assert_eq!(
        claim_ids(&current, ObserverScope::Member(fixture::member(3))),
        vec![record(70), record(71)]
    );
    assert_eq!(
        claim_ids(&current, ObserverScope::Member(fixture::member(5))),
        vec![record(71)]
    );
    assert_eq!(claim_ids(&current, ObserverScope::Shared), vec![record(71)]);
    for observer in [
        ObserverScope::Shared,
        ObserverScope::Member(fixture::member(3)),
        ObserverScope::Member(fixture::member(5)),
    ] {
        assert!(fact_ids(&current, observer).contains(&fact(30)));
    }
    let owner = SourceOwner::new(&fixture, &current);
    let next = registered(&fixture, &owner, &current, &fixture.input(current.basis())).unwrap();
    assert_eq!(next.state().knowledge, support::expected_grants());
    assert_eq!(next.state().facts, original.state().facts);
    assert_eq!(next.state().beliefs, original.state().beliefs);
    assert_eq!(
        next.state().continuity.npcs,
        original.state().continuity.npcs
    );
    assert_eq!(current, original);
}

#[test]
fn hidden_false_belief_variants_do_not_change_public_evidence_or_witness_grants() {
    let fixture = Fixture::new();
    let mut first = claim_state(&fixture);
    first.beliefs[0].audience = AudienceScope::Members(vec![fixture::member(3)]);
    let mut second = first.clone();
    second.beliefs[0].claim = "different synthetic private false belief".to_owned();
    let first = fixture.checkpoint(fixture::basis(), first);
    let second = fixture.checkpoint(fixture::basis(), second);

    assert_eq!(
        claim_ids(&first, ObserverScope::Shared),
        claim_ids(&second, ObserverScope::Shared)
    );
    for observer in [
        ObserverScope::Shared,
        ObserverScope::Member(fixture::member(5)),
    ] {
        assert_eq!(fact_ids(&first, observer), fact_ids(&second, observer));
    }
    for current in [&first, &second] {
        let owner = SourceOwner::new(&fixture, current);
        let next = registered(&fixture, &owner, current, &fixture.input(current.basis())).unwrap();
        assert_eq!(next.state().knowledge, support::expected_grants());
        assert_eq!(next.state().facts, current.state().facts);
        assert_eq!(next.state().beliefs, current.state().beliefs);
    }
}

#[test]
fn malformed_or_competing_claim_ownership_suppresses_only_the_exact_claim() {
    let fixture = Fixture::new();
    for case in ["holder", "container", "competing"] {
        let mut state = claim_state(&fixture);
        match case {
            "holder" => state.continuity.npcs[0].secrets[0].holder = fixture::entity(6),
            "container" => state.continuity.npcs[0].entity = fixture::entity(6),
            _ => {
                let duplicate = state.continuity.npcs[0].secrets[0].clone();
                state.continuity.npcs[0].secrets.push(duplicate);
            }
        }
        let current = fixture.checkpoint(fixture::basis(), state);
        let original = current.clone();

        assert_eq!(
            claim_ids(&current, ObserverScope::Member(fixture::member(3))),
            vec![record(71)]
        );
        assert!(fact_ids(&current, ObserverScope::Shared).contains(&fact(30)));
        assert_eq!(current, original);
    }
}

#[test]
fn policy_permission_never_widens_claim_audience_and_unknown_policy_is_refused() {
    let fixture = Fixture::new();
    let mut state = claim_state(&fixture);
    state.beliefs[0].audience = AudienceScope::Members(vec![fixture::member(3)]);
    state.continuity.npcs[0].secrets[0].permitted_audience = AudienceScope::Shared;
    let current = fixture.checkpoint(fixture::basis(), state.clone());
    assert_eq!(
        claim_ids(&current, ObserverScope::Member(fixture::member(5))),
        vec![record(71)]
    );
    assert_eq!(claim_ids(&current, ObserverScope::Shared), vec![record(71)]);

    state.continuity.npcs[0].secrets[0].policy = support::content("unadmitted-secret-policy");
    assert_eq!(
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            fixture::basis(),
            fixture.pins.clone(),
            state,
            fixture.inventory(),
            fixture::limits(),
        ),
        Err(CheckpointError::InvalidReference)
    );
}

#[test]
fn claim_projection_does_not_release_private_evidence_ids_through_public_claims() {
    let fixture = Fixture::new();
    let mut state = claim_state(&fixture);
    state.beliefs[1].evidence = vec![fact(32)];
    let current = fixture.checkpoint(fixture::basis(), state);
    assert!(claim_ids(&current, ObserverScope::Shared).is_empty());
    assert_eq!(
        claim_ids(&current, ObserverScope::Member(fixture::member(3))),
        vec![record(70)]
    );
    assert!(
        current
            .state()
            .beliefs
            .iter()
            .any(|claim| claim.id == record(71))
    );
    assert_eq!(current.state().facts[2].audience, AudienceScope::Host);
}

#[test]
fn current_claim_and_fact_permissions_override_retained_grants_after_revocation() {
    let fixture = Fixture::new();
    let mut state = claim_state(&fixture);
    state.continuity.npcs[0].secrets[0].permitted_audience =
        AudienceScope::Members(vec![fixture::member(3), fixture::member(5)]);
    state.knowledge.push(KnowledgeGrant {
        observer: fixture::member(5),
        fact: fact(30),
        source: fact(30),
    });
    let admitted = fixture.checkpoint(fixture::basis(), state.clone());
    assert_eq!(
        claim_ids(&admitted, ObserverScope::Member(fixture::member(5))),
        vec![record(70), record(71)]
    );
    state.continuity.npcs[0].secrets[0].permitted_audience =
        AudienceScope::Members(vec![fixture::member(3)]);
    state.facts[0].audience = AudienceScope::Members(vec![fixture::member(3)]);
    let revoked = fixture.checkpoint(fixture::basis(), state);
    assert!(claim_ids(&revoked, ObserverScope::Member(fixture::member(5))).is_empty());
    assert!(!fact_ids(&revoked, ObserverScope::Member(fixture::member(5))).contains(&fact(30)));
    let owner = SourceOwner::new(&fixture, &revoked);
    let listener = [fixture.eligible()[2]];
    assert_eq!(
        stage(&fixture, &owner, &revoked, Some(&listener)),
        Err(WitnessStagingError::Witness(WitnessError::AudienceDenied))
    );
    assert_eq!(revoked.state().knowledge, admitted.state().knowledge);
}

#[test]
fn legitimate_multiple_grant_sources_and_duplicate_witness_noop_remain_valid() {
    let fixture = Fixture::new();
    let mut state = fixture.state();
    state.knowledge = vec![
        KnowledgeGrant {
            observer: fixture::member(3),
            fact: fact(30),
            source: fact(30),
        },
        KnowledgeGrant {
            observer: fixture::member(3),
            fact: fact(30),
            source: fact(31),
        },
    ];
    let current = fixture.checkpoint(fixture::basis(), state);
    let owner = SourceOwner::new(&fixture, &current);
    let eligible = [direct(), direct()];
    let next = stage(&fixture, &owner, &current, Some(&eligible)).unwrap();

    assert_eq!(owner.calls.get(), 1);
    assert_eq!(next.state().knowledge, current.state().knowledge);
    let proposal = propose_witness_grants(
        &current,
        current.basis(),
        current.pins(),
        &fixture.registration.witness_policy,
        &eligible,
        fixture
            .handler(&owner, current.basis(), Some(&eligible))
            .limits
            .witness,
    )
    .unwrap();
    assert!(proposal.grants().is_empty());
    owner.refusal.set(Some(SourceRefusal::Withdrawn));
    assert_eq!(
        stage(&fixture, &owner, &current, Some(&eligible)),
        Err(WitnessStagingError::Source(SourceRefusal::Withdrawn))
    );
    assert_eq!(current.state().knowledge.len(), 2);
}

#[test]
fn captured_native_source_refuses_altered_witness_wrong_member_and_missing_producer() {
    let fixture = Fixture::new();
    let prepared = fixture.current();
    let source = SourceOwner::new(&fixture, &prepared);
    assert_eq!(
        stage(&fixture, &source, &prepared, None),
        Err(WitnessStagingError::MissingProducer)
    );
    for case in ["witness", "member"] {
        let mut state = prepared.state().clone();
        match case {
            "witness" => state.continuity.witnesses[0].perceived_at.ticks -= 1,
            _ => state.members[1].character = None,
        }
        let current = fixture.checkpoint(prepared.basis(), state);
        let eligible = fixture.eligible();
        assert_eq!(
            stage(&fixture, &source, &current, Some(&eligible)),
            Err(WitnessStagingError::Source(SourceRefusal::Eligibility))
        );
        assert!(current.state().knowledge.is_empty());
    }
}

#[test]
fn claim_selection_rechecks_basis_pins_membership_and_each_finite_dimension() {
    let fixture = Fixture::new();
    let current = fixture.checkpoint(fixture::basis(), claim_state(&fixture));
    let original = current.clone();
    let mut stale = current.basis();
    stale.revision = stale.revision.next_sequence().unwrap();
    assert_eq!(
        perceive_claims(
            &current,
            stale,
            current.pins(),
            ObserverScope::Shared,
            claim_limits()
        )
        .unwrap_err(),
        PerceptionError::Checkpoint(CheckpointError::StaleBasis)
    );
    let mut pins = current.pins().clone();
    pins.content.content_digest = ContentDigest([99; 32]);
    assert_eq!(
        perceive_claims(
            &current,
            current.basis(),
            &pins,
            ObserverScope::Shared,
            claim_limits()
        )
        .unwrap_err(),
        PerceptionError::Checkpoint(CheckpointError::ContentMismatch)
    );
    assert_eq!(
        perceive_claims(
            &current,
            current.basis(),
            current.pins(),
            ObserverScope::Member(fixture::member(99)),
            claim_limits(),
        )
        .unwrap_err(),
        PerceptionError::ObserverUnavailable
    );
    for (limits, error) in [
        (
            ClaimPerceptionLimits {
                maximum_scan_records: 0,
                ..claim_limits()
            },
            PerceptionError::ScanCapacity,
        ),
        (
            ClaimPerceptionLimits {
                maximum_record_comparisons: 0,
                ..claim_limits()
            },
            PerceptionError::ComparisonCapacity,
        ),
        (
            ClaimPerceptionLimits {
                maximum_selected_claims: 0,
                ..claim_limits()
            },
            PerceptionError::ResultCapacity,
        ),
    ] {
        assert_eq!(
            perceive_claims(
                &current,
                current.basis(),
                current.pins(),
                ObserverScope::Shared,
                limits
            )
            .unwrap_err(),
            error
        );
    }
    let selected = perceive_claims(
        &current,
        current.basis(),
        current.pins(),
        ObserverScope::Member(fixture::member(3)),
        claim_limits(),
    )
    .unwrap();
    assert_eq!(selected.basis(), current.basis());
    assert_eq!(selected.pins(), current.pins());
    assert!(std::ptr::eq(
        selected.claims()[0],
        &current.state().beliefs[0]
    ));
    let debug = format!("{selected:?}");
    assert!(!debug.contains("synthetic") && !debug.contains("70"));
    assert_eq!(current, original);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn committed_witness_receipt_rechecks_current_claim_scope_and_projects_exact_member_facts() {
    use df_session::submission::SubmissionOutcome;
    use support::durable::{Failure, setup, submit};

    let (mut owner, database, scope) = setup(Failure::None);
    let original = owner.checkpoint().clone();
    let receipt = submit(&mut owner, &scope);
    assert!(matches!(receipt, SubmissionOutcome::Confirmed(_)));
    let current = owner.checkpoint();

    assert_eq!(current, &database.borrow().checkpoint);
    support::assert_preserved(&original, current);
    assert!(claim_ids(current, ObserverScope::Shared).is_empty());
    assert_eq!(
        fact_ids(current, ObserverScope::Member(fixture::member(3))),
        vec![fact(30), fact(31)]
    );
    assert_eq!(
        fact_ids(current, ObserverScope::Member(fixture::member(5))),
        vec![fact(30), fact(33)]
    );
    assert_eq!(fact_ids(current, ObserverScope::Shared), vec![fact(30)]);
    assert_eq!(
        perceive_claims(
            current,
            original.basis(),
            current.pins(),
            ObserverScope::Member(fixture::member(3)),
            claim_limits(),
        )
        .unwrap_err(),
        PerceptionError::Checkpoint(CheckpointError::StaleBasis)
    );

    assert_eq!(submit(&mut owner, &scope), receipt);
    let database = database.borrow();
    assert_eq!(owner.checkpoint(), &database.checkpoint);
    assert_eq!(
        (
            database.decisions,
            database.commits,
            database.publications,
            database.wakes,
            database.ledger.len(),
        ),
        (1, 1, 1, 1, 1)
    );
}
