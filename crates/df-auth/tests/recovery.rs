use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Barrier, Mutex},
};

use df_auth::{
    recovery::{
        AccountAuthority, AccountObservation, AccountOperation, AccountOutcome, AccountReceipt,
        AccountRefusal, LinkMembership, LinkRecord, LinkRequest, RecoveryRecord, RecoveryRequest,
        RecoveryRequested, link_guest, recover_credentials, request_recovery,
    },
    rotation::{
        CredentialFence, CurrentPublicationState, PublicationAuthority, PublicationError,
        PublicationLease, PublicationRefusal, PublicationScope, publish_current,
    },
};
use df_types::{ClientBindingId, MemberId, OperationId, SessionId};

// Synthetic identities/proofs are private, never credential issuers or a channel.
// A finite serialized fixture supplies verified facts, not cryptographic evidence.
#[derive(Clone, Eq, PartialEq)]
struct Principal(u8);

#[derive(Clone, Copy, Eq, PartialEq)]
enum Purpose {
    Recovery,
    GuestLink,
    AccountLink,
}

#[derive(Clone, Eq, PartialEq)]
struct Proof {
    principal: Principal,
    generation: u64,
    purpose: Purpose,
    verified: bool,
    scope: u8,
    challenge: Option<u8>,
}

#[derive(Clone)]
struct State {
    scope: u8,
    guest: Principal,
    account: Principal,
    recoverable: bool,
    channel_verified: bool,
    account_verified: bool,
    guest_is_guest: bool,
    challenge: u8,
    challenge_principal: Principal,
    recovery_purpose: bool,
    expires_at: u64,
    attempts: u32,
    attempt_limit: u32,
    consumed: bool,
    credential: CredentialFence<u64>,
    grants_active: bool,
    binding_generation: u64,
    guest_credential: CredentialFence<u64>,
    guest_grants_active: bool,
    linked_account: Option<Principal>,
    membership_revision: u64,
    memberships: Vec<(MemberId, Principal)>,
    audience_authorized: bool,
    // These independent records must not be created, transferred or undone.
    campaign: SessionId,
    payer: Principal,
    committed_game_decisions: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Unavailable,
    LostAcknowledgement,
    Capacity,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Fault {
    None,
    Before,
    UnknownBefore,
    FailedCommit,
    UnknownAfter,
}

#[derive(Clone, Eq, PartialEq)]
enum Intent {
    Recovery {
        challenge: u8,
        proof: Proof,
    },
    Link {
        guest: Principal,
        account: Principal,
        members: Vec<MemberId>,
        revision: u64,
        guest_proof: Proof,
        account_proof: Proof,
    },
}

#[derive(Clone)]
struct Entry {
    fingerprint: u8,
    command: Purpose,
    intent: Intent,
    result: Result<AccountReceipt<Principal>, AccountRefusal>,
}

struct Authority {
    state: State,
    ledger: BTreeMap<(u8, OperationId), Entry>,
    retired: BTreeSet<(u8, OperationId)>,
    fault: Fault,
    callbacks: u8,
    starts: u8,
    start_failures: u8,
    start_unavailable: bool,
}

fn member(byte: u8) -> MemberId {
    MemberId::from_bytes(&[byte; 16]).unwrap()
}

fn key(byte: u8) -> OperationId {
    OperationId::from_bytes(&[byte; 16]).unwrap()
}

impl Authority {
    fn new() -> Self {
        Self {
            state: State {
                scope: 1,
                guest: Principal(11),
                account: Principal(22),
                recoverable: true,
                channel_verified: true,
                account_verified: true,
                guest_is_guest: true,
                challenge: 7,
                challenge_principal: Principal(22),
                recovery_purpose: true,
                expires_at: 100,
                attempts: 0,
                attempt_limit: 2,
                consumed: false,
                credential: CredentialFence::new(10),
                grants_active: true,
                binding_generation: 4,
                guest_credential: CredentialFence::new(3),
                guest_grants_active: true,
                linked_account: None,
                membership_revision: 5,
                memberships: vec![
                    (member(1), Principal(11)),
                    (member(2), Principal(11)),
                    (member(3), Principal(33)),
                ],
                audience_authorized: true,
                campaign: SessionId::from_bytes(&[8; 16]).unwrap(),
                payer: Principal(44),
                committed_game_decisions: 3,
            },
            ledger: BTreeMap::new(),
            retired: BTreeSet::new(),
            fault: Fault::None,
            callbacks: 0,
            starts: 0,
            start_failures: 0,
            start_unavailable: false,
        }
    }

