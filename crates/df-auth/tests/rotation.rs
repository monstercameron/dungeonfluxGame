use std::cell::Cell;
use std::sync::{Arc, Barrier, Mutex};

use df_auth::rotation::{
    CredentialFence, CurrentPublicationState, PublicationAuthority, PublicationError,
    PublicationLease, PublicationRefusal, PublicationScope, RotationRefusal, publish_current,
};
use df_types::{ClientBindingId, SessionId};

struct Authority {
    scope: PublicationScope<u32, u32, u32>,
    credential: CredentialFence<u64>,
    binding_generation: u32,
    binding_active: bool,
    credential_unexpired: bool,
    binding_unexpired: bool,
    audience_version: u64,
    audience_authorized: bool,
    missing: bool,
    unavailable: bool,
}

impl Authority {
    fn fixture() -> Self {
        Self {
            scope: PublicationScope {
                audience: 7,
                principal: 11,
                credential: 22,
                session: SessionId::from_bytes(&[1; 16]).unwrap(),
                binding: ClientBindingId::from_bytes(&[2; 16]).unwrap(),
            },
            credential: CredentialFence::new(3),
            binding_generation: 4,
            binding_active: true,
            credential_unexpired: true,
            binding_unexpired: true,
            audience_version: 5,
            audience_authorized: true,
            missing: false,
            unavailable: false,
        }
    }

    fn lease(&self) -> PublicationLease<Self> {
        PublicationLease::new(
            self.scope.clone(),
            *self.credential.generation(),
            self.binding_generation,
            self.audience_version,
        )
    }
}

impl PublicationAuthority for Authority {
    type Principal = u32;
    type Credential = u32;
    type Audience = u32;
    type CredentialGeneration = u64;
    type BindingGeneration = u32;
    type AudienceVersion = u64;
    type Error = &'static str;

