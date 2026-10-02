use df_interaction::debts::{
    DebtAction, DebtAuthorization, DebtStatus, DebtTransitionOutcome, DebtTransitionProposal,
    DebtTransitionRefusal, DebtTransitionRequest, ObligationView, propose_debt_transition,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObligationId(u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Participant(u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GameTick(u64);
#[derive(Debug, Clone, PartialEq, Eq)]
struct Terms {
    authored_terms: u64,
    quantity: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Evidence {
    explicit_action: u64,
    policy_revision: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct ObligationRecord {
    id: ObligationId,
    debtor: Participant,
    creditor: Participant,
    terms: Terms,
    basis: u64,
    status: DebtStatus,
    last_at: GameTick,
    due: Option<GameTick>,
    agreement: Evidence,
}
impl ObligationView for ObligationRecord {
    type Id = ObligationId;
    type Party = Participant;
    type Terms = Terms;
    type Basis = u64;
    type LogicalTime = GameTick;
    type Provenance = Evidence;
    fn obligation_id(&self) -> &Self::Id {
        &self.id
    }
    fn debtor(&self) -> &Self::Party {
        &self.debtor
    }
    fn creditor(&self) -> &Self::Party {
        &self.creditor
    }
    fn terms(&self) -> &Self::Terms {
        &self.terms
    }
    fn basis(&self) -> &Self::Basis {
        &self.basis
    }
    fn status(&self) -> DebtStatus {
        self.status
    }
    fn last_transition_at(&self) -> &Self::LogicalTime {
        &self.last_at
    }
    fn due_at(&self) -> Option<&Self::LogicalTime> {
        self.due.as_ref()
    }
    fn agreement_provenance(&self) -> &Self::Provenance {
        &self.agreement
    }
}
fn active() -> ObligationRecord {
    ObligationRecord {
        id: ObligationId(3),
        debtor: Participant(1),
        creditor: Participant(2),
        terms: Terms {
            authored_terms: 19,
            quantity: 5,
        },
        basis: 8,
        status: DebtStatus::Active,
        last_at: GameTick(10),
        due: Some(GameTick(30)),
        agreement: Evidence {
            explicit_action: 47,
            policy_revision: 6,
        },
    }
}
fn action_evidence() -> Evidence {
    Evidence {
        explicit_action: 91,
        policy_revision: 6,
    }
}
fn request<'a>(
    record: &'a ObligationRecord,
    action: DebtAction,
    at: &'a GameTick,
    evidence: &'a Evidence,
) -> DebtTransitionRequest<'a, ObligationRecord> {
    DebtTransitionRequest {
        obligation_id: &record.id,
        expected_basis: &record.basis,
        at,
        action,
        authorization: DebtAuthorization::Approved,
        action_provenance: evidence,
    }
}
fn proposal(
    outcome: DebtTransitionOutcome<ObligationRecord>,
) -> DebtTransitionProposal<ObligationRecord> {
    match outcome {
        DebtTransitionOutcome::Proposed(proposal) => proposal,
        DebtTransitionOutcome::Refused(reason) => panic!("unexpected refusal: {reason:?}"),
    }
}
fn assert_refused(
    outcome: DebtTransitionOutcome<ObligationRecord>,
    expected: DebtTransitionRefusal,
) {
    match outcome {
        DebtTransitionOutcome::Refused(reason) => assert_eq!(reason, expected),
        DebtTransitionOutcome::Proposed(_) => panic!("unexpected proposal"),
    }
}

#[test]
fn all_four_transitions_retain_typed_terms_parties_and_both_provenance_sources() {
    let record = active();
    let original = record.clone();
    let at = GameTick(30);
    let evidence = action_evidence();
    for (action, expected) in [
        (DebtAction::Complete, DebtStatus::Completed),
        (DebtAction::Breach, DebtStatus::Breached),
        (DebtAction::Expire, DebtStatus::Expired),
        (DebtAction::Cancel, DebtStatus::Cancelled),
    ] {
        let result = proposal(propose_debt_transition(
            &record,
            request(&record, action, &at, &evidence),
        ));
        assert_eq!(result.status, expected);
        assert_eq!(result.action, action);
        assert_eq!(result.obligation_id, record.id);
        assert_eq!(result.debtor, record.debtor);
        assert_eq!(result.creditor, record.creditor);
        assert_eq!(result.terms, record.terms);
        assert_eq!(result.expected_basis, record.basis);
        assert_eq!(result.at, at);
        assert_eq!(result.agreement_provenance, record.agreement);
        assert_eq!(result.action_provenance, evidence);
        assert_eq!(record, original);
    }
}

#[test]
fn expiry_requires_supplied_deadline_and_logical_time_at_or_after_due() {
    let mut record = active();
    let original = record.clone();
    let evidence = action_evidence();
    let early = GameTick(29);
    assert_refused(
        propose_debt_transition(
            &record,
            request(&record, DebtAction::Expire, &early, &evidence),
        ),
        DebtTransitionRefusal::ExpiryNotDue,
    );
    assert_eq!(record, original);
    for at in [GameTick(30), GameTick(99)] {
        let result = proposal(propose_debt_transition(
            &record,
            request(&record, DebtAction::Expire, &at, &evidence),
        ));
        assert_eq!(result.status, DebtStatus::Expired);
    }
    record.due = None;
    let now = GameTick(30);
    assert_refused(
        propose_debt_transition(
            &record,
            request(&record, DebtAction::Expire, &now, &evidence),
        ),
        DebtTransitionRefusal::NoExpiryDeadline,
    );
}

#[test]
fn cancellation_requires_explicit_authority_and_never_alters_the_original_record() {
    let record = active();
    let original = record.clone();
    let at = GameTick(20);
    let evidence = action_evidence();
    for (authorization, expected) in [
        (
            DebtAuthorization::Denied,
            DebtTransitionRefusal::PermissionDenied,
        ),
        (
            DebtAuthorization::NeedsRuling,
            DebtTransitionRefusal::NeedsRuling,
        ),
    ] {
        let mut command = request(&record, DebtAction::Cancel, &at, &evidence);
        command.authorization = authorization;
        assert_refused(propose_debt_transition(&record, command), expected);
        assert_eq!(record, original);
    }
    let result = proposal(propose_debt_transition(
        &record,
        request(&record, DebtAction::Cancel, &at, &evidence),
    ));
    assert_eq!(result.status, DebtStatus::Cancelled);
    assert_eq!(result.action_provenance, evidence);
    assert_eq!(record, original);
}

#[test]
fn wrong_identity_stale_basis_and_backward_time_cannot_produce_a_transition() {
    let record = active();
    let original = record.clone();
    let at = GameTick(20);
    let evidence = action_evidence();
    let wrong_id = ObligationId(99);
    let mut command = request(&record, DebtAction::Complete, &at, &evidence);
    command.obligation_id = &wrong_id;
    assert_refused(
        propose_debt_transition(&record, command),
        DebtTransitionRefusal::WrongObligation,
    );
    let stale = 7;
    let mut command = request(&record, DebtAction::Breach, &at, &evidence);
    command.expected_basis = &stale;
    assert_refused(
        propose_debt_transition(&record, command),
        DebtTransitionRefusal::StaleBasis,
    );
    let before = GameTick(9);
    assert_refused(
        propose_debt_transition(
            &record,
            request(&record, DebtAction::Cancel, &before, &evidence),
        ),
        DebtTransitionRefusal::TimeBeforeCurrentState,
    );
    assert_eq!(record, original);
}

#[test]
fn every_terminal_state_rejects_all_later_lifecycle_actions() {
    let evidence = action_evidence();
    let at = GameTick(40);
    for status in [
        DebtStatus::Completed,
        DebtStatus::Breached,
        DebtStatus::Expired,
        DebtStatus::Cancelled,
    ] {
        let mut record = active();
        record.status = status;
        let original = record.clone();
        for action in [
            DebtAction::Complete,
            DebtAction::Breach,
            DebtAction::Expire,
            DebtAction::Cancel,
        ] {
            assert_refused(
                propose_debt_transition(&record, request(&record, action, &at, &evidence)),
                DebtTransitionRefusal::AlreadyTerminal(status),
            );
            assert_eq!(record, original);
        }
    }
}

#[test]
fn a_candidate_is_replayable_but_a_fresh_basis_must_be_supplied_after_commit() {
    let record = active();
    let at = GameTick(20);
    let evidence = action_evidence();
    let first = proposal(propose_debt_transition(
        &record,
        request(&record, DebtAction::Complete, &at, &evidence),
    ));
    let replay = proposal(propose_debt_transition(
        &record,
        request(&record, DebtAction::Complete, &at, &evidence),
    ));
    assert_eq!(first.obligation_id, replay.obligation_id);
    assert_eq!(first.expected_basis, replay.expected_basis);
    assert_eq!(first.status, replay.status);
    assert_eq!(first.at, replay.at);
    assert_eq!(first.agreement_provenance, replay.agreement_provenance);
    assert_eq!(first.action_provenance, replay.action_provenance);
    let mut committed = record.clone();
    committed.status = first.status;
    committed.basis = 9;
    let command = request(&record, DebtAction::Complete, &at, &evidence);
    assert_refused(
        propose_debt_transition(&committed, command),
        DebtTransitionRefusal::StaleBasis,
    );
    assert_eq!(record.status, DebtStatus::Active);
}

#[test]
fn denied_or_unresolved_authority_refuses_every_action_even_after_the_due_time() {
    let record = active();
    let original = record.clone();
    let at = GameTick(40);
    let evidence = action_evidence();
    for action in [
        DebtAction::Complete,
        DebtAction::Breach,
        DebtAction::Expire,
        DebtAction::Cancel,
    ] {
        for (authorization, expected) in [
            (
                DebtAuthorization::Denied,
                DebtTransitionRefusal::PermissionDenied,
            ),
            (
                DebtAuthorization::NeedsRuling,
                DebtTransitionRefusal::NeedsRuling,
            ),
        ] {
            let mut command = request(&record, action, &at, &evidence);
            command.authorization = authorization;
            assert_refused(propose_debt_transition(&record, command), expected);
            assert_eq!(record, original);
        }
    }
}
