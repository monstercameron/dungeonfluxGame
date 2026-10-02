use std::time::Duration;

use df_experience::cooldown::{
    DeclineOutcome, RefusalEligibility, RefusalError, RefusalMemory, RefusalScope,
};
use df_types::{RunId, SessionId};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Participant(u16);

#[derive(Clone, Copy, PartialEq, Eq)]
struct Opportunity(u16);

fn scope(session: u8, run: u8) -> RefusalScope {
    RefusalScope {
        session: SessionId::from_bytes(&[session; 16]).unwrap(),
        run: RunId::from_bytes(&[run; 16]).unwrap(),
    }
}

fn time(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

fn memory(capacity: usize) -> RefusalMemory<Participant, Opportunity> {
    RefusalMemory::new(scope(1, 2), capacity, time(10)).unwrap()
}

#[test]
fn explicit_decline_cools_only_voluntary_eligibility_until_exact_deadline() {
    let mut memory = memory(4);
    assert_eq!(
        memory.record_decline(scope(1, 2), Participant(1), Opportunity(9), time(5)),
        Ok(DeclineOutcome::Recorded { ready_at: time(15) })
    );
    assert_eq!(
        memory.eligibility(scope(1, 2), Participant(1), time(14)),
        Ok(RefusalEligibility::CoolingDown { remaining: time(1) })
    );
    assert_eq!(
        memory.eligibility(scope(1, 2), Participant(1), time(15)),
        Ok(RefusalEligibility::Eligible)
    );
    assert_eq!(memory.len(), 1);
}

#[test]
fn quiet_or_absent_input_never_creates_refusal() {
    let memory = memory(4);
    for at in [0, 5, 100, u64::MAX] {
        assert_eq!(
            memory.eligibility(scope(1, 2), Participant(1), time(at)),
            Ok(RefusalEligibility::Eligible)
        );
    }
    assert!(memory.is_empty());
}

#[test]
fn retained_opportunity_retries_do_not_extend_deadline_even_after_expiry() {
    let mut memory = memory(4);
    memory
        .record_decline(scope(1, 2), Participant(1), Opportunity(9), time(5))
        .unwrap();
    for at in [5, 12, 50] {
        assert_eq!(
            memory.record_decline(scope(1, 2), Participant(1), Opportunity(9), time(at)),
            Ok(DeclineOutcome::AlreadyRecorded { ready_at: time(15) })
        );
    }
    assert_eq!(memory.len(), 1);
    assert_eq!(
        memory.eligibility(scope(1, 2), Participant(1), time(50)),
        Ok(RefusalEligibility::Eligible)
    );
}

#[test]
fn only_new_explicit_decline_extends_participant_cooldown() {
    let mut memory = memory(4);
    memory
        .record_decline(scope(1, 2), Participant(1), Opportunity(9), time(5))
        .unwrap();
    assert_eq!(
        memory.record_decline(scope(1, 2), Participant(1), Opportunity(10), time(8)),
        Ok(DeclineOutcome::Recorded { ready_at: time(18) })
    );
    assert_eq!(
        memory.record_decline(scope(1, 2), Participant(1), Opportunity(9), time(9)),
        Ok(DeclineOutcome::AlreadyRecorded { ready_at: time(15) })
    );
    assert_eq!(
        memory.eligibility(scope(1, 2), Participant(1), time(15)),
        Ok(RefusalEligibility::CoolingDown { remaining: time(3) })
    );
    assert_eq!(
        memory.eligibility(scope(1, 2), Participant(2), time(15)),
        Ok(RefusalEligibility::Eligible)
    );
}

#[test]
fn active_capacity_refuses_without_erasing_another_participants_decline() {
    let mut memory = memory(1);
    memory
        .record_decline(scope(1, 2), Participant(1), Opportunity(9), time(5))
        .unwrap();
    assert_eq!(
        memory.record_decline(scope(1, 2), Participant(2), Opportunity(10), time(6)),
        Err(RefusalError::Capacity)
    );
    assert_eq!(memory.len(), 1);
    assert_eq!(
        memory.eligibility(scope(1, 2), Participant(1), time(6)),
        Ok(RefusalEligibility::CoolingDown { remaining: time(9) })
    );
    assert_eq!(
        memory.eligibility(scope(1, 2), Participant(2), time(6)),
        Ok(RefusalEligibility::Eligible)
    );
}

#[test]
fn capacity_reuses_only_expired_records_in_admission_order() {
    let mut memory = memory(2);
    memory
        .record_decline(scope(1, 2), Participant(1), Opportunity(9), time(0))
        .unwrap();
    memory
        .record_decline(scope(1, 2), Participant(2), Opportunity(10), time(5))
        .unwrap();
    memory
        .record_decline(scope(1, 2), Participant(3), Opportunity(11), time(10))
        .unwrap();
    assert_eq!(memory.len(), 2);
    assert_eq!(
        memory.eligibility(scope(1, 2), Participant(2), time(10)),
        Ok(RefusalEligibility::CoolingDown { remaining: time(5) })
    );
    assert_eq!(
        memory.eligibility(scope(1, 2), Participant(3), time(10)),
        Ok(RefusalEligibility::CoolingDown {
            remaining: time(10)
        })
    );
}

#[test]
fn scope_mismatch_and_backwards_time_cannot_change_refusals() {
    let mut memory = memory(4);
    memory
        .record_decline(scope(1, 2), Participant(1), Opportunity(9), time(5))
        .unwrap();
    for (invalid_scope, at, expected) in [
        (scope(3, 2), time(6), RefusalError::WrongSession),
        (scope(1, 3), time(6), RefusalError::WrongRun),
        (scope(1, 2), time(4), RefusalError::TimeRegression),
    ] {
        assert_eq!(
            memory.record_decline(invalid_scope, Participant(2), Opportunity(10), at),
            Err(expected)
        );
        assert_eq!(
            memory.eligibility(invalid_scope, Participant(1), at),
            Err(expected)
        );
    }
    assert_eq!(memory.len(), 1);
    assert_eq!(
        memory.eligibility(scope(1, 2), Participant(1), time(5)),
        Ok(RefusalEligibility::CoolingDown {
            remaining: time(10)
        })
    );
    let independent =
        RefusalMemory::<Participant, Opportunity>::new(scope(3, 2), 4, time(10)).unwrap();
    assert_eq!(
        independent.eligibility(scope(3, 2), Participant(1), time(5)),
        Ok(RefusalEligibility::Eligible)
    );
}

#[test]
fn overflow_refusal_does_not_consume_capacity_or_advance_clock() {
    let mut memory = memory(1);
    assert_eq!(
        memory.record_decline(scope(1, 2), Participant(1), Opportunity(9), Duration::MAX),
        Err(RefusalError::TimeOverflow)
    );
    assert!(memory.is_empty());
    assert_eq!(
        memory.record_decline(scope(1, 2), Participant(1), Opportunity(9), time(0)),
        Ok(DeclineOutcome::Recorded { ready_at: time(10) })
    );
}

#[test]
fn zero_excessive_or_large_inline_bounds_are_typed_refusals() {
    for capacity in [0, 4097, usize::MAX] {
        assert!(matches!(
            RefusalMemory::<Participant, Opportunity>::new(scope(1, 2), capacity, time(10)),
            Err(RefusalError::InvalidCapacity)
        ));
    }
    assert!(matches!(
        RefusalMemory::<Participant, Opportunity>::new(scope(1, 2), 1, Duration::ZERO),
        Err(RefusalError::InvalidCooldown)
    ));
    assert!(matches!(
        RefusalMemory::<[u8; 4096], Opportunity>::new(scope(1, 2), 4096, time(10)),
        Err(RefusalError::RecordBudget)
    ));
}

#[test]
fn cloning_stages_refusal_without_mutating_the_accepted_candidate() {
    let accepted = memory(4);
    let mut candidate = accepted.clone();
    candidate
        .record_decline(scope(1, 2), Participant(1), Opportunity(9), time(5))
        .unwrap();
    assert!(accepted.is_empty());
    assert_eq!(
        accepted.eligibility(scope(1, 2), Participant(1), time(5)),
        Ok(RefusalEligibility::Eligible)
    );
    assert_eq!(
        candidate.eligibility(scope(1, 2), Participant(1), time(5)),
        Ok(RefusalEligibility::CoolingDown {
            remaining: time(10)
        })
    );
}
