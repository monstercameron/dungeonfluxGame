mod common;

use common::*;
use df_audio::{AudioQueue, BufferCompletion, PcmBuffer, PcmFormat, QueueError, QueueState};
use df_model::checkpoint::{AssetKind, AudioDestination};
use df_types::{ClientBindingId, RunId};

#[test]
fn finite_close_preserves_supplied_offsets_and_drains_exact_tail() {
    let mut queue = queue(3, 24);
    let receipt = start(&mut queue, 11, 100);
    queue
        .enqueue(&receipt, &lease(), manifest(), 0, 100, pcm(&[0.1, 0.2]))
        .unwrap();
    queue
        .enqueue(&receipt, &lease(), manifest(), 1, 102, pcm(&[0.3, 0.4]))
        .unwrap();
    assert_eq!(
        queue.close(&receipt, &lease(), 2, 104),
        Ok(QueueState::Draining)
    );
    let rejected = queue
        .enqueue(&receipt, &lease(), manifest(), 2, 104, pcm(&[0.5]))
        .unwrap_err();
    assert_eq!(rejected.reason, QueueError::Closed);
    for (sequence, offset, expected_bytes) in [(0, 100, 16), (1, 102, 8)] {
        let ticket = queue.begin(&receipt, &lease()).unwrap().unwrap();
        let view = queue.dispatched(&ticket).unwrap();
        assert_eq!(view.sequence, sequence);
        assert_eq!(view.offset_frames, offset);
        assert_eq!(view.asset, &asset());
        assert_eq!(view.buffer.frames(), 2);
        assert_eq!(queue.snapshot().sample_bytes, expected_bytes);
        assert_eq!(
            queue.begin(&receipt, &lease()).unwrap_err(),
            QueueError::Busy
        );
        let expected = if sequence == 0 {
            QueueState::Draining
        } else {
            QueueState::Drained
        };
        assert_eq!(
            queue.complete(&ticket),
            Ok(BufferCompletion::Current(expected))
        );
        assert_eq!(queue.complete(&ticket), Err(QueueError::StaleCallback));
    }
    assert!(queue.begin(&receipt, &lease()).unwrap().is_none());
    assert_eq!(queue.snapshot().sample_bytes, 0);
}

#[test]
fn in_flight_pcm_stays_counted_and_backpressure_returns_unchanged_samples() {
    let mut queue = queue(1, 8);
    let receipt = start(&mut queue, 11, 0);
    queue
        .enqueue(&receipt, &lease(), manifest(), 0, 0, pcm(&[0.1, 0.2]))
        .unwrap();
    let ticket = queue.begin(&receipt, &lease()).unwrap().unwrap();
    let buffer = pcm(&[0.3, 0.4]);
    let address = buffer.samples().as_ptr();
    let refused = queue
        .enqueue(&receipt, &lease(), manifest(), 1, 2, buffer)
        .unwrap_err();
    assert_eq!(refused.reason, QueueError::BufferCapacity);
    assert_eq!(refused.buffer.samples().as_ptr(), address);
    assert_eq!(queue.snapshot().sample_bytes, 8);
    assert_eq!(
        queue.dispatched(&ticket).unwrap().buffer.samples(),
        &[0.1, 0.2]
    );
    queue.complete(&ticket).unwrap();
    queue
        .enqueue(&receipt, &lease(), manifest(), 1, 2, refused.buffer)
        .unwrap();
    assert_eq!(queue.snapshot().sample_bytes, 8);
}

