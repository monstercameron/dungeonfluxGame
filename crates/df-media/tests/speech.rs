use df_media::schedule::{ScheduleError, ScheduleLimits};
use df_media::speech::{
    SpeechAdmissionRefusal, SpeechError, SpeechIdentity, SpeechLimits, SpeechReceipt,
    SpeechScheduler, SpeechStopReason,
};
use df_model::checkpoint::{Basis, JobId, NativeFailure};
use df_types::{OperationId, RecoveryEpoch, RunId, SessionId, SessionRevision};

fn basis(epoch: u64, sequence: u64) -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence),
    }
}

fn identity(value: u8) -> SpeechIdentity {
    SpeechIdentity {
        basis: basis(1, 7),
        job: JobId::from_bytes(&[value; 16]).unwrap(),
        operation: OperationId::from_bytes(&[8; 16]).unwrap(),
        generation: 11,
    }
}

fn owner() -> SpeechScheduler {
    SpeechScheduler::new(
        basis(1, 7),
        ScheduleLimits {
            queue_items: 3,
            queue_bytes: 8,
            speech_items: 1,
            speech_bytes: 1,
            execution_slots: 2,
            speech_slots: 1,
        },
        SpeechLimits {
            maximum_chunks: 3,
            maximum_bytes: 6,
        },
    )
    .unwrap()
}

fn start(owner: &mut SpeechScheduler, identity: SpeechIdentity) -> SpeechReceipt {
    owner.admit(identity, Box::from([7])).unwrap();
    owner.begin().unwrap().unwrap()
}

fn queue(owner: &mut SpeechScheduler, receipt: &SpeechReceipt, ordinal: u64, bytes: &[u8]) {
    owner
        .queue_chunk(receipt, receipt.identity(), ordinal, bytes.into())
        .unwrap();
}

fn close(owner: &mut SpeechScheduler, receipt: &SpeechReceipt, count: u64) {
    owner.end(receipt, receipt.identity(), count).unwrap();
    owner.eof(receipt, receipt.identity()).unwrap();
}

#[test]
fn actual_dispatch_drains_ordered_tail_before_one_completion_and_returns_original_command() {
    let mut owner = owner();
    let receipt = start(&mut owner, identity(3));
    assert_eq!(owner.snapshot().schedule.active_dispatches, 1);
    assert_eq!(owner.snapshot().schedule.active_bytes, 1);
    queue(&mut owner, &receipt, 2, &[3, 3]);
    queue(&mut owner, &receipt, 0, &[1, 1]);
    queue(&mut owner, &receipt, 1, &[2, 2]);
    assert!(owner.next(&receipt).unwrap().is_none());
    assert_eq!(owner.finish(&receipt).unwrap_err(), SpeechError::Waiting);
    close(&mut owner, &receipt, 3);
    for ordinal in 0..3 {
        let chunk = owner.next(&receipt).unwrap().unwrap();
        assert_eq!(chunk.sequence, ordinal);
        assert_eq!(chunk.bytes, &[ordinal as u8 + 1; 2]);
        assert_eq!(owner.snapshot().buffered_bytes, 6 - ordinal as usize * 2);
        assert_eq!(owner.finish(&receipt).unwrap_err(), SpeechError::Waiting);
        owner.acknowledge(&receipt, ordinal).unwrap();
    }
    let request = owner.finish(&receipt).unwrap();
    assert_eq!(*request.id(), identity(3).job);
    assert_eq!(request.operation(), identity(3).operation);
    assert_eq!(request.payload(), &[7]);
    let snapshot = owner.snapshot();
    assert_eq!(snapshot.buffered_bytes, 0);
    assert_eq!(snapshot.buffered_chunks, 0);
    assert_eq!(snapshot.schedule.active_dispatches, 0);
    assert_eq!(snapshot.schedule.completed_dispatches, 1);
    assert_eq!(owner.finish(&receipt).unwrap_err(), SpeechError::Stale);
}

