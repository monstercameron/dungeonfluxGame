use std::collections::BTreeMap;

use df_knowledge::beliefs::{
    BeliefBasis, BeliefUpdate, BeliefValidationError, validate_belief_update,
};
use df_types::{MemberId, OperationId, RecoveryEpoch, SessionRevision};

#[derive(Clone, PartialEq, Eq)]
struct FixtureBasis {
    world_revision: SessionRevision,
    knowledge_revision: SessionRevision,
    policy_revision: u64,
    operation: OperationId,
}

#[derive(Clone, PartialEq, Eq)]
struct FixtureBelief {
    basis: FixtureBasis,
    actor: MemberId,
    subject: &'static str,
    claim: bool,
    evidence: &'static str,
}

impl BeliefUpdate for FixtureBelief {
    type Basis = FixtureBasis;
    type Actor = MemberId;
    type Subject = &'static str;
    type Claim = bool;
    type Evidence = &'static str;

    fn basis(&self) -> &Self::Basis {
        &self.basis
    }

    fn actor(&self) -> &Self::Actor {
        &self.actor
    }

    fn subject(&self) -> &Self::Subject {
        &self.subject
    }

    fn claim(&self) -> &Self::Claim {
        &self.claim
    }

    fn evidence(&self) -> &Self::Evidence {
        &self.evidence
    }
}

struct FixtureOwner<'a> {
    basis: &'a FixtureBasis,
    world: &'a BTreeMap<&'static str, bool>,
    actor: MemberId,
    observations: &'a [FixtureBelief],
}

impl BeliefBasis<FixtureBelief> for FixtureOwner<'_> {
    fn current_basis(&self) -> &FixtureBasis {
        self.basis
    }

    fn permits_subject(&self, subject: &&'static str) -> bool {
        *subject == "door-is-locked" && self.world.contains_key(subject)
    }

    fn permits_attribution(&self, actor: &MemberId, _: &&'static str) -> bool {
        *actor == self.actor
    }

    fn permits_evidence(&self, update: &FixtureBelief) -> bool {
        self.observations.iter().any(|observed| {
            observed.actor == update.actor
                && observed.subject == update.subject
                && observed.claim == update.claim
                && observed.evidence == update.evidence
                && observed.basis == update.basis
        })
    }
}

fn basis() -> FixtureBasis {
    let revision = SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 4);
    FixtureBasis {
        world_revision: revision,
        knowledge_revision: revision,
        policy_revision: 2,
        operation: OperationId::from_bytes(&[3; 16]).unwrap(),
    }
}

fn belief(basis: &FixtureBasis, claim: bool) -> FixtureBelief {
    FixtureBelief {
        basis: basis.clone(),
        actor: MemberId::from_bytes(&[1; 16]).unwrap(),
        subject: "door-is-locked",
        claim,
        evidence: "attributed-witness-report",
    }
}

fn world() -> BTreeMap<&'static str, bool> {
    BTreeMap::from([("door-is-locked", true), ("hidden-treasure-exists", true)])
}

#[test]
fn attributed_false_belief_updates_only_the_callers_belief_map() {
    let world = world();
    let original_world = world.clone();
    let basis = basis();
    let false_belief = belief(&basis, false);
    let observations = [false_belief.clone()];
    let owner = FixtureOwner {
        basis: &basis,
        world: &world,
        actor: false_belief.actor,
        observations: &observations,
    };
    let proposal = validate_belief_update(&owner, false_belief.clone()).unwrap();
    assert!(proposal.update() == &false_belief);
    let mut committed_beliefs = BTreeMap::new();
    let record = proposal.into_update();
    committed_beliefs.insert((record.actor, record.subject), record.claim);
    assert!(!committed_beliefs[&(false_belief.actor, false_belief.subject)]);
    assert_eq!(world, original_world);
    assert!(world["door-is-locked"]);
}

#[test]
fn true_and_false_claims_use_the_same_provenance_validation() {
    let world = world();
    let basis = basis();
    let observations = [belief(&basis, true), belief(&basis, false)];
    let owner = FixtureOwner {
        basis: &basis,
        world: &world,
        actor: observations[0].actor,
        observations: &observations,
    };
    for original in &observations {
        let proposal = validate_belief_update(&owner, original.clone()).unwrap();
        assert!(proposal.update() == original);
    }
}