    fn proof(&self, purpose: Purpose) -> Proof {
        let (principal, generation) = if purpose == Purpose::GuestLink {
            (&self.state.guest, *self.state.guest_credential.generation())
        } else {
            (&self.state.account, *self.state.credential.generation())
        };
        Proof {
            principal: principal.clone(),
            generation,
            purpose,
            verified: true,
            scope: self.state.scope,
            challenge: (purpose == Purpose::Recovery).then_some(self.state.challenge),
        }
    }

    fn resolve(
        &self,
        operation: &AccountOperation<'_, Self>,
        command: Option<Purpose>,
        intent: Option<&Intent>,
    ) -> Option<AccountOutcome<Principal, Failure>> {
        let scoped = (*operation.scope, operation.key);
        if self.retired.contains(&scoped) {
            return Some(AccountOutcome::Refused(AccountRefusal::Retired));
        }
        self.ledger.get(&scoped).map(|entry| {
            if entry.fingerprint != *operation.fingerprint
                || command.is_some_and(|command| command != entry.command)
                || intent.is_some_and(|intent| intent != &entry.intent)
            {
                AccountOutcome::Refused(AccountRefusal::Conflict)
            } else {
                match &entry.result {
                    Ok(receipt) => AccountOutcome::Committed {
                        receipt: receipt.clone(),
                        replayed: true,
                    },
                    Err(refusal) => AccountOutcome::Refused(*refusal),
                }
            }
        })
    }

    fn admission(&self) -> Option<AccountOutcome<Principal, Failure>> {
        if self.ledger.len() == 8 {
            Some(AccountOutcome::Unavailable(Failure::Capacity))
        } else {
            match self.fault {
                Fault::Before => Some(AccountOutcome::Unavailable(Failure::Unavailable)),
                Fault::UnknownBefore => Some(AccountOutcome::Unknown(Failure::LostAcknowledgement)),
                _ => None,
            }
        }
    }

    fn finish(
        &mut self,
        operation: &AccountOperation<'_, Self>,
        command: Purpose,
        intent: Intent,
        before: State,
        result: Result<AccountReceipt<Principal>, AccountRefusal>,
    ) -> AccountOutcome<Principal, Failure> {
        if self.fault == Fault::FailedCommit {
            self.state = before;
            return AccountOutcome::Unavailable(Failure::Unavailable);
        }
        self.ledger.insert(
            (*operation.scope, operation.key),
            Entry {
                fingerprint: *operation.fingerprint,
                command,
                intent,
                result: result.clone(),
            },
        );
        if self.fault == Fault::UnknownAfter {
            return AccountOutcome::Unknown(Failure::LostAcknowledgement);
        }
        match result {
            Ok(receipt) => AccountOutcome::Committed {
                receipt,
                replayed: false,
            },
            Err(refusal) => AccountOutcome::Refused(refusal),
        }
    }

    fn recovery(
        &mut self,
        operation: u8,
        fingerprint: u8,
        proof: &Proof,
        now: u64,
    ) -> AccountOutcome<Principal, Failure> {
        recover_credentials(
            self,
            &RecoveryRequest {
                operation: AccountOperation {
                    scope: &1,
                    key: key(operation),
                    fingerprint: &fingerprint,
                },
                challenge: &7,
                proof,
            },
            now,
        )
    }

    fn link(
        &mut self,
        operation: u8,
        members: &[MemberId],
        revision: u64,
    ) -> AccountOutcome<Principal, Failure> {
        let guest = self.state.guest.clone();
        let account = self.state.account.clone();
        let guest_proof = self.proof(Purpose::GuestLink);
        let account_proof = self.proof(Purpose::AccountLink);
        link_guest(
            self,
            &LinkRequest {
                operation: AccountOperation {
                    scope: &1,
                    key: key(operation),
                    fingerprint: &1,
                },
                guest: &guest,
                account: &account,
                guest_proof: &guest_proof,
                account_proof: &account_proof,
                members,
                expected_revision: revision,
            },
        )
    }

    fn lease(&self) -> PublicationLease<Self> {
        PublicationLease::new(
            self.publication_scope(),
            *self.state.credential.generation(),
            self.state.binding_generation,
            1,
        )
    }

    fn publication_scope(&self) -> PublicationScope<Principal, u8, u8> {
        PublicationScope {
            principal: self.state.account.clone(),
            credential: 9,
            audience: 1,
            session: self.state.campaign,
            binding: ClientBindingId::from_bytes(&[9; 16]).unwrap(),
        }
    }

