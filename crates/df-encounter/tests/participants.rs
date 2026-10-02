use std::cell::Cell;

use df_encounter::participants::{
    ParticipantError, ParticipantLimits, ParticipantOwner, ParticipantStatus, validate_participants,
};

#[derive(Debug, Eq, PartialEq)]
struct Actor(u16);

#[derive(Debug, Eq, PartialEq)]
struct Basis {
    run: u16,
    world: u16,
    disclosure: u16,
}

#[derive(Debug, Eq, PartialEq)]
enum PolicyError {
    StaleBasis,
    PolicyUnavailable,
}

struct Authority {
    basis_checks: Cell<usize>,
    actor_checks: Cell<usize>,
    hidden_actor: u16,
    refuse_actor: bool,
}

impl Authority {
    fn new() -> Self {
        Self {
            basis_checks: Cell::new(0),
            actor_checks: Cell::new(0),
            hidden_actor: 4,
            refuse_actor: false,
        }
    }
}

impl ParticipantOwner for Authority {
    type Actor = Actor;
    type Basis = Basis;
    type Error = PolicyError;

    fn validate_basis(&self, proposed: &Basis, current: &Basis) -> Result<(), PolicyError> {
        self.basis_checks.set(self.basis_checks.get() + 1);
        if proposed != current {
            return Err(PolicyError::StaleBasis);
        }
        Ok(())
    }

    fn participant_status(
        &self,
        _: &Basis,
        actor: &Actor,
    ) -> Result<ParticipantStatus, PolicyError> {
        self.actor_checks.set(self.actor_checks.get() + 1);
        if self.refuse_actor {
            return Err(PolicyError::PolicyUnavailable);
        }
        Ok(match actor.0 {
            1 | 2 => ParticipantStatus::Available,
            3 => ParticipantStatus::Unavailable,
            value if value == self.hidden_actor => ParticipantStatus::Undisclosed,
            _ => ParticipantStatus::Unknown,
        })
    }
}

fn basis() -> Basis {
    Basis {
        run: 1,
        world: 2,
        disclosure: 3,
    }
}

fn limits() -> ParticipantLimits {
    ParticipantLimits {
        max_participants: 3,
        max_identity_comparisons: 3,
    }
}

#[test]
fn available_participants_preserve_exact_borrowed_order_and_current_basis() {
    let owner = Authority::new();
    let current = basis();
    let proposal = basis();
    let actors = [Actor(2), Actor(1)];
    let admitted = validate_participants(&owner, &proposal, &current, &actors, limits()).unwrap();
    assert!(std::ptr::eq(admitted.actors(), actors.as_slice()));
    assert!(std::ptr::eq(admitted.current_basis(), &current));
    assert_eq!(admitted.actors(), &[Actor(2), Actor(1)]);
    assert_eq!(current, basis());
    assert_eq!(owner.basis_checks.get(), 1);
    assert_eq!(owner.actor_checks.get(), 2);
}

#[test]
fn unknown_unavailable_and_undisclosed_actors_are_rejected_without_substitution() {
    for (actor, status) in [
        (9, ParticipantStatus::Unknown),
        (3, ParticipantStatus::Unavailable),
        (4, ParticipantStatus::Undisclosed),
    ] {
        let owner = Authority::new();
        let current = basis();
        let actors = [Actor(1), Actor(actor), Actor(2)];
        let error = validate_participants(&owner, &current, &current, &actors, limits()).err();
        assert_eq!(
            error,
            Some(ParticipantError::Inadmissible { index: 1, status })
        );
        assert_eq!(actors, [Actor(1), Actor(actor), Actor(2)]);
        assert_eq!(owner.actor_checks.get(), 2);
    }
}

#[test]
fn stale_run_world_or_disclosure_basis_prevents_actor_checks() {
    for proposed in [
        Basis { run: 2, ..basis() },
        Basis {
            world: 3,
            ..basis()
        },
        Basis {
            disclosure: 4,
            ..basis()
        },
    ] {
        let owner = Authority::new();
        let current = basis();
        let actors = [Actor(1)];
        let error = validate_participants(&owner, &proposed, &current, &actors, limits()).err();
        assert_eq!(
            error,
            Some(ParticipantError::Owner(PolicyError::StaleBasis))
        );
        assert_eq!(owner.actor_checks.get(), 0);
    }
}

#[test]
fn duplicate_identity_is_rejected_before_status_lookup() {
    let owner = Authority::new();
    let current = basis();
    let actors = [Actor(2), Actor(1), Actor(2)];
    let error = validate_participants(&owner, &current, &current, &actors, limits()).err();
    assert_eq!(
        error,
        Some(ParticipantError::Duplicate {
            first: 0,
            duplicate: 2
        })
    );
    assert_eq!(owner.actor_checks.get(), 0);
}

#[test]
fn participant_and_comparison_capacity_reject_before_owner_work() {
    let current = basis();
    let actors = [Actor(1), Actor(2)];
    for (capacity, expected) in [
        (
            ParticipantLimits {
                max_participants: 1,
                max_identity_comparisons: 1,
            },
            ParticipantError::ParticipantCapacity {
                required: 2,
                limit: 1,
            },
        ),
        (
            ParticipantLimits {
                max_participants: 2,
                max_identity_comparisons: 0,
            },
            ParticipantError::ComparisonCapacity {
                required: 1,
                limit: 0,
            },
        ),
    ] {
        let owner = Authority::new();
        let error = validate_participants(&owner, &current, &current, &actors, capacity).err();
        assert_eq!(error, Some(expected));
        assert_eq!(owner.basis_checks.get(), 0);
        assert_eq!(owner.actor_checks.get(), 0);
    }
}

#[test]
fn empty_and_exact_capacity_proposals_are_admitted() {
    let owner = Authority::new();
    let current = basis();
    let empty: [Actor; 0] = [];
    let zero_limits = ParticipantLimits {
        max_participants: 0,
        max_identity_comparisons: 0,
    };
    assert!(validate_participants(&owner, &current, &current, &empty, zero_limits).is_ok());
    let actors = [Actor(1), Actor(2)];
    let exact_limits = ParticipantLimits {
        max_participants: 2,
        max_identity_comparisons: 1,
    };
    assert!(validate_participants(&owner, &current, &current, &actors, exact_limits).is_ok());
}

#[test]
fn authority_failure_returns_typed_error_and_no_partial_success() {
    let owner = Authority {
        refuse_actor: true,
        ..Authority::new()
    };
    let current = basis();
    let actors = [Actor(1), Actor(2)];
    let error = validate_participants(&owner, &current, &current, &actors, limits()).err();
    assert_eq!(
        error,
        Some(ParticipantError::Owner(PolicyError::PolicyUnavailable))
    );
    assert_eq!(owner.actor_checks.get(), 1);
    assert_eq!(actors, [Actor(1), Actor(2)]);
}

#[test]
fn unrelated_hidden_actor_changes_do_not_change_visible_admission() {
    let current = basis();
    let actors = [Actor(2), Actor(1)];
    for hidden_actor in [4, 5] {
        let owner = Authority {
            hidden_actor,
            ..Authority::new()
        };
        let admitted =
            validate_participants(&owner, &current, &current, &actors, limits()).unwrap();
        assert!(std::ptr::eq(admitted.actors(), actors.as_slice()));
        assert!(std::ptr::eq(admitted.current_basis(), &current));
        assert_eq!(admitted.actors(), &[Actor(2), Actor(1)]);
    }
}
