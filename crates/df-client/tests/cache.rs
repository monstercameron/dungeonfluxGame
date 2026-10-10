use df_assets::AssetManifest;
use df_client::cache::{AssetCache, CacheError, CacheKey, CacheLimits, CacheScope};
use df_types::{ClientBindingId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision};
use sha2::{Digest, Sha256};

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

fn key(version: &str, bytes: &[u8]) -> CacheKey {
    CacheKey {
        version: RevisionLabel::new(Some(version)).unwrap(),
        bytes: AssetManifest {
            byte_len: bytes.len() as u64,
            sha256: Sha256::digest(bytes).into(),
        },
    }
}

fn cache(max_bytes: usize, max_pending: usize) -> AssetCache {
    cache_in_scope(scope(3), max_bytes, max_pending)
}

fn cache_in_scope(scope: CacheScope, max_bytes: usize, max_pending: usize) -> AssetCache {
    AssetCache::new(
        scope,
        CacheLimits {
            max_assets: 3,
            max_pending,
            max_leases: 3,
            max_bytes,
        },
    )
    .unwrap()
}

fn publish(cache: &mut AssetCache, key: &CacheKey, bytes: &[u8]) {
    let token = cache.fetch(key).unwrap();
    cache.complete(&token, bytes.to_vec()).unwrap();
}

#[test]
fn exact_complete_sha256_bytes_are_the_only_cache_hit() {
    // SHA-256 published known-answer vector; expectation is independent of fixture hashing.
    let mut asset = key("abc-v1", b"abc");
    asset.bytes.sha256 = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    let mut cache = cache(16, 2);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    assert_eq!(cache.get(&asset).unwrap(), None);
    let token = cache.fetch(&asset).unwrap();
    assert_eq!(cache.get(&asset).unwrap(), None);
    cache.complete(&token, b"abc".to_vec()).unwrap();
    assert_eq!(cache.get(&asset).unwrap(), Some(b"abc".as_slice()));
    assert_eq!(
        cache.complete(&token, b"abc".to_vec()),
        Err(CacheError::StaleFetch)
    );
    assert_eq!(cache.resident_bytes(), 3);
}

#[test]
fn truncated_corrupt_and_overallocated_results_are_discarded() {
    let asset = key("abc-v1", b"abc");
    let mut cache = cache(8, 1);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    let short = cache.fetch(&asset).unwrap();
    assert_eq!(
        cache.complete(&short, b"ab".to_vec()),
        Err(CacheError::Incomplete)
    );
    let corrupt = cache.fetch(&asset).unwrap();
    assert_eq!(
        cache.complete(&corrupt, b"abd".to_vec()),
        Err(CacheError::HashMismatch)
    );
    let excessive = cache.fetch(&asset).unwrap();
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(b"abc");
    assert_eq!(
        cache.complete(&excessive, bytes),
        Err(CacheError::ByteCapacity)
    );
    assert_eq!(cache.get(&asset).unwrap(), None);
    assert_eq!(cache.pending_count(), 0);
    assert_eq!(cache.resident_bytes(), 0);
}

#[test]
fn lru_access_drives_byte_eviction_and_misses_can_refetch() {
    let a = key("a", b"aa");
    let b = key("b", b"bb");
    let c = key("c", b"cc");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(1, 1), &[a.clone(), b.clone(), c.clone()])
        .unwrap();
    publish(&mut cache, &a, b"aa");
    publish(&mut cache, &b, b"bb");
    assert!(cache.get(&a).unwrap().is_some());
    publish(&mut cache, &c, b"cc");
    assert_eq!(cache.resident_bytes(), 4);
    assert_eq!(cache.get(&b).unwrap(), None);
    assert!(cache.get(&a).unwrap().is_some());
    publish(&mut cache, &b, b"bb");
    assert!(cache.get(&b).unwrap().is_some());
    assert_eq!(cache.resident_bytes(), 4);
}

