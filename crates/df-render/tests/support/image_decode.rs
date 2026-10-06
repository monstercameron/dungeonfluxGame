use df_assets::AssetManifest;
use df_client::cache::{AssetCache, CacheKey, CacheLimits, CacheScope};
use df_render::{ImageDecodeLimits, SceneOwner};
use df_types::{ClientBindingId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision};
use std::cell::RefCell;
use std::rc::Rc;

// Complete 1x1 RGBA8 PNG, stored Deflate block, opaque pixel [16,32,48,255].
pub const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 16, 73, 68, 65, 84, 120, 1, 1, 5, 0, 250, 255, 0, 16, 32, 48,
    255, 2, 4, 1, 96, 145, 5, 159, 157, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
pub const CORRUPT_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 16, 73, 68, 65, 84, 120, 1, 7, 5, 0, 250, 255, 0, 16, 32, 48,
    255, 2, 4, 1, 96, 105, 199, 212, 43, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
pub const TOO_WIDE_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 16, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 177, 227, 0, 66, 0, 0, 0, 16, 73, 68, 65, 84, 120, 1, 1, 5, 0, 250, 255, 0, 16, 32, 48,
    255, 2, 4, 1, 96, 145, 5, 159, 157, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
pub const UNSUPPORTED_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 16, 6, 0,
    0, 0, 79, 133, 24, 202, 0, 0, 0, 16, 73, 68, 65, 84, 120, 1, 1, 5, 0, 250, 255, 0, 16, 32, 48,
    255, 2, 4, 1, 96, 145, 5, 159, 157, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
pub const PNG_DIGEST: [u8; 32] = [
    222, 192, 39, 112, 37, 240, 57, 61, 121, 122, 164, 72, 115, 222, 203, 189, 117, 216, 238, 33,
    18, 247, 83, 188, 89, 237, 149, 178, 217, 61, 158, 233,
];
pub const CORRUPT_DIGEST: [u8; 32] = [
    75, 159, 41, 196, 9, 137, 105, 56, 167, 87, 88, 209, 184, 86, 59, 122, 64, 15, 40, 129, 185,
    81, 156, 51, 191, 91, 36, 29, 240, 137, 0, 44,
];
pub const WIDE_DIGEST: [u8; 32] = [
    142, 168, 21, 165, 198, 1, 21, 117, 83, 238, 248, 142, 81, 36, 132, 90, 222, 177, 13, 174, 185,
    202, 194, 41, 138, 210, 169, 120, 229, 41, 194, 90,
];

pub fn label(text: &str) -> RevisionLabel {
    RevisionLabel::new(Some(text)).unwrap()
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
pub fn limits() -> ImageDecodeLimits {
    ImageDecodeLimits {
        max_encoded_bytes: 1024,
        max_dimension: 4096,
        max_decoded_bytes: 1024,
        max_work_bytes: 4096,
    }
}
pub fn cached(source: &[u8], digest: [u8; 32]) -> (Rc<RefCell<AssetCache>>, CacheKey) {
    let key = CacheKey {
        version: label("current-png-fixture-v1"),
        bytes: AssetManifest {
            byte_len: source.len() as u64,
            sha256: digest,
        },
    };
    let mut cache = AssetCache::new(
        scope(),
        CacheLimits {
            max_assets: 2,
            max_pending: 1,
            max_leases: 4,
            max_bytes: 1024,
        },
    )
    .unwrap();
    cache
        .apply_current(scope(), revision(1, 0), std::slice::from_ref(&key))
        .unwrap();
    let fetch = cache.fetch(&key).unwrap();
    cache.complete(&fetch, source.to_vec()).unwrap();
    (Rc::new(RefCell::new(cache)), key)
}

// Exact current fixed public route fixtures, not server publication or authorization.
pub struct PreparedAsset {
    pub route: &'static str,
    pub bytes: &'static [u8],
    pub digest: [u8; 32],
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
}
impl PreparedAsset {
    pub fn metadata(&self) -> df_render::PreparedImageMetadata<'static> {
        df_render::PreparedImageMetadata {
            mime: self.mime,
            width: self.width,
            height: self.height,
            max_ancillary_bytes: 32 * 1024,
        }
    }
    pub fn key(&self) -> CacheKey {
        CacheKey {
            version: label(self.route),
            bytes: AssetManifest {
                byte_len: self.bytes.len() as u64,
                sha256: self.digest,
            },
        }
    }
}
pub fn prepared_limits() -> ImageDecodeLimits {
    ImageDecodeLimits {
        max_encoded_bytes: 4 * 1024 * 1024,
        max_dimension: 2048,
        max_decoded_bytes: 8 * 1024 * 1024,
        max_work_bytes: 64 * 1024 * 1024,
    }
}
pub const PREPARED_ASSETS: &[PreparedAsset] = &[
    PreparedAsset {
        route: "/assets/ui/scenes/mara-harbor-v4.png",
        bytes: include_bytes!("../../../../assets/ui/scenes/mara-harbor-v4.png"),
        digest: [
            73, 131, 190, 194, 29, 120, 124, 64, 103, 104, 106, 154, 68, 0, 236, 94, 5, 183, 195,
            221, 175, 182, 245, 166, 225, 136, 113, 198, 247, 118, 94, 162,
        ],
        mime: "image/png",
        width: 1672,
        height: 941,
    },
    PreparedAsset {
        route: "/assets/concept-art/scene-campfire-under-stars.webp",
        bytes: include_bytes!("../../../../assets/concept-art/scene-campfire-under-stars.webp"),
        digest: [
            156, 164, 177, 68, 61, 251, 33, 8, 183, 251, 107, 138, 29, 104, 17, 160, 22, 84, 50,
            190, 219, 183, 243, 140, 75, 175, 94, 29, 151, 38, 202, 19,
        ],
        mime: "image/webp",
        width: 1672,
        height: 941,
    },
    PreparedAsset {
        route: "/assets/concept-art/vell-avatar.webp",
        bytes: include_bytes!("../../../../assets/concept-art/vell-avatar.webp"),
        digest: [
            140, 159, 26, 187, 62, 5, 185, 154, 41, 80, 238, 123, 213, 168, 102, 182, 74, 123, 131,
            95, 213, 162, 78, 191, 172, 134, 162, 56, 55, 201, 155, 28,
        ],
        mime: "image/webp",
        width: 1536,
        height: 1024,
    },
    PreparedAsset {
        route: "/assets/concept-art/scene-tavern-barkeep-talk-rain.webp",
        bytes: include_bytes!("../../../../assets/concept-art/scene-tavern-barkeep-talk-rain.webp"),
        digest: [
            58, 212, 27, 247, 254, 235, 224, 7, 2, 181, 122, 131, 142, 17, 119, 86, 244, 113, 141,
            146, 5, 203, 177, 9, 74, 151, 171, 158, 201, 164, 236, 216,
        ],
        mime: "image/webp",
        width: 1672,
        height: 941,
    },
];