#[test]
fn replacement_waits_for_old_actual_stop_and_rejects_late_current_callbacks() {
    let mut queue = queue(2, 16);
    let old = start(&mut queue, 11, 0);
    queue
        .enqueue(&old, &lease(), manifest(), 0, 0, pcm(&[0.1, 0.2]))
        .unwrap();
    queue
        .enqueue(&old, &lease(), manifest(), 1, 2, pcm(&[0.3, 0.4]))
        .unwrap();
    let old_buffer = queue.begin(&old, &lease()).unwrap().unwrap();
    let replacement = queue
        .replace(&lease(), basis(1, 7), identity(12), asset(), format(), 20)
        .unwrap();
    assert_eq!(replacement.retired.discarded_buffers, 1);
    assert_eq!(replacement.retired.discarded_sample_bytes, 8);
    let requested = replacement.retired.stop_required.unwrap();
    assert_eq!(requested.identity(), old_buffer.identity());
    assert_eq!(queue.snapshot().state, QueueState::WaitingForStop);
    assert_eq!(queue.snapshot().sample_bytes, 8);
    assert_eq!(
        queue.dispatched(&old_buffer).err(),
        Some(QueueError::WaitingForStop)
    );
    let refused = queue
        .enqueue(
            &replacement.receipt,
            &lease(),
            manifest(),
            0,
            20,
            pcm(&[0.5]),
        )
        .unwrap_err();
    assert_eq!(refused.reason, QueueError::WaitingForStop);
    assert_eq!(
        queue.cancel(&old, &lease()).unwrap_err(),
        QueueError::StaleReceipt
    );
    assert_eq!(
        queue.close(&old, &lease(), 2, 4),
        Err(QueueError::StaleReceipt)
    );
    assert_eq!(
        queue.confirm_stopped(&requested),
        Ok(BufferCompletion::Retired(QueueState::Accepting))
    );
    queue
        .enqueue(
            &replacement.receipt,
            &lease(),
            manifest(),
            0,
            20,
            refused.buffer,
        )
        .unwrap();
    let current = queue
        .begin(&replacement.receipt, &lease())
        .unwrap()
        .unwrap();
    assert_eq!(queue.complete(&old_buffer), Err(QueueError::StaleCallback));
    assert_eq!(queue.snapshot().sample_bytes, 4);
    assert_eq!(queue.dispatched(&current).unwrap().identity, identity(12));
}

#[test]
fn natural_old_end_only_retires_old_buffer_and_latest_pending_cue_wins() {
    let mut queue = queue(1, 8);
    let old = start(&mut queue, 11, 0);
    queue
        .enqueue(&old, &lease(), manifest(), 0, 0, pcm(&[0.1]))
        .unwrap();
    let old_buffer = queue.begin(&old, &lease()).unwrap().unwrap();
    let superseded = start(&mut queue, 12, 10);
    let current = start(&mut queue, 13, 20);
    assert_eq!(
        queue.complete(&old_buffer),
        Ok(BufferCompletion::Retired(QueueState::Accepting))
    );
    assert_eq!(
        queue
            .enqueue(&superseded, &lease(), manifest(), 0, 10, pcm(&[0.2]))
            .unwrap_err()
            .reason,
        QueueError::StaleReceipt
    );
    queue
        .enqueue(&current, &lease(), manifest(), 0, 20, pcm(&[0.3]))
        .unwrap();
    assert_eq!(queue.snapshot().retained_buffers, 1);
}

#[test]
fn repeated_cancel_and_dispose_preserve_one_stop_request_and_never_reopen_admission() {
    let mut queue = queue(2, 16);
    let receipt = start(&mut queue, 11, 0);
    queue
        .enqueue(&receipt, &lease(), manifest(), 0, 0, pcm(&[0.1]))
        .unwrap();
    queue
        .enqueue(&receipt, &lease(), manifest(), 1, 1, pcm(&[0.2]))
        .unwrap();
    let ticket = queue.begin(&receipt, &lease()).unwrap().unwrap();
    assert_eq!(
        queue.confirm_stopped(&ticket),
        Err(QueueError::StopNotRequested)
    );
    let first = queue.cancel(&receipt, &lease()).unwrap();
    assert_eq!(first.discarded_buffers, 1);
    let repeated = queue.cancel(&receipt, &lease()).unwrap();
    assert_eq!(repeated.discarded_buffers, 0);
    assert!(repeated.stop_required.is_some());
    assert_eq!(queue.snapshot().retained_buffers, 1);
    assert!(queue.dispose().stop_required.is_some());
    assert_eq!(
        queue
            .enqueue(&receipt, &lease(), manifest(), 2, 2, pcm(&[0.3]))
            .unwrap_err()
            .reason,
        QueueError::Disposed
    );
    assert_eq!(
        queue.confirm_stopped(&ticket),
        Ok(BufferCompletion::Retired(QueueState::Disposed))
    );
    assert_eq!(queue.snapshot().sample_bytes, 0);
    assert!(queue.dispose().stop_required.is_none());
}