    fn assert_independent_records(&self) {
        assert!(self.state.payer == Principal(44));
        assert_eq!(
            self.state.campaign,
            SessionId::from_bytes(&[8; 16]).unwrap()
        );
        assert_eq!(self.state.committed_game_decisions, 3);
    }
}

impl AccountAuthority for Authority {
    type Scope = u8;
    type Principal = Principal;
    type Lookup = Principal;
    type Challenge = u8;
    type Proof = Proof;
    type Fingerprint = u8;
    type Error = Failure;

    fn request_challenge(&mut self, _lookup: &Principal) -> Result<(), Failure> {
        self.starts += 1;
        if self.start_unavailable {
            self.start_failures += 1;
            Err(Failure::Unavailable)
        } else {
            // No fixture assertion promises external delivery or existence.
            Ok(())
        }
    }

    fn with_recovery(
        &mut self,
        request: &RecoveryRequest<'_, Self>,
        apply: impl FnOnce(
            RecoveryRecord<'_, Self>,
        ) -> Result<AccountReceipt<Principal>, AccountRefusal>,
    ) -> AccountOutcome<Principal, Failure> {
        let intent = Intent::Recovery {
            challenge: *request.challenge,
            proof: request.proof.clone(),
        };
        if let Some(result) =
            self.resolve(&request.operation, Some(Purpose::Recovery), Some(&intent))
        {
            return result;
        }
        if let Some(result) = self.admission() {
            return result;
        }
        let before = self.state.clone();
        self.callbacks += 1;
        let state = &mut self.state;
        let proof_verified = request.proof.verified
            && state.credential.is_active()
            && request.proof.principal == state.account
            && request.proof.generation == *state.credential.generation()
            && request.proof.purpose == Purpose::Recovery
            && request.proof.scope == state.scope
            && request.proof.scope == *request.operation.scope
            && request.proof.challenge == Some(*request.challenge);
        let result = apply(RecoveryRecord {
            scope: &state.scope,
            principal: &state.account,
            linked_account: state.recoverable,
            challenge: &state.challenge,
            challenge_principal: &state.challenge_principal,
            channel_verified: state.channel_verified,
            proof_verified,
            recovery_purpose: state.recovery_purpose,
            expires_at: state.expires_at,
            attempts: &mut state.attempts,
            attempt_limit: state.attempt_limit,
            consumed: &mut state.consumed,
            credential: &mut state.credential,
            prior_bindings_active: &mut state.grants_active,
        });
        self.finish(
            &request.operation,
            Purpose::Recovery,
            intent,
            before,
            result,
        )
    }

    fn with_link(
        &mut self,
        request: &LinkRequest<'_, Self>,
        apply: impl FnOnce(LinkRecord<'_, Self>) -> Result<AccountReceipt<Principal>, AccountRefusal>,
    ) -> AccountOutcome<Principal, Failure> {
        let intent = Intent::Link {
            guest: request.guest.clone(),
            account: request.account.clone(),
            members: request.members.to_vec(),
            revision: request.expected_revision,
            guest_proof: request.guest_proof.clone(),
            account_proof: request.account_proof.clone(),
        };
        if let Some(result) =
            self.resolve(&request.operation, Some(Purpose::GuestLink), Some(&intent))
        {
            return result;
        }
        if let Some(result) = self.admission() {
            return result;
        }
        assert!(self.state.memberships.len() <= 8);
        let before = self.state.clone();
        self.callbacks += 1;
        let state = &mut self.state;
        let guest_proof_verified = request.guest_proof.verified
            && state.guest_credential.is_active()
            && request.guest_proof.principal == state.guest
            && request.guest_proof.generation == *state.guest_credential.generation()
            && request.guest_proof.purpose == Purpose::GuestLink
            && request.guest_proof.scope == state.scope
            && request.guest_proof.scope == *request.operation.scope
            && request.guest_proof.challenge.is_none();
        let account_proof_verified = request.account_proof.verified
            && state.credential.is_active()
            && request.account_proof.principal == state.account
            && request.account_proof.generation == *state.credential.generation()
            && request.account_proof.purpose == Purpose::AccountLink
            && request.account_proof.scope == state.scope
            && request.account_proof.scope == *request.operation.scope
            && request.account_proof.challenge.is_none();
        let mut memberships: Vec<_> = state
            .memberships
            .iter_mut()
            .map(|(member, principal)| LinkMembership {
                member: *member,
                principal,
            })
            .collect();
        let result = apply(LinkRecord {
            scope: &state.scope,
            guest: &state.guest,
            account: &state.account,
            guest_is_guest: state.guest_is_guest,
            account_verified: state.account_verified,
            guest_proof_verified,
            account_proof_verified,
            guest_credential: &mut state.guest_credential,
            guest_bindings_active: &mut state.guest_grants_active,
            linked_account: &mut state.linked_account,
            revision: &mut state.membership_revision,
            selection_limit: 2,
            memberships: &mut memberships,
        });
        self.finish(
            &request.operation,
            Purpose::GuestLink,
            intent,
            before,
            result,
        )
    }

    fn lookup(
        &mut self,
        operation: &AccountOperation<'_, Self>,
    ) -> AccountOutcome<Principal, Failure> {
        self.resolve(operation, None, None)
            .unwrap_or(AccountOutcome::Pending)
    }
}

impl PublicationAuthority for Authority {
    type Principal = Principal;
    type Credential = u8;
    type Audience = u8;
    type CredentialGeneration = u64;
    type BindingGeneration = u64;
    type AudienceVersion = u8;
    type Error = Failure;

