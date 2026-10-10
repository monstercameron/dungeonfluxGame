use df_session::readiness::{ReadyDecisionError, decide_ready_change};
use df_types::{MemberId, RecoveryEpoch, SessionRevision};

fn member(value: u8) -> MemberId {
    MemberId::from_bytes(&[value; 16]).expect("fixture identity is valid")
}

#[test]
fn authenticated_ready_decision_uses_current_member_and_snapshot_revision() {
    let alice = member(1);
    let bob = member(2);
    let stale = member(3);
    let current = [alice, bob];
    let ready = [bob];
    let both_ready = [alice, bob];
    let revision = SessionRevision::new(RecoveryEpoch::new(4).unwrap(), 17);

    let ready_alice = decide_ready_change(alice, 5, revision, &current, &ready, true).unwrap();
    assert_eq!(ready_alice.authenticated_member(), alice);
    assert_eq!(ready_alice.basis_revision(), revision);
    assert!(ready_alice.desired_ready());
    assert_eq!(ready_alice.projected_aggregate().capacity(), 5);
    assert_eq!(ready_alice.projected_aggregate().occupied(), 2);
    assert_eq!(ready_alice.projected_aggregate().ready(), 2);
    assert!(ready_alice.projected_aggregate().all_ready());

    let repeated = decide_ready_change(alice, 5, revision, &current, &ready, true).unwrap();
    assert_eq!(repeated, ready_alice);

    let not_ready = decide_ready_change(alice, 5, revision, &current, &both_ready, false).unwrap();
    assert_eq!(not_ready.authenticated_member(), alice);
    assert_eq!(not_ready.basis_revision(), revision);
    assert!(!not_ready.desired_ready());
    assert_eq!(not_ready.projected_aggregate().capacity(), 5);
    assert_eq!(not_ready.projected_aggregate().occupied(), 2);
    assert_eq!(not_ready.projected_aggregate().ready(), 1);
    assert!(!not_ready.projected_aggregate().all_ready());
    let repeated_not_ready =
        decide_ready_change(alice, 5, revision, &current, &both_ready, false).unwrap();
    assert_eq!(repeated_not_ready, not_ready);

    let already_not_ready = [bob];
    let unchanged =
        decide_ready_change(alice, 5, revision, &current, &already_not_ready, false).unwrap();
    assert_eq!(unchanged.projected_aggregate().ready(), 1);

    assert_eq!(
        decide_ready_change(stale, 5, revision, &current, &ready, true),
        Err(ReadyDecisionError::AuthenticatedMemberNotCurrent)
    );
    assert_eq!(
        decide_ready_change(stale, 5, revision, &current, &both_ready, false),
        Err(ReadyDecisionError::AuthenticatedMemberNotCurrent)
    );
    assert_eq!(
        decide_ready_change(alice, 0, revision, &current, &ready, true),
        Err(ReadyDecisionError::InvalidSnapshot(
            df_auth::readiness::ReadinessError::ZeroCapacity
        ))
    );
    assert_eq!(
        decide_ready_change(alice, 1, revision, &current, &ready, true),
        Err(ReadyDecisionError::InvalidSnapshot(
            df_auth::readiness::ReadinessError::CapacityExceeded
        ))
    );
    assert_eq!(
        decide_ready_change(alice, 5, revision, &[alice, alice], &[], true),
        Err(ReadyDecisionError::InvalidSnapshot(
            df_auth::readiness::ReadinessError::DuplicateCurrentMember
        ))
    );
    assert_eq!(
        decide_ready_change(alice, 5, revision, &current, &[bob, bob], true),
        Err(ReadyDecisionError::InvalidSnapshot(
            df_auth::readiness::ReadinessError::DuplicateReadyMember
        ))
    );
    assert_eq!(
        decide_ready_change(alice, 5, revision, &current, &[stale], true),
        Err(ReadyDecisionError::InvalidSnapshot(
            df_auth::readiness::ReadinessError::ReadyMemberNotCurrent
        ))
    );
}
