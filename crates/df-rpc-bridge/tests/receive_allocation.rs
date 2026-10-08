//! Observed canonical Rust receive allocation and credit facts; no browser platform proof.
use df_rpc_bridge::{
    CONCURRENT_STREAMS, FRAME_BYTES, MESSAGE_BYTES, RECEIVE_BYTES, RPC_MESSAGE_BYTES,
};
use std::{io, sync::Arc};
#[expect(
    dead_code,
    reason = "Unchanged private owner is imported; sibling transport adapters are outside this focused test."
)]
#[path = "../src/resources.rs"]
mod resources;
use resources::ConnectionMetrics;

fn callback(capacity: usize, length: usize, value: u8) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(capacity);
    bytes.resize(length, value);
    bytes
}

fn frame(kind: u8, stream: u32, payload: &[u8]) -> Vec<u8> {
    let length = payload.len();
    assert!(length <= FRAME_BYTES);
    let mut wire = vec![
        (length >> 16) as u8,
        (length >> 8) as u8,
        length as u8,
        kind,
        0,
    ];
    wire.extend(stream.to_be_bytes());
    wire.extend(payload);
    wire
}

#[test]
fn receive_allocation_retains_original_vec_until_last_alias() {
    let metrics = Arc::new(ConnectionMetrics::new(true));
    let original = callback(MESSAGE_BYTES, FRAME_BYTES, 7);
    let capacity = original.capacity();
    let mut queued = metrics.browser_callback_from_vec(original).unwrap();
    metrics.queue(queued.len(), true, false).unwrap();
    let prefix = queued.split_to(3);
    metrics.queue(prefix.len(), false, false).unwrap();
    let alias = queued.clone();
    let held = metrics.snapshot().unwrap();
    assert_eq!(held.callback_bytes.current, FRAME_BYTES - 3);
    assert_eq!(held.browser_callback_vec_capacity.current, capacity);
    assert!(held.browser_callback_vec_capacity.current > held.callback_bytes.current);
    assert_eq!(&prefix[..], &[7; 3]);
    drop(prefix);
    metrics.queue(queued.len(), false, true).unwrap();
    drop(queued);
    metrics.close();
    let closed = metrics.snapshot().unwrap();
    assert_eq!(closed.callback_bytes.current, 0);
    assert_eq!(closed.callback_items.current, 0);
    assert!(closed.closed);
    assert_eq!(closed.browser_callback_vec_capacity.current, capacity);
    assert_eq!(alias.len(), FRAME_BYTES - 3);
    assert!(alias.iter().all(|value| *value == 7));
    drop(alias);
    let settled = metrics.snapshot().unwrap();
    assert_eq!(settled.browser_callback_vec_capacity.current, 0);
    assert_eq!(settled.browser_callback_vec_capacity.peak, capacity);
    assert_eq!(settled.browser_callback_vec_capacity.total, capacity);
    assert!(!settled.rejected);
    println!(
        "receive_allocation native Rust Vec capacity={capacity}; split_length={}; queue_empty_capacity={}; final_alias_drop_capacity={}",
        FRAME_BYTES - 3,
        closed.browser_callback_vec_capacity.current,
        settled.browser_callback_vec_capacity.current
    );
}

#[test]
fn receive_allocation_failure_releases_only_dropped_backing() {
    let metrics = Arc::new(ConnectionMetrics::new(true));
    let first = callback(FRAME_BYTES, 11, 1);
    let second = callback(MESSAGE_BYTES, 19, 2);
    let first_capacity = first.capacity();
    let second_capacity = second.capacity();
    let first = metrics.browser_callback_from_vec(first).unwrap();
    let second = metrics.browser_callback_from_vec(second).unwrap();
    let retained_second = second.clone();
    metrics.queue(first.len(), true, false).unwrap();
    metrics.queue(second.len(), true, false).unwrap();
    assert_eq!(
        metrics
            .snapshot()
            .unwrap()
            .browser_callback_vec_capacity
            .current,
        first_capacity + second_capacity
    );
    metrics.reject();
    metrics.queue(first.len(), false, true).unwrap();
    drop(first);
    assert_eq!(
        metrics
            .snapshot()
            .unwrap()
            .browser_callback_vec_capacity
            .current,
        second_capacity
    );
    metrics.queue(second.len(), false, true).unwrap();
    drop(second);
    metrics.close();
    let failed = metrics.snapshot().unwrap();
    assert!(failed.rejected && failed.closed);
    assert_eq!(failed.callback_bytes.current, 0);
    assert_eq!(
        failed.browser_callback_vec_capacity.current,
        second_capacity
    );
    assert_eq!(&retained_second[..], &[2; 19]);
    drop(retained_second);
    let settled = metrics.snapshot().unwrap();
    assert_eq!(settled.browser_callback_vec_capacity.current, 0);
    assert_eq!(
        settled.browser_callback_vec_capacity.peak,
        first_capacity + second_capacity
    );
    assert_eq!(
        settled.browser_callback_vec_capacity.total,
        first_capacity + second_capacity
    );
    println!(
        "receive_allocation two native Rust Vec capacities={first_capacity}+{second_capacity}; failure_retained={}; settled={}",
        failed.browser_callback_vec_capacity.current, settled.browser_callback_vec_capacity.current
    );
}

