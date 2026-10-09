use df_assets::AssetManifest;
use df_audio::{
    BufferCompletion, PcmBuffer, PcmFormat, PcmSource, PlaybackBasis, PlaybackCue,
    PlaybackDestination, PlaybackOutput, PresentationQueue, QueueError, QueueLimits, QueueState,
};
use df_types::{
    ClientBindingId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};

fn basis(epoch: u64, sequence: u64) -> PlaybackBasis {
    PlaybackBasis::new(
        SessionId::from_bytes(&[1; 16]).unwrap(),
        RunId::from_bytes(&[2; 16]).unwrap(),
        SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence),
    )
}
fn output() -> PlaybackOutput {
    PlaybackOutput::new(
        [4; 16],
        ClientBindingId::from_bytes(&[3; 16]).unwrap(),
        9,
        PlaybackDestination::PublicRoom,
        true,
    )
    .unwrap()
}
fn cue(job: u8, generation: u64, current: PlaybackBasis) -> PlaybackCue {
    PlaybackCue::new(
        current,
        [job; 16],
        OperationId::from_bytes(&[6; 16]).unwrap(),
        generation,
    )
    .unwrap()
}
fn manifest() -> AssetManifest {
    AssetManifest {
        byte_len: 3,
        sha256: [7; 32],
    }
}
fn source() -> PcmSource {
    PcmSource::new(
        RevisionLabel::new(Some("synthetic-source")).unwrap(),
        manifest(),
    )
    .unwrap()
}
fn format() -> PcmFormat {
    PcmFormat::new(1, 48_000).unwrap()
}
fn pcm(value: f32) -> PcmBuffer {
    PcmBuffer::new(format(), Box::from([value])).unwrap()
}
fn queue() -> PresentationQueue {
    PresentationQueue::new(
        output().binding(),
        output(),
        basis(1, 7),
        QueueLimits {
            retained_buffers: 2,
            sample_bytes: 16,
            cue_chunks: 4,
            cue_frames: 8,
        },
    )
    .unwrap()
}

#[test]
fn checked_projection_refuses_zero_labels_and_empty_source() {
    let operation = OperationId::from_bytes(&[6; 16]).unwrap();
    assert_eq!(
        PlaybackCue::new(basis(1, 7), [0; 16], operation, 1),
        Err(QueueError::InvalidGeneration)
    );
    assert_eq!(
        PlaybackCue::new(basis(1, 7), [1; 16], operation, 0),
        Err(QueueError::InvalidGeneration)
    );
    assert_eq!(
        PlaybackOutput::new(
            [0; 16],
            output().binding(),
            1,
            PlaybackDestination::PublicRoom,
            true
        ),
        Err(QueueError::InvalidLease)
    );
    assert_eq!(
        PlaybackOutput::new(
            [1; 16],
            output().binding(),
            0,
            PlaybackDestination::PrivateListener,
            true
        ),
        Err(QueueError::InvalidLease)
    );
    assert_eq!(
        PcmSource::new(
            RevisionLabel::new(Some("empty")).unwrap(),
            AssetManifest {
                byte_len: 0,
                sha256: [7; 32]
            }
        ),
        Err(QueueError::WrongAsset)
    );
}

#[test]
fn projected_stop_requires_original_receipt_and_real_dispatch_retirement() {
    let mut owner = queue();
    let identity = cue(5, 11, basis(1, 7));
    let original = owner
        .replace(&output(), basis(1, 7), identity, source(), format(), 0)
        .unwrap()
        .receipt;
    owner
        .replace(
            &output(),
            basis(1, 7),
            cue(8, 1, basis(1, 7)),
            source(),
            format(),
            10,
        )
        .unwrap();
    let current = owner
        .replace(&output(), basis(1, 7), identity, source(), format(), 20)
        .unwrap()
        .receipt;
    owner
        .enqueue(&current, &output(), manifest(), 0, 20, pcm(0.1))
        .unwrap();
    owner
        .enqueue(&current, &output(), manifest(), 1, 21, pcm(0.2))
        .unwrap();
    let dispatch = owner.begin(&current, &output()).unwrap().unwrap();
    let before = owner.snapshot();
    assert_eq!(
        owner
            .cancel_media(&original, &output(), &identity)
            .unwrap_err(),
        QueueError::StaleReceipt
    );
    assert_eq!(owner.snapshot(), before);
    assert_eq!(
        owner.dispatched(&dispatch).unwrap().buffer.samples(),
        &[0.1]
    );
    let cancelled = owner.cancel_media(&current, &output(), &identity).unwrap();
    assert_eq!(cancelled.discarded_buffers, 1);
    assert_eq!(owner.snapshot().state, QueueState::WaitingForStop);
    assert_eq!(owner.snapshot().sample_bytes, 4);
    assert_eq!(
        owner.complete(&dispatch),
        Ok(BufferCompletion::Retired(QueueState::Cancelled))
    );
    assert_eq!(owner.snapshot().sample_bytes, 0);
}

