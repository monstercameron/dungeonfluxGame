// B-F35-D02: the existing tick/RecordId cursor is the selected design.
// Arrival-order ties and deleting the retained last-processed record are rejected.
// These pure proposals do not qualify a durable dispatcher or exactly-once effects.
include!("due_events.rs");

fn cursor_contract_id(last_byte: u8) -> RecordId {
    let mut bytes = [0x10; 16];
    bytes[15] = last_byte;
    RecordId::from_bytes(&bytes).unwrap()
}

fn cursor_contract_event(ticks: u64, last_byte: u8) -> ScheduledEvent {
    ScheduledEvent {
        id: cursor_contract_id(last_byte),
        ..event(ticks, last_byte, "event")
    }
}

#[test]
fn canonical_record_bytes_order_equal_ticks_independently_of_arrival() {
    let policy = content();
    for arrival in [
        [1, 2, 3],
        [1, 3, 2],
        [2, 1, 3],
        [2, 3, 1],
        [3, 1, 2],
        [3, 2, 1],
    ] {
        let mut supplied = state();
        supplied.schedules = arrival
            .into_iter()
            .map(|last_byte| cursor_contract_event(10, last_byte))
            .collect();
        supplied.schedules.push(cursor_contract_event(9, 9));
        let checkpoint = checkpoint(supplied);
        let before = checkpoint.state().clone();
        let selected = select_due_events(
            &checkpoint,
            &pins(),
            request(10, &policy),
            DueSelectionLimits {
                selected_events: 4,
                ..limits()
            },
        )
        .unwrap();

        assert_eq!(
            selected_ids(&selected),
            vec![
                cursor_contract_id(9),
                cursor_contract_id(1),
                cursor_contract_id(2),
                cursor_contract_id(3),
            ]
        );
        assert_eq!(selected.cursor.last_processed, Some(cursor_contract_id(3)));
        assert_eq!(selected.remaining_due(), 0);
        assert_eq!(selected.basis, checkpoint.basis());
        assert_eq!(selected.pins, checkpoint.pins());
        assert_eq!(checkpoint.state(), &before);
    }
}

#[test]
fn resumed_cursor_orders_only_remaining_due_records_and_replays() {
    let policy = content();
    let mut supplied = state();
    supplied.schedules = vec![
        cursor_contract_event(10, 3),
        cursor_contract_event(12, 4),
        cursor_contract_event(9, 9),
        cursor_contract_event(10, 2),
        cursor_contract_event(10, 1),
    ];
    let first_checkpoint = checkpoint(supplied);
    let first_before = first_checkpoint.state().clone();
    let bounded = DueSelectionLimits {
        queue_events: 5,
        ..limits()
    };
    let first =
        select_due_events(&first_checkpoint, &pins(), request(10, &policy), bounded).unwrap();
    assert_eq!(
        selected_ids(&first),
        vec![cursor_contract_id(9), cursor_contract_id(1)]
    );
    assert_eq!(
        first.cursor.pending_events,
        vec![cursor_contract_id(2), cursor_contract_id(3)]
    );
    assert_eq!(first.cursor.last_processed, Some(cursor_contract_id(1)));
    assert_eq!(first.cursor.policy, policy);
    assert_eq!(first.cursor.time, time(10));
    assert_eq!(first_checkpoint.state(), &first_before);

    // A test-local replacement exercises resume admission; no repository commit occurs.
    let mut resumed_state = first_checkpoint.state().clone();
    resumed_state.logical_time = first.proposed_time;
    resumed_state.continuity.catch_up = Some(first.cursor.clone());
    let resumed_checkpoint = checkpoint(resumed_state);
    let resumed_before = resumed_checkpoint.state().clone();
    let next =
        select_due_events(&resumed_checkpoint, &pins(), request(10, &policy), bounded).unwrap();
    let replay =
        select_due_events(&resumed_checkpoint, &pins(), request(10, &policy), bounded).unwrap();
    assert_eq!(
        selected_ids(&next),
        vec![cursor_contract_id(2), cursor_contract_id(3)]
    );
    assert_eq!(next, replay);
    assert_eq!(next.cursor.last_processed, Some(cursor_contract_id(3)));
    assert!(next.cursor.pending_events.is_empty());
    assert_eq!(resumed_checkpoint.state(), &resumed_before);

    let mut drained_state = resumed_checkpoint.state().clone();
    drained_state.continuity.catch_up = Some(next.cursor.clone());
    let drained_checkpoint = checkpoint(drained_state);
    let drained_before = drained_checkpoint.state().clone();
    let drained =
        select_due_events(&drained_checkpoint, &pins(), request(10, &policy), bounded).unwrap();
    assert!(drained.events.is_empty());
    assert_eq!(drained.cursor, next.cursor);
    let later =
        select_due_events(&drained_checkpoint, &pins(), request(12, &policy), bounded).unwrap();
    assert_eq!(selected_ids(&later), vec![cursor_contract_id(4)]);
    assert_eq!(later.remaining_due(), 0);
    assert_eq!(drained_checkpoint.state(), &drained_before);
}