    fn with_current<R>(
        &mut self,
        _scope: &PublicationScope<Principal, u8, u8>,
        publish: impl FnOnce(Option<CurrentPublicationState<'_, Self>>) -> R,
    ) -> Result<R, Failure> {
        let scope = self.publication_scope();
        Ok(publish(Some(CurrentPublicationState {
            scope: &scope,
            credential: &self.state.credential,
            binding_generation: &self.state.binding_generation,
            binding_active: self.state.grants_active,
            credential_unexpired: true,
            binding_unexpired: true,
            audience_version: &1,
            audience_authorized: self.state.audience_authorized,
        })))
    }
}

fn refused(outcome: AccountOutcome<Principal, Failure>, expected: AccountRefusal) {
    assert!(matches!(outcome, AccountOutcome::Refused(actual) if actual == expected));
}

fn recovered(outcome: AccountOutcome<Principal, Failure>, generation: u64, replay: bool) {
    assert!(matches!(
        outcome,
        AccountOutcome::Committed {
            receipt: AccountReceipt::Recovered { principal, credential_generation },
            replayed,
        } if principal == Principal(22) && credential_generation == generation && replayed == replay
    ));
}

fn publication_refused(
    authority: &mut Authority,
    lease: &PublicationLease<Authority>,
    expected: PublicationRefusal,
) {
    let serialized = Cell::new(false);
    let result = publish_current(authority, lease, || {
        serialized.set(true);
        Ok::<_, ()>(b"private fixture payload")
    });
    assert!(matches!(result, Err(PublicationError::Refused(actual)) if actual == expected));
    assert!(!serialized.get());
}

#[test]
fn linked_guest_lost_local_storage_recovers_same_account_without_new_campaign() {
    let mut authority = Authority::new();
    let mut browser_credential = Some((Principal(11), 3));
    assert!(browser_credential.is_some());
    let linked = authority.link(1, &[member(1)], 5);
    assert!(matches!(linked, AccountOutcome::Committed {
        receipt: AccountReceipt::Linked { account, membership_revision: 6, transferred: 1 },
        replayed: false,
    } if account == Principal(22)));
    assert!(authority.state.memberships[0].1 == Principal(22));
    assert!(authority.state.memberships[1].1 == Principal(11));
    assert!(authority.state.memberships[2].1 == Principal(33));
    assert!(!authority.state.guest_credential.is_active());
    assert!(!authority.state.guest_grants_active);
    browser_credential.take();
    assert!(browser_credential.is_none());
    assert_eq!(
        request_recovery(&mut authority, &Principal(22)),
        RecoveryRequested
    );
    let proof = authority.proof(Purpose::Recovery);
    recovered(authority.recovery(2, 2, &proof, 99), 11, false);
    browser_credential = Some((authority.state.account.clone(), 11));
    assert!(
        browser_credential
            .is_some_and(|(principal, generation)| principal == Principal(22) && generation == 11)
    );
    assert!(authority.state.linked_account == Some(Principal(22)));
    authority.assert_independent_records();
}

#[test]
fn recovery_uses_actual_publication_fence_before_serialization_and_requires_new_binding_and_audience()
 {
    let mut authority = Authority::new();
    let old = authority.lease();
    let proof = authority.proof(Purpose::Recovery);
    recovered(authority.recovery(1, 1, &proof, 99), 11, false);
    publication_refused(&mut authority, &old, PublicationRefusal::CredentialRotated);
    let no_binding = authority.lease();
    publication_refused(
        &mut authority,
        &no_binding,
        PublicationRefusal::BindingRevoked,
    );
    // The separate binding issuer supplies a fresh grant; recovery did not do it.
    authority.state.binding_generation += 1;
    authority.state.grants_active = true;
    authority.state.audience_authorized = false;
    let current = authority.lease();
    publication_refused(&mut authority, &current, PublicationRefusal::AudienceDenied);
    authority.state.audience_authorized = true;
    assert!(matches!(
        publish_current(&mut authority, &current, || Ok::<_, ()>(b"current")),
        Ok(b"current")
    ));
    assert_ne!(*authority.state.credential.generation(), 10);
    authority.assert_independent_records();
}

#[test]
fn unlinked_guest_and_missing_channel_have_honest_refusals_without_replacement_identity() {
    for (recoverable, channel, expected) in [
        (false, true, AccountRefusal::UnlinkedGuest),
        (true, false, AccountRefusal::NoRecoveryChannel),
    ] {
        let mut authority = Authority::new();
        authority.state.recoverable = recoverable;
        authority.state.channel_verified = channel;
        let proof = authority.proof(Purpose::Recovery);
        refused(authority.recovery(1, 1, &proof, 1), expected);
        assert_eq!(*authority.state.credential.generation(), 10);
        assert_eq!(authority.state.attempts, 0);
        assert!(!authority.state.consumed);
        assert!(authority.state.grants_active);
        authority.assert_independent_records();
    }
}

#[test]
fn public_start_response_does_not_enumerate_known_unknown_or_source_failure() {
    let mut authority = Authority::new();
    for unavailable in [false, true] {
        authority.start_unavailable = unavailable;
        for lookup in [Principal(22), Principal(99)] {
            assert_eq!(request_recovery(&mut authority, &lookup), RecoveryRequested);
        }
    }
    assert_eq!(authority.starts, 4);
    assert_eq!(authority.start_failures, 2);
    assert_eq!(authority.callbacks, 0);
    assert!(authority.ledger.is_empty());
}

#[test]
fn challenge_identity_purpose_expiry_consumption_and_revocation_fail_before_generation_change() {
    for (mutation, expected) in [
        (0, AccountRefusal::WrongIdentity),
        (1, AccountRefusal::WrongPurpose),
        (2, AccountRefusal::Expired),
        (3, AccountRefusal::ChallengeUsed),
        (4, AccountRefusal::AttemptsExhausted),
        (5, AccountRefusal::CredentialRevoked),
        (6, AccountRefusal::WrongChallenge),
        (7, AccountRefusal::WrongScope),
    ] {
        let mut authority = Authority::new();
        match mutation {
            0 => authority.state.challenge_principal = Principal(99),
            1 => authority.state.recovery_purpose = false,
            2 => authority.state.expires_at = 99,
            3 => authority.state.consumed = true,
            4 => authority.state.attempt_limit = 0,
            5 => authority.state.credential.revoke(),
            6 => authority.state.challenge = 8,
            7 => authority.state.scope = 2,
            _ => unreachable!(),
        }
        let proof = authority.proof(Purpose::Recovery);
        refused(authority.recovery(1, 1, &proof, 99), expected);
        assert_eq!(*authority.state.credential.generation(), 10);
        assert_eq!(authority.state.attempts, 0);
        assert!(authority.state.grants_active);
    }
}

#[test]
fn failed_proof_attempts_are_bounded_and_exact_failed_replay_does_not_spend_another_attempt() {
    let mut authority = Authority::new();
    let mut proof = authority.proof(Purpose::Recovery);
    proof.verified = false;
    refused(
        authority.recovery(1, 1, &proof, 1),
        AccountRefusal::UnverifiedProof,
    );
    refused(
        authority.recovery(1, 1, &proof, 1),
        AccountRefusal::UnverifiedProof,
    );
    assert_eq!(authority.state.attempts, 1);
    refused(
        authority.recovery(2, 2, &proof, 1),
        AccountRefusal::UnverifiedProof,
    );
    refused(
        authority.recovery(3, 3, &proof, 1),
        AccountRefusal::AttemptsExhausted,
    );
    assert_eq!(authority.state.attempts, 2);
    assert_eq!(*authority.state.credential.generation(), 10);
    assert!(!authority.state.consumed);
}

#[test]
fn opaque_proof_must_match_current_principal_generation_and_purpose() {
    for mutation in 0..3 {
        let mut authority = Authority::new();
        let mut proof = authority.proof(Purpose::Recovery);
        match mutation {
            0 => proof.principal = Principal(99),
            1 => proof.generation = 9,
            2 => proof.purpose = Purpose::AccountLink,
            _ => unreachable!(),
        }
        refused(
            authority.recovery(1, 1, &proof, 1),
            AccountRefusal::UnverifiedProof,
        );
        assert_eq!(*authority.state.credential.generation(), 10);
        assert!(!authority.state.consumed);
    }
}

#[test]
fn committed_recovery_replay_precedes_consumed_challenge_and_changed_intent_conflicts() {
    let mut authority = Authority::new();
    let proof = authority.proof(Purpose::Recovery);
    recovered(authority.recovery(1, 1, &proof, 1), 11, false);
    recovered(authority.recovery(1, 1, &proof, 1000), 11, true);
    refused(
        authority.recovery(1, 2, &proof, 1),
        AccountRefusal::Conflict,
    );
    refused(authority.link(1, &[member(1)], 5), AccountRefusal::Conflict);
    assert_eq!(authority.callbacks, 1);
    assert_eq!(authority.state.attempts, 1);
    assert_eq!(*authority.state.credential.generation(), 11);
}

#[test]
fn committed_link_replay_returns_original_selection_without_transferring_again() {
    let mut authority = Authority::new();
    assert!(matches!(
        authority.link(1, &[member(1)], 5),
        AccountOutcome::Committed {
            replayed: false,
            ..
        }
    ));
    assert!(matches!(
        authority.link(1, &[member(1)], 5),
        AccountOutcome::Committed {
            receipt: AccountReceipt::Linked {
                membership_revision: 6,
                transferred: 1,
                ..
            },
            replayed: true,
        }
    ));
    refused(authority.link(1, &[member(2)], 5), AccountRefusal::Conflict);
    refused(authority.link(1, &[member(1)], 6), AccountRefusal::Conflict);
    assert_eq!(authority.callbacks, 1);
    assert_eq!(authority.state.membership_revision, 6);
    assert!(authority.state.memberships[1].1 == Principal(11));
}

#[test]
fn consumed_challenge_and_retired_namespace_do_not_allocate_again() {
    let mut authority = Authority::new();
    let proof = authority.proof(Purpose::Recovery);
    recovered(authority.recovery(1, 1, &proof, 1), 11, false);
    refused(
        authority.recovery(2, 2, &proof, 1),
        AccountRefusal::ChallengeUsed,
    );
    authority.ledger.remove(&(1, key(1)));
    authority.retired.insert((1, key(1)));
    refused(authority.recovery(1, 1, &proof, 1), AccountRefusal::Retired);
    assert_eq!(*authority.state.credential.generation(), 11);
}

#[test]
fn checked_generation_exhaustion_leaves_challenge_and_grants_unchanged() {
    let mut authority = Authority::new();
    authority.state.credential = CredentialFence::new(u64::MAX);
    let proof = authority.proof(Purpose::Recovery);
    refused(
        authority.recovery(1, 1, &proof, 1),
        AccountRefusal::GenerationExhausted,
    );
    assert_eq!(authority.state.attempts, 0);
    assert!(!authority.state.consumed);
    assert!(authority.state.grants_active);
    assert_eq!(*authority.state.credential.generation(), u64::MAX);
}

#[test]
fn link_validates_entire_selection_revision_and_both_identity_proofs_before_mutation() {
    for (mutation, expected) in [
        (0, AccountRefusal::UnverifiedAccount),
        (1, AccountRefusal::NotGuest),
        (2, AccountRefusal::CredentialRevoked),
        (3, AccountRefusal::AlreadyLinked),
        (4, AccountRefusal::StaleRevision),
        (5, AccountRefusal::RevisionExhausted),
        (6, AccountRefusal::InvalidSelection),
        (7, AccountRefusal::MembershipDenied),
        (8, AccountRefusal::MembershipDenied),
        (9, AccountRefusal::SelectionTooLarge),
        (10, AccountRefusal::WrongIdentity),
        (11, AccountRefusal::WrongScope),
    ] {
        let mut authority = Authority::new();
        let mut selection = vec![member(1)];
        let mut revision = 5;
        match mutation {
            0 => authority.state.account_verified = false,
            1 => authority.state.guest_is_guest = false,
            2 => authority.state.guest_credential.revoke(),
            3 => authority.state.linked_account = Some(Principal(22)),
            4 => revision = 4,
            5 => {
                authority.state.membership_revision = u64::MAX;
                revision = u64::MAX;
            }
            6 => selection = vec![member(1), member(1)],
            7 => selection = vec![member(1), member(3)],
            8 => selection = vec![member(1), member(4)],
            9 => selection = vec![member(1), member(2), member(3)],
            10 => authority.state.account = authority.state.guest.clone(),
            11 => authority.state.scope = 2,
            _ => unreachable!(),
        }
        let before = authority.state.membership_revision;
        refused(authority.link(1, &selection, revision), expected);
        assert_eq!(authority.state.membership_revision, before);
        assert!(authority.state.memberships[0].1 == Principal(11));
        assert!(authority.state.memberships[1].1 == Principal(11));
        assert!(authority.state.memberships[2].1 == Principal(33));
        authority.assert_independent_records();
    }
}

#[test]
fn link_rejects_each_stale_or_missing_identity_proof_without_transfer() {
    for mutation in 0..6 {
        let mut authority = Authority::new();
        let mut guest_proof = authority.proof(Purpose::GuestLink);
        let mut account_proof = authority.proof(Purpose::AccountLink);
        match mutation {
            0 => guest_proof.verified = false,
            1 => account_proof.verified = false,
            2 => guest_proof.generation = 2,
            3 => account_proof.generation = 9,
            4 => guest_proof.principal = Principal(99),
            5 => account_proof.purpose = Purpose::Recovery,
            _ => unreachable!(),
        }
        let outcome = link_guest(
            &mut authority,
            &LinkRequest {
                operation: AccountOperation {
                    scope: &1,
                    key: key(1),
                    fingerprint: &1,
                },
                guest: &Principal(11),
                account: &Principal(22),
                guest_proof: &guest_proof,
                account_proof: &account_proof,
                members: &[member(1)],
                expected_revision: 5,
            },
        );
        refused(outcome, AccountRefusal::UnverifiedProof);
        assert_eq!(authority.state.membership_revision, 5);
        assert!(authority.state.guest_credential.is_active());
        assert!(authority.state.linked_account.is_none());
    }
}

#[test]
fn empty_selection_links_without_inferring_any_membership_and_duplicate_owner_rows_refuse() {
    let mut authority = Authority::new();
    assert!(matches!(
        authority.link(1, &[], 5),
        AccountOutcome::Committed {
            receipt: AccountReceipt::Linked { transferred: 0, .. },
            ..
        }
    ));
    assert!(authority.state.memberships[0].1 == Principal(11));
    let mut authority = Authority::new();
    authority.state.memberships.push((member(1), Principal(11)));
    refused(
        authority.link(1, &[member(1)], 5),
        AccountRefusal::MembershipDenied,
    );
    assert_eq!(authority.state.membership_revision, 5);
}

#[test]
fn unknown_before_and_after_commit_use_same_key_lookup_without_automatic_retry() {
    for fault in [Fault::UnknownBefore, Fault::UnknownAfter] {
        let mut authority = Authority::new();
        authority.fault = fault;
        let proof = authority.proof(Purpose::Recovery);
        let outcome = authority.recovery(1, 1, &proof, 1);
        assert_eq!(outcome.observation(), AccountObservation::Unknown);
        let lookup = authority.lookup(&AccountOperation {
            scope: &1,
            key: key(1),
            fingerprint: &1,
        });
        if fault == Fault::UnknownBefore {
            assert_eq!(lookup.observation(), AccountObservation::Pending);
            assert_eq!(authority.callbacks, 0);
            assert_eq!(*authority.state.credential.generation(), 10);
        } else {
            recovered(lookup, 11, true);
            assert_eq!(authority.callbacks, 1);
            recovered(authority.recovery(1, 1, &proof, 1), 11, true);
            assert_eq!(authority.callbacks, 1);
        }
        let changed = authority.lookup(&AccountOperation {
            scope: &1,
            key: key(1),
            fingerprint: &2,
        });
        if fault == Fault::UnknownAfter {
            refused(changed, AccountRefusal::Conflict);
        } else {
            assert_eq!(changed.observation(), AccountObservation::Pending);
        }
    }
}

#[test]
fn unavailable_and_failed_atomic_commit_do_not_acknowledge_partial_changes() {
    for fault in [Fault::Before, Fault::FailedCommit] {
        let mut authority = Authority::new();
        authority.fault = fault;
        let proof = authority.proof(Purpose::Recovery);
        assert!(matches!(
            authority.recovery(1, 1, &proof, 1),
            AccountOutcome::Unavailable(Failure::Unavailable)
        ));
        assert_eq!(*authority.state.credential.generation(), 10);
        assert!(!authority.state.consumed);
        assert_eq!(authority.state.attempts, 0);
        assert!(authority.state.grants_active);
        assert!(authority.ledger.is_empty());
        authority.assert_independent_records();
    }
}

#[test]
fn eight_owned_duplicate_completions_commit_one_result_and_join_all_workers() {
    let authority = Arc::new(Mutex::new(Authority::new()));
    let barrier = Arc::new(Barrier::new(8));
    let mut workers = Vec::new();
    for _ in 0..8 {
        let authority = Arc::clone(&authority);
        let barrier = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            let proof = Proof {
                principal: Principal(22),
                generation: 10,
                purpose: Purpose::Recovery,
                verified: true,
                scope: 1,
                challenge: Some(7),
            };
            barrier.wait();
            let outcome = authority.lock().unwrap().recovery(1, 1, &proof, 1);
            matches!(
                outcome,
                AccountOutcome::Committed {
                    receipt: AccountReceipt::Recovered {
                        credential_generation: 11,
                        ..
                    },
                    ..
                }
            )
        }));
    }
    for worker in workers {
        assert!(worker.join().unwrap());
    }
    let authority = authority.lock().unwrap();
    assert_eq!(authority.callbacks, 1);
    assert_eq!(authority.ledger.len(), 1);
    assert_eq!(authority.state.attempts, 1);
    authority.assert_independent_records();
}

