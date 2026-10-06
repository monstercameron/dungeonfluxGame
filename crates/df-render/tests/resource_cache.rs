#![cfg(not(target_arch = "wasm32"))]

#[path = "support/authorized_image.rs"]
mod support;
use df_assets::{AccessFailure, RangeError};
use df_client::cache::CacheKey;
use df_render::{
    DecodeBudget, DecodedImage, ResourceCache, ResourceError, ResourceLimits, ResourceReadiness,
    WorkOutcome,
};
use std::cell::Cell;
use std::rc::Rc;
use support::{Consumer, label, owner, revision};

fn cache(consumer: &Consumer) -> ResourceCache<DecodedImage<CacheKey>> {
    let mut cache = ResourceCache::new(
        owner(),
        ResourceLimits {
            max_references: 4,
            max_resident: 1,
            max_pending: 2,
            max_decoded_bytes: 8,
            max_work_bytes: 48,
        },
        label("rgba8-surface-v1"),
    )
    .unwrap();
    cache
        .apply_scene(
            owner(),
            revision(1, 0),
            label("scene-one"),
            std::slice::from_ref(&consumer.key),
        )
        .unwrap();
    cache
}
fn budget() -> DecodeBudget {
    DecodeBudget {
        decoded_bytes: 4,
        work_bytes: 24,
    }
}

#[test]
fn current_authorized_stream_delivers_separate_decoded_surface_and_retires_on_revocation() {
    let consumer = Consumer::new();
    let mut cache = cache(&consumer);
    assert!(matches!(
        consumer.read_current(2, 1),
        Err(RangeError::Access(AccessFailure::Denied))
    ));
    assert!(matches!(
        consumer.read_current(1, 2),
        Err(RangeError::Access(AccessFailure::Denied))
    ));
    let token = cache.begin(&consumer.key, budget(), || {}).unwrap();
    consumer.cache_streamed();
    cache.complete(&token, consumer.decode_fixture()).unwrap();
    assert_eq!(cache.decoded_bytes(), 4);
    assert_eq!(consumer.bytes.borrow().resident_bytes(), 16);
    assert_eq!(
        cache.get(&consumer.key).unwrap().unwrap().pixels().unwrap(),
        [16, 32, 48, 255]
    );
    consumer.authority.revoked.set(true);
    assert_eq!(
        cache.get(&consumer.key).err(),
        Some(ResourceError::RevokedResource)
    );
    assert_eq!(cache.decoded_bytes(), 0);
    assert_eq!(consumer.released.get(), 1);
}

#[test]
fn mid_stream_revocation_never_installs_cache_or_decoded_surface() {
    let consumer = Consumer::new();
    let mut cache = cache(&consumer);
    let token = cache.begin(&consumer.key, budget(), || {}).unwrap();
    consumer.authority.revoke_at.set(Some(6));
    assert!(matches!(
        consumer.read_current(1, 1),
        Err(RangeError::Access(AccessFailure::Denied))
    ));
    assert_eq!(cache.finish_failed(&token), Ok(WorkOutcome::Failed));
    assert_eq!(cache.decoded_bytes(), 0);
    assert_eq!(cache.work_bytes(), 0);
    assert_eq!(consumer.bytes.borrow().resident_bytes(), 0);
}

#[test]
fn cancellation_keeps_work_counted_until_terminal_and_cannot_admit_duplicate() {
    let consumer = Consumer::new();
    consumer.cache_streamed();
    let mut cache = cache(&consumer);
    let aborts = Rc::new(Cell::new(0));
    let observed = aborts.clone();
    let token = cache
        .begin(&consumer.key, budget(), move || {
            observed.set(observed.get() + 1)
        })
        .unwrap();
    cache.cancel(&token).unwrap();
    cache.cancel(&token).unwrap();
    assert_eq!(aborts.get(), 1);
    assert_eq!(cache.work_bytes(), 24);
    assert_eq!(cache.pending_count(), 1);
    assert_eq!(
        cache.begin(&consumer.key, budget(), || {}).err(),
        Some(ResourceError::AlreadyPending)
    );
    assert_eq!(
        cache.complete(&token, consumer.decode_fixture()),
        Err(ResourceError::Cancelled)
    );
    assert_eq!(cache.work_bytes(), 0);
    assert_eq!(consumer.released.get(), 1);
    let next = cache.begin(&consumer.key, budget(), || {}).unwrap();
    assert_eq!(
        cache.complete(&token, consumer.decode_fixture()),
        Err(ResourceError::StaleDecode)
    );
    assert_eq!(cache.pending_count(), 1);
    cache.complete(&next, consumer.decode_fixture()).unwrap();
}

