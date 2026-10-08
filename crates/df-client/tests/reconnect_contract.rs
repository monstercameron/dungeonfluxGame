// The source-bound decision manifest is part of this test build.
const _: &str = include_str!("reconnect_contract.json");

#[path = "support/reconnect_contract.rs"]
mod contract;

use contract::{
    ClientContract, FixtureAuthority, FixtureCredential, MemberReference, MemberState,
    PermittedPayload, Refusal, Role, Stamp,
};
use df_client::{
    connection::RpcConnection,
    connection_views::{ConnectionViewAcceptance, ConnectionViews},
    revisions::ViewAcceptance,
};
use df_types::{ClientBindingId, MemberId, RecoveryEpoch, SessionId, SessionRevision};

fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
}
fn member(role: Role) -> MemberState {
    MemberState {
        reference: MemberReference {
            session: SessionId::from_bytes(&[1; 16]).unwrap(),
            member: MemberId::from_bytes(&[2; 16]).unwrap(),
        },
        role,
        ready: true,
    }
}
fn authority(role: Role) -> FixtureAuthority {
    FixtureAuthority::new(
        member(role),
        ClientBindingId::from_bytes(&[3; 16]).unwrap(),
        FixtureCredential(11),
    )
}
fn stamp(generation: u64, epoch: u64, sequence: u64) -> Stamp {
    Stamp {
        binding_generation: generation,
        revision: revision(epoch, sequence),
        presentation_epoch: 3,
        view_sequence: 1,
        full_snapshot: true,
    }
}

#[test]
fn both_roles_resume_the_same_member_without_join_seat_allocation_or_readiness_reset() {
    for role in [Role::Player, Role::Display] {
        let mut owner = authority(role);
        let before = owner.member;
        let old = RpcConnection::new(());
        let old_generation = old.generation();
        let mut client = ClientContract::new(&owner, old_generation.clone());
        let initial = owner
            .snapshot(
                before.reference,
                role,
                FixtureCredential(11),
                stamp(7, 4, 0),
            )
            .unwrap();
        assert_eq!(
            client.accept(&old_generation, initial),
            Ok(ViewAcceptance::Applied)
        );
        old.close();
        client.disconnect();
        let new = RpcConnection::new(());
        let new_generation = new.generation();
        assert!(!old_generation.same_scope(&new_generation));
        assert_eq!(
            client.resume(&mut owner, FixtureCredential(11), new_generation.clone()),
            Ok(())
        );
        assert_eq!(owner.observations.last_resume, Some(before.reference));
        assert_eq!(
            (owner.observations.resumes, owner.observations.binds),
            (1, 1)
        );
        assert_eq!(
            (
                owner.observations.joins,
                owner.observations.allocations,
                owner.seats
            ),
            (0, 0, 1)
        );
        assert_eq!(owner.member, before);
        assert_eq!(client.member, before);
        assert_eq!(client.watermark(), Some(revision(4, 0)));
        assert!(client.current().is_none());
        let resumed = owner
            .snapshot(
                before.reference,
                role,
                FixtureCredential(11),
                stamp(8, 4, 0),
            )
            .unwrap();
        assert_eq!(
            client.accept(&new_generation, resumed),
            Ok(ViewAcceptance::Duplicate {
                current: revision(4, 0)
            })
        );
        assert_eq!(client.current(), Some(&resumed));
        assert_eq!(client.binding_generation, 8);
        println!(
            "observed {role:?}: Resume same member; Bind generation8; Join0; allocations0; seats1; readiness preserved; current snapshot retained"
        );
    }
}