#[test]
fn pending_capacity_cancellation_and_release_preserve_newer_work() {
    let a = key("a", b"a");
    let b = key("b", b"b");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(1, 1), &[a.clone(), b.clone()])
        .unwrap();
    let old = cache.fetch(&a).unwrap();
    assert!(matches!(cache.fetch(&a), Err(CacheError::AlreadyPending)));
    assert!(matches!(cache.fetch(&b), Err(CacheError::PendingCapacity)));
    cache.cancel(&old).unwrap();
    let replacement = cache.fetch(&a).unwrap();
    assert_eq!(
        cache.complete(&old, b"a".to_vec()),
        Err(CacheError::StaleFetch)
    );
    assert_eq!(cache.pending_count(), 1);
    cache.complete(&replacement, b"a".to_vec()).unwrap();
    cache.release(&a).unwrap();
    assert_eq!(cache.get(&a).unwrap(), None);
    let released = cache.fetch(&a).unwrap();
    cache.release(&a).unwrap();
    assert_eq!(
        cache.complete(&released, b"a".to_vec()),
        Err(CacheError::StaleFetch)
    );
}

#[test]
fn current_revocation_discards_bytes_and_fences_queued_completion() {
    let a = key("a", b"a");
    let b = key("b", b"b");
    let mut cache = cache(4, 2);
    cache
        .apply_current(scope(3), revision(1, 1), &[a.clone(), b.clone()])
        .unwrap();
    publish(&mut cache, &a, b"a");
    let old = cache.fetch(&b).unwrap();
    cache.apply_current(scope(3), revision(1, 2), &[]).unwrap();
    assert_eq!(cache.resident_bytes(), 0);
    assert_eq!(cache.pending_count(), 0);
    assert_eq!(cache.get(&a), Err(CacheError::NotCurrent));
    assert_eq!(
        cache.complete(&old, b"b".to_vec()),
        Err(CacheError::StaleFetch)
    );
    cache
        .apply_current(scope(3), revision(2, 0), std::slice::from_ref(&b))
        .unwrap();
    let replacement = cache.fetch(&b).unwrap();
    assert_eq!(
        cache.complete(&old, b"b".to_vec()),
        Err(CacheError::StaleFetch)
    );
    assert_eq!(cache.pending_count(), 1);
    cache.complete(&replacement, b"b".to_vec()).unwrap();
}

#[test]
fn rejected_scope_revision_and_reference_updates_are_atomic() {
    let a = key("a", b"a");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(2, 1), std::slice::from_ref(&a))
        .unwrap();
    let active = cache.fetch(&a).unwrap();
    assert_eq!(
        cache.apply_current(scope(4), revision(2, 2), &[]),
        Err(CacheError::WrongScope)
    );
    assert_eq!(
        cache.apply_current(scope(3), revision(1, 999), &[]),
        Err(CacheError::StaleRevision)
    );
    assert_eq!(
        cache.apply_current(scope(3), revision(2, 1), &[]),
        Err(CacheError::StaleRevision)
    );
    let same_version = key("a", b"different");
    assert_eq!(
        cache.apply_current(scope(3), revision(2, 2), &[a.clone(), same_version]),
        Err(CacheError::ConflictingReference)
    );
    let excessive = [a.clone(), key("b", b"b"), key("c", b"c"), key("d", b"d")];
    assert_eq!(
        cache.apply_current(scope(3), revision(2, 2), &excessive),
        Err(CacheError::ReferenceCapacity)
    );
    assert_eq!(cache.pending_count(), 1);
    cache.complete(&active, b"a".to_vec()).unwrap();
    assert_eq!(cache.get(&a).unwrap(), Some(b"a".as_slice()));
}

#[test]
fn digest_length_and_version_all_participate_in_current_identity() {
    let asset = key("a", b"a");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    publish(&mut cache, &asset, b"a");
    let mut altered = asset.clone();
    altered.bytes.byte_len = 2;
    assert_eq!(cache.get(&altered), Err(CacheError::NotCurrent));
    altered = asset.clone();
    altered.bytes.sha256[0] ^= 1;
    assert_eq!(cache.get(&altered), Err(CacheError::NotCurrent));
    altered.version = RevisionLabel::new(Some("new-version")).unwrap();
    assert_eq!(cache.get(&altered), Err(CacheError::NotCurrent));
    cache
        .apply_current(scope(3), revision(1, 2), std::slice::from_ref(&asset))
        .unwrap();
    assert_eq!(cache.get(&asset).unwrap(), Some(b"a".as_slice()));
}

