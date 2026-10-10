//! Session-owned invitation redemption policy and atomic allocation port.
//!
//! The invitation owner resolves an opaque proof to this policy under its storage
//! transaction. It must commit the consumed state, membership, grant, and operation
//! receipt together. This module supplies no token lookup, persistence, or authority.

use std::fmt;

use df_types::{OperationId, SessionId};

/// A typed refusal that never carries a membership allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvitationFailure {
    InvalidOrGuessed,
    Expired,
    Consumed,
    Revoked,
    OperationConflict,
}

/// Result of applying a request to one invitation's deterministic state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvitationUse {
    ConsumedNow,
    ExactReplay,
}

#[derive(Clone)]
enum InvitationState<Principal, Fingerprint> {
    Available,
    Consumed {
        principal: Principal,
        session: SessionId,
        operation: OperationId,
        fingerprint: Fingerprint,
    },
    Revoked,
}

/// One invitation's one-shot use policy. The owner persists changes atomically
/// with membership allocation and the exact operation receipt.
#[derive(Clone)]
pub struct OneShotInvitation<Principal, Fingerprint> {
    expires_at_seconds: u64,
    state: InvitationState<Principal, Fingerprint>,
}

impl<Principal, Fingerprint> OneShotInvitation<Principal, Fingerprint> {
    /// Creates an unused invitation policy with an exclusive expiry time.
    pub fn new(expires_at_seconds: u64) -> Self {
        Self {
            expires_at_seconds,
            state: InvitationState::Available,
        }
    }

    /// Returns the exclusive expiry timestamp in seconds.
    pub fn expires_at_seconds(&self) -> u64 {
        self.expires_at_seconds
    }

    /// Revokes an unused invitation. A consumed invitation keeps its committed
    /// operation identity so an exact lost-response retry can still be resolved.
    pub fn revoke(&mut self) -> Result<(), InvitationFailure> {
        match &self.state {
            InvitationState::Available => {
                self.state = InvitationState::Revoked;
                Ok(())
            }
            InvitationState::Consumed { .. } => Err(InvitationFailure::Consumed),
            InvitationState::Revoked => Err(InvitationFailure::Revoked),
        }
    }
}

impl<Principal: Eq, Fingerprint: Eq> OneShotInvitation<Principal, Fingerprint> {
    /// Atomically advances this policy for one authenticated allocation request.
    ///
    /// The owner calls this on transaction-local state and commits the returned
    /// `ConsumedNow` together with membership and receipt. `ExactReplay` is checked
    /// before expiry so an accepted operation remains resolvable after its invite
    /// expires. The operation ID is scoped by the caller to principal and session.
    pub fn consume(
        &mut self,
        principal: Principal,
        session: SessionId,
        operation: OperationId,
        fingerprint: Fingerprint,
        now_seconds: u64,
    ) -> Result<InvitationUse, InvitationFailure> {
        match &self.state {
            InvitationState::Consumed {
                principal: consumed_principal,
                session: consumed_session,
                operation: consumed_operation,
                fingerprint: consumed_fingerprint,
            } => {
                if consumed_principal == &principal
                    && *consumed_session == session
                    && *consumed_operation == operation
                {
                    if consumed_fingerprint == &fingerprint {
                        return Ok(InvitationUse::ExactReplay);
                    }
                    return Err(InvitationFailure::OperationConflict);
                }
                Err(InvitationFailure::Consumed)
            }
            InvitationState::Revoked => Err(InvitationFailure::Revoked),
            InvitationState::Available => {
                if now_seconds >= self.expires_at_seconds {
                    return Err(InvitationFailure::Expired);
                }
                self.state = InvitationState::Consumed {
                    principal,
                    session,
                    operation,
                    fingerprint,
                };
                Ok(InvitationUse::ConsumedNow)
            }
        }
    }
}

impl<Principal, Fingerprint> fmt::Debug for OneShotInvitation<Principal, Fingerprint> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = match &self.state {
            InvitationState::Available => "available",
            InvitationState::Consumed { .. } => "consumed",
            InvitationState::Revoked => "revoked",
        };
        formatter
            .debug_struct("OneShotInvitation")
            .field("expires_at_seconds", &self.expires_at_seconds)
            .field("state", &state)
            .finish()
    }
}

/// A membership allocation acknowledged by the owner after its atomic commit.
///
/// Only the durable owner may create this value, and only after it commits the
/// invitation use, membership/grant, and operation receipt in one transaction.
#[derive(Clone, Eq, PartialEq)]
pub struct InvitationReceipt<Member> {
    session: SessionId,
    operation: OperationId,
    member: Member,
}

impl<Member> InvitationReceipt<Member> {
    /// Constructs a receipt for a membership allocation already durably committed.
    pub fn new_after_commit(session: SessionId, operation: OperationId, member: Member) -> Self {
        Self {
            session,
            operation,
            member,
        }
    }

    pub fn session(&self) -> SessionId {
        self.session
    }

    pub fn operation(&self) -> OperationId {
        self.operation
    }

    pub fn member(&self) -> &Member {
        &self.member
    }
}

/// Result of an invitation owner call. Only committed or exact-replay outcomes
/// contain a membership receipt; ambiguous and unavailable writes never do.
#[derive(Clone, Eq, PartialEq)]
pub enum InvitationOutcome<Member> {
    Committed(InvitationReceipt<Member>),
    Replayed(InvitationReceipt<Member>),
    Rejected(InvitationFailure),
    Indeterminate,
    Unavailable,
}

impl<Member> fmt::Debug for InvitationOutcome<Member> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variant = match self {
            Self::Committed(_) => "Committed",
            Self::Replayed(_) => "Replayed",
            Self::Rejected(_) => "Rejected",
            Self::Indeterminate => "Indeterminate",
            Self::Unavailable => "Unavailable",
        };
        formatter.write_str(variant)
    }
}

/// Atomic persistence boundary for invitation redemption and membership allocation.
///
/// Implementations authenticate `principal`, resolve `proof`, look up the exact
/// principal/session/operation/fingerprint before expiry rejection, and apply
/// `OneShotInvitation::consume` in the same transaction as membership, grant, and
/// receipt writes. A committed consumed operation identity must remain durable even
/// if its receipt is later lost or expires. In that case an exact retry is
/// indeterminate; absence of a prior receipt never authorizes another allocation.
/// Production implementations supply trusted database time through `now_seconds`;
/// this value is never taken from an untrusted client.
pub trait InvitationRedeemer {
    type Principal;
    type Proof: ?Sized;
    type Fingerprint;
    type Member;

    fn redeem_atomically(
        &mut self,
        principal: &Self::Principal,
        proof: &Self::Proof,
        session: SessionId,
        operation: OperationId,
        fingerprint: &Self::Fingerprint,
        now_seconds: u64,
    ) -> InvitationOutcome<Self::Member>;
}