#[test]
fn overtaking_end_waits_for_exact_frames_and_clean_eof() {
    let mut owner = owner();
    let receipt = start(&mut owner, identity(3));
    owner.end(&receipt, receipt.identity(), 2).unwrap();
    queue(&mut owner, &receipt, 1, &[2]);
    assert!(owner.next(&receipt).unwrap().is_none());
    queue(&mut owner, &receipt, 0, &[1]);
    assert!(owner.next(&receipt).unwrap().is_none());
    owner.eof(&receipt, receipt.identity()).unwrap();
    assert_eq!(owner.next(&receipt).unwrap().unwrap().bytes, &[1]);
    owner.acknowledge(&receipt, 0).unwrap();
    assert_eq!(owner.next(&receipt).unwrap().unwrap().bytes, &[2]);
    owner.acknowledge(&receipt, 1).unwrap();
    owner.finish(&receipt).unwrap();
    assert_eq!(owner.snapshot().schedule.completed_dispatches, 1);
}

#[test]
fn missing_end_or_gapped_eof_keeps_partial_bytes_owned_and_never_completes() {
    for with_end in [false, true] {
        let mut owner = owner();
        let receipt = start(&mut owner, identity(3));
        queue(&mut owner, &receipt, 1, &[2]);
        if with_end {
            owner.end(&receipt, receipt.identity(), 2).unwrap();
        }
        assert_eq!(
            owner.eof(&receipt, receipt.identity()),
            Err(SpeechError::IncompleteEof)
        );
        assert!(matches!(
            owner.next(&receipt),
            Err(SpeechError::Failed(NativeFailure::Rejected))
        ));
        assert_eq!(owner.snapshot().buffered_bytes, 1);
        assert_eq!(owner.snapshot().schedule.active_dispatches, 1);
        assert_eq!(owner.snapshot().schedule.completed_dispatches, 0);
        let stopped = owner.stop(&receipt, SpeechStopReason::Cancelled).unwrap();
        assert_eq!(
            stopped.reason,
            SpeechStopReason::Failed(NativeFailure::Rejected)
        );
        assert_eq!((stopped.discarded_chunks, stopped.discarded_bytes), (1, 1));
        assert_eq!(owner.snapshot().schedule.failed_dispatches, 1);
        assert_eq!(owner.snapshot().schedule.completed_dispatches, 0);
    }
}

#[test]
fn borrowed_and_unacknowledged_chunks_share_global_budget_until_consumed() {
    let mut owner = owner();
    let first = start(&mut owner, identity(3));
    let second = start(&mut owner, identity(4));
    queue(&mut owner, &first, 0, &[1; 6]);
    close(&mut owner, &first, 1);
    let pointer = owner.next(&first).unwrap().unwrap().bytes.as_ptr();
    assert_eq!(owner.next(&first).unwrap().unwrap().bytes.as_ptr(), pointer);
    assert_eq!(owner.snapshot().buffered_bytes, 6);
    let payload: Box<[u8]> = Box::from([2]);
    let address = payload.as_ptr();
    let refusal = owner
        .queue_chunk(&second, second.identity(), 0, payload)
        .unwrap_err();
    assert_eq!(refusal.reason, SpeechError::ByteCapacity);
    assert_eq!(refusal.bytes.as_ptr(), address);
    assert_eq!(
        owner.acknowledge(&first, 1),
        Err(SpeechError::WrongAcknowledgement)
    );
    assert_eq!(owner.snapshot().buffered_bytes, 6);
    owner.acknowledge(&first, 0).unwrap();
    queue(&mut owner, &second, 0, &[2]);
    owner.finish(&first).unwrap();
    owner.stop(&second, SpeechStopReason::Cancelled).unwrap();
}

#[test]
fn empty_chunks_consume_global_item_budget_and_overflow_retains_bytes() {
    let mut owner = owner();
    let first = start(&mut owner, identity(3));
    let second = start(&mut owner, identity(4));
    for sequence in 0..3 {
        queue(&mut owner, &first, sequence, &[]);
    }
    let refusal = owner
        .queue_chunk(&second, second.identity(), 0, Box::from([2]))
        .unwrap_err();
    assert_eq!(refusal.reason, SpeechError::ChunkCapacity);
    assert_eq!(&*refusal.bytes, &[2]);
    assert_eq!(owner.snapshot().buffered_chunks, 3);
    let stopped = owner.stop(&first, SpeechStopReason::Cancelled).unwrap();
    assert_eq!((stopped.discarded_chunks, stopped.discarded_bytes), (3, 0));
    queue(&mut owner, &second, 0, &[2]);
    owner.stop(&second, SpeechStopReason::Cancelled).unwrap();
}

