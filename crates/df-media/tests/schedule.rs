use df_media::schedule::{
    DispatchOutcome, MediaPriority, MediaScheduler, ScheduleError, ScheduleLimits,
};
use df_types::OperationId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CallerJobId(u32);

fn operation() -> OperationId {
    OperationId::from_bytes(&[1; 16]).unwrap()
}

fn limits() -> ScheduleLimits {
    ScheduleLimits {
        queue_items: 6,
        queue_bytes: 64,
        speech_items: 2,
        speech_bytes: 16,
        execution_slots: 3,
        speech_slots: 1,
    }
}

#[test]
fn speculative_item_saturation_keeps_speech_queue_slots_available() {
    let mut queue = MediaScheduler::new(limits()).unwrap();
    for id in 1..=4 {
        queue
            .admit(
                CallerJobId(id),
                operation(),
                MediaPriority::Optional,
                vec![3; 8].into_boxed_slice(),
            )
            .unwrap();
    }
    let refused = queue
        .admit(
            CallerJobId(5),
            operation(),
            MediaPriority::Optional,
            vec![9; 8].into_boxed_slice(),
        )
        .unwrap_err();
    assert_eq!(refused.reason, ScheduleError::ItemCapacity);
    assert_eq!(refused.request.payload(), &[9; 8]);
    for id in 6..=7 {
        queue
            .admit(
                CallerJobId(id),
                operation(),
                MediaPriority::Speech,
                vec![2; 8].into_boxed_slice(),
            )
            .unwrap();
    }
    let refused = queue
        .admit(
            CallerJobId(8),
            operation(),
            MediaPriority::Speech,
            Box::new([]),
        )
        .unwrap_err();
    assert_eq!(refused.reason, ScheduleError::ItemCapacity);
    assert_eq!(queue.snapshot().queued_items, 6);
    assert_eq!(queue.snapshot().queued_bytes, 48);
    assert_eq!(queue.snapshot().refused_admissions, 2);
}

#[test]
fn speculative_byte_saturation_including_active_work_keeps_speech_bytes() {
    let mut policy = limits();
    policy.queue_items = 10;
    policy.queue_bytes = 24;
    policy.speech_bytes = 8;
    let mut queue = MediaScheduler::new(policy).unwrap();
    queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Optional,
            vec![1; 16].into_boxed_slice(),
        )
        .unwrap();
    let active = queue.begin().unwrap().unwrap();
    assert_eq!(queue.snapshot().queued_bytes, 0);
    assert_eq!(queue.snapshot().active_bytes, 16);
    let refused = queue
        .admit(
            CallerJobId(2),
            operation(),
            MediaPriority::Likely,
            vec![2].into_boxed_slice(),
        )
        .unwrap_err();
    assert_eq!(refused.reason, ScheduleError::ByteCapacity);
    queue
        .admit(
            CallerJobId(3),
            operation(),
            MediaPriority::Speech,
            vec![3; 8].into_boxed_slice(),
        )
        .unwrap();
    assert_eq!(
        queue.snapshot().queued_bytes + queue.snapshot().active_bytes,
        24
    );
    let refused = queue
        .admit(
            CallerJobId(4),
            operation(),
            MediaPriority::Speech,
            vec![4].into_boxed_slice(),
        )
        .unwrap_err();
    assert_eq!(refused.reason, ScheduleError::ByteCapacity);
    queue.finish(active, DispatchOutcome::Completed).unwrap();
    queue
        .admit(
            CallerJobId(4),
            operation(),
            MediaPriority::Likely,
            vec![4; 16].into_boxed_slice(),
        )
        .unwrap();
}