#[test]
fn receive_allocation_credit_changes_only_with_data_and_window_update() {
    for browser in [false, true] {
        let metrics = ConnectionMetrics::new(browser);
        metrics
            .observe(!browser, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
            .unwrap();
        let original = metrics.snapshot().unwrap().receive_credit.available;
        let data = frame(0, 1, &[7; 13]);
        for byte in data {
            metrics.observe(true, &[byte]).unwrap();
        }
        let received = metrics.snapshot().unwrap();
        assert_eq!(received.receive_credit.available, original - 13);
        assert_eq!(received.receive_credit.data_bytes, 13);
        assert_eq!(received.receive_credit.update_bytes, 0);
        assert_eq!(received.receive_credit.peak_held, 13);
        // Observing bytes and reading a snapshot never grants received DATA credit.
        assert_eq!(
            metrics.snapshot().unwrap().receive_credit.available,
            original - 13
        );
        let update = frame(8, 0, &13u32.to_be_bytes());
        for part in update.chunks(2) {
            metrics.observe(false, part).unwrap();
        }
        let granted = metrics.snapshot().unwrap();
        assert_eq!(granted.receive_credit.available, original);
        assert_eq!(granted.receive_credit.update_bytes, 13);
        assert_eq!(granted.receive_credit.data_bytes, 13);
        assert_eq!(granted.received_frames, 1);
        assert_eq!(granted.sent_frames, 1);
        assert!(!granted.rejected);
        println!(
            "receive_allocation passive wire browser_orientation={browser}; DATA=13; available_before={original}; after_DATA={}; after_opposite_WINDOW_UPDATE={}",
            received.receive_credit.available, granted.receive_credit.available
        );
    }
}

#[test]
fn receive_allocation_peer_exceeding_credit_is_visibly_refused() {
    let metrics = ConnectionMetrics::new(false);
    metrics
        .observe(true, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
        .unwrap();
    let payload = vec![9; FRAME_BYTES];
    let data = frame(0, 1, &payload);
    for _ in 0..3 {
        metrics.observe(true, &data).unwrap();
    }
    let before = metrics.snapshot().unwrap();
    assert_eq!(before.receive_credit.available, 65_535 - 3 * FRAME_BYTES);
    let error = metrics.observe(true, &data).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    let refused = metrics.snapshot().unwrap();
    assert!(refused.rejected);
    assert_eq!(
        refused.receive_credit.available,
        before.receive_credit.available
    );
    assert_eq!(refused.receive_credit.data_bytes, 3 * FRAME_BYTES);
    println!(
        "receive_allocation passive native refusal={:?}; retained_credit={}; accepted_DATA={}",
        error.kind(),
        refused.receive_credit.available,
        refused.receive_credit.data_bytes
    );
}

#[test]
fn receive_allocation_reservations_are_distinct_from_observed_vec_capacity() {
    let native = ConnectionMetrics::new(false).snapshot().unwrap();
    let metrics = Arc::new(ConnectionMetrics::new(true));
    let browser = metrics.snapshot().unwrap();
    assert!(native.envelope_within_limit() && browser.envelope_within_limit());
    assert_eq!(
        native.receive_reservation_bytes - browser.receive_reservation_bytes,
        RECEIVE_BYTES
    );
    assert_eq!(native.browser_callback_vec_capacity.current, 0);
    assert_eq!(browser.browser_callback_vec_capacity.current, 0);
    let original = callback(FRAME_BYTES, 5, 3);
    let capacity = original.capacity();
    let owned = metrics.browser_callback_from_vec(original).unwrap();
    let observed = metrics.snapshot().unwrap();
    assert_eq!(observed.browser_callback_vec_capacity.current, capacity);
    assert_eq!(
        observed.receive_reservation_bytes,
        browser.receive_reservation_bytes
    );
    assert_ne!(observed.receive_reservation_bytes, capacity);
    drop(owned);
    assert_eq!(
        metrics
            .snapshot()
            .unwrap()
            .browser_callback_vec_capacity
            .current,
        0
    );
    println!(
        "receive_allocation conservative native_reservation={}; browser_reservation={}; actual_Rust_Vec_capacity={capacity}; browser_engine_pre_callback=UNPERFORMED",
        native.receive_reservation_bytes, browser.receive_reservation_bytes
    );
}