#[test]
fn projected_reconstruction_refuses_old_owner_without_touching_current_pcm() {
    let identity = cue(5, 11, basis(1, 7));
    let mut old = queue();
    let foreign = old
        .replace(&output(), basis(1, 7), identity, source(), format(), 0)
        .unwrap()
        .receipt;
    old.dispose();
    drop(old);
    let mut current = queue();
    let receipt = current
        .replace(&output(), basis(1, 7), identity, source(), format(), 0)
        .unwrap()
        .receipt;
    current
        .enqueue(&receipt, &output(), manifest(), 0, 0, pcm(0.1))
        .unwrap();
    let dispatch = current.begin(&receipt, &output()).unwrap().unwrap();
    let before = current.snapshot();
    assert_eq!(
        current
            .cancel_media(&foreign, &output(), &identity)
            .unwrap_err(),
        QueueError::ForeignOwner
    );
    assert_eq!(current.snapshot(), before);
    assert_eq!(
        current.dispatched(&dispatch).unwrap().buffer.samples(),
        &[0.1]
    );
    assert!(
        current
            .cancel_media(&receipt, &output(), &identity)
            .unwrap()
            .stop_required
            .is_some()
    );
    current.confirm_stopped(&dispatch).unwrap();
    assert_eq!(current.snapshot().sample_bytes, 0);
}

#[test]
fn projected_generations_remain_per_job_and_revisions_remain_monotonic() {
    let mut owner = queue();
    owner
        .replace(
            &output(),
            basis(1, 7),
            cue(5, 11, basis(1, 7)),
            source(),
            format(),
            0,
        )
        .unwrap();
    assert_eq!(
        owner
            .replace(
                &output(),
                basis(1, 7),
                cue(5, 10, basis(1, 7)),
                source(),
                format(),
                0
            )
            .unwrap_err(),
        QueueError::StaleGeneration
    );
    owner
        .replace(
            &output(),
            basis(1, 7),
            cue(8, 1, basis(1, 7)),
            source(),
            format(),
            0,
        )
        .unwrap();
    owner
        .replace(
            &output(),
            basis(2, 0),
            cue(5, 1, basis(2, 0)),
            source(),
            format(),
            0,
        )
        .unwrap();
    let before = owner.snapshot();
    assert_eq!(
        owner
            .replace(
                &output(),
                basis(1, 99),
                cue(9, 1, basis(1, 99)),
                source(),
                format(),
                0
            )
            .unwrap_err(),
        QueueError::WrongBasis
    );
    let other_run = PlaybackBasis::new(
        basis(2, 0).session(),
        RunId::from_bytes(&[9; 16]).unwrap(),
        basis(2, 0).revision(),
    );
    assert_eq!(
        owner
            .replace(
                &output(),
                other_run,
                cue(9, 1, other_run),
                source(),
                format(),
                0
            )
            .unwrap_err(),
        QueueError::WrongBasis
    );
    assert_eq!(owner.snapshot(), before);
}

#[test]
fn projected_locked_prerequisite_and_manifest_refusal_preserve_admission() {
    let locked = PlaybackOutput::new(
        [4; 16],
        output().binding(),
        9,
        PlaybackDestination::PrivateListener,
        false,
    )
    .unwrap();
    let mut owner = PresentationQueue::new(
        locked.binding(),
        locked,
        basis(1, 7),
        QueueLimits {
            retained_buffers: 1,
            sample_bytes: 8,
            cue_chunks: 1,
            cue_frames: 1,
        },
    )
    .unwrap();
    assert_eq!(
        owner
            .replace(
                &locked,
                basis(1, 7),
                cue(5, 1, basis(1, 7)),
                source(),
                format(),
                0
            )
            .unwrap_err(),
        QueueError::Locked
    );
    assert_eq!(owner.snapshot().state, QueueState::Idle);
    let mut owner = queue();
    let receipt = owner
        .replace(
            &output(),
            basis(1, 7),
            cue(5, 1, basis(1, 7)),
            source(),
            format(),
            0,
        )
        .unwrap()
        .receipt;
    let refused = owner
        .enqueue(
            &receipt,
            &output(),
            AssetManifest {
                byte_len: 3,
                sha256: [8; 32],
            },
            0,
            0,
            pcm(0.1),
        )
        .unwrap_err();
    assert_eq!(refused.reason, QueueError::WrongAsset);
    assert_eq!(refused.buffer.samples(), &[0.1]);
    assert_eq!(owner.snapshot().sample_bytes, 0);
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "common/mod.rs"]
mod common;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_full_lease_equality_precedes_projection_for_audience_and_private_member() {
    use df_audio::AudioQueue;
    use df_model::checkpoint::{AudienceScope, AudioDestination};
    use df_types::MemberId;
    let mut owner = common::queue(2, 16);
    let receipt = common::start(&mut owner, 11, 0);
    owner
        .enqueue(
            &receipt,
            &common::lease(),
            common::manifest(),
            0,
            0,
            common::pcm(&[0.1]),
        )
        .unwrap();
    let dispatch = owner.begin(&receipt, &common::lease()).unwrap().unwrap();
    let before = owner.snapshot();
    let mut audience_changed = common::lease();
    audience_changed.audience = AudienceScope::Host;
    assert_eq!(
        owner.cancel(&receipt, &audience_changed).unwrap_err(),
        QueueError::WrongLease
    );
    assert_eq!(
        owner
            .replace(
                &audience_changed,
                common::basis(1, 7),
                common::identity(12),
                common::asset(),
                common::format(),
                0
            )
            .unwrap_err(),
        QueueError::WrongLease
    );
    assert_eq!(owner.snapshot(), before);
    assert_eq!(
        owner.dispatched(&dispatch).unwrap().buffer.samples(),
        &[0.1]
    );

