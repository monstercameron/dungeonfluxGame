use df_types::{RecoveryEpoch, RevisionError, SessionRevision};

#[test]
fn sequence_advances_monotonically_within_each_epoch_boundary() {
    for epoch_value in [1, 8, u64::MAX] {
        let epoch = RecoveryEpoch::new(epoch_value).unwrap();

        for sequence in [0, 1, u64::MAX - 1] {
            let current = SessionRevision::new(epoch, sequence);
            let next = current.next_sequence().unwrap();

            assert_eq!(next.epoch(), epoch);
            assert_eq!(next.sequence(), sequence + 1);
            assert!(next > current);
        }

        let exhausted = SessionRevision::new(epoch, u64::MAX);
        assert_eq!(
            exhausted.next_sequence(),
            Err(RevisionError::SequenceOverflow)
        );
        assert_eq!(
            exhausted.next_sequence(),
            Err(RevisionError::SequenceOverflow)
        );
        assert_eq!(exhausted.epoch(), epoch);
        assert_eq!(exhausted.sequence(), u64::MAX);
    }
}

#[test]
fn epoch_order_prevents_an_earlier_revision_after_sequence_exhaustion() {
    let earlier_epoch = RecoveryEpoch::new(7).unwrap();
    let later_epoch = RecoveryEpoch::new(8).unwrap();
    let earlier_maximum = SessionRevision::new(earlier_epoch, u64::MAX);
    let later_zero = SessionRevision::new(later_epoch, 0);
    let later_one = later_zero.next_sequence().unwrap();

    assert!(later_zero > earlier_maximum);
    assert!(later_one > later_zero);
    assert_eq!(later_one.epoch(), later_epoch);
    assert_eq!(later_one.sequence(), 1);
}

#[test]
fn zero_epoch_is_rejected_and_maximum_epoch_is_a_valid_supplied_value() {
    assert_eq!(RecoveryEpoch::new(0), Err(RevisionError::ZeroEpoch));

    let maximum_epoch = RecoveryEpoch::new(u64::MAX).unwrap();
    let initial = SessionRevision::new(maximum_epoch, 0);
    let advanced = initial.next_sequence().unwrap();

    assert_eq!(initial.epoch().get(), u64::MAX);
    assert_eq!(advanced.epoch(), maximum_epoch);
    assert_eq!(advanced.sequence(), 1);
}