#[test]
fn occupied_speculative_execution_slots_do_not_block_speech() {
    let mut queue = MediaScheduler::new(limits()).unwrap();
    for id in 1..=3 {
        queue
            .admit(
                CallerJobId(id),
                operation(),
                MediaPriority::Optional,
                vec![id as u8].into_boxed_slice(),
            )
            .unwrap();
    }
    let first = queue.begin().unwrap().unwrap();
    let second = queue.begin().unwrap().unwrap();
    assert!(queue.begin().unwrap().is_none());
    queue
        .admit(
            CallerJobId(4),
            operation(),
            MediaPriority::Speech,
            vec![4].into_boxed_slice(),
        )
        .unwrap();
    let speech = queue.begin().unwrap().unwrap();
    assert_eq!(*speech.request().id(), CallerJobId(4));
    assert_eq!(speech.request().priority(), MediaPriority::Speech);
    assert_eq!(queue.snapshot().active_dispatches, 3);
    assert_eq!(queue.snapshot().active_non_speech, 2);
    assert!(queue.begin().unwrap().is_none());
    queue.finish(first, DispatchOutcome::Completed).unwrap();
    let third = queue.begin().unwrap().unwrap();
    assert_eq!(*third.request().id(), CallerJobId(3));
    queue.finish(second, DispatchOutcome::Cancelled).unwrap();
    queue.finish(speech, DispatchOutcome::Completed).unwrap();
    queue.finish(third, DispatchOutcome::Failed).unwrap();
    let snapshot = queue.snapshot();
    assert_eq!(snapshot.active_dispatches, 0);
    assert_eq!(
        (
            snapshot.completed_dispatches,
            snapshot.cancelled_dispatches,
            snapshot.failed_dispatches
        ),
        (2, 1, 1)
    );
}

#[test]
fn weighted_fifo_services_optional_demand_under_continuous_higher_priority_load() {
    let policy = ScheduleLimits {
        queue_items: 64,
        queue_bytes: 1024,
        speech_items: 16,
        speech_bytes: 64,
        execution_slots: 2,
        speech_slots: 1,
    };
    let mut queue = MediaScheduler::new(policy).unwrap();
    let classes = [
        MediaPriority::Speech,
        MediaPriority::Required,
        MediaPriority::Likely,
        MediaPriority::Optional,
    ];
    for (class, count) in classes.into_iter().zip([24, 12, 6, 3]) {
        for sequence in 0..count {
            let class_id = match class {
                MediaPriority::Speech => 0,
                MediaPriority::Required => 1,
                MediaPriority::Likely => 2,
                MediaPriority::Optional => 3,
            };
            queue
                .admit(
                    CallerJobId(class_id * 100 + sequence),
                    operation(),
                    class,
                    Box::new([]),
                )
                .unwrap();
        }
    }
    let expected = [MediaPriority::Speech; 8]
        .into_iter()
        .chain([MediaPriority::Required; 4])
        .chain([MediaPriority::Likely; 2])
        .chain([MediaPriority::Optional]);
    let mut next = [0; 4];
    for class in expected.cycle().take(45) {
        let dispatch = queue.begin().unwrap().unwrap();
        assert_eq!(dispatch.request().priority(), class);
        let index = match class {
            MediaPriority::Speech => 0,
            MediaPriority::Required => 1,
            MediaPriority::Likely => 2,
            MediaPriority::Optional => 3,
        };
        assert_eq!(
            *dispatch.request().id(),
            CallerJobId(index as u32 * 100 + next[index])
        );
        next[index] += 1;
        queue.finish(dispatch, DispatchOutcome::Completed).unwrap();
    }
    assert_eq!(next, [24, 12, 6, 3]);
    assert!(queue.begin().unwrap().is_none());
}

#[test]
fn queued_and_running_duplicate_ids_refuse_without_losing_original_work() {
    let mut queue = MediaScheduler::new(limits()).unwrap();
    queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Required,
            vec![1, 2, 3].into_boxed_slice(),
        )
        .unwrap();
    let duplicate = queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Speech,
            vec![4, 5].into_boxed_slice(),
        )
        .unwrap_err();
    assert_eq!(duplicate.reason, ScheduleError::Duplicate);
    assert_eq!(duplicate.request.payload(), &[4, 5]);
    let dispatch = queue.begin().unwrap().unwrap();
    let duplicate = queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Speech,
            vec![6].into_boxed_slice(),
        )
        .unwrap_err();
    assert_eq!(duplicate.reason, ScheduleError::Duplicate);
    let request = queue.finish(dispatch, DispatchOutcome::Completed).unwrap();
    assert_eq!(request.payload(), &[1, 2, 3]);
    assert_eq!(request.operation(), operation());
    queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Speech,
            vec![6].into_boxed_slice(),
        )
        .unwrap();
}

