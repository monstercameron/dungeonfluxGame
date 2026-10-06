use df_assets::AssetManifest;
use df_audio::{AudioQueue, PcmBuffer, PcmFormat, QueueLimits};
use df_client::cache::{AssetCache, CacheKey, CacheLease, CacheLimits, CacheScope};
use df_media::speech::SpeechIdentity;
use df_model::checkpoint::{
    AssetKind, AssetReference, AudienceScope, AudioDestination, AudioOutputLease, Basis,
    ContentDigest, JobId, RecordId,
};
use df_types::{
    ClientBindingId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};

pub const FRAMES: u64 = 24_000;

pub fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 7),
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
        basis: basis(),
        job: JobId::from_bytes(&[5; 16]).unwrap(),
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        generation,
    }
}

pub fn queue() -> AudioQueue {
    AudioQueue::new(
        binding(),
        lease(),
        basis(),
        QueueLimits {
            retained_buffers: 2,
            sample_bytes: 512 * 1024,
            cue_chunks: 4,
            cue_frames: FRAMES * 2,
        },
    )
    .unwrap()
}

/// Exact local square-tone f32 fixture; no provider codec or admitted speech is claimed.
/// Cache SHA verification covers these actual PCM bytes, not an unrelated placeholder.
pub struct LocalPcm {
    pub cache: AssetCache,
    pub source: CacheLease,
    pub reference: AssetReference,
    pub manifest: AssetManifest,
    pub buffer: Option<PcmBuffer>,
}

pub fn local_pcm(channels: u16) -> LocalPcm {
    assert!((1..=2).contains(&channels));
    let samples: Vec<f32> = (0..FRAMES)
        .flat_map(|frame| {
            let amplitude = if frame % 200 < 100 {
                0.125_f32
            } else {
                -0.125_f32
            };
            (0..channels).map(move |channel| if channel == 0 { amplitude } else { -amplitude })
        })
        .collect();
    let bytes: Vec<u8> = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    let digest = if channels == 1 {
        MONO_DIGEST
    } else {
        STEREO_DIGEST
    };
    let manifest = AssetManifest {
        byte_len: bytes.len() as u64,
        sha256: digest,
    };
    let reference = AssetReference {
        key: RevisionLabel::new(Some(if channels == 1 {
            "local-pcm-mono"
        } else {
            "local-pcm-stereo"
        }))
        .unwrap(),
        digest: ContentDigest(digest),
        byte_length: manifest.byte_len,
        kind: AssetKind::Audio,
    };
    let key = CacheKey {
        version: reference.key.clone(),
        bytes: manifest,
    };
    let scope = CacheScope {
        session: basis().session,
        run: basis().run,
        binding: binding(),
    };
    let mut cache = AssetCache::new(
        scope,
        CacheLimits {
            max_assets: 1,
            max_pending: 1,
            max_leases: 1,
            max_bytes: 256 * 1024,
        },
    )
    .unwrap();
    cache
        .apply_current(scope, basis().revision, std::slice::from_ref(&key))
        .unwrap();
    let fetch = cache.fetch(&key).unwrap();
    cache.complete(&fetch, bytes).unwrap();
    let source = cache.acquire(&key).unwrap().unwrap();
    let buffer = PcmBuffer::new(
        PcmFormat::new(channels, 48_000).unwrap(),
        samples.into_boxed_slice(),
    )
    .unwrap();
    LocalPcm {
        cache,
        source,
        reference,
        manifest,
        buffer: Some(buffer),
    }
}

const MONO_DIGEST: [u8; 32] = [
    105, 89, 99, 177, 16, 210, 56, 244, 116, 76, 44, 70, 254, 59, 146, 190, 240, 134, 190, 211, 58,
    130, 166, 161, 74, 6, 253, 11, 20, 86, 164, 229,
];
const STEREO_DIGEST: [u8; 32] = [
    70, 150, 54, 237, 161, 235, 129, 44, 88, 46, 52, 178, 140, 23, 177, 47, 251, 139, 158, 47, 229,
    113, 136, 95, 82, 115, 252, 215, 69, 115, 242, 231,
];