#[test]
fn foreign_receipts_and_callback_tokens_fail_after_owner_reconstruction() {
    let mut first = queue(1, 8);
    let mut second = queue(1, 8);
    let receipt = start(&mut first, 11, 0);
    let current = start(&mut second, 11, 0);
    first
        .enqueue(&receipt, &lease(), manifest(), 0, 0, pcm(&[0.1]))
        .unwrap();
    second
        .enqueue(&current, &lease(), manifest(), 0, 0, pcm(&[0.2]))
        .unwrap();
    let foreign = first.begin(&receipt, &lease()).unwrap().unwrap();
    let own = second.begin(&current, &lease()).unwrap().unwrap();
    assert_eq!(second.complete(&foreign), Err(QueueError::ForeignOwner));
    assert_eq!(
        second.cancel(&receipt, &lease()).unwrap_err(),
        QueueError::ForeignOwner
    );
    assert_eq!(second.snapshot().sample_bytes, 4);
    assert_eq!(second.dispatched(&own).unwrap().buffer.samples(), &[0.2]);
}

#[test]
fn source_scope_lease_generation_and_asset_refusals_preserve_existing_work() {
    let mut queue = queue(2, 16);
    let receipt = start(&mut queue, 11, 0);
    queue
        .enqueue(&receipt, &lease(), manifest(), 0, 0, pcm(&[0.1]))
        .unwrap();
    let before = queue.snapshot();
    let mut wrong_lease = lease();
    wrong_lease.generation += 1;
    assert_eq!(
        queue.begin(&receipt, &wrong_lease).unwrap_err(),
        QueueError::WrongLease
    );
    assert_eq!(
        queue
            .replace(&lease(), basis(1, 7), identity(11), asset(), format(), 0)
            .unwrap_err(),
        QueueError::DuplicateCue
    );
    assert_eq!(
        queue
            .replace(&lease(), basis(1, 7), identity(10), asset(), format(), 0)
            .unwrap_err(),
        QueueError::StaleGeneration
    );
    let mut wrong_basis = basis(1, 7);
    wrong_basis.run = RunId::from_bytes(&[8; 16]).unwrap();
    assert_eq!(
        queue
            .replace(&lease(), wrong_basis, identity(12), asset(), format(), 0)
            .unwrap_err(),
        QueueError::WrongBasis
    );
    let mut image = asset();
    image.kind = AssetKind::Image;
    assert_eq!(
        queue
            .replace(&lease(), basis(1, 7), identity(12), image, format(), 0)
            .unwrap_err(),
        QueueError::WrongAsset
    );
    let mut wrong_manifest = manifest();
    wrong_manifest.sha256[0] ^= 1;
    assert_eq!(
        queue
            .enqueue(&receipt, &lease(), wrong_manifest, 1, 1, pcm(&[0.2]))
            .unwrap_err()
            .reason,
        QueueError::WrongAsset
    );
    assert_eq!(queue.snapshot(), before);
}

