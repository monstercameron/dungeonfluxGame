use df_assets::AssetManifest;
use df_audio::{AudioQueue, AudioReceipt, PcmBuffer, PcmFormat, QueueLimits};
use df_media::speech::SpeechIdentity;
use df_model::checkpoint::{
    AssetKind, AssetReference, AudienceScope, AudioDestination, AudioOutputLease, Basis,
    ContentDigest, JobId, RecordId,
};
use df_types::{
    ClientBindingId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};

pub fn basis(epoch: u64, sequence: u64) -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence),
    }
}

pub fn binding() -> ClientBindingId {
    ClientBindingId::from_bytes(&[3; 16]).unwrap()
}
pub fn lease() -> AudioOutputLease {
    AudioOutputLease {
        id: RecordId::from_bytes(&[4; 16]).unwrap(),
        destination: AudioDestination::PublicRoom(binding()),
        generation: 9,
        audience: AudienceScope::Shared,
        device_unlocked: true,
    }
}
pub fn identity(generation: u64) -> SpeechIdentity {
    SpeechIdentity {
        basis: basis(1, 7),
        job: JobId::from_bytes(&[5; 16]).unwrap(),
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        generation,
    }
}
pub fn asset() -> AssetReference {
    AssetReference {
        key: RevisionLabel::new(Some("synthetic-source")).unwrap(),
        digest: ContentDigest([7; 32]),
        byte_length: 3,
        kind: AssetKind::Audio,
    }
}
pub fn manifest() -> AssetManifest {
    AssetManifest {
        byte_len: 3,
        sha256: [7; 32],
    }
}
pub fn format() -> PcmFormat {
    PcmFormat::new(1, 48000).unwrap()
}
pub fn pcm(samples: &[f32]) -> PcmBuffer {
    PcmBuffer::new(format(), samples.into()).unwrap()
}
pub fn limits(buffers: usize, bytes: usize) -> QueueLimits {
    QueueLimits {
        retained_buffers: buffers,
        sample_bytes: bytes,
        cue_chunks: 8,
        cue_frames: 16,
    }
}
pub fn queue(buffers: usize, bytes: usize) -> AudioQueue {
    AudioQueue::new(binding(), lease(), basis(1, 7), limits(buffers, bytes)).unwrap()
}
pub fn start(queue: &mut AudioQueue, generation: u64, first_frame: u64) -> AudioReceipt {
    queue
        .replace(
            &lease(),
            basis(1, 7),
            identity(generation),
            asset(),
            format(),
            first_frame,
        )
        .unwrap()
        .receipt
}
