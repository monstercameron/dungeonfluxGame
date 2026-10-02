/// Lifecycle state, separate from transport request cancellation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebtStatus {
    Active,
    Completed,
    Breached,
    Expired,
    Cancelled,
}

/// A server-supplied determination for one explicit obligation action.
/// This value does not authenticate a caller or establish the truth of evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebtAuthorization {
    Approved,
    Denied,
    NeedsRuling,
}

/// An explicit source-backed lifecycle action; no action is inferred from dialogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebtAction {
    Complete,
    Breach,
    Expire,
    Cancel,
}

/// Borrowed access to the authoritative caller's obligation record.
///
/// Associated types preserve canonical identity, typed terms, provenance and time
/// without defining another durable model. The caller validates contained data,
/// including explicit approved agreement, party identity, time units and byte
/// bounds. Each accessor must return the same validated immutable snapshot.
/// No implementation may
/// consult clocks, storage or providers while this pure operation reads the view.
pub trait ObligationView {
    type Id: Clone + Eq;
    type Party: Clone;
    type Terms: Clone;
    type Basis: Clone + Eq;
    type LogicalTime: Clone + Ord;
    type Provenance: Clone;

    fn obligation_id(&self) -> &Self::Id;
    fn debtor(&self) -> &Self::Party;
    fn creditor(&self) -> &Self::Party;
    fn terms(&self) -> &Self::Terms;
    fn basis(&self) -> &Self::Basis;
    fn status(&self) -> DebtStatus;
    fn last_transition_at(&self) -> &Self::LogicalTime;
    fn due_at(&self) -> Option<&Self::LogicalTime>;
    fn agreement_provenance(&self) -> &Self::Provenance;
}

/// One explicit admitted command, bound to the obligation and its current basis.
///
/// The native authority supplies authorization and validates action evidence,
/// including any due-condition policy. In particular `Cancel` is an approved
/// change of obligation terms, not cancellation of an RPC or proposed decision.
pub struct DebtTransitionRequest<'a, V: ObligationView> {
    pub obligation_id: &'a V::Id,
    pub expected_basis: &'a V::Basis,
    pub at: &'a V::LogicalTime,
    pub action: DebtAction,
    pub authorization: DebtAuthorization,
    pub action_provenance: &'a V::Provenance,
}

/// An owned candidate with the original agreement and new action evidence.
///
/// It does not commit or mutate its source record. The session owner revalidates
/// `expected_basis`, retains prior history and commits the selected candidate
/// atomically with the decision. Agreement provenance is retained verbatim, not
/// replaced by completion/cancellation evidence. Contained clone implementations
/// must preserve value semantics and the caller-admitted size bounds.
/// Rejecting this candidate changes nothing.
pub struct DebtTransitionProposal<V: ObligationView> {
    pub obligation_id: V::Id,
    pub debtor: V::Party,
    pub creditor: V::Party,
    pub terms: V::Terms,
    pub expected_basis: V::Basis,
    pub at: V::LogicalTime,
    pub action: DebtAction,
    pub status: DebtStatus,
    pub agreement_provenance: V::Provenance,
    pub action_provenance: V::Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebtTransitionRefusal {
    WrongObligation,
    StaleBasis,
    PermissionDenied,
    NeedsRuling,
    AlreadyTerminal(DebtStatus),
    TimeBeforeCurrentState,
    NoExpiryDeadline,
    ExpiryNotDue,
}

/// A domain refusal is an explicit successful policy outcome.
pub enum DebtTransitionOutcome<V: ObligationView> {
    Proposed(DebtTransitionProposal<V>),
    Refused(DebtTransitionRefusal),
}

/// Proposes one lifecycle transition, preserving agreement and action provenance.
///
/// Explicit caller time controls expiry; equality with the due time is eligible.
/// Other actions require the caller's source-backed authorization. A terminal
/// obligation cannot transition again. There are no loops, side effects or hidden
/// reads; retrying against the same supplied basis yields the same candidate.
pub fn propose_debt_transition<V: ObligationView>(
    current: &V,
    request: DebtTransitionRequest<'_, V>,
) -> DebtTransitionOutcome<V> {
    use DebtTransitionOutcome::{Proposed, Refused};
    use DebtTransitionRefusal::{
        AlreadyTerminal, ExpiryNotDue, NeedsRuling, NoExpiryDeadline, PermissionDenied, StaleBasis,
        TimeBeforeCurrentState, WrongObligation,
    };

    if current.obligation_id() != request.obligation_id {
        return Refused(WrongObligation);
    }
    if current.basis() != request.expected_basis {
        return Refused(StaleBasis);
    }
    match request.authorization {
        DebtAuthorization::Denied => return Refused(PermissionDenied),
        DebtAuthorization::NeedsRuling => return Refused(NeedsRuling),
        DebtAuthorization::Approved => {}
    }
    let status = current.status();
    if status != DebtStatus::Active {
        return Refused(AlreadyTerminal(status));
    }
    if request.at < current.last_transition_at() {
        return Refused(TimeBeforeCurrentState);
    }
    if request.action == DebtAction::Expire {
        let Some(due_at) = current.due_at() else {
            return Refused(NoExpiryDeadline);
        };
        if request.at < due_at {
            return Refused(ExpiryNotDue);
        }
    }
    let status = match request.action {
        DebtAction::Complete => DebtStatus::Completed,
        DebtAction::Breach => DebtStatus::Breached,
        DebtAction::Expire => DebtStatus::Expired,
        DebtAction::Cancel => DebtStatus::Cancelled,
    };
    Proposed(DebtTransitionProposal {
        obligation_id: current.obligation_id().clone(),
        debtor: current.debtor().clone(),
        creditor: current.creditor().clone(),
        terms: current.terms().clone(),
        expected_basis: current.basis().clone(),
        at: request.at.clone(),
        action: request.action,
        status,
        agreement_provenance: current.agreement_provenance().clone(),
        action_provenance: request.action_provenance.clone(),
    })
}