#[test]
fn limits_sequence_offset_format_and_incomplete_end_never_synthesize_success() {
    let mut queue = queue(2, 8);
    let receipt = start(&mut queue, 11, 0);
    assert_eq!(
        queue
            .enqueue(&receipt, &lease(), manifest(), 1, 0, pcm(&[0.1]))
            .unwrap_err()
            .reason,
        QueueError::WrongSequence
    );
    assert_eq!(
        queue
            .enqueue(&receipt, &lease(), manifest(), 0, 1, pcm(&[0.1]))
            .unwrap_err()
            .reason,
        QueueError::WrongOffset
    );
    let stereo = PcmBuffer::new(PcmFormat::new(2, 48000).unwrap(), Box::from([0.1, 0.2])).unwrap();
    assert_eq!(
        queue
            .enqueue(&receipt, &lease(), manifest(), 0, 0, stereo)
            .unwrap_err()
            .reason,
        QueueError::InvalidFormat
    );
    queue
        .enqueue(&receipt, &lease(), manifest(), 0, 0, pcm(&[0.1, 0.2]))
        .unwrap();
    assert_eq!(
        queue
            .enqueue(&receipt, &lease(), manifest(), 1, 2, pcm(&[0.3]))
            .unwrap_err()
            .reason,
        QueueError::SampleCapacity
    );
    assert_eq!(
        queue.close(&receipt, &lease(), 2, 3),
        Err(QueueError::Incomplete)
    );
    assert_eq!(queue.snapshot().state, QueueState::Accepting);
    assert_eq!(queue.snapshot().sample_bytes, 8);
    let mut constrained = limits(1, 8);
    constrained.cue_frames = 1;
    let mut bounded = AudioQueue::new(binding(), lease(), basis(1, 7), constrained).unwrap();
    let scope = start(&mut bounded, 11, 0);
    assert_eq!(
        bounded
            .enqueue(&scope, &lease(), manifest(), 0, 0, pcm(&[0.1, 0.2]))
            .unwrap_err()
            .reason,
        QueueError::CueCapacity
    );
    let mut overflow = common::queue(1, 8);
    let scope = start(&mut overflow, 11, u64::MAX);
    assert_eq!(
        overflow
            .enqueue(&scope, &lease(), manifest(), 0, u64::MAX, pcm(&[0.1]))
            .unwrap_err()
            .reason,
        QueueError::FrameOverflow
    );
}

#[test]
fn new_epoch_refuses_late_old_basis_without_replaying_completed_scope() {
    let mut queue = queue(1, 8);
    let old = start(&mut queue, 11, 0);
    queue.close(&old, &lease(), 0, 0).unwrap();
    let mut identity = identity(12);
    identity.basis = basis(2, 0);
    queue
        .replace(&lease(), basis(2, 0), identity, asset(), format(), 50)
        .unwrap();
    assert_eq!(
        queue
            .replace(
                &lease(),
                basis(1, 99),
                common::identity(13),
                asset(),
                format(),
                0
            )
            .unwrap_err(),
        QueueError::WrongBasis
    );
    assert_eq!(
        queue.begin(&old, &lease()).unwrap_err(),
        QueueError::StaleReceipt
    );
}

#[test]
fn invalid_pcm_is_returned_and_unlocked_routing_is_not_inferred() {
    assert_eq!(PcmFormat::new(0, 48000), Err(QueueError::InvalidFormat));
    assert_eq!(PcmFormat::new(1, 0), Err(QueueError::InvalidFormat));
    assert_eq!(
        PcmBuffer::new(format(), Box::from([f32::NAN]))
            .unwrap_err()
            .reason,
        QueueError::InvalidSamples
    );
    assert_eq!(
        PcmBuffer::new(PcmFormat::new(2, 48000).unwrap(), Box::from([0.1]))
            .unwrap_err()
            .reason,
        QueueError::InvalidSamples
    );
    let mut locked = lease();
    locked.device_unlocked = false;
    let mut queue = AudioQueue::new(binding(), locked.clone(), basis(1, 7), limits(1, 8)).unwrap();
    assert_eq!(
        queue
            .replace(&locked, basis(1, 7), identity(11), asset(), format(), 0)
            .unwrap_err(),
        QueueError::Locked
    );
    let mut foreign = lease();
    foreign.destination =
        AudioDestination::PublicRoom(ClientBindingId::from_bytes(&[9; 16]).unwrap());
    assert!(matches!(
        AudioQueue::new(binding(), foreign, basis(1, 7), limits(1, 8)),
        Err(QueueError::InvalidLease)
    ));
}