#[test]
fn cross_owner_completion_returns_token_without_freeing_either_owner() {
    let mut first = MediaScheduler::new(limits()).unwrap();
    let mut second = MediaScheduler::new(limits()).unwrap();
    first
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Speech,
            vec![1].into_boxed_slice(),
        )
        .unwrap();
    second
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Speech,
            vec![2].into_boxed_slice(),
        )
        .unwrap();
    let first_dispatch = first.begin().unwrap().unwrap();
    let second_dispatch = second.begin().unwrap().unwrap();
    let rejected = second
        .finish(first_dispatch, DispatchOutcome::Completed)
        .unwrap_err();
    assert_eq!(rejected.reason, ScheduleError::ForeignDispatch);
    assert_eq!(first.snapshot().active_dispatches, 1);
    assert_eq!(second.snapshot().active_dispatches, 1);
    let request = first
        .finish(rejected.dispatch, DispatchOutcome::Cancelled)
        .unwrap();
    assert_eq!(request.payload(), &[1]);
    second
        .finish(second_dispatch, DispatchOutcome::Completed)
        .unwrap();
}

#[test]
fn queued_cancellation_returns_exact_bytes_and_cannot_cancel_running_work() {
    let mut queue = MediaScheduler::new(limits()).unwrap();
    queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Required,
            vec![1; 48].into_boxed_slice(),
        )
        .unwrap();
    let cancelled = queue.cancel_queued(CallerJobId(1)).unwrap();
    assert_eq!(cancelled.payload(), &[1; 48]);
    assert_eq!(queue.snapshot().queued_bytes, 0);
    queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Required,
            vec![2; 48].into_boxed_slice(),
        )
        .unwrap();
    let active = queue.begin().unwrap().unwrap();
    assert!(queue.cancel_queued(CallerJobId(1)).is_none());
    queue.finish(active, DispatchOutcome::Cancelled).unwrap();
    assert_eq!(queue.snapshot().active_bytes, 0);
}

#[test]
fn invalid_limits_and_lost_dispatch_fail_closed_without_unbounded_admission() {
    let base = limits();
    for invalid in [
        ScheduleLimits {
            queue_items: 0,
            ..base
        },
        ScheduleLimits {
            queue_items: usize::MAX,
            ..base
        },
        ScheduleLimits {
            queue_bytes: usize::MAX,
            ..base
        },
        ScheduleLimits {
            speech_items: 7,
            ..base
        },
        ScheduleLimits {
            speech_bytes: 65,
            ..base
        },
        ScheduleLimits {
            speech_slots: 0,
            ..base
        },
        ScheduleLimits {
            execution_slots: usize::MAX,
            ..base
        },
    ] {
        assert!(matches!(
            MediaScheduler::<CallerJobId>::new(invalid),
            Err(ScheduleError::InvalidLimits)
        ));
    }
    let mut queue = MediaScheduler::new(base).unwrap();
    queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Optional,
            vec![1].into_boxed_slice(),
        )
        .unwrap();
    drop(queue.begin().unwrap().unwrap());
    assert_eq!(queue.snapshot().active_dispatches, 1);
    assert_eq!(queue.snapshot().active_bytes, 1);
    let refused = queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Speech,
            Box::new([]),
        )
        .unwrap_err();
    assert_eq!(refused.reason, ScheduleError::Duplicate);
}

#[test]
fn admission_diagnostics_omit_opaque_payload_bytes() {
    let mut queue = MediaScheduler::new(limits()).unwrap();
    queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Optional,
            Box::new([]),
        )
        .unwrap();
    let private_bytes = vec![123, 45, 67, 89].into_boxed_slice();
    let refused = queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Speech,
            private_bytes,
        )
        .unwrap_err();
    assert_eq!(refused.request.payload(), &[123, 45, 67, 89]);
    let diagnostic = format!("{:?}", refused);
    assert!(diagnostic.contains("payload_bytes: 4"));
    assert!(!diagnostic.contains("[123, 45, 67, 89]"));
}

#[test]
fn dispatch_returns_from_an_owned_native_worker_with_owner_identity_intact() {
    let mut queue = MediaScheduler::new(limits()).unwrap();
    queue
        .admit(
            CallerJobId(1),
            operation(),
            MediaPriority::Speech,
            vec![7, 8].into_boxed_slice(),
        )
        .unwrap();
    let dispatch = queue.begin().unwrap().unwrap();
    let owned_worker = std::thread::spawn(move || dispatch);
    let dispatch = owned_worker.join().unwrap();
    assert_eq!(dispatch.request().payload(), &[7, 8]);
    assert_eq!(dispatch.request().operation(), operation());
    queue.finish(dispatch, DispatchOutcome::Completed).unwrap();
    assert_eq!(queue.snapshot().active_dispatches, 0);
}
