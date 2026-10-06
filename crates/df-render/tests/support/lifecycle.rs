use df_assets::AssetManifest;
use df_client::cache::{CacheKey, CacheLimits, CacheScope};
use df_render::{ImageDecodeLimits, ResourceLifecycle, ResourceLimits, SceneOwner};
use df_types::{ClientBindingId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision};

// Same immutable complete PNG used by the actual bounded browser codec fixture.
pub const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 16, 73, 68, 65, 84, 120, 1, 1, 5, 0, 250, 255, 0, 16, 32, 48,
    255, 2, 4, 1, 96, 145, 5, 159, 157, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
pub fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}
pub fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
}
pub fn owner() -> SceneOwner {
    (
        SessionId::from_bytes(&[1; 16]).unwrap(),
        RunId::from_bytes(&[2; 16]).unwrap(),
        ClientBindingId::from_bytes(&[3; 16]).unwrap(),
    )
}
pub fn scope() -> CacheScope {
    let (session, run, binding) = owner();
    CacheScope {
        session,
        run,
        binding,
    }
}
pub fn key(version: &str) -> CacheKey {
    CacheKey {
        version: label(version),
        bytes: AssetManifest {
            byte_len: PNG.len() as u64,
            sha256: [
                222, 192, 39, 112, 37, 240, 57, 61, 121, 122, 164, 72, 115, 222, 203, 189, 117,
                216, 238, 33, 18, 247, 83, 188, 89, 237, 149, 178, 217, 61, 158, 233,
            ],
        },
    }
}
pub fn cache_limits() -> CacheLimits {
    CacheLimits {
        max_assets: 2,
        max_pending: 2,
        max_leases: 4,
        max_bytes: PNG.len(),
    }
}
pub fn two_asset_cache_limits() -> CacheLimits {
    CacheLimits {
        max_bytes: PNG.len() * 2,
        ..cache_limits()
    }
}
pub fn resource_limits() -> ResourceLimits {
    ResourceLimits {
        max_references: 2,
        max_resident: 2,
        max_pending: 2,
        max_decoded_bytes: 8,
        max_work_bytes: 512,
    }
}
pub fn decode_limits() -> ImageDecodeLimits {
    ImageDecodeLimits {
        max_encoded_bytes: PNG.len(),
        max_dimension: 1,
        max_decoded_bytes: 4,
        max_work_bytes: 512,
    }
}
pub fn install(lifecycle: &mut ResourceLifecycle, key: &CacheKey) {
    let fetch = lifecycle.fetch(key).unwrap();
    lifecycle.complete_fetch(&fetch, PNG.to_vec()).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
pub fn lifecycle() -> ResourceLifecycle {
    ResourceLifecycle::from_scene(
        df_render::SceneRenderer::new(owner()),
        scope(),
        cache_limits(),
        resource_limits(),
        label("bounded-png-v1"),
    )
    .unwrap()
}