#[test]
fn duplicate_trailing_and_unbounded_ordinals_never_replace_terminal_tail() {
    let mut owner = owner();
    let receipt = start(&mut owner, identity(3));
    queue(&mut owner, &receipt, 0, &[1]);
    assert_eq!(
        owner
            .queue_chunk(&receipt, receipt.identity(), 0, Box::from([9]))
            .unwrap_err()
            .reason,
        SpeechError::DuplicateChunk
    );
    assert_eq!(
        owner
            .queue_chunk(&receipt, receipt.identity(), u64::MAX, Box::from([9]))
            .unwrap_err()
            .reason,
        SpeechError::SequenceRange
    );
    assert_eq!(
        owner.end(&receipt, receipt.identity(), 0),
        Err(SpeechError::SequenceRange)
    );
    assert_eq!(
        owner.end(&receipt, receipt.identity(), u64::MAX),
        Err(SpeechError::SequenceRange)
    );
    owner.end(&receipt, receipt.identity(), 1).unwrap();
    assert_eq!(
        owner.end(&receipt, receipt.identity(), 1),
        Err(SpeechError::DuplicateEnd)
    );
    assert_eq!(
        owner
            .queue_chunk(&receipt, receipt.identity(), 1, Box::from([9]))
            .unwrap_err()
            .reason,
        SpeechError::SequenceRange
    );
    owner.eof(&receipt, receipt.identity()).unwrap();
    assert_eq!(
        owner
            .queue_chunk(&receipt, receipt.identity(), 0, Box::from([9]))
            .unwrap_err()
            .reason,
        SpeechError::Closed
    );
    assert_eq!(
        owner.eof(&receipt, receipt.identity()),
        Err(SpeechError::Closed)
    );
    assert_eq!(owner.next(&receipt).unwrap().unwrap().bytes, &[1]);
    owner.acknowledge(&receipt, 0).unwrap();
    owner.finish(&receipt).unwrap();
}

#[test]
fn cancellation_disposes_owned_buffers_and_old_receipts_cannot_release_reused_ids() {
    let mut owner = owner();
    let old = start(&mut owner, identity(3));
    queue(&mut owner, &old, 0, &[9]);
    close(&mut owner, &old, 1);
    let stopped = owner.stop(&old, SpeechStopReason::Cancelled).unwrap();
    assert_eq!(stopped.identity, identity(3));
    assert_eq!(stopped.discarded_bytes, 1);
    assert_eq!(owner.snapshot().schedule.cancelled_dispatches, 1);
    let current = start(&mut owner, identity(3));
    queue(&mut owner, &current, 0, &[1]);
    assert!(matches!(owner.next(&old), Err(SpeechError::Stale)));
    assert_eq!(owner.acknowledge(&old, 0), Err(SpeechError::Stale));
    assert!(matches!(
        owner.stop(&old, SpeechStopReason::Cancelled),
        Err(SpeechError::Stale)
    ));
    assert_eq!(owner.end(&old, old.identity(), 1), Err(SpeechError::Stale));
    assert_eq!(owner.snapshot().schedule.active_dispatches, 1);
    close(&mut owner, &current, 1);
    assert_eq!(owner.next(&current).unwrap().unwrap().bytes, &[1]);
    owner.acknowledge(&current, 0).unwrap();
    owner.finish(&current).unwrap();
    assert_eq!(owner.snapshot().schedule.completed_dispatches, 1);
}

#[test]
fn receipt_from_another_or_recreated_owner_cannot_touch_matching_current_work() {
    let mut first = owner();
    let old = start(&mut first, identity(3));
    drop(first);
    let mut second = owner();
    let current = start(&mut second, identity(3));
    assert_eq!(
        second
            .queue_chunk(&old, old.identity(), 0, Box::from([9]))
            .unwrap_err()
            .reason,
        SpeechError::Stale
    );
    assert!(matches!(
        second.stop(&old, SpeechStopReason::Cancelled),
        Err(SpeechError::Stale)
    ));
    assert_eq!(second.snapshot().schedule.active_dispatches, 1);
    second.stop(&current, SpeechStopReason::Cancelled).unwrap();
}

