//! Bounded ADR 0002 example; eligibility never authorizes deletion.
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtifactClass {
    BuildOrTemporary,
    Source,
    Git,
    Database,
    DatabaseSidecar,
    RuntimeData,
    UserDeliverable,
    SharedCache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Boundary {
    ApprovedRegularOutput,
    OutsideApprovedRoots,
    SymlinkOrUnknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Terminal(Duration),
    ConfirmedAbandoned(Duration),
    Active,
    ExpiredLease,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UseClaim {
    None,
    Active,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Retention {
    Disposable,
    Evidence,
    LatestVerifiedBuild,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Artifact {
    class: ArtifactClass,
    boundary: Boundary,
    owner: Owner,
    use_claim: UseClaim,
    reproducible_or_superseded: bool,
    retention: Retention,
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    EligibleForCoordinatorOwnershipRecheck,
    RetainProtected,
    RetainUnknownBoundary,
    RetainUnconfirmedOwner,
    RetainUseClaim,
    RetainUnreproducible,
    RetainEvidence,
    RetainYoungTerminal,
}

fn classify_current(artifact: Artifact) -> Outcome {
    if artifact.class != ArtifactClass::BuildOrTemporary {
        return Outcome::RetainProtected;
    }
    if artifact.boundary != Boundary::ApprovedRegularOutput {
        return Outcome::RetainUnknownBoundary;
    }
    let terminal_age = match artifact.owner {
        Owner::Terminal(age) | Owner::ConfirmedAbandoned(age) => age,
        Owner::Active | Owner::ExpiredLease | Owner::Unknown => {
            return Outcome::RetainUnconfirmedOwner;
        }
    };
    if artifact.use_claim != UseClaim::None {
        return Outcome::RetainUseClaim;
    }
    if !artifact.reproducible_or_superseded {
        return Outcome::RetainUnreproducible;
    }
    if artifact.retention != Retention::Disposable {
        return Outcome::RetainEvidence;
    }
    if terminal_age < Duration::from_secs(30 * 60) {
        return Outcome::RetainYoungTerminal;
    }
    Outcome::EligibleForCoordinatorOwnershipRecheck
}

fn otherwise_eligible() -> Artifact {
    Artifact {
        class: ArtifactClass::BuildOrTemporary,
        boundary: Boundary::ApprovedRegularOutput,
        owner: Owner::Terminal(Duration::from_secs(1800)),
        use_claim: UseClaim::None,
        reproducible_or_superseded: true,
        retention: Retention::Disposable,
    }
}

#[test]
fn all_conditions_are_required_before_coordinator_recheck() {
    for owner in [
        Owner::Terminal(Duration::from_secs(1800)),
        Owner::ConfirmedAbandoned(Duration::from_secs(1800)),
    ] {
        assert_eq!(
            classify_current(Artifact {
                owner,
                ..otherwise_eligible()
            }),
            Outcome::EligibleForCoordinatorOwnershipRecheck
        );
    }
}

#[test]
fn protected_artifacts_never_become_disposable() {
    for class in [
        ArtifactClass::Source,
        ArtifactClass::Git,
        ArtifactClass::Database,
        ArtifactClass::DatabaseSidecar,
        ArtifactClass::RuntimeData,
        ArtifactClass::UserDeliverable,
        ArtifactClass::SharedCache,
    ] {
        assert_eq!(
            classify_current(Artifact {
                class,
                ..otherwise_eligible()
            }),
            Outcome::RetainProtected
        );
    }
}

#[test]
fn outside_symlink_and_unknown_boundaries_are_retained() {
    for boundary in [Boundary::OutsideApprovedRoots, Boundary::SymlinkOrUnknown] {
        assert_eq!(
            classify_current(Artifact {
                boundary,
                ..otherwise_eligible()
            }),
            Outcome::RetainUnknownBoundary
        );
    }
}

#[test]
fn age_or_expired_lease_does_not_establish_terminal_ownership() {
    for owner in [Owner::Active, Owner::ExpiredLease, Owner::Unknown] {
        assert_eq!(
            classify_current(Artifact {
                owner,
                ..otherwise_eligible()
            }),
            Outcome::RetainUnconfirmedOwner
        );
    }
}

#[test]
fn active_and_unknown_use_claims_are_retained() {
    for use_claim in [UseClaim::Active, UseClaim::Unknown] {
        assert_eq!(
            classify_current(Artifact {
                use_claim,
                ..otherwise_eligible()
            }),
            Outcome::RetainUseClaim
        );
    }
}

#[test]
fn evidence_and_latest_verified_build_are_retained() {
    for retention in [Retention::Evidence, Retention::LatestVerifiedBuild] {
        assert_eq!(
            classify_current(Artifact {
                retention,
                ..otherwise_eligible()
            }),
            Outcome::RetainEvidence
        );
    }
}

#[test]
fn unknown_reproduction_is_retained() {
    assert_eq!(
        classify_current(Artifact {
            reproducible_or_superseded: false,
            ..otherwise_eligible()
        }),
        Outcome::RetainUnreproducible
    );
}

#[test]
fn thirty_minute_boundary_uses_terminal_age() {
    for owner in [
        Owner::Terminal(Duration::from_secs(1799)),
        Owner::ConfirmedAbandoned(Duration::from_secs(1799)),
    ] {
        assert_eq!(
            classify_current(Artifact {
                owner,
                ..otherwise_eligible()
            }),
            Outcome::RetainYoungTerminal
        );
    }
}

#[test]
fn a_new_claim_invalidates_previous_eligibility() {
    let previous = otherwise_eligible();
    assert_eq!(
        classify_current(previous),
        Outcome::EligibleForCoordinatorOwnershipRecheck
    );
    let current = Artifact {
        use_claim: UseClaim::Active,
        ..previous
    };
    assert_eq!(classify_current(current), Outcome::RetainUseClaim);
}