#[test]
fn teardown_drops_allocations_and_old_tokens_cannot_target_a_new_instance() {
    let asset = key("a", b"a");
    let mut first = cache(4, 1);
    first
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    publish(&mut first, &asset, b"a");
    let old = first.fetch(&asset).unwrap();
    assert_eq!(first.resident_bytes(), 1);
    first.dispose();
    first.dispose();
    assert_eq!(first.resident_bytes(), 0);
    assert_eq!(first.pending_count(), 0);
    assert_eq!(first.complete(&old, b"a".to_vec()), Err(CacheError::Closed));
    assert_eq!(first.get(&asset), Err(CacheError::Closed));
    assert!(matches!(first.fetch(&asset), Err(CacheError::Closed)));
    let mut next = cache(4, 1);
    next.apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    let current = next.fetch(&asset).unwrap();
    assert_eq!(
        next.complete(&old, b"a".to_vec()),
        Err(CacheError::StaleFetch)
    );
    assert_eq!(next.pending_count(), 1);
    next.complete(&current, b"a".to_vec()).unwrap();
}

#[test]
fn invalid_limits_and_oversized_manifest_never_admit_work() {
    assert!(matches!(
        AssetCache::new(
            scope(3),
            CacheLimits {
                max_assets: 0,
                max_pending: 1,
                max_leases: 1,
                max_bytes: 1
            }
        ),
        Err(CacheError::InvalidLimits)
    ));
    assert!(matches!(
        AssetCache::new(
            scope(3),
            CacheLimits {
                max_assets: 1,
                max_pending: 1,
                max_leases: 0,
                max_bytes: 1,
            }
        ),
        Err(CacheError::InvalidLimits)
    ));
    let large = key("large", b"12345");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&large))
        .unwrap();
    assert!(matches!(cache.fetch(&large), Err(CacheError::ByteCapacity)));
    assert_eq!(cache.pending_count(), 0);
}

#[test]
fn unchanged_current_keys_keep_fetch_ownership_through_view_updates() {
    let asset = key("a", b"a");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    let token = cache.fetch(&asset).unwrap();
    cache
        .apply_current(scope(3), revision(1, 2), std::slice::from_ref(&asset))
        .unwrap();
    assert_eq!(cache.pending_count(), 1);
    cache.complete(&token, b"a".to_vec()).unwrap();
    assert_eq!(cache.get(&asset).unwrap(), Some(b"a".as_slice()));
    let failed_refresh = cache.fetch(&asset).unwrap();
    assert_eq!(
        cache.complete(&failed_refresh, b"x".to_vec()),
        Err(CacheError::HashMismatch)
    );
    assert_eq!(cache.get(&asset).unwrap(), Some(b"a".as_slice()));
}

#[test]
fn recovery_epoch_change_fences_fetch_even_when_the_key_is_unchanged() {
    let asset = key("a", b"a");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(1, 99), std::slice::from_ref(&asset))
        .unwrap();
    let old = cache.fetch(&asset).unwrap();
    cache
        .apply_current(scope(3), revision(2, 0), std::slice::from_ref(&asset))
        .unwrap();
    let current = cache.fetch(&asset).unwrap();
    assert_eq!(
        cache.complete(&old, b"a".to_vec()),
        Err(CacheError::StaleFetch)
    );
    assert_eq!(cache.pending_count(), 1);
    cache.complete(&current, b"a".to_vec()).unwrap();
}

#[test]
fn consumer_lease_requires_complete_verified_resident_bytes() {
    let asset = key("a", b"abc");
    let mut cache = cache(8, 1);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    assert!(cache.acquire(&asset).unwrap().is_none());
    let partial = cache.fetch(&asset).unwrap();
    assert!(cache.acquire(&asset).unwrap().is_none());
    assert_eq!(
        cache.complete(&partial, b"ab".to_vec()),
        Err(CacheError::Incomplete)
    );
    assert!(cache.acquire(&asset).unwrap().is_none());
    publish(&mut cache, &asset, b"abc");
    let consumer = cache.acquire(&asset).unwrap().unwrap();
    assert_eq!(consumer.key(), &asset);
    assert_eq!(cache.lease_bytes(&consumer).unwrap(), b"abc");
    assert_eq!(cache.resident_bytes(), 3);
    assert_eq!(cache.lease_count(), 1);
}