#[test]
fn every_actual_event_identity_field_is_checked_before_changing_owned_bytes_or_end() {
    let mut owner = owner();
    let receipt = start(&mut owner, identity(3));
    for field in 0..6 {
        let mut changed = receipt.identity();
        match field {
            0 => changed.basis.session = SessionId::from_bytes(&[9; 16]).unwrap(),
            1 => changed.basis.run = RunId::from_bytes(&[9; 16]).unwrap(),
            2 => changed.basis.revision = basis(2, 0).revision,
            3 => changed.job = identity(4).job,
            4 => changed.operation = OperationId::from_bytes(&[9; 16]).unwrap(),
            _ => changed.generation += 1,
        }
        assert_eq!(
            owner
                .queue_chunk(&receipt, changed, 0, Box::from([9]))
                .unwrap_err()
                .reason,
            SpeechError::IdentityMismatch
        );
        assert_eq!(
            owner.end(&receipt, changed, 0),
            Err(SpeechError::IdentityMismatch)
        );
        assert_eq!(
            owner.eof(&receipt, changed),
            Err(SpeechError::IdentityMismatch)
        );
    }
    assert_eq!(owner.snapshot().buffered_bytes, 0);
    owner.stop(&receipt, SpeechStopReason::Cancelled).unwrap();
}

#[test]
fn new_revision_or_recovery_epoch_keeps_old_buffers_owned_but_disables_delivery() {
    for new_basis in [basis(1, 8), basis(2, 0)] {
        let mut owner = owner();
        let old = start(&mut owner, identity(3));
        queue(&mut owner, &old, 0, &[9]);
        close(&mut owner, &old, 1);
        owner.replace_basis(new_basis).unwrap();
        assert!(matches!(owner.next(&old), Err(SpeechError::Stale)));
        assert_eq!(owner.acknowledge(&old, 0), Err(SpeechError::Stale));
        assert_eq!(owner.snapshot().buffered_bytes, 1);
        assert_eq!(owner.snapshot().schedule.active_dispatches, 1);
        let stopped = owner
            .stop(&old, SpeechStopReason::Failed(NativeFailure::Stale))
            .unwrap();
        assert_eq!(stopped.discarded_bytes, 1);
        let mut fresh = identity(3);
        fresh.basis = new_basis;
        let current = start(&mut owner, fresh);
        assert_eq!(owner.end(&old, old.identity(), 0), Err(SpeechError::Stale));
        close(&mut owner, &current, 0);
        owner.finish(&current).unwrap();
    }
}

#[test]
fn scheduler_admission_cancellation_and_occupied_slots_preserve_original_command_bytes() {
    let mut owner = owner();
    let queued = owner.admit(identity(3), Box::from([7; 8])).unwrap();
    let refusal = owner.admit(identity(4), Box::from([9])).unwrap_err();
    match refusal {
        SpeechAdmissionRefusal::Schedule(refusal) => {
            assert_eq!(refusal.reason, ScheduleError::ByteCapacity);
            assert_eq!(refusal.request.payload(), &[9]);
        }
        other => panic!("unexpected admission refusal: {other:?}"),
    }
    assert_eq!(
        owner.cancel_queued(&queued).unwrap().unwrap().payload(),
        &[7; 8]
    );
    let first = start(&mut owner, identity(3));
    let second = start(&mut owner, identity(4));
    owner.admit(identity(5), Box::from([5])).unwrap();
    assert!(owner.begin().unwrap().is_none());
    assert!(owner.cancel_queued(&first).unwrap().is_none());
    owner.stop(&first, SpeechStopReason::Cancelled).unwrap();
    let third = owner.begin().unwrap().unwrap();
    assert_eq!(third.identity(), identity(5));
    owner
        .stop(&second, SpeechStopReason::Failed(NativeFailure::Deadline))
        .unwrap();
    owner.stop(&third, SpeechStopReason::Cancelled).unwrap();
    assert_eq!(owner.snapshot().schedule.active_dispatches, 0);
    assert_eq!(owner.snapshot().schedule.failed_dispatches, 1);
}