#[test]
fn paused_current_time_keeps_exact_cursor_and_backlog() {
    let mut supplied = scheduled();
    supplied.logical_time = time(10);
    let cursor = CatchUpCursor {
        last_processed: Some(id(1)),
        pending_events: vec![id(2), id(3)],
        policy: content(),
        time: time(10),
    };
    supplied.continuity.catch_up = Some(cursor.clone());
    let checkpoint = checkpoint(supplied);
    let before = checkpoint.state().clone();
    let policy = content();

    for budget in [Duration::from_nanos(1), Duration::from_secs(60)] {
        let mut paused = request(10, &policy);
        paused.paused = true;
        paused.deadline_remaining = budget;
        let selected = select_due_events(&checkpoint, &pins(), paused, limits()).unwrap();
        assert!(selected.events.is_empty());
        assert_eq!(selected.cursor, cursor);
        assert_eq!(selected.proposed_time, before.logical_time);
        assert_eq!(selected.remaining_due(), 2);
        assert_eq!(checkpoint.state(), &before);
    }
}

#[test]
fn cursor_policy_order_time_and_bounds_refuse_without_mutation() {
    let policy = content();
    let valid_cursor = CatchUpCursor {
        last_processed: Some(id(1)),
        pending_events: vec![id(2), id(3)],
        policy: content(),
        time: time(10),
    };
    let mut wrong_order = valid_cursor.clone();
    wrong_order.pending_events.swap(0, 1);
    let mut missing_due = valid_cursor.clone();
    assert_eq!(missing_due.pending_events.pop(), Some(id(3)));
    let mut wrong_policy = valid_cursor.clone();
    wrong_policy.policy.entry = label("different-policy");
    let mut wrong_time = valid_cursor.clone();
    wrong_time.time = time(9);

    for stale in [wrong_order, missing_due, wrong_policy, wrong_time] {
        let mut supplied = scheduled();
        supplied.logical_time = time(10);
        supplied.continuity.catch_up = Some(stale);
        let checkpoint = checkpoint(supplied);
        let before = checkpoint.state().clone();
        assert_eq!(
            select_due_events(&checkpoint, &pins(), request(10, &policy), limits()),
            Err(DueSelectionError::StaleCursor)
        );
        assert_eq!(checkpoint.state(), &before);
    }

    let current_checkpoint = checkpoint(scheduled());
    let before = current_checkpoint.state().clone();
    for (bounded, rejection) in [
        (
            DueSelectionLimits {
                queue_events: 3,
                ..limits()
            },
            DueSelectionError::QueueCapacity,
        ),
        (
            DueSelectionLimits {
                output_bytes: 1,
                ..limits()
            },
            DueSelectionError::OutputCapacity,
        ),
        (
            DueSelectionLimits {
                selected_events: 0,
                ..limits()
            },
            DueSelectionError::InvalidLimits,
        ),
    ] {
        assert_eq!(
            select_due_events(&current_checkpoint, &pins(), request(10, &policy), bounded),
            Err(rejection)
        );
        assert_eq!(current_checkpoint.state(), &before);
    }
    let mut expired = request(10, &policy);
    expired.deadline_remaining = Duration::ZERO;
    assert_eq!(
        select_due_events(&current_checkpoint, &pins(), expired, limits()),
        Err(DueSelectionError::Deadline)
    );
    assert_eq!(current_checkpoint.state(), &before);
}