#[test]
fn consumers_are_bounded_without_owning_or_pinning_byte_clones() {
    let asset = key("a", b"abc");
    let mut cache = AssetCache::new(
        scope(3),
        CacheLimits {
            max_assets: 1,
            max_pending: 1,
            max_leases: 1,
            max_bytes: 3,
        },
    )
    .unwrap();
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    publish(&mut cache, &asset, b"abc");
    let consumer = cache.acquire(&asset).unwrap().unwrap();
    let clone = consumer.clone();
    assert!(matches!(
        cache.acquire(&asset),
        Err(CacheError::LeaseCapacity)
    ));
    drop(consumer);
    assert_eq!(cache.lease_count(), 1);
    assert!(matches!(
        cache.acquire(&asset),
        Err(CacheError::LeaseCapacity)
    ));
    drop(clone);
    assert_eq!(cache.lease_count(), 0);
    let replacement = cache.acquire(&asset).unwrap().unwrap();
    assert_eq!(cache.lease_bytes(&replacement).unwrap(), b"abc");
    assert_eq!(cache.resident_bytes(), 3);
}

#[test]
fn lease_release_invalidates_clones_and_preserves_other_consumer() {
    let asset = key("a", b"a");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    publish(&mut cache, &asset, b"a");
    let first = cache.acquire(&asset).unwrap().unwrap();
    let clone = first.clone();
    let second = cache.acquire(&asset).unwrap().unwrap();
    cache.release_lease(&clone).unwrap();
    assert_eq!(cache.lease_bytes(&first), Err(CacheError::StaleLease));
    assert_eq!(cache.release_lease(&first), Err(CacheError::StaleLease));
    assert_eq!(cache.lease_bytes(&second).unwrap(), b"a");
    assert_eq!(cache.lease_count(), 1);
    assert_eq!(cache.resident_bytes(), 1);
}

#[test]
fn revocation_invalidates_consumer_before_repeated_identity_can_be_reused() {
    let asset = key("a", b"a");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    publish(&mut cache, &asset, b"a");
    let consumer = cache.acquire(&asset).unwrap().unwrap();
    assert_eq!(cache.lease_bytes(&consumer).unwrap(), b"a");
    cache.apply_current(scope(3), revision(1, 2), &[]).unwrap();
    assert_eq!(cache.lease_bytes(&consumer), Err(CacheError::StaleLease));
    assert_eq!(cache.resident_bytes(), 0);
    assert_eq!(cache.lease_count(), 0);
    cache
        .apply_current(scope(3), revision(1, 3), std::slice::from_ref(&asset))
        .unwrap();
    publish(&mut cache, &asset, b"a");
    let replacement = cache.acquire(&asset).unwrap().unwrap();
    assert_eq!(cache.lease_bytes(&consumer), Err(CacheError::StaleLease));
    assert_eq!(cache.lease_bytes(&replacement).unwrap(), b"a");
}

#[test]
fn eviction_frees_bytes_even_while_consumer_holds_old_lease() {
    let a = key("a", b"aa");
    let b = key("b", b"bb");
    let mut cache = cache(2, 1);
    cache
        .apply_current(scope(3), revision(1, 1), &[a.clone(), b.clone()])
        .unwrap();
    publish(&mut cache, &a, b"aa");
    let consumer = cache.acquire(&a).unwrap().unwrap();
    publish(&mut cache, &b, b"bb");
    assert_eq!(cache.resident_bytes(), 2);
    assert_eq!(cache.lease_count(), 0);
    assert_eq!(cache.lease_bytes(&consumer), Err(CacheError::StaleLease));
    publish(&mut cache, &a, b"aa");
    assert_eq!(cache.lease_bytes(&consumer), Err(CacheError::StaleLease));
    let replacement = cache.acquire(&a).unwrap().unwrap();
    assert_eq!(cache.lease_bytes(&replacement).unwrap(), b"aa");
}

#[test]
fn released_fetch_late_completion_cannot_invalidate_replacement_consumer() {
    let asset = key("a", b"a");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    publish(&mut cache, &asset, b"a");
    let old_consumer = cache.acquire(&asset).unwrap().unwrap();
    let stale_fetch = cache.fetch(&asset).unwrap();
    cache.release(&asset).unwrap();
    assert_eq!(
        cache.lease_bytes(&old_consumer),
        Err(CacheError::StaleLease)
    );
    publish(&mut cache, &asset, b"a");
    let consumer = cache.acquire(&asset).unwrap().unwrap();
    assert_eq!(
        cache.complete(&stale_fetch, b"a".to_vec()),
        Err(CacheError::StaleFetch)
    );
    assert_eq!(cache.lease_bytes(&consumer).unwrap(), b"a");
    let corrupt_refresh = cache.fetch(&asset).unwrap();
    assert_eq!(
        cache.complete(&corrupt_refresh, b"x".to_vec()),
        Err(CacheError::HashMismatch)
    );
    assert_eq!(cache.lease_bytes(&consumer).unwrap(), b"a");
}