#[test]
fn disposed_owner_consumes_late_surface_and_retains_only_bounded_terminal_work() {
    let consumer = Consumer::new();
    consumer.cache_streamed();
    let mut cache = cache(&consumer);
    let token = cache.begin(&consumer.key, budget(), || {}).unwrap();
    cache.dispose();
    cache.dispose();
    assert_eq!(cache.work_bytes(), 24);
    assert_eq!(cache.get(&consumer.key).err(), Some(ResourceError::Closed));
    assert_eq!(
        cache.complete(&token, consumer.decode_fixture()),
        Err(ResourceError::Closed)
    );
    assert_eq!(consumer.released.get(), 1);
    assert_eq!(cache.work_bytes(), 0);
    assert_eq!(cache.pending_count(), 0);
}

#[test]
fn failed_admission_and_wrong_or_oversized_surface_preserve_prior_resident() {
    let consumer = Consumer::new();
    consumer.cache_streamed();
    let mut cache = cache(&consumer);
    let first = cache.begin(&consumer.key, budget(), || {}).unwrap();
    cache.complete(&first, consumer.decode_fixture()).unwrap();
    let mut other = consumer.key.clone();
    other.version = label("other-canonical-key");
    cache
        .apply_scene(
            owner(),
            revision(1, 1),
            label("scene-one"),
            &[consumer.key.clone(), other.clone()],
        )
        .unwrap();
    let huge = DecodeBudget {
        decoded_bytes: 4,
        work_bytes: 49,
    };
    assert_eq!(
        cache.begin(&other, huge, || {}).err(),
        Some(ResourceError::ByteCapacity)
    );
    assert_eq!(
        cache.readiness(&consumer.key),
        Ok(ResourceReadiness::Resident)
    );
    assert_eq!(cache.decoded_bytes(), 4);
    let pending = cache.begin(&other, budget(), || {}).unwrap();
    assert_eq!(
        cache.complete(&pending, consumer.decode_fixture()),
        Err(ResourceError::WrongResource)
    );
    assert_eq!(cache.decoded_bytes(), 4);
    assert_eq!(cache.work_bytes(), 0);
    consumer.bytes.borrow_mut().release(&consumer.key).unwrap();
    consumer.cache_streamed();
    let pending = cache.begin(&consumer.key, budget(), || {}).unwrap();
    assert_eq!(
        cache.complete(
            &pending,
            consumer.decoded(consumer.key.clone(), vec![1; 8].into_boxed_slice())
        ),
        Err(ResourceError::ReservationExceeded)
    );
    assert_eq!(cache.decoded_bytes(), 4);
    assert_eq!(
        cache.get(&consumer.key).err(),
        Some(ResourceError::RevokedResource)
    );
}

#[test]
fn scene_recovery_rejects_old_callback_without_releasing_its_work_early() {
    let consumer = Consumer::new();
    consumer.cache_streamed();
    let mut cache = cache(&consumer);
    let token = cache.begin(&consumer.key, budget(), || {}).unwrap();
    cache
        .apply_scene(
            owner(),
            revision(2, 0),
            label("scene-one"),
            std::slice::from_ref(&consumer.key),
        )
        .unwrap();
    assert_eq!(cache.work_bytes(), 24);
    assert_eq!(
        cache.complete(&token, consumer.decode_fixture()),
        Err(ResourceError::Cancelled)
    );
    assert_eq!(cache.decoded_bytes(), 0);
    assert_eq!(cache.pending_count(), 0);
}

