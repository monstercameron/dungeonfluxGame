//! Pure session-owner decision for an authenticated member's ready command.
//!
//! The caller supplies one authoritative membership/readiness snapshot and the
//! authenticated member from its existing binding. This module does not own
//! membership, commit commands, advance revisions, or publish subscriptions.

use df_auth::readiness::{ReadinessAggregate, ReadinessError, aggregate_current_readiness};
use df_types::{MemberId, SessionRevision};

/// Typed refusal when the authenticated member is not in the current snapshot
/// or when the supplied roster/readiness facts are inconsistent.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReadyDecisionError {
    AuthenticatedMemberNotCurrent,
    InvalidSnapshot(ReadinessError),
}

/// A pure candidate ready change tied to its authenticated member and basis.
///
/// `basis_revision` is the owner snapshot on which the server may decide
/// whether to apply the command. It is not an assigned result revision.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ReadyDecision {
    authenticated_member: MemberId,
    basis_revision: SessionRevision,
    desired_ready: bool,
    projected_aggregate: ReadinessAggregate,
}

impl ReadyDecision {
    /// The member supplied by the authenticated server binding.
    pub fn authenticated_member(self) -> MemberId {
        self.authenticated_member
    }

    /// The exact session revision used as the decision basis.
    pub fn basis_revision(self) -> SessionRevision {
        self.basis_revision
    }

    /// The requested ready value after projecting this decision.
    pub fn desired_ready(self) -> bool {
        self.desired_ready
    }

    /// Readiness counts after applying only this member's desired value.
    pub fn projected_aggregate(self) -> ReadinessAggregate {
        self.projected_aggregate
    }
}

/// Decides one ready change from the server's authenticated identity and a
/// single authoritative session snapshot.
///
/// The request contains no member ID: the server supplies `authenticated_member`
/// from its existing authenticated binding. The returned revision remains the
/// supplied basis; the session owner applies the command and assigns any
/// committed revision.
pub fn decide_ready_change(
    authenticated_member: MemberId,
    campaign_capacity: usize,
    basis_revision: SessionRevision,
    current_members: &[MemberId],
    ready_members: &[MemberId],
    desired_ready: bool,
) -> Result<ReadyDecision, ReadyDecisionError> {
    aggregate_current_readiness(campaign_capacity, current_members, ready_members)
        .map_err(ReadyDecisionError::InvalidSnapshot)?;

    if !current_members.contains(&authenticated_member) {
        return Err(ReadyDecisionError::AuthenticatedMemberNotCurrent);
    }

    let mut projected_ready = ready_members
        .iter()
        .copied()
        .filter(|member| *member != authenticated_member)
        .collect::<Vec<_>>();
    if desired_ready {
        projected_ready.push(authenticated_member);
    }

    let projected_aggregate =
        aggregate_current_readiness(campaign_capacity, current_members, &projected_ready)
            .map_err(ReadyDecisionError::InvalidSnapshot)?;

    Ok(ReadyDecision {
        authenticated_member,
        basis_revision,
        desired_ready,
        projected_aggregate,
    })
}