#[test]
fn cue_chunk_budget_remains_exhausted_after_resources_are_released() {
    let mut bounds = limits(1, 8);
    bounds.cue_chunks = 1;
    let mut audio = AudioQueue::new(binding(), lease(), basis(1, 7), bounds).unwrap();
    let receipt = start(&mut audio, 11, 0);
    audio
        .enqueue(&receipt, &lease(), manifest(), 0, 0, pcm(&[0.1]))
        .unwrap();
    let first = audio.begin(&receipt, &lease()).unwrap().unwrap();
    audio.complete(&first).unwrap();
    assert_eq!(audio.snapshot().sample_bytes, 0);
    let refused = audio
        .enqueue(&receipt, &lease(), manifest(), 1, 1, pcm(&[0.2]))
        .unwrap_err();
    assert_eq!(refused.reason, QueueError::CueCapacity);
    assert_eq!(refused.buffer.samples(), &[0.2]);
    assert_eq!(
        audio.close(&receipt, &lease(), 1, 1),
        Ok(QueueState::Drained)
    );
}

#[test]
fn queued_only_cancel_and_idle_dispose_release_all_resources_without_a_fake_stop() {
    let mut audio = queue(2, 16);
    let receipt = start(&mut audio, 11, 0);
    audio
        .enqueue(&receipt, &lease(), manifest(), 0, 0, pcm(&[0.1]))
        .unwrap();
    audio
        .enqueue(&receipt, &lease(), manifest(), 1, 1, pcm(&[0.2]))
        .unwrap();
    let cancelled = audio.cancel(&receipt, &lease()).unwrap();
    assert_eq!(cancelled.discarded_buffers, 2);
    assert_eq!(cancelled.discarded_sample_bytes, 8);
    assert!(cancelled.stop_required.is_none());
    assert_eq!(audio.snapshot().state, QueueState::Cancelled);
    assert_eq!(audio.snapshot().sample_bytes, 0);
    assert_eq!(
        audio.begin(&receipt, &lease()).unwrap_err(),
        QueueError::Closed
    );
    let mut idle = queue(1, 8);
    assert_eq!(idle.snapshot().state, QueueState::Idle);
    assert!(idle.dispose().stop_required.is_none());
    assert_eq!(idle.snapshot().state, QueueState::Disposed);
    assert_eq!(
        idle.replace(&lease(), basis(1, 7), identity(11), asset(), format(), 0)
            .unwrap_err(),
        QueueError::Disposed
    );
}

#[test]
fn invalid_resource_limits_and_oversized_retained_audience_are_refused() {
    for bounds in [
        limits(0, 8),
        limits(257, 8),
        limits(1, 0),
        limits(1, 16 * 1024 * 1024 + 1),
    ] {
        assert!(matches!(
            AudioQueue::new(binding(), lease(), basis(1, 7), bounds),
            Err(QueueError::InvalidLimits)
        ));
    }
    let mut bounds = limits(1, 8);
    bounds.cue_chunks = 4097;
    assert!(matches!(
        AudioQueue::new(binding(), lease(), basis(1, 7), bounds),
        Err(QueueError::InvalidLimits)
    ));
    let mut oversized = lease();
    oversized.audience = df_model::checkpoint::AudienceScope::Members(Vec::with_capacity(257));
    assert!(matches!(
        AudioQueue::new(binding(), oversized, basis(1, 7), limits(1, 8)),
        Err(QueueError::InvalidLease)
    ));
}