    let first_member = MemberId::from_bytes(&[8; 16]).unwrap();
    let second_member = MemberId::from_bytes(&[9; 16]).unwrap();
    let mut original = common::lease();
    original.destination = AudioDestination::PrivateListener {
        member: first_member,
        binding: common::binding(),
    };
    original.audience = AudienceScope::Members(vec![first_member]);
    let mut private = AudioQueue::new(
        common::binding(),
        original.clone(),
        common::basis(1, 7),
        common::limits(1, 8),
    )
    .unwrap();
    let receipt = private
        .replace(
            &original,
            common::basis(1, 7),
            common::identity(11),
            common::asset(),
            common::format(),
            0,
        )
        .unwrap()
        .receipt;
    let mut changed = original.clone();
    changed.destination = AudioDestination::PrivateListener {
        member: second_member,
        binding: common::binding(),
    };
    assert_eq!(
        private.cancel(&receipt, &changed).unwrap_err(),
        QueueError::WrongLease
    );
    changed = original.clone();
    changed.audience = AudienceScope::Members(vec![second_member]);
    assert_eq!(
        private.cancel(&receipt, &changed).unwrap_err(),
        QueueError::WrongLease
    );
    assert_eq!(private.snapshot().state, QueueState::Accepting);
    private.cancel(&receipt, &original).unwrap();
    assert_eq!(private.snapshot().state, QueueState::Cancelled);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_zero_generation_stop_keeps_original_stale_receipt_classification() {
    use df_media::schedule::ScheduleLimits;
    use df_media::speech::{SpeechLimits, SpeechScheduler, SpeechStopReason};
    let mut owner = common::queue(2, 16);
    let receipt = common::start(&mut owner, 11, 0);
    owner
        .enqueue(
            &receipt,
            &common::lease(),
            common::manifest(),
            0,
            0,
            common::pcm(&[0.1]),
        )
        .unwrap();
    let dispatch = owner.begin(&receipt, &common::lease()).unwrap().unwrap();
    let before = owner.snapshot();
    let identity = receipt.identity();
    let mut producer = SpeechScheduler::new(
        identity.basis,
        ScheduleLimits {
            queue_items: 1,
            queue_bytes: 1,
            speech_items: 1,
            speech_bytes: 1,
            execution_slots: 1,
            speech_slots: 1,
        },
        SpeechLimits {
            maximum_chunks: 1,
            maximum_bytes: 1,
        },
    )
    .unwrap();
    producer.admit(identity, Box::from([9_u8])).unwrap();
    let source_dispatch = producer.begin().unwrap().unwrap();
    let mut stopped = producer
        .stop(&source_dispatch, SpeechStopReason::Cancelled)
        .unwrap();
    stopped.identity.generation = 0;
    assert_eq!(
        owner
            .cancel_media(&receipt, &common::lease(), &stopped)
            .unwrap_err(),
        QueueError::StaleReceipt
    );
    assert_eq!(owner.snapshot(), before);
    assert_eq!(
        owner.dispatched(&dispatch).unwrap().buffer.samples(),
        &[0.1]
    );
    stopped.identity = identity;
    assert!(
        owner
            .cancel_media(&receipt, &common::lease(), &stopped)
            .unwrap()
            .stop_required
            .is_some()
    );
    assert_eq!(owner.snapshot().state, QueueState::WaitingForStop);
    owner.confirm_stopped(&dispatch).unwrap();
    assert_eq!(owner.snapshot().sample_bytes, 0);
}
