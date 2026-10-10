use df_session::invitation::{
    InvitationFailure, InvitationOutcome, InvitationReceipt, InvitationRedeemer, InvitationUse,
    OneShotInvitation,
};
use df_types::{MemberId, OperationId, SessionId};

const ACTIVE_PROOF: &[u8] = b"invitation-proof-secret";
const WRONG_PROOF: &[u8] = b"guessed-invitation-proof";
const FINGERPRINT: [u8; 4] = *b"req1";
const OTHER_FINGERPRINT: [u8; 4] = *b"req2";

struct InMemoryRedeemer {
    expected_proof: Vec<u8>,
    invitation: OneShotInvitation<u64, [u8; 4]>,
    committed: Option<InvitationReceipt<MemberId>>,
    allocations: usize,
    uncertain_after_commit: bool,
    calls: usize,
}

impl InMemoryRedeemer {
    fn new(expires_at_seconds: u64) -> Self {
        Self {
            expected_proof: ACTIVE_PROOF.to_vec(),
            invitation: OneShotInvitation::new(expires_at_seconds),
            committed: None,
            allocations: 0,
            uncertain_after_commit: false,
            calls: 0,
        }
    }

    fn call(
        &mut self,
        proof: &[u8],
        session: SessionId,
        operation: OperationId,
        fingerprint: [u8; 4],
        now_seconds: u64,
    ) -> InvitationOutcome<MemberId> {
        self.redeem_atomically(&7, proof, session, operation, &fingerprint, now_seconds)
    }
}

impl InvitationRedeemer for InMemoryRedeemer {
    type Principal = u64;
    type Proof = [u8];
    type Fingerprint = [u8; 4];
    type Member = MemberId;

    fn redeem_atomically(
        &mut self,
        principal: &Self::Principal,
        proof: &Self::Proof,
        session: SessionId,
        operation: OperationId,
        fingerprint: &Self::Fingerprint,
        now_seconds: u64,
    ) -> InvitationOutcome<Self::Member> {
        self.calls += 1;
        if proof != self.expected_proof.as_slice() {
            return InvitationOutcome::Rejected(InvitationFailure::InvalidOrGuessed);
        }

        let mut transaction_local_invitation = self.invitation.clone();
        match transaction_local_invitation.consume(
            *principal,
            session,
            operation,
            *fingerprint,
            now_seconds,
        ) {
            Ok(InvitationUse::ExactReplay) => match &self.committed {
                Some(receipt) => InvitationOutcome::Replayed(receipt.clone()),
                None => InvitationOutcome::Indeterminate,
            },
            Ok(InvitationUse::ConsumedNow) => {
                let next_member = match u8::try_from(self.allocations + 1) {
                    Ok(value) => value,
                    Err(_) => return InvitationOutcome::Unavailable,
                };
                let member = MemberId::from_bytes(&[next_member; 16]);
                let Ok(member) = member else {
                    return InvitationOutcome::Unavailable;
                };
                let receipt = InvitationReceipt::new_after_commit(session, operation, member);

                // These assignments model one transaction commit of the invitation
                // state and membership receipt; no state is exposed before both exist.
                self.invitation = transaction_local_invitation;
                self.allocations += 1;
                self.committed = Some(receipt.clone());
                if self.uncertain_after_commit {
                    self.uncertain_after_commit = false;
                    InvitationOutcome::Indeterminate
                } else {
                    InvitationOutcome::Committed(receipt)
                }
            }
            Err(failure) => InvitationOutcome::Rejected(failure),
        }
    }
}

fn session() -> SessionId {
    SessionId::from_bytes(&[1; 16]).unwrap()
}

fn operation(value: u8) -> OperationId {
    OperationId::from_bytes(&[value; 16]).unwrap()
}

#[test]
fn guessed_invitation_never_returns_membership() {
    let mut owner = InMemoryRedeemer::new(100);

    let result = owner.call(WRONG_PROOF, session(), operation(1), FINGERPRINT, 20);

    assert_eq!(
        result,
        InvitationOutcome::Rejected(InvitationFailure::InvalidOrGuessed)
    );
    assert_eq!(owner.allocations, 0);
    assert!(owner.committed.is_none());
}

#[test]
fn expiry_is_exclusive_and_never_returns_membership() {
    let mut owner = InMemoryRedeemer::new(20);

    let result = owner.call(ACTIVE_PROOF, session(), operation(1), FINGERPRINT, 20);

    assert_eq!(
        result,
        InvitationOutcome::Rejected(InvitationFailure::Expired)
    );
    assert_eq!(owner.allocations, 0);
    assert!(owner.committed.is_none());
}

#[test]
fn valid_invitation_commits_once_and_exact_retry_replays_after_expiry() {
    let mut owner = InMemoryRedeemer::new(20);

    let first = owner.call(ACTIVE_PROOF, session(), operation(1), FINGERPRINT, 19);
    let InvitationOutcome::Committed(first_receipt) = first else {
        panic!("valid invitation did not commit");
    };
    let retry = owner.call(ACTIVE_PROOF, session(), operation(1), FINGERPRINT, 21);

    let InvitationOutcome::Replayed(retry_receipt) = retry else {
        panic!("exact retry did not replay");
    };
    assert_eq!(first_receipt.member(), retry_receipt.member());
    assert_eq!(retry_receipt.session(), session());
    assert_eq!(retry_receipt.operation(), operation(1));
    assert_eq!(owner.allocations, 1);
}