    fn with_current<R>(
        &mut self,
        _scope: &PublicationScope<Self::Principal, Self::Credential, Self::Audience>,
        publish: impl FnOnce(Option<CurrentPublicationState<'_, Self>>) -> R,
    ) -> Result<R, Self::Error> {
        if self.unavailable {
            return Err("authority unavailable");
        }
        Ok(publish((!self.missing).then_some(
            CurrentPublicationState {
                scope: &self.scope,
                credential: &self.credential,
                binding_generation: &self.binding_generation,
                binding_active: self.binding_active,
                credential_unexpired: self.credential_unexpired,
                binding_unexpired: self.binding_unexpired,
                audience_version: &self.audience_version,
                audience_authorized: self.audience_authorized,
            },
        )))
    }
}

fn assert_refused(
    authority: &mut Authority,
    lease: &PublicationLease<Authority>,
    refusal: PublicationRefusal,
) {
    let serialized = Cell::new(false);
    let result = publish_current(authority, lease, || {
        serialized.set(true);
        Ok::<_, ()>(b"private payload")
    });
    assert_eq!(result, Err(PublicationError::Refused(refusal)));
    assert!(!serialized.get());
}

#[test]
fn rotation_fences_old_stream_before_serialization_and_new_lease_publishes() {
    let mut authority = Authority::fixture();
    let old = authority.lease();
    assert_eq!(
        publish_current(&mut authority, &old, || Ok::<_, ()>(b"first")),
        Ok(b"first")
    );
    authority.credential.rotate(4).unwrap();
    assert_refused(&mut authority, &old, PublicationRefusal::CredentialRotated);
    let new = authority.lease();
    assert_eq!(
        publish_current(&mut authority, &new, || Ok::<_, ()>(b"new")),
        Ok(b"new")
    );
}

#[test]
fn revoked_credential_cannot_publish_or_be_rotated_back_to_life() {
    let mut authority = Authority::fixture();
    let lease = authority.lease();
    authority.credential.revoke();
    authority.credential.revoke();
    assert_eq!(
        authority.credential.rotate(4),
        Err(RotationRefusal::Revoked)
    );
    assert_refused(
        &mut authority,
        &lease,
        PublicationRefusal::CredentialRevoked,
    );
}

#[test]
fn equal_or_older_generation_rejects_without_mutating_current_credential() {
    let mut credential = CredentialFence::new(9_u64);
    for generation in [0, 8, 9] {
        assert_eq!(
            credential.rotate(generation),
            Err(RotationRefusal::NonIncreasingGeneration)
        );
        assert_eq!(*credential.generation(), 9);
        assert!(credential.is_active());
    }
    credential.rotate(u64::MAX).unwrap();
    assert_eq!(
        credential.rotate(0),
        Err(RotationRefusal::NonIncreasingGeneration)
    );
    assert_eq!(*credential.generation(), u64::MAX);
}

#[test]
fn binding_replacement_membership_changes_and_scope_mismatch_discard_unsent_data() {
    for (mutation, expected) in [
        (0, PublicationRefusal::BindingRevoked),
        (1, PublicationRefusal::BindingReplaced),
        (2, PublicationRefusal::AudienceDenied),
        (3, PublicationRefusal::AudienceChanged),
        (4, PublicationRefusal::ScopeChanged),
        (5, PublicationRefusal::ScopeChanged),
        (6, PublicationRefusal::ScopeChanged),
        (7, PublicationRefusal::ScopeChanged),
        (8, PublicationRefusal::MissingAuthority),
        (9, PublicationRefusal::CredentialExpired),
        (10, PublicationRefusal::BindingExpired),
        (11, PublicationRefusal::ScopeChanged),
    ] {
        let mut authority = Authority::fixture();
        let lease = authority.lease();
        match mutation {
            0 => authority.binding_active = false,
            1 => authority.binding_generation += 1,
            2 => authority.audience_authorized = false,
            3 => authority.audience_version += 1,
            4 => authority.scope.principal += 1,
            5 => authority.scope.credential += 1,
            6 => authority.scope.session = SessionId::from_bytes(&[3; 16]).unwrap(),
            7 => authority.scope.binding = ClientBindingId::from_bytes(&[4; 16]).unwrap(),
            8 => authority.missing = true,
            9 => authority.credential_unexpired = false,
            10 => authority.binding_unexpired = false,
            11 => authority.scope.audience += 1,
            _ => unreachable!(),
        }
        assert_refused(&mut authority, &lease, expected);
    }
}

#[test]
fn authority_and_publisher_failures_are_distinct_and_do_not_fabricate_success() {
    let mut authority = Authority::fixture();
    let lease = authority.lease();
    authority.unavailable = true;
    let called = Cell::new(false);
    assert_eq!(
        publish_current(&mut authority, &lease, || {
            called.set(true);
            Ok::<_, &'static str>(())
        }),
        Err(PublicationError::Authority("authority unavailable"))
    );
    assert!(!called.get());
    authority.unavailable = false;
    assert_eq!(
        publish_current(&mut authority, &lease, || Err::<(), _>("write failed")),
        Err(PublicationError::Publisher("write failed"))
    );
}

#[test]
fn serialized_publication_finishes_before_revoke_and_following_chunk_is_refused() {
    let authority = Arc::new(Mutex::new(Authority::fixture()));
    let lease = authority.lock().unwrap().lease();
    let entered = Arc::new(Barrier::new(2));
    let revoke_requested = Arc::new(Barrier::new(2));
    let revoke_authority = Arc::clone(&authority);
    let revoke_entered = Arc::clone(&entered);
    let revoke_barrier = Arc::clone(&revoke_requested);
    let revoked = std::thread::spawn(move || {
        revoke_entered.wait();
        revoke_barrier.wait();
        revoke_authority.lock().unwrap().credential.revoke();
    });
    let first_chunk = {
        let mut owner = authority.lock().unwrap();
        publish_current(&mut *owner, &lease, || {
            entered.wait();
            revoke_requested.wait();
            // The revoker is ready, but cannot mutate the owner before delivery ends.
            Ok::<_, ()>(b"authorized first chunk")
        })
    };
    assert_eq!(first_chunk, Ok(b"authorized first chunk"));
    revoked.join().unwrap();
    assert_refused(
        &mut authority.lock().unwrap(),
        &lease,
        PublicationRefusal::CredentialRevoked,
    );
}