#[test]
fn expired_or_wrong_credentials_require_recovery_without_join_fallback_or_old_view_revival() {
    for expired in [false, true] {
        let mut owner = authority(Role::Player);
        let old = RpcConnection::new(());
        let mut client = ClientContract::new(&owner, old.generation());
        let initial = owner
            .snapshot(
                owner.member.reference,
                Role::Player,
                FixtureCredential(11),
                stamp(7, 4, 0),
            )
            .unwrap();
        client.accept(&old.generation(), initial).unwrap();
        old.close();
        owner.credential_live = !expired;
        let credential = if expired {
            FixtureCredential(11)
        } else {
            FixtureCredential(99)
        };
        let new = RpcConnection::new(());
        assert_eq!(
            client.resume(&mut owner, credential, new.generation()),
            Err(Refusal::RecoveryRequired)
        );
        assert_eq!(
            client.accept(&new.generation(), initial),
            Err(Refusal::RecoveryRequired)
        );
        assert!(client.current().is_none());
        assert_eq!(client.watermark(), Some(revision(4, 0)));
        assert_eq!(client.member, member(Role::Player));
        assert_eq!(
            (
                owner.observations.resumes,
                owner.observations.binds,
                owner.observations.joins,
                owner.observations.allocations,
                owner.seats
            ),
            (1, 0, 0, 0, 1)
        );
    }
}

#[test]
fn mismatched_resume_identity_or_role_cannot_rebind_the_existing_member() {
    for wrong_role in [false, true] {
        let mut owner = authority(Role::Player);
        let old = RpcConnection::new(());
        let mut client = ClientContract::new(&owner, old.generation());
        old.close();
        let mut replacement = owner.member;
        if wrong_role {
            replacement.role = Role::Display;
        } else {
            replacement.reference.member = MemberId::from_bytes(&[9; 16]).unwrap();
        }
        owner.reply_override = Some(replacement);
        let new = RpcConnection::new(());
        assert_eq!(
            client.resume(&mut owner, FixtureCredential(11), new.generation()),
            Err(if wrong_role {
                Refusal::WrongRole
            } else {
                Refusal::WrongIdentity
            })
        );
        assert_eq!(client.member, member(Role::Player));
        assert_eq!(
            (
                owner.observations.binds,
                owner.observations.joins,
                owner.observations.allocations,
                owner.seats
            ),
            (0, 0, 0, 1)
        );
        assert!(client.current().is_none());
    }
}

#[test]
fn authority_filters_member_and_role_before_publication_and_display_ignores_hidden_clues() {
    let mut owner = authority(Role::Player);
    let mut foreign = owner.member.reference;
    foreign.member = MemberId::from_bytes(&[9; 16]).unwrap();
    assert_eq!(
        owner.snapshot(foreign, Role::Player, FixtureCredential(11), stamp(7, 4, 0)),
        Err(Refusal::RecoveryRequired)
    );
    assert_eq!(
        owner.snapshot(
            owner.member.reference,
            Role::Display,
            FixtureCredential(11),
            stamp(7, 4, 0)
        ),
        Err(Refusal::WrongRole)
    );
    assert_eq!(owner.observations.publications, 0);
    let mut display = authority(Role::Display);
    let first = display
        .snapshot(
            display.member.reference,
            Role::Display,
            FixtureCredential(11),
            stamp(7, 4, 0),
        )
        .unwrap();
    display.private_clue = "different synthetic hidden clue";
    let second = display
        .snapshot(
            display.member.reference,
            Role::Display,
            FixtureCredential(11),
            stamp(7, 4, 0),
        )
        .unwrap();
    assert_eq!(first, second);
    assert!(matches!(first.payload, PermittedPayload::Display { .. }));
}

#[test]
fn current_consumer_fences_old_connections_and_preserves_composite_revision_watermark() {
    let owner = authority(Role::Player);
    let old = RpcConnection::new(());
    let old_generation = old.generation();
    let mut views = ConnectionViews::new(owner.binding, old_generation.clone());
    assert_eq!(
        views.accept(&old_generation, owner.binding, revision(4, 0), "current"),
        ConnectionViewAcceptance::View(ViewAcceptance::Applied)
    );
    old.close();
    let new = RpcConnection::new(());
    let new_generation = new.generation();
    assert!(views.reconnect(new_generation.clone()));
    assert_eq!(
        views.accept(
            &old_generation,
            owner.binding,
            revision(99, 0),
            "old private view"
        ),
        ConnectionViewAcceptance::ObsoleteGeneration
    );
    assert_eq!(
        views.accept(
            &new_generation,
            owner.binding,
            revision(3, u64::MAX),
            "older recovery"
        ),
        ConnectionViewAcceptance::View(ViewAcceptance::Stale {
            current: revision(4, 0)
        })
    );
    assert_eq!(
        views.accept(
            &new_generation,
            ClientBindingId::from_bytes(&[9; 16]).unwrap(),
            revision(99, 0),
            "wrong binding"
        ),
        ConnectionViewAcceptance::View(ViewAcceptance::WrongBinding)
    );
    assert_eq!(views.current(), Some((revision(4, 0), &"current")));
    assert_eq!(
        views.accept(
            &new_generation,
            owner.binding,
            revision(4, 1),
            "same phase update"
        ),
        ConnectionViewAcceptance::View(ViewAcceptance::Applied)
    );
    new.close();
    assert!(!views.reconnect(new.generation()));
}

