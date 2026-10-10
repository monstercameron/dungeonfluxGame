use df_audio::{
    CaptureAcquisition, CaptureChunk, CaptureError, CaptureLimits, CaptureSession, CaptureState,
};
use df_model::checkpoint::{CaptureLease, RecordId};
use df_types::{ClientBindingId, MemberId, RevisionLabel};

fn id(byte: u8) -> RecordId {
    RecordId::from_bytes(&[byte; 16]).expect("nonzero test identity")
}

fn lease() -> CaptureLease {
    CaptureLease {
        id: id(1),
        member: MemberId::from_bytes(&[2; 16]).expect("nonzero test member"),
        binding: ClientBindingId::from_bytes(&[3; 16]).expect("nonzero test binding"),
        generation: 7,
        permitted_offer: None,
        format: RevisionLabel::new(Some("pcm_s16le_16k_mono")).expect("valid test format"),
    }
}

fn session() -> CaptureSession {
    let lease = lease();
    CaptureSession::new(
        lease.clone(),
        lease.binding,
        lease.generation,
        lease.member,
        None,
        CaptureLimits {
            maximum_chunks: 2,
            maximum_chunk_bytes: 4,
            maximum_total_bytes: 8,
        },
    )
    .expect("valid capture lease")
}

#[test]
fn capture_start_requires_the_exact_current_lease_and_gesture() {
    let mut capture = session();
    let lease = lease();
    let mut stale_lease = lease.clone();
    stale_lease.generation += 1;
    assert_eq!(
        capture.start(
            &stale_lease,
            lease.binding,
            stale_lease.generation,
            true,
            CaptureAcquisition::Opened
        ),
        Err(CaptureError::StaleLease)
    );
    assert_eq!(
        capture.start(&lease, lease.binding, 8, true, CaptureAcquisition::Opened),
        Err(CaptureError::StaleLease)
    );
    assert_eq!(
        capture.start(
            &lease,
            lease.binding,
            lease.generation,
            false,
            CaptureAcquisition::Opened
        ),
        Err(CaptureError::UserGestureRequired)
    );
    assert_eq!(
        capture.start(
            &lease,
            lease.binding,
            lease.generation,
            true,
            CaptureAcquisition::Unsupported
        ),
        Err(CaptureError::Unsupported)
    );
    assert_eq!(
        capture.start(
            &lease,
            lease.binding,
            lease.generation,
            true,
            CaptureAcquisition::Pending
        ),
        Err(CaptureError::Pending)
    );
    assert_eq!(
        capture.start(
            &lease,
            lease.binding,
            lease.generation,
            true,
            CaptureAcquisition::PermissionDenied
        ),
        Err(CaptureError::PermissionDenied)
    );
    assert_eq!(capture.snapshot().state, CaptureState::Ready);
}

#[test]
fn capture_chunks_preserve_sequence_offset_format_and_bounded_bytes() {
    let mut capture = session();
    let lease = lease();
    capture
        .start(
            &lease,
            lease.binding,
            lease.generation,
            true,
            CaptureAcquisition::Opened,
        )
        .expect("adapter reported an opened stream");
    capture
        .chunk(
            &lease,
            lease.binding,
            lease.generation,
            CaptureChunk::new(0, 10, vec![1, 2, 3]),
        )
        .expect("first bounded chunk");
    let refusal = capture
        .chunk(
            &lease,
            lease.binding,
            lease.generation,
            CaptureChunk::new(1, 10, vec![9]),
        )
        .expect_err("capture offsets must advance");
    assert_eq!(refusal.error, CaptureError::OffsetMismatch);
    let refusal = capture
        .chunk(
            &lease,
            lease.binding,
            lease.generation,
            CaptureChunk::new(2, 11, vec![4]),
        )
        .expect_err("out of order sequence is refused");
    assert_eq!(refusal.error, CaptureError::SequenceMismatch);
    assert_eq!(refusal.chunk.bytes(), &[4]);
    capture
        .chunk(
            &lease,
            lease.binding,
            lease.generation,
            CaptureChunk::new(1, 11, vec![4, 5]),
        )
        .expect("next sequence is retained");
    let recording = capture
        .finish(&lease, lease.binding, lease.generation)
        .expect("nonempty recording finishes");
    assert_eq!(recording.format().as_str(), "pcm_s16le_16k_mono");
    assert_eq!(recording.chunks()[0].offset_frames(), 10);
    assert_eq!(recording.chunks()[1].sequence(), 1);
    assert_eq!(recording.retained_bytes(), 5);
}

#[test]
fn cancellation_disposal_and_capacity_keep_ownership_explicit() {
    let mut capture = session();
    let lease = lease();
    capture
        .start(
            &lease,
            lease.binding,
            lease.generation,
            true,
            CaptureAcquisition::Opened,
        )
        .expect("adapter reported an opened stream");
    capture
        .chunk(
            &lease,
            lease.binding,
            lease.generation,
            CaptureChunk::new(0, 0, vec![1, 2, 3, 4]),
        )
        .expect("chunk is at the per-chunk limit");
    capture
        .chunk(
            &lease,
            lease.binding,
            lease.generation,
            CaptureChunk::new(1, 1, vec![5, 6, 7, 8]),
        )
        .expect("second chunk fills the total byte bound");
    let refusal = capture
        .chunk(
            &lease,
            lease.binding,
            lease.generation,
            CaptureChunk::new(2, 2, vec![9]),
        )
        .expect_err("chunk beyond the count bound is returned to its producer");
    assert_eq!(refusal.error, CaptureError::Capacity);
    capture
        .cancel(&lease, lease.binding, lease.generation)
        .expect("cancel releases session-owned bytes");
    assert_eq!(capture.snapshot().state, CaptureState::Cancelled);
    assert_eq!(capture.snapshot().retained_bytes, 0);
    capture
        .dispose(&lease, lease.binding, lease.generation)
        .expect("dispose releases the owner");
    assert_eq!(capture.snapshot().state, CaptureState::Disposed);
    assert_eq!(
        capture.start(
            &lease,
            lease.binding,
            lease.generation,
            true,
            CaptureAcquisition::Opened
        ),
        Err(CaptureError::InvalidState)
    );
}

#[test]
fn capture_bounds_retained_vec_capacity_not_only_initialized_bytes() {
    let mut capture = session();
    let lease = lease();
    capture
        .start(
            &lease,
            lease.binding,
            lease.generation,
            true,
            CaptureAcquisition::Opened,
        )
        .expect("adapter reported an opened stream");

    let mut bounded = Vec::with_capacity(4);
    bounded.push(1);
    capture
        .chunk(
            &lease,
            lease.binding,
            lease.generation,
            CaptureChunk::new(0, 0, bounded),
        )
        .expect("allocation at the per-chunk limit is retained");
    assert_eq!(capture.snapshot().retained_bytes, 4);

    let mut oversized = Vec::with_capacity(64);
    oversized.push(2);
    let refusal = capture
        .chunk(
            &lease,
            lease.binding,
            lease.generation,
            CaptureChunk::new(1, 1, oversized),
        )
        .expect_err("a short chunk with oversized backing storage is refused");
    assert_eq!(refusal.error, CaptureError::ChunkTooLarge);
    assert_eq!(refusal.chunk.bytes(), &[2]);
    assert_eq!(capture.snapshot().retained_bytes, 4);

    capture
        .cancel(&lease, lease.binding, lease.generation)
        .expect("cancel releases the retained allocation accounting");
    assert_eq!(capture.snapshot().retained_bytes, 0);
}