#[test]
fn unrelated_hidden_truth_cannot_change_a_belief_proposal() {
    let mut world_a = world();
    let mut world_b = world_a.clone();
    world_b.insert("hidden-treasure-exists", false);
    world_a.insert("unrelated-secret", true);
    let basis = basis();
    let update = belief(&basis, false);
    let observations = [update.clone()];
    let make_owner = |world| FixtureOwner {
        basis: &basis,
        world,
        actor: update.actor,
        observations: &observations,
    };
    let first = validate_belief_update(&make_owner(&world_a), update.clone()).unwrap();
    let second = validate_belief_update(&make_owner(&world_b), update).unwrap();
    assert!(first.update() == second.update());
}

#[test]
fn forged_actor_is_rejected_without_changing_truth() {
    let world = world();
    let original_world = world.clone();
    let basis = basis();
    let allowed = belief(&basis, false);
    let observations = [allowed.clone()];
    let owner = FixtureOwner {
        basis: &basis,
        world: &world,
        actor: allowed.actor,
        observations: &observations,
    };
    let mut forged = allowed;
    forged.actor = MemberId::from_bytes(&[2; 16]).unwrap();
    assert!(matches!(
        validate_belief_update(&owner, forged),
        Err(BeliefValidationError::InvalidAttribution)
    ));
    assert_eq!(world, original_world);
}

#[test]
fn unknown_and_undisclosed_subjects_have_the_same_safe_rejection() {
    let world = world();
    let basis = basis();
    let allowed = belief(&basis, false);
    let observations = [allowed.clone()];
    let owner = FixtureOwner {
        basis: &basis,
        world: &world,
        actor: allowed.actor,
        observations: &observations,
    };
    for subject in ["hidden-treasure-exists", "nonexistent-subject"] {
        let mut update = allowed.clone();
        update.subject = subject;
        assert!(matches!(
            validate_belief_update(&owner, update),
            Err(BeliefValidationError::UnknownSubject)
        ));
    }
}

#[test]
fn evidence_cannot_be_reused_for_a_different_claim_or_source() {
    let world = world();
    let basis = basis();
    let allowed = belief(&basis, false);
    let observations = [allowed.clone()];
    let owner = FixtureOwner {
        basis: &basis,
        world: &world,
        actor: allowed.actor,
        observations: &observations,
    };
    let mut changed_claim = allowed.clone();
    changed_claim.claim = true;
    let mut changed_evidence = allowed;
    changed_evidence.evidence = "undisclosed-private-evidence";
    for update in [changed_claim, changed_evidence] {
        assert!(matches!(
            validate_belief_update(&owner, update),
            Err(BeliefValidationError::UndisclosedEvidence)
        ));
    }
}

#[test]
fn every_changed_owner_basis_component_rejects_stale_updates() {
    let world = world();
    let basis = basis();
    let allowed = belief(&basis, false);
    let observations = [allowed.clone()];
    let owner = FixtureOwner {
        basis: &basis,
        world: &world,
        actor: allowed.actor,
        observations: &observations,
    };
    for component in 0..4 {
        let mut stale = allowed.clone();
        match component {
            0 => stale.basis.world_revision = basis.world_revision.next_sequence().unwrap(),
            1 => stale.basis.knowledge_revision = basis.knowledge_revision.next_sequence().unwrap(),
            2 => stale.basis.policy_revision += 1,
            _ => stale.basis.operation = OperationId::from_bytes(&[4; 16]).unwrap(),
        }
        assert!(matches!(
            validate_belief_update(&owner, stale),
            Err(BeliefValidationError::StaleBasis)
        ));
    }
}

#[test]
fn repeated_validation_preserves_exact_record_without_committing_anything() {
    let world = world();
    let original_world = world.clone();
    let basis = basis();
    let original = belief(&basis, false);
    let observations = [original.clone()];
    let owner = FixtureOwner {
        basis: &basis,
        world: &world,
        actor: original.actor,
        observations: &observations,
    };
    let first = validate_belief_update(&owner, original.clone()).unwrap();
    let second = validate_belief_update(&owner, original.clone()).unwrap();
    assert!(first.update() == second.update());
    assert!(second.into_update() == original);
    assert_eq!(world, original_world);
}