#[test]
fn resumed_snapshot_and_presentation_cursor_refuse_stale_or_incomplete_delivery() {
    let mut owner = authority(Role::Player);
    let old = RpcConnection::new(());
    let mut client = ClientContract::new(&owner, old.generation());
    let initial = owner
        .snapshot(
            owner.member.reference,
            Role::Player,
            FixtureCredential(11),
            stamp(7, 4, 0),
        )
        .unwrap();
    client.accept(&old.generation(), initial).unwrap();
    old.close();
    let new = RpcConnection::new(());
    client
        .resume(&mut owner, FixtureCredential(11), new.generation())
        .unwrap();
    let mut current = initial;
    current.stamp.binding_generation = 8;
    current.stamp.full_snapshot = false;
    assert_eq!(
        client.accept(&new.generation(), current),
        Err(Refusal::SnapshotRequired)
    );
    current.stamp.full_snapshot = true;
    client.accept(&new.generation(), current).unwrap();
    let mut incoming = current;
    incoming.stamp.binding_generation = 7;
    incoming.stamp.revision = revision(99, 0);
    assert_eq!(
        client.accept(&new.generation(), incoming),
        Err(Refusal::InactiveBinding)
    );
    assert_eq!(
        client.accept(&old.generation(), incoming),
        Err(Refusal::Consumer(
            ConnectionViewAcceptance::ObsoleteGeneration
        ))
    );
    incoming = current;
    incoming.stamp.revision = revision(3, u64::MAX);
    assert_eq!(
        client.accept(&new.generation(), incoming),
        Err(Refusal::RevisionRegressed)
    );
    incoming = current;
    incoming.stamp.presentation_epoch = 2;
    assert_eq!(
        client.accept(&new.generation(), incoming),
        Err(Refusal::PresentationEpochRegressed)
    );
    assert_eq!(
        client.accept(&new.generation(), current),
        Err(Refusal::ViewSequenceNotIncreasing)
    );
    incoming = current;
    incoming.stamp.presentation_epoch = 4;
    incoming.stamp.full_snapshot = false;
    assert_eq!(
        client.accept(&new.generation(), incoming),
        Err(Refusal::SnapshotRequired)
    );
    assert_eq!(client.current(), Some(&current));
    incoming = current;
    incoming.stamp.view_sequence = 2;
    assert_eq!(
        client.accept(&new.generation(), incoming),
        Ok(ViewAcceptance::Duplicate {
            current: revision(4, 0)
        })
    );
    assert_eq!(client.current(), Some(&incoming));
    assert_eq!(client.watermark(), Some(revision(4, 0)));
    incoming.stamp.presentation_epoch = 4;
    incoming.stamp.view_sequence = 1;
    incoming.stamp.revision = revision(5, 0);
    assert_eq!(
        client.accept(&new.generation(), incoming),
        Ok(ViewAcceptance::Applied)
    );
    assert_eq!(client.watermark(), Some(revision(5, 0)));
}

#[test]
fn explicit_join_positive_control_proves_allocation_observations_are_not_constant_zero() {
    let mut owner = authority(Role::Player);
    owner.explicit_join_control();
    assert_eq!(
        (
            owner.observations.joins,
            owner.observations.allocations,
            owner.seats
        ),
        (1, 1, 2)
    );
}