#[test]
fn disposal_and_cache_reconstruction_cannot_revive_consumer_lease() {
    let asset = key("a", b"a");
    let mut original = cache(4, 1);
    original
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    publish(&mut original, &asset, b"a");
    let consumer = original.acquire(&asset).unwrap().unwrap();
    original.dispose();
    assert_eq!(original.resident_bytes(), 0);
    assert_eq!(original.lease_count(), 0);
    assert_eq!(original.lease_bytes(&consumer), Err(CacheError::Closed));
    assert_eq!(original.release_lease(&consumer), Err(CacheError::Closed));
    let mut replacement = cache(4, 1);
    replacement
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    publish(&mut replacement, &asset, b"a");
    let current = replacement.acquire(&asset).unwrap().unwrap();
    assert_eq!(
        replacement.lease_bytes(&consumer),
        Err(CacheError::StaleLease)
    );
    assert_eq!(replacement.lease_bytes(&current).unwrap(), b"a");
}

#[test]
fn consumer_lifetime_survives_same_epoch_but_not_recovery_epoch() {
    let asset = key("a", b"a");
    let mut cache = cache(4, 1);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    publish(&mut cache, &asset, b"a");
    let consumer = cache.acquire(&asset).unwrap().unwrap();
    cache
        .apply_current(scope(3), revision(1, 2), std::slice::from_ref(&asset))
        .unwrap();
    assert_eq!(cache.lease_bytes(&consumer).unwrap(), b"a");
    let refresh = cache.fetch(&asset).unwrap();
    cache.complete(&refresh, b"a".to_vec()).unwrap();
    assert_eq!(cache.lease_bytes(&consumer).unwrap(), b"a");
    cache
        .apply_current(scope(3), revision(2, 0), std::slice::from_ref(&asset))
        .unwrap();
    assert_eq!(cache.lease_bytes(&consumer), Err(CacheError::StaleLease));
    assert_eq!(cache.resident_bytes(), 1);
    let current = cache.acquire(&asset).unwrap().unwrap();
    assert_eq!(cache.lease_bytes(&current).unwrap(), b"a");
}

fn assert_current_manifest_conflict_is_atomic(conflicting: CacheKey) {
    let retained = key("immutable-version", b"old");
    let pending = key("pending-version", b"job");
    let proposed = key("fresh-version", b"fresh");
    let mut cache = cache(16, 2);
    cache
        .apply_current(
            scope(3),
            revision(1, 1),
            &[retained.clone(), pending.clone()],
        )
        .unwrap();
    publish(&mut cache, &retained, b"old");
    let lease = cache.acquire(&retained).unwrap().unwrap();
    let token = cache.fetch(&pending).unwrap();

    // This proposal would replace resident bytes/lease, remove pending work, and add a key.
    assert_eq!(
        cache.apply_current(
            scope(3),
            revision(1, 2),
            &[conflicting.clone(), proposed.clone()]
        ),
        Err(CacheError::ConflictingReference)
    );
    assert_eq!(cache.scope(), scope(3));
    assert_eq!(cache.resident_bytes(), 3);
    assert_eq!(cache.lease_count(), 1);
    assert_eq!(cache.pending_count(), 1);
    assert_eq!(cache.get(&retained).unwrap(), Some(b"old".as_slice()));
    assert_eq!(cache.lease_bytes(&lease).unwrap(), b"old");
    assert_eq!(cache.get(&conflicting), Err(CacheError::NotCurrent));
    assert_eq!(cache.fetch(&proposed).err(), Some(CacheError::NotCurrent));
    assert_eq!(
        cache.fetch(&pending).err(),
        Some(CacheError::AlreadyPending)
    );

    // The same requested revision would be stale if rejection advanced the watermark.
    cache
        .apply_current(
            scope(3),
            revision(1, 2),
            &[retained.clone(), pending.clone()],
        )
        .unwrap();
    assert_eq!(cache.lease_bytes(&lease).unwrap(), b"old");
    assert_eq!(cache.pending_count(), 1);
    cache.complete(&token, b"job".to_vec()).unwrap();
    assert_eq!(cache.get(&pending).unwrap(), Some(b"job".as_slice()));
    assert_eq!(cache.lease_bytes(&lease).unwrap(), b"old");
    assert_eq!(cache.resident_bytes(), 6);
}