#[test]
fn missing_terminal_and_dropped_receipt_fail_closed_until_explicit_owner_teardown() {
    let mut owner = owner();
    let receipt = start(&mut owner, identity(3));
    queue(&mut owner, &receipt, 0, &[1]);
    assert_eq!(owner.finish(&receipt).unwrap_err(), SpeechError::Waiting);
    drop(receipt);
    assert_eq!(owner.snapshot().schedule.active_dispatches, 1);
    assert_eq!(owner.snapshot().buffered_bytes, 1);
    assert!(matches!(
        owner.admit(identity(3), Box::from([9])),
        Err(SpeechAdmissionRefusal::Schedule(_))
    ));
}

#[test]
fn delayed_queued_cancellation_cannot_remove_reused_identity_and_no_data_precedes_dispatch() {
    let mut owner = owner();
    let old = owner.admit(identity(3), Box::from([7])).unwrap();
    assert_eq!(
        owner
            .queue_chunk(&old, old.identity(), 0, Box::from([9]))
            .unwrap_err()
            .reason,
        SpeechError::NotStarted
    );
    assert_eq!(
        owner.end(&old, old.identity(), 0),
        Err(SpeechError::NotStarted)
    );
    assert_eq!(
        owner.eof(&old, old.identity()),
        Err(SpeechError::NotStarted)
    );
    owner.cancel_queued(&old).unwrap().unwrap();
    let current = owner.admit(identity(3), Box::from([1])).unwrap();
    assert!(matches!(owner.cancel_queued(&old), Err(SpeechError::Stale)));
    assert_eq!(owner.snapshot().schedule.queued_items, 1);
    let begun = owner.begin().unwrap().unwrap();
    queue(&mut owner, &current, 0, &[1]);
    close(&mut owner, &begun, 1);
    assert_eq!(owner.next(&current).unwrap().unwrap().bytes, &[1]);
    owner.acknowledge(&begun, 0).unwrap();
    assert_eq!(owner.finish(&current).unwrap().payload(), &[1]);
}

#[test]
fn invalid_identity_and_duplicate_admission_preserve_exact_owned_payloads() {
    let mut owner = owner();
    for error in [SpeechError::Stale, SpeechError::InvalidGeneration] {
        let mut invalid = identity(3);
        if error == SpeechError::Stale {
            invalid.basis = basis(1, 6);
        } else {
            invalid.generation = 0;
        }
        let bytes: Box<[u8]> = Box::from([9, 8]);
        let address = bytes.as_ptr();
        match owner.admit(invalid, bytes).unwrap_err() {
            SpeechAdmissionRefusal::Invalid(reason, bytes) => {
                assert_eq!(reason, error);
                assert_eq!(bytes.as_ptr(), address);
            }
            other => panic!("unexpected refusal: {other:?}"),
        }
    }
    let queued = owner.admit(identity(3), Box::from([1])).unwrap();
    let bytes: Box<[u8]> = Box::from([9, 8]);
    let address = bytes.as_ptr();
    match owner.admit(identity(3), bytes).unwrap_err() {
        SpeechAdmissionRefusal::Schedule(refusal) => {
            assert_eq!(refusal.reason, ScheduleError::Duplicate);
            assert_eq!(refusal.request.payload().as_ptr(), address);
        }
        other => panic!("unexpected refusal: {other:?}"),
    }
    owner.cancel_queued(&queued).unwrap();
    assert_eq!(
        owner.replace_basis(basis(1, 7)),
        Err(SpeechError::RevisionNotAdvanced)
    );
}

#[test]
fn refusal_diagnostics_omit_private_chunk_bytes() {
    let mut owner = owner();
    let receipt = start(&mut owner, identity(3));
    queue(&mut owner, &receipt, 0, &[1; 6]);
    let refusal = owner
        .queue_chunk(
            &receipt,
            receipt.identity(),
            1,
            Box::from([123, 45, 67, 89]),
        )
        .unwrap_err();
    assert_eq!(refusal.reason, SpeechError::ByteCapacity);
    assert_eq!(&*refusal.bytes, &[123, 45, 67, 89]);
    let diagnostic = format!("{refusal:?}");
    assert!(diagnostic.contains("payload_bytes: 4"));
    assert!(!diagnostic.contains("[123, 45, 67, 89]"));
    owner.stop(&receipt, SpeechStopReason::Cancelled).unwrap();
}
