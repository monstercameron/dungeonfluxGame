#[path = "../src/gameplay_browser/revisions.rs"]
mod revisions;

use df_protocol::common::{RecoveryEpoch, SessionRevision};
use revisions::{IncompleteRevision, accept_revision};

fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision {
        epoch: Some(RecoveryEpoch { value: Some(epoch) }),
        sequence: Some(sequence),
    }
}

#[test]
fn reconnect_retains_recovery_watermark_when_old_epoch_arrives_late() {
    let before_recovery = revision(1, 99);
    let recovered = revision(2, 1);
    let mut current = Some(before_recovery);
    if accept_revision(current, recovered).unwrap() {
        current = Some(recovered);
    }
    assert_eq!(current, Some(recovered));

    // A high sequence in a retired epoch must not become the displayed view or
    // the observed revision used by the next player input after reconnect.
    if accept_revision(current, before_recovery).unwrap() {
        current = Some(before_recovery);
    }
    assert_eq!(current, Some(recovered));
}

#[test]
fn current_epoch_rejects_old_sequence_but_allows_receipt_redraw_and_new_views() {
    let current = Some(revision(3, 7));
    assert_eq!(accept_revision(current, revision(3, 6)), Ok(false));
    assert_eq!(accept_revision(current, revision(3, 7)), Ok(true));
    assert_eq!(accept_revision(current, revision(3, 8)), Ok(true));
    assert_eq!(accept_revision(current, revision(4, 0)), Ok(true));
    assert_eq!(accept_revision(None, revision(3, 7)), Ok(true));
}

#[test]
fn incomplete_wire_revisions_never_replace_the_recovery_watermark() {
    let current = Some(revision(2, 1));
    for incomplete in [
        SessionRevision {
            epoch: None,
            sequence: Some(2),
        },
        SessionRevision {
            epoch: Some(RecoveryEpoch { value: Some(2) }),
            sequence: None,
        },
        SessionRevision {
            epoch: Some(RecoveryEpoch { value: None }),
            sequence: Some(2),
        },
        SessionRevision::default(),
    ] {
        assert_eq!(
            accept_revision(current, incomplete),
            Err(IncompleteRevision)
        );
        assert_eq!(accept_revision(None, incomplete), Err(IncompleteRevision));
        assert_eq!(
            accept_revision(Some(incomplete), revision(3, 1)),
            Err(IncompleteRevision)
        );
    }
}