#[test]
fn digest_only_current_version_conflict_preserves_bytes_lease_work_and_revision() {
    let original = key("immutable-version", b"old");
    let conflicting = key("immutable-version", b"new");
    assert_eq!(original.bytes.byte_len, conflicting.bytes.byte_len);
    assert_ne!(original.bytes.sha256, conflicting.bytes.sha256);
    assert_current_manifest_conflict_is_atomic(conflicting);
}

#[test]
fn length_only_current_version_conflict_preserves_bytes_lease_work_and_revision() {
    let original = key("immutable-version", b"old");
    let mut conflicting = original.clone();
    conflicting.bytes.byte_len += 1;
    assert_eq!(original.bytes.sha256, conflicting.bytes.sha256);
    assert_ne!(original.bytes.byte_len, conflicting.bytes.byte_len);
    assert_current_manifest_conflict_is_atomic(conflicting);
}

#[test]
fn identical_current_manifest_and_fresh_version_retain_existing_consumers_and_work() {
    let retained = key("immutable-version", b"old");
    let pending = key("pending-version", b"job");
    let fresh = key("fresh-version", b"new");
    let mut cache = cache(16, 2);
    cache
        .apply_current(
            scope(3),
            revision(1, 1),
            &[retained.clone(), pending.clone()],
        )
        .unwrap();
    publish(&mut cache, &retained, b"old");
    let lease = cache.acquire(&retained).unwrap().unwrap();
    let token = cache.fetch(&pending).unwrap();

    cache
        .apply_current(
            scope(3),
            revision(1, 2),
            &[retained.clone(), pending.clone(), fresh.clone()],
        )
        .unwrap();
    assert_eq!(cache.resident_bytes(), 3);
    assert_eq!(cache.lease_count(), 1);
    assert_eq!(cache.pending_count(), 1);
    assert_eq!(cache.lease_bytes(&lease).unwrap(), b"old");
    cache.complete(&token, b"job".to_vec()).unwrap();
    publish(&mut cache, &fresh, b"new");
    assert_eq!(cache.get(&fresh).unwrap(), Some(b"new".as_slice()));
    assert_eq!(cache.lease_bytes(&lease).unwrap(), b"old");
    assert_eq!(cache.resident_bytes(), 9);
}

#[test]
fn recovery_version_conflict_is_atomic_but_identical_epoch_update_fences_old_work() {
    let retained = key("immutable-version", b"old");
    let pending = key("pending-version", b"job");
    let conflicting = key("immutable-version", b"new");
    let mut cache = cache(16, 2);
    cache
        .apply_current(
            scope(3),
            revision(1, 1),
            &[retained.clone(), pending.clone()],
        )
        .unwrap();
    publish(&mut cache, &retained, b"old");
    let lease = cache.acquire(&retained).unwrap().unwrap();
    let usable = cache.fetch(&pending).unwrap();

    assert_eq!(
        cache.apply_current(scope(3), revision(2, 0), &[conflicting, pending.clone()]),
        Err(CacheError::ConflictingReference)
    );
    assert_eq!(cache.resident_bytes(), 3);
    assert_eq!(cache.pending_count(), 1);
    assert_eq!(cache.lease_bytes(&lease).unwrap(), b"old");
    cache.complete(&usable, b"job".to_vec()).unwrap();
    let old_epoch = cache.fetch(&pending).unwrap();

    // Correct the refused manifest at the same requested recovery revision.
    cache
        .apply_current(
            scope(3),
            revision(2, 0),
            &[retained.clone(), pending.clone()],
        )
        .unwrap();
    assert_eq!(cache.pending_count(), 0);
    assert_eq!(cache.lease_count(), 0);
    assert_eq!(cache.lease_bytes(&lease), Err(CacheError::StaleLease));
    assert_eq!(
        cache.complete(&old_epoch, b"job".to_vec()),
        Err(CacheError::StaleFetch)
    );
    assert_eq!(cache.get(&retained).unwrap(), Some(b"old".as_slice()));
    let current_lease = cache.acquire(&retained).unwrap().unwrap();
    assert_eq!(cache.lease_bytes(&current_lease).unwrap(), b"old");
}

