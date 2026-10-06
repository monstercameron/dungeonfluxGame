#![cfg(not(target_arch = "wasm32"))]
#[path = "../src/gameplay_browser/public_scene_assets.rs"]
mod public_scene_assets;
use df_client::cache::{AssetCache, CacheError, CacheLimits, CacheScope};
use df_types::{ClientBindingId, RecoveryEpoch, RunId, SessionId, SessionRevision};
use public_scene_assets::{Admission, PublicSceneError, PublicSceneKind, SceneAdmission, resolve};
use sha2::{Digest, Sha256};

const HARBOR: &[u8] = include_bytes!("../../../assets/ui/scenes/mara-harbor-v4.png");
const TAVERN: &[u8] =
    include_bytes!("../../../assets/concept-art/scene-tavern-barkeep-talk-rain.webp");
fn scope(binding: u8) -> CacheScope {
    CacheScope {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        binding: ClientBindingId::from_bytes(&[binding; 16]).unwrap(),
    }
}
fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
}
fn cache() -> AssetCache {
    AssetCache::new(
        scope(3),
        CacheLimits {
            max_assets: 2,
            max_pending: 1,
            max_leases: 1,
            max_bytes: HARBOR.len(),
        },
    )
    .unwrap()
}
#[test]
fn catalogue_pins_the_actual_public_files_and_no_url_authority() {
    for (path, bytes, kind) in [
        (
            "assets/ui/scenes/mara-harbor-v4.png",
            HARBOR,
            PublicSceneKind::Harbor,
        ),
        (
            "assets/concept-art/scene-tavern-barkeep-talk-rain.webp",
            TAVERN,
            PublicSceneKind::Tavern,
        ),
    ] {
        let asset = resolve(path).unwrap();
        let key = asset.key().unwrap();
        assert_eq!(asset.path, format!("/{path}"));
        assert_eq!(asset.kind, kind);
        assert_eq!(key.bytes.byte_len, bytes.len() as u64);
        assert_eq!(key.bytes.sha256, <[u8; 32]>::from(Sha256::digest(bytes)));
        assert_eq!(key.bytes.sha256, asset.sha256);
        assert_eq!(key.bytes.byte_len, asset.byte_len);
    }
    for path in [
        "/assets/ui/scenes/mara-harbor-v4.png",
        "assets/ui/scenes/mara-harbor-v4.png?private=1",
        "assets/ui/scenes/../mara-harbor-v4.png",
        "https://example.com/scene.png",
        "generated/private.png",
        "assets/concept-art/dm-avatar.webp",
        "",
        "assets/ui/scenes/mara-harbor-v4.png#fragment",
    ] {
        assert!(matches!(resolve(path), Err(PublicSceneError::UnknownAsset)));
    }
}
#[test]
fn rejected_snapshot_does_not_advance_the_selection_or_watermark() {
    let harbor = resolve("assets/ui/scenes/mara-harbor-v4.png").unwrap();
    let tavern = resolve("assets/concept-art/scene-tavern-barkeep-talk-rain.webp").unwrap();
    let mut owner = SceneAdmission::new();
    assert_eq!(
        owner.check(scope(3), revision(1, 8), harbor),
        Ok(Admission::New)
    );
    owner.accept(scope(3), revision(1, 8), harbor);
    assert_eq!(
        owner.check(scope(3), revision(1, 7), tavern),
        Err(PublicSceneError::StaleRevision)
    );
    assert_eq!(
        owner.check(scope(3), revision(1, 8), tavern),
        Err(PublicSceneError::ConflictingSnapshot)
    );
    assert_eq!(
        owner.check(scope(3), revision(1, 8), harbor),
        Ok(Admission::Retained)
    );
    assert_eq!(
        owner.check(scope(3), revision(1, 9), harbor),
        Ok(Admission::New)
    );
    assert_eq!(
        owner.check(scope(3), revision(1, 9), tavern),
        Ok(Admission::Replaced)
    );
}
#[test]
fn recovery_scope_and_repeated_key_require_replacement_not_receipt_redraw() {
    let harbor = resolve("assets/ui/scenes/mara-harbor-v4.png").unwrap();
    let mut owner = SceneAdmission::new();
    owner.accept(scope(3), revision(1, 99), harbor);
    assert_eq!(
        owner.check(scope(3), revision(2, 0), harbor),
        Ok(Admission::Replaced)
    );
    assert_eq!(
        owner.check(scope(4), revision(1, 99), harbor),
        Ok(Admission::Replaced)
    );
    owner.clear();
    assert_eq!(
        owner.check(scope(4), revision(1, 0), harbor),
        Ok(Admission::New)
    );
}
#[test]
fn real_public_bytes_require_complete_canonical_hash_and_length() {
    let key = resolve("assets/ui/scenes/mara-harbor-v4.png")
        .unwrap()
        .key()
        .unwrap();
    let mut owner = cache();
    owner
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&key))
        .unwrap();
    let short = owner.fetch(&key).unwrap();
    assert_eq!(
        owner.complete(&short, HARBOR[..HARBOR.len() - 1].to_vec()),
        Err(CacheError::Incomplete)
    );
    let corrupt = owner.fetch(&key).unwrap();
    let mut bytes = HARBOR.to_vec();
    bytes[100] ^= 1;
    assert_eq!(
        owner.complete(&corrupt, bytes),
        Err(CacheError::HashMismatch)
    );
    let oversized = owner.fetch(&key).unwrap();
    let mut bytes = HARBOR.to_vec();
    bytes.push(0);
    assert_eq!(
        owner.complete(&oversized, bytes),
        Err(CacheError::ByteCapacity)
    );
    assert!(owner.acquire(&key).unwrap().is_none());
    let actual = owner.fetch(&key).unwrap();
    owner.complete(&actual, HARBOR.to_vec()).unwrap();
    let lease = owner.acquire(&key).unwrap().unwrap();
    assert_eq!(owner.lease_bytes(&lease).unwrap(), HARBOR);
}
#[test]
fn same_phase_revision_retains_real_pending_fetch_and_verified_lease() {
    let key = resolve("assets/concept-art/scene-tavern-barkeep-talk-rain.webp")
        .unwrap()
        .key()
        .unwrap();
    let mut owner = cache();
    owner
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&key))
        .unwrap();
    let token = owner.fetch(&key).unwrap();
    owner
        .apply_current(scope(3), revision(1, 2), std::slice::from_ref(&key))
        .unwrap();
    owner.complete(&token, TAVERN.to_vec()).unwrap();
    let lease = owner.acquire(&key).unwrap().unwrap();
    owner
        .apply_current(scope(3), revision(1, 3), std::slice::from_ref(&key))
        .unwrap();
    assert_eq!(owner.lease_bytes(&lease).unwrap(), TAVERN);
    assert_eq!(owner.lease_count(), 1);
}
#[test]
fn epoch_recovery_and_revocation_refuse_exact_old_public_completion() {
    let key = resolve("assets/concept-art/scene-tavern-barkeep-talk-rain.webp")
        .unwrap()
        .key()
        .unwrap();
    let mut owner = cache();
    owner
        .apply_current(scope(3), revision(1, 5), std::slice::from_ref(&key))
        .unwrap();
    let old = owner.fetch(&key).unwrap();
    owner
        .apply_current(scope(3), revision(2, 0), std::slice::from_ref(&key))
        .unwrap();
    let current = owner.fetch(&key).unwrap();
    assert_eq!(
        owner.complete(&old, TAVERN.to_vec()),
        Err(CacheError::StaleFetch)
    );
    owner.complete(&current, TAVERN.to_vec()).unwrap();
    let lease = owner.acquire(&key).unwrap().unwrap();
    owner.apply_current(scope(3), revision(2, 1), &[]).unwrap();
    assert_eq!(owner.lease_bytes(&lease), Err(CacheError::StaleLease));
    assert_eq!(owner.resident_bytes(), 0);
    assert_eq!(
        owner.apply_current(scope(4), revision(2, 2), &[]),
        Err(CacheError::WrongScope)
    );
}