#[test]
fn malformed_surface_releases_actual_lease_and_no_pixels_escape() {
    let consumer = Consumer::new();
    consumer.cache_streamed();
    let lease = consumer
        .bytes
        .borrow_mut()
        .acquire(&consumer.key)
        .unwrap()
        .unwrap();
    let guard = lease.clone();
    let guard_cache = consumer.bytes.clone();
    let release_cache = consumer.bytes.clone();
    let released = Rc::new(Cell::new(0));
    let observed = released.clone();
    assert!(matches!(
        DecodedImage::new(
            consumer.key.clone(),
            u32::MAX,
            u32::MAX,
            vec![1; 4].into_boxed_slice(),
            move |key| guard.key() == key && guard_cache.borrow().lease_bytes(&guard).is_ok(),
            move || {
                release_cache.borrow_mut().release_lease(&lease).unwrap();
                observed.set(observed.get() + 1);
            }
        ),
        Err(ResourceError::ReservationExceeded)
    ));
    assert_eq!(released.get(), 1);
    assert_eq!(consumer.bytes.borrow().lease_count(), 0);
}

#[test]
fn successful_eviction_releases_only_lru_decoded_lease_and_keeps_verified_sources() {
    let first = Consumer::with_version("image-one");
    let second = Consumer::with_version("image-two");
    first.cache_streamed();
    second.cache_streamed();
    let mut cache = cache(&first);
    cache
        .apply_scene(
            owner(),
            revision(1, 1),
            label("scene-one"),
            &[first.key.clone(), second.key.clone()],
        )
        .unwrap();
    let one = cache.begin(&first.key, budget(), || {}).unwrap();
    cache.complete(&one, first.decode_fixture()).unwrap();
    let two = cache.begin(&second.key, budget(), || {}).unwrap();
    cache.complete(&two, second.decode_fixture()).unwrap();
    assert_eq!(cache.decoded_bytes(), 4);
    assert_eq!(first.released.get(), 1);
    assert_eq!(second.released.get(), 0);
    assert_eq!(cache.readiness(&first.key), Ok(ResourceReadiness::Missing));
    assert_eq!(
        cache.readiness(&second.key),
        Ok(ResourceReadiness::Resident)
    );
    assert_eq!(first.bytes.borrow().resident_bytes(), 16);
    assert_eq!(second.bytes.borrow().resident_bytes(), 16);
    cache.release(&second.key).unwrap();
    assert_eq!(second.released.get(), 1);
    assert_eq!(cache.decoded_bytes(), 0);
}

#[test]
fn pending_capacity_includes_cancelled_jobs_for_distinct_current_keys() {
    let consumer = Consumer::new();
    let mut cache = cache(&consumer);
    let mut second = consumer.key.clone();
    second.version = label("image-two");
    let mut third = consumer.key.clone();
    third.version = label("image-three");
    cache
        .apply_scene(
            owner(),
            revision(1, 1),
            label("scene-one"),
            &[consumer.key.clone(), second.clone(), third.clone()],
        )
        .unwrap();
    let one = cache.begin(&consumer.key, budget(), || {}).unwrap();
    let two = cache.begin(&second, budget(), || {}).unwrap();
    cache.cancel(&one).unwrap();
    assert_eq!(cache.work_bytes(), 48);
    assert_eq!(
        cache.begin(&third, budget(), || {}).err(),
        Some(ResourceError::PendingCapacity)
    );
    cache.finish_failed(&one).unwrap();
    assert_eq!(cache.work_bytes(), 24);
    let next = cache.begin(&third, budget(), || {}).unwrap();
    assert_eq!(cache.work_bytes(), 48);
    cache.finish_failed(&two).unwrap();
    cache.finish_failed(&next).unwrap();
    assert_eq!(cache.work_bytes(), 0);
}