#[test]
fn finite_operation_capacity_does_not_block_exact_replay_or_leak_secret_payloads() {
    let mut authority = Authority::new();
    authority.state.expires_at = 0;
    let proof = authority.proof(Purpose::Recovery);
    for operation in 1..=8 {
        refused(
            authority.recovery(operation, operation, &proof, 1),
            AccountRefusal::Expired,
        );
    }
    assert_eq!(authority.ledger.len(), 8);
    refused(authority.recovery(1, 1, &proof, 1), AccountRefusal::Expired);
    let outcome = authority.recovery(9, 9, &proof, 1);
    assert_eq!(outcome.observation(), AccountObservation::Unavailable);
    assert_eq!(format!("{:?}", outcome.observation()), "Unavailable");
    assert_eq!(authority.callbacks, 8);
}

#[test]
fn cross_challenge_and_account_command_scope_proofs_refuse_without_fencing_grants() {
    for mutation in 0..2 {
        let mut authority = Authority::new();
        let mut proof = authority.proof(Purpose::Recovery);
        if mutation == 0 {
            proof.challenge = Some(8);
        } else {
            proof.scope = 2;
        }
        refused(
            authority.recovery(1, 1, &proof, 1),
            AccountRefusal::UnverifiedProof,
        );
        assert_eq!(*authority.state.credential.generation(), 10);
        assert_eq!(authority.state.attempts, 1);
        assert!(!authority.state.consumed);
        assert!(authority.state.grants_active);
        authority.assert_independent_records();
    }
    for destination in [false, true] {
        let mut authority = Authority::new();
        let mut guest_proof = authority.proof(Purpose::GuestLink);
        let mut account_proof = authority.proof(Purpose::AccountLink);
        if destination {
            account_proof.scope = 2;
        } else {
            guest_proof.scope = 2;
        }
        refused(
            link_guest(
                &mut authority,
                &LinkRequest {
                    operation: AccountOperation {
                        scope: &1,
                        key: key(1),
                        fingerprint: &1,
                    },
                    guest: &Principal(11),
                    account: &Principal(22),
                    guest_proof: &guest_proof,
                    account_proof: &account_proof,
                    members: &[member(1)],
                    expected_revision: 5,
                },
            ),
            AccountRefusal::UnverifiedProof,
        );
        assert_eq!(authority.state.membership_revision, 5);
        assert!(authority.state.memberships[0].1 == Principal(11));
        assert!(authority.state.linked_account.is_none());
        assert!(authority.state.guest_credential.is_active());
        assert!(authority.state.guest_grants_active);
        authority.assert_independent_records();
    }
}