#[test]
fn a_distinct_operation_cannot_consume_the_same_invitation_twice() {
    let mut owner = InMemoryRedeemer::new(100);

    let first = owner.call(ACTIVE_PROOF, session(), operation(1), FINGERPRINT, 20);
    assert!(matches!(first, InvitationOutcome::Committed(_)));
    let second = owner.call(ACTIVE_PROOF, session(), operation(2), FINGERPRINT, 20);

    assert_eq!(
        second,
        InvitationOutcome::Rejected(InvitationFailure::Consumed)
    );
    assert_eq!(owner.allocations, 1);
}

#[test]
fn same_operation_with_changed_fingerprint_is_a_conflict() {
    let mut owner = InMemoryRedeemer::new(100);

    let first = owner.call(ACTIVE_PROOF, session(), operation(1), FINGERPRINT, 20);
    assert!(matches!(first, InvitationOutcome::Committed(_)));
    let changed = owner.call(ACTIVE_PROOF, session(), operation(1), OTHER_FINGERPRINT, 20);

    assert_eq!(
        changed,
        InvitationOutcome::Rejected(InvitationFailure::OperationConflict)
    );
    assert_eq!(owner.allocations, 1);
}

#[test]
fn changed_principal_or_session_cannot_replay_consumed_operation() {
    let mut invitation = OneShotInvitation::<u64, [u8; 4]>::new(100);
    assert_eq!(
        invitation.consume(7, session(), operation(1), FINGERPRINT, 20),
        Ok(InvitationUse::ConsumedNow)
    );

    assert_eq!(
        invitation.consume(8, session(), operation(1), FINGERPRINT, 20),
        Err(InvitationFailure::Consumed)
    );
    let other_session = SessionId::from_bytes(&[2; 16]).unwrap();
    assert_eq!(
        invitation.consume(7, other_session, operation(1), FINGERPRINT, 20),
        Err(InvitationFailure::Consumed)
    );
}

#[test]
fn missing_receipt_keeps_tombstone_and_never_reallocates() {
    let mut owner = InMemoryRedeemer::new(100);
    let first = owner.call(ACTIVE_PROOF, session(), operation(1), FINGERPRINT, 20);
    assert!(matches!(first, InvitationOutcome::Committed(_)));
    assert_eq!(owner.allocations, 1);

    // Model receipt retention loss while the consumed operation tombstone remains.
    owner.committed = None;
    let retry = owner.call(ACTIVE_PROOF, session(), operation(1), FINGERPRINT, 20);
    assert_eq!(retry, InvitationOutcome::Indeterminate);
    assert_eq!(owner.allocations, 1);

    let distinct = owner.call(ACTIVE_PROOF, session(), operation(2), FINGERPRINT, 20);
    assert_eq!(
        distinct,
        InvitationOutcome::Rejected(InvitationFailure::Consumed)
    );
    assert_eq!(owner.allocations, 1);
}

#[test]
fn indeterminate_commit_returns_no_receipt_and_exact_retry_resolves_it() {
    let mut owner = InMemoryRedeemer::new(20);
    owner.uncertain_after_commit = true;

    let first = owner.call(ACTIVE_PROOF, session(), operation(1), FINGERPRINT, 19);

    assert_eq!(first, InvitationOutcome::Indeterminate);
    assert_eq!(owner.calls, 1);
    assert_eq!(owner.allocations, 1);
    let retry = owner.call(ACTIVE_PROOF, session(), operation(1), FINGERPRINT, 21);

    assert!(matches!(retry, InvitationOutcome::Replayed(_)));
    assert_eq!(owner.calls, 2);
    assert_eq!(owner.allocations, 1);
}

#[test]
fn revoked_invitation_never_returns_membership() {
    let mut invitation = OneShotInvitation::<u64, [u8; 4]>::new(100);
    assert_eq!(invitation.revoke(), Ok(()));

    let result = invitation.consume(7, session(), operation(1), FINGERPRINT, 20);

    assert_eq!(result, Err(InvitationFailure::Revoked));
}

#[test]
fn default_debug_omits_principal_fingerprint_and_proof_values() {
    let mut owner = InMemoryRedeemer::new(100);
    let mut invitation = OneShotInvitation::<u64, [u8; 4]>::new(100);
    let _ = invitation.consume(9_876_543, session(), operation(1), *b"hide", 20);
    let outcome = owner.call(ACTIVE_PROOF, session(), operation(2), *b"cred", 20);

    let policy_debug = format!("{invitation:?}");
    let outcome_debug = format!("{outcome:?}");
    assert!(!policy_debug.contains("9876543"));
    assert!(!policy_debug.contains("hide"));
    assert!(!outcome_debug.contains("invitation-proof-secret"));
    assert!(!outcome_debug.contains("cred"));
}

struct CredentialSentinel;

impl std::fmt::Debug for CredentialSentinel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("credential-sentinel")
    }
}

#[test]
fn outcome_debug_redacts_generic_receipt_member_data() {
    let receipt = InvitationReceipt::new_after_commit(session(), operation(1), CredentialSentinel);
    let outcome = InvitationOutcome::Committed(receipt);

    assert!(!format!("{outcome:?}").contains("credential-sentinel"));
}