#[test]
fn actual_byte_cache_eviction_and_access_basis_change_reject_cached_or_late_surfaces() {
    let consumer = Consumer::new();
    consumer.cache_streamed();
    let mut cache = cache(&consumer);
    let one = cache.begin(&consumer.key, budget(), || {}).unwrap();
    cache.complete(&one, consumer.decode_fixture()).unwrap();
    consumer.bytes.borrow_mut().release(&consumer.key).unwrap();
    assert_eq!(
        cache.get(&consumer.key).err(),
        Some(ResourceError::RevokedResource)
    );
    assert_eq!(consumer.released.get(), 1);
    consumer.cache_streamed();
    let two = cache.begin(&consumer.key, budget(), || {}).unwrap();
    let image = consumer.decode_fixture();
    consumer.authority.basis.set(2);
    assert_eq!(
        cache.complete(&two, image),
        Err(ResourceError::RevokedResource)
    );
    assert_eq!(cache.decoded_bytes(), 0);
    assert_eq!(consumer.released.get(), 2);
    assert_eq!(cache.work_bytes(), 0);
}

#[test]
fn recovery_retires_same_key_surface_instead_of_reusing_old_generation() {
    let consumer = Consumer::new();
    consumer.cache_streamed();
    let mut cache = cache(&consumer);
    let one = cache.begin(&consumer.key, budget(), || {}).unwrap();
    cache.complete(&one, consumer.decode_fixture()).unwrap();
    cache
        .apply_scene(
            owner(),
            revision(2, 0),
            label("scene-one"),
            std::slice::from_ref(&consumer.key),
        )
        .unwrap();
    assert_eq!(consumer.released.get(), 1);
    assert_eq!(cache.decoded_bytes(), 0);
    assert_eq!(
        cache.readiness(&consumer.key),
        Ok(ResourceReadiness::Missing)
    );
}

#[test]
fn rejected_updates_and_same_scene_revision_preserve_resident_and_actual_pending_job() {
    let consumer = Consumer::with_version("resident-image");
    let pending_source = Consumer::with_version("pending-image");
    consumer.cache_streamed();
    pending_source.cache_streamed();
    let mut cache = cache(&consumer);
    cache
        .apply_scene(
            owner(),
            revision(1, 1),
            label("scene-one"),
            &[consumer.key.clone(), pending_source.key.clone()],
        )
        .unwrap();
    let one = cache.begin(&consumer.key, budget(), || {}).unwrap();
    cache.complete(&one, consumer.decode_fixture()).unwrap();
    let pending = cache.begin(&pending_source.key, budget(), || {}).unwrap();
    assert_eq!(
        cache.apply_scene(owner(), revision(1, 0), label("scene-two"), &[]),
        Err(ResourceError::StaleRevision)
    );
    let mut wrong = owner();
    wrong.0 = df_types::SessionId::from_bytes(&[8; 16]).unwrap();
    assert_eq!(
        cache.apply_scene(wrong, revision(1, 2), label("scene-one"), &[]),
        Err(ResourceError::WrongOwner)
    );
    assert_eq!(
        cache.apply_scene(
            owner(),
            revision(1, 2),
            label("scene-one"),
            &[consumer.key.clone(), consumer.key.clone()]
        ),
        Err(ResourceError::DuplicateReference)
    );
    assert_eq!(cache.work_bytes(), 24);
    assert_eq!(cache.pending_count(), 1);
    assert_eq!(consumer.released.get(), 0);
    assert_eq!(
        cache.get(&consumer.key).unwrap().unwrap().pixels().unwrap(),
        [16, 32, 48, 255]
    );
    cache
        .apply_scene(
            owner(),
            revision(1, 2),
            label("scene-one"),
            &[consumer.key.clone(), pending_source.key.clone()],
        )
        .unwrap();
    assert_eq!(consumer.released.get(), 0);
    assert_eq!(cache.work_bytes(), 24);
    assert_eq!(
        cache.readiness(&consumer.key),
        Ok(ResourceReadiness::Resident)
    );
    cache
        .complete(&pending, pending_source.decode_fixture())
        .unwrap();
    assert_eq!(cache.work_bytes(), 0);
    assert_eq!(
        cache.readiness(&pending_source.key),
        Ok(ResourceReadiness::Resident)
    );
    assert_eq!(consumer.released.get(), 1);
}