#[test]
fn revoked_destination_proof_refuses_link_but_exact_committed_replay_remains_committed() {
    let mut authority = Authority::new();
    authority.state.credential.revoke();
    refused(
        authority.link(1, &[member(1)], 5),
        AccountRefusal::UnverifiedProof,
    );
    assert_eq!(authority.state.membership_revision, 5);
    assert!(authority.state.memberships[0].1 == Principal(11));
    assert!(authority.state.memberships[1].1 == Principal(11));
    assert!(authority.state.memberships[2].1 == Principal(33));
    assert!(authority.state.linked_account.is_none());
    assert!(authority.state.guest_credential.is_active());
    assert!(authority.state.guest_grants_active);
    assert!(authority.state.grants_active);
    assert!(!authority.state.credential.is_active());
    authority.assert_independent_records();

    let mut authority = Authority::new();
    assert!(matches!(
        authority.link(1, &[member(1)], 5),
        AccountOutcome::Committed {
            replayed: false,
            ..
        }
    ));
    authority.state.credential.revoke();
    assert!(matches!(
        authority.link(1, &[member(1)], 5),
        AccountOutcome::Committed {
            receipt: AccountReceipt::Linked {
                membership_revision: 6,
                transferred: 1,
                ..
            },
            replayed: true,
        }
    ));
    assert_eq!(authority.callbacks, 1);
    assert_eq!(authority.state.membership_revision, 6);
    assert!(authority.state.memberships[0].1 == Principal(22));
    assert!(authority.state.memberships[1].1 == Principal(11));
    assert!(!authority.state.credential.is_active());
    authority.assert_independent_records();
}