#[test]
fn unfetched_current_version_manifest_conflict_cannot_discard_its_pending_token() {
    let original = key("immutable-version", b"old");
    let conflicting = key("immutable-version", b"new");
    let mut cache = cache(16, 1);
    cache
        .apply_current(scope(3), revision(1, 1), std::slice::from_ref(&original))
        .unwrap();
    let pending = cache.fetch(&original).unwrap();
    assert_eq!(cache.resident_bytes(), 0);
    assert_eq!(
        cache.apply_current(scope(3), revision(1, 2), std::slice::from_ref(&conflicting)),
        Err(CacheError::ConflictingReference)
    );
    assert_eq!(cache.pending_count(), 1);
    cache.complete(&pending, b"old".to_vec()).unwrap();
    assert_eq!(cache.get(&original).unwrap(), Some(b"old".as_slice()));
    cache
        .apply_current(scope(3), revision(1, 2), std::slice::from_ref(&original))
        .unwrap();
}

#[test]
fn private_bytes_and_fetch_ownership_stay_in_their_cache_scope() {
    let asset = key("private-v1", b"same private bytes");
    let first_scope = scope(3);
    let second_scope = scope(4);
    let mut first = cache_in_scope(first_scope, 64, 1);
    let mut second = cache_in_scope(second_scope, 64, 1);
    first
        .apply_current(first_scope, revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();
    second
        .apply_current(second_scope, revision(1, 1), std::slice::from_ref(&asset))
        .unwrap();

    let first_fetch = first.fetch(&asset).unwrap();
    assert_eq!(
        second.complete(&first_fetch, b"same private bytes".to_vec()),
        Err(CacheError::StaleFetch)
    );
    assert_eq!(first.pending_count(), 1);
    assert_eq!(second.pending_count(), 0);
    first
        .complete(&first_fetch, b"same private bytes".to_vec())
        .unwrap();
    assert_eq!(
        first.get(&asset).unwrap(),
        Some(b"same private bytes".as_slice())
    );
    assert_eq!(second.get(&asset).unwrap(), None);

    let second_fetch = second.fetch(&asset).unwrap();
    assert_eq!(first.cancel(&second_fetch), Err(CacheError::StaleFetch));
    assert_eq!(second.pending_count(), 1);
    second
        .complete(&second_fetch, b"same private bytes".to_vec())
        .unwrap();

    first.release(&asset).unwrap();
    assert_eq!(first.get(&asset).unwrap(), None);
    assert_eq!(
        second.get(&asset).unwrap(),
        Some(b"same private bytes".as_slice())
    );
}

#[test]
fn release_only_fences_its_key_and_allows_a_fresh_fetch() {
    let first_asset = key("first", b"first bytes");
    let second_asset = key("second", b"second bytes");
    let mut cache = cache(64, 2);
    cache
        .apply_current(
            scope(3),
            revision(1, 1),
            &[first_asset.clone(), second_asset.clone()],
        )
        .unwrap();
    publish(&mut cache, &first_asset, b"first bytes");
    publish(&mut cache, &second_asset, b"second bytes");
    let stale_fetch = cache.fetch(&first_asset).unwrap();
    let retained_fetch = cache.fetch(&second_asset).unwrap();

    cache.release(&first_asset).unwrap();
    assert_eq!(cache.get(&first_asset).unwrap(), None);
    assert_eq!(
        cache.get(&second_asset).unwrap(),
        Some(b"second bytes".as_slice())
    );
    assert_eq!(cache.pending_count(), 1);
    assert_eq!(
        cache.complete(&stale_fetch, b"first bytes".to_vec()),
        Err(CacheError::StaleFetch)
    );
    cache
        .complete(&retained_fetch, b"second bytes".to_vec())
        .unwrap();

    let fresh_fetch = cache.fetch(&first_asset).unwrap();
    cache
        .complete(&fresh_fetch, b"first bytes".to_vec())
        .unwrap();
    assert_eq!(
        cache.get(&first_asset).unwrap(),
        Some(b"first bytes".as_slice())
    );
    assert_eq!(
        cache.get(&second_asset).unwrap(),
        Some(b"second bytes".as_slice())
    );
}
