#![cfg(not(target_arch = "wasm32"))]
#[path = "support/resource_flat.rs"]
mod geometry;
#[path = "support/lifecycle.rs"]
mod support;

use df_client::cache::CacheError;
use df_render::{ImageDecodeError, ResourceError, ResourceLifecycleError, WorkOutcome};
use std::cell::Cell;
use std::rc::Rc;
use support::*;

fn initialized() -> (df_render::ResourceLifecycle, df_client::cache::CacheKey) {
    let mut owner = lifecycle();
    let key = key("first-png");
    owner
        .apply_current(scope(), revision(1, 0), std::slice::from_ref(&key))
        .unwrap();
    owner
        .update_scene(
            label("same-scene"),
            geometry::flat(revision(1, 0)),
            std::slice::from_ref(&key),
        )
        .unwrap();
    install(&mut owner, &key);
    (owner, key)
}

#[test]
fn canonical_byte_release_scrubs_decoded_surface_without_a_scene_revision() {
    let (mut owner, key) = initialized();
    let input = owner.prepare_png(&key, decode_limits()).unwrap();
    let token = owner.begin(&input, || {}).unwrap();
    owner
        .complete(
            &token,
            input
                .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        owner.get(&key).unwrap().unwrap().pixels().unwrap(),
        [16, 32, 48, 255]
    );
    assert_eq!(owner.decoded_bytes(), 4);
    owner.release(&key).unwrap();
    assert_eq!(owner.decoded_bytes(), 0);
    assert!(owner.get(&key).unwrap().is_none());
    assert_eq!(owner.resident_bytes(), 0);
    assert_eq!(owner.lease_count(), 0);
}

#[test]
fn exact_decoder_lease_revocation_cancels_but_keeps_work_until_real_terminal_failure() {
    let (mut owner, key) = initialized();
    let input = owner.prepare_png(&key, decode_limits()).unwrap();
    let budget = input.plan().budget.work_bytes;
    let aborts = Rc::new(Cell::new(0));
    let observed = aborts.clone();
    let token = owner
        .begin(&input, move || observed.set(observed.get() + 1))
        .unwrap();
    owner.release_lease(&input).unwrap();
    assert_eq!(aborts.get(), 1);
    assert_eq!(owner.work_bytes(), budget);
    assert_eq!(
        input.validate_current(),
        Err(ImageDecodeError::Lease(CacheError::StaleLease))
    );
    let next = owner.prepare_png(&key, decode_limits()).unwrap();
    assert_eq!(
        owner.begin(&next, || {}).err(),
        Some(ResourceLifecycleError::Resource(
            ResourceError::AlreadyPending
        ))
    );
    assert_eq!(owner.finish_failed(&token), Ok(WorkOutcome::Cancelled));
    assert_eq!(owner.work_bytes(), 0);
    let replacement = owner.begin(&next, || {}).unwrap();
    assert_eq!(owner.finish_failed(&token), Err(ResourceError::StaleDecode));
    assert_eq!(owner.work_bytes(), budget);
    owner.release(&key).unwrap();
    assert_eq!(
        owner.finish_failed(&replacement),
        Ok(WorkOutcome::Cancelled)
    );
    assert_eq!(owner.work_bytes(), 0);
}

#[test]
fn accepted_epoch_recovery_cancels_old_input_even_with_identical_current_key() {
    let (mut owner, key) = initialized();
    let input = owner.prepare_png(&key, decode_limits()).unwrap();
    let budget = input.plan().budget.work_bytes;
    let aborts = Rc::new(Cell::new(0));
    let observed = aborts.clone();
    let token = owner
        .begin(&input, move || observed.set(observed.get() + 1))
        .unwrap();
    assert_eq!(
        owner.apply_current(scope(), revision(1, 0), &[]),
        Err(ResourceLifecycleError::Cache(CacheError::StaleRevision))
    );
    assert_eq!(aborts.get(), 0);
    owner
        .apply_current(scope(), revision(2, 0), std::slice::from_ref(&key))
        .unwrap();
    assert_eq!(aborts.get(), 1);
    assert_eq!(owner.work_bytes(), budget);
    assert!(input.validate_current().is_err());
    assert_eq!(owner.finish_failed(&token), Ok(WorkOutcome::Cancelled));
}

#[test]
fn canonical_lru_eviction_scrubs_a_mounted_consumer_and_does_not_invent_access() {
    let mut owner = lifecycle();
    let first = key("first-png");
    let second = key("second-png");
    let references = [first.clone(), second.clone()];
    owner
        .apply_current(scope(), revision(1, 0), &references)
        .unwrap();
    owner
        .update_scene(
            label("same-scene"),
            geometry::flat(revision(1, 0)),
            &references,
        )
        .unwrap();
    install(&mut owner, &first);
    let input = owner.prepare_png(&first, decode_limits()).unwrap();
    let token = owner.begin(&input, || {}).unwrap();
    owner
        .complete(
            &token,
            input
                .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
                .unwrap(),
        )
        .unwrap();
    install(&mut owner, &second);
    assert_eq!(owner.decoded_bytes(), 0);
    assert!(owner.get(&first).unwrap().is_none());
    assert_eq!(owner.resident_bytes(), PNG.len());
    assert_eq!(
        owner.prepare_png(&first, decode_limits()).err(),
        Some(ImageDecodeError::MissingBytes)
    );
}

#[test]
fn rejected_fetch_and_wrong_scope_preserve_pixels_but_epoch_revocation_scrubs_them() {
    let (mut owner, key) = initialized();
    let input = owner.prepare_png(&key, decode_limits()).unwrap();
    let token = owner.begin(&input, || {}).unwrap();
    owner
        .complete(
            &token,
            input
                .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
                .unwrap(),
        )
        .unwrap();
    let fetch = owner.fetch(&key).unwrap();
    assert_eq!(
        owner.complete_fetch(&fetch, vec![0]),
        Err(ResourceLifecycleError::Cache(CacheError::Incomplete))
    );
    let mut wrong = scope();
    wrong.binding = df_types::ClientBindingId::from_bytes(&[9; 16]).unwrap();
    assert_eq!(
        owner.apply_current(wrong, revision(1, 1), &[]),
        Err(ResourceLifecycleError::Cache(CacheError::WrongScope))
    );
    assert_eq!(
        owner.get(&key).unwrap().unwrap().pixels().unwrap(),
        [16, 32, 48, 255]
    );
    owner
        .apply_current(scope(), revision(2, 0), std::slice::from_ref(&key))
        .unwrap();
    assert_eq!(owner.decoded_bytes(), 0);
    assert!(owner.get(&key).unwrap().is_none());
    assert_eq!(owner.lease_count(), 0);
}

#[test]
fn owner_replacement_disposes_old_scope_and_retains_original_pending_reservation() {
    let (mut owner, key) = initialized();
    let input = owner.prepare_png(&key, decode_limits()).unwrap();
    let budget = input.plan().budget.work_bytes;
    let token = owner.begin(&input, || {}).unwrap();
    assert_eq!(owner.replace_scope(scope()).unwrap(), None);
    let mut replacement = scope();
    replacement.binding = df_types::ClientBindingId::from_bytes(&[9; 16]).unwrap();
    assert_eq!(
        owner.replace_scope(replacement).unwrap(),
        Some(df_render::PresentationOutcome::Disposed)
    );
    assert_eq!(owner.resident_bytes(), 0);
    assert_eq!(owner.work_bytes(), budget);
    assert_eq!(
        owner.prepare_png(&key, decode_limits()).err(),
        Some(ImageDecodeError::Lease(CacheError::Closed))
    );
    assert_eq!(owner.finish_failed(&token), Ok(WorkOutcome::Cancelled));
    assert_eq!(owner.work_bytes(), 0);
    assert_eq!(
        owner.dispose().unwrap(),
        df_render::PresentationOutcome::AlreadyDisposed
    );
}

#[test]
fn a_real_png_input_from_another_canonical_owner_cannot_enter_or_revoke_this_owner() {
    let (mut first, first_key) = initialized();
    let (second, _) = initialized();
    let input = second.prepare_png(&first_key, decode_limits()).unwrap();
    assert_eq!(
        first.begin(&input, || {}).err(),
        Some(ResourceLifecycleError::Decode(ImageDecodeError::WrongOwner))
    );
    assert_eq!(
        first.release_lease(&input),
        Err(ResourceLifecycleError::Decode(ImageDecodeError::WrongOwner))
    );
    assert_eq!(first.work_bytes(), 0);
    assert_eq!(input.validate_current(), Ok(()));
}

#[test]
fn unrelated_resident_invalidation_preserves_current_pixels_and_reconciliation_preserves_lru() {
    let cache = df_client::cache::CacheLimits {
        max_assets: 3,
        max_bytes: PNG.len() * 3,
        ..two_asset_cache_limits()
    };
    let resources = df_render::ResourceLimits {
        max_references: 3,
        ..resource_limits()
    };
    let mut lifecycle = df_render::ResourceLifecycle::from_scene(
        df_render::SceneRenderer::new(owner()),
        scope(),
        cache,
        resources,
        label("bounded-png-v1"),
    )
    .unwrap();
    let current = key("current-png");
    let unrelated = key("unrelated-png");
    let replacement = key("replacement-png");
    let references = [current.clone(), unrelated.clone(), replacement.clone()];
    lifecycle
        .apply_current(scope(), revision(1, 0), &references)
        .unwrap();
    lifecycle
        .update_scene(
            label("same-scene"),
            geometry::flat(revision(1, 0)),
            &references,
        )
        .unwrap();
    for key in &references {
        install(&mut lifecycle, key);
    }
    for key in [&current, &unrelated] {
        let input = lifecycle.prepare_png(key, decode_limits()).unwrap();
        let token = lifecycle.begin(&input, || {}).unwrap();
        lifecycle
            .complete(
                &token,
                input
                    .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
                    .unwrap(),
            )
            .unwrap();
    }
    assert_eq!(lifecycle.decoded_bytes(), 8);
    assert_eq!(lifecycle.lease_count(), 2);
    assert_eq!(
        lifecycle.get(&current).unwrap().unwrap().pixels().unwrap(),
        [16, 32, 48, 255]
    );
    lifecycle.release(&unrelated).unwrap();
    assert_eq!(
        lifecycle.get(&current).unwrap().unwrap().pixels().unwrap(),
        [16, 32, 48, 255]
    );
    assert!(lifecycle.get(&unrelated).unwrap().is_none());
    assert_eq!(lifecycle.decoded_bytes(), 4);
    assert_eq!(lifecycle.lease_count(), 1);
    assert_eq!(lifecycle.resident_bytes(), PNG.len() * 2);
    assert_eq!(lifecycle.work_bytes(), 0);

    install(&mut lifecycle, &unrelated);
    let input = lifecycle.prepare_png(&unrelated, decode_limits()).unwrap();
    let token = lifecycle.begin(&input, || {}).unwrap();
    lifecycle
        .complete(
            &token,
            input
                .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
                .unwrap(),
        )
        .unwrap();
    // An explicit current-image read makes unrelated the LRU resident. A same-epoch
    // canonical lease sweep must not turn its registry traversal into decoded access.
    assert!(lifecycle.get(&current).unwrap().is_some());
    lifecycle
        .apply_current(scope(), revision(1, 1), &references)
        .unwrap();
    let input = lifecycle
        .prepare_png(&replacement, decode_limits())
        .unwrap();
    let token = lifecycle.begin(&input, || {}).unwrap();
    lifecycle
        .complete(
            &token,
            input
                .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
                .unwrap(),
        )
        .unwrap();
    assert!(lifecycle.get(&unrelated).unwrap().is_none());
    assert_eq!(
        lifecycle.get(&current).unwrap().unwrap().pixels().unwrap(),
        [16, 32, 48, 255]
    );
    assert!(lifecycle.get(&replacement).unwrap().is_some());
    assert_eq!(lifecycle.decoded_bytes(), 8);
    assert_eq!(lifecycle.lease_count(), 2);
    assert_eq!(lifecycle.resident_bytes(), PNG.len() * 3);
    assert_eq!(lifecycle.work_bytes(), 0);
}

#[test]
fn late_old_success_cannot_retire_the_replacement_pending_lease_or_work() {
    let (mut owner, key) = initialized();
    let old_input = owner.prepare_png(&key, decode_limits()).unwrap();
    let old_token = owner.begin(&old_input, || {}).unwrap();
    owner.cancel(&old_token).unwrap();
    let late_image = old_input
        .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
        .unwrap();
    assert_eq!(owner.finish_failed(&old_token), Ok(WorkOutcome::Cancelled));
    let replacement_input = owner.prepare_png(&key, decode_limits()).unwrap();
    let work = replacement_input.plan().budget.work_bytes;
    let aborts = Rc::new(Cell::new(0));
    let observed = aborts.clone();
    let replacement = owner
        .begin(&replacement_input, move || observed.set(observed.get() + 1))
        .unwrap();
    assert_eq!(
        owner.complete(&old_token, late_image),
        Err(ResourceError::StaleDecode)
    );
    assert_eq!(owner.work_bytes(), work);
    assert_eq!(owner.lease_count(), 1);
    assert_eq!(replacement_input.validate_current(), Ok(()));
    // This invalidation must still find the replacement's pending lease registry entry.
    owner.release_lease(&replacement_input).unwrap();
    assert_eq!(aborts.get(), 1);
    assert_eq!(owner.work_bytes(), work);
    assert_eq!(
        owner.finish_failed(&replacement),
        Ok(WorkOutcome::Cancelled)
    );
    assert_eq!(owner.work_bytes(), 0);
}

#[test]
fn foreign_same_key_and_unbound_surfaces_cannot_consume_the_original_pending_operation() {
    let (mut owner, key) = initialized();
    let (foreign, _) = initialized();
    let input = owner.prepare_png(&key, decode_limits()).unwrap();
    let work = input.plan().budget.work_bytes;
    let aborts = Rc::new(Cell::new(0));
    let observed = aborts.clone();
    let token = owner
        .begin(&input, move || observed.set(observed.get() + 1))
        .unwrap();
    let foreign_input = foreign.prepare_png(&key, decode_limits()).unwrap();
    let foreign_image = foreign_input
        .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
        .unwrap();
    assert_eq!(
        owner.complete(&token, foreign_image),
        Err(ResourceError::WrongResource)
    );
    assert_eq!(owner.work_bytes(), work);
    assert_eq!(owner.decoded_bytes(), 0);
    assert_eq!(owner.lease_count(), 1);
    assert_eq!(foreign.lease_count(), 0);
    assert_eq!(input.validate_current(), Ok(()));
    assert_eq!(aborts.get(), 0);

    let released = Rc::new(Cell::new(0));
    let retire = released.clone();
    let unbound = df_render::DecodedImage::new(
        key.clone(),
        1,
        1,
        vec![16, 32, 48, 255].into_boxed_slice(),
        |_| true,
        move || retire.set(retire.get() + 1),
    )
    .unwrap();
    assert_eq!(
        owner.complete(&token, unbound),
        Err(ResourceError::WrongResource)
    );
    assert_eq!(released.get(), 1);
    assert_eq!(owner.work_bytes(), work);
    assert_eq!(owner.decoded_bytes(), 0);
    assert_eq!(owner.lease_count(), 1);
    assert_eq!(input.validate_current(), Ok(()));
    assert_eq!(aborts.get(), 0);

    // Both refused objects retire, while the exact admitted original input can still
    // produce and publish its genuine terminal surface with one canonical lease slot.
    let image = input
        .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
        .unwrap();
    owner.complete(&token, image).unwrap();
    assert_eq!(owner.work_bytes(), 0);
    assert_eq!(owner.decoded_bytes(), 4);
    assert_eq!(owner.lease_count(), 1);
    assert_eq!(
        owner.get(&key).unwrap().unwrap().pixels().unwrap(),
        [16, 32, 48, 255]
    );
    owner.release(&key).unwrap();
    assert_eq!(owner.decoded_bytes(), 0);
    assert_eq!(owner.lease_count(), 0);
}

#[test]
fn prepared_description_uses_the_exact_existing_lifecycle_lease_and_refuses_foreign_completion() {
    let mut owner = support::lifecycle();
    let key = support::key("prepared-description");
    owner
        .apply_current(
            support::scope(),
            support::revision(1, 0),
            std::slice::from_ref(&key),
        )
        .unwrap();
    owner
        .update_scene(
            support::label("prepared-scene"),
            geometry::flat(support::revision(1, 0)),
            std::slice::from_ref(&key),
        )
        .unwrap();
    support::install(&mut owner, &key);
    let metadata = df_render::PreparedImageMetadata {
        mime: "image/png",
        width: 1,
        height: 1,
        max_ancillary_bytes: 0,
    };
    let input = owner
        .prepare_image(&key, metadata, support::decode_limits())
        .unwrap();
    assert_eq!(owner.lease_count(), 1);
    let token = owner.begin_image(&input, || {}).unwrap();
    let foreign = owner
        .prepare_image(&key, metadata, support::decode_limits())
        .unwrap();
    let foreign = foreign
        .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
        .unwrap();
    assert_eq!(
        owner.complete(&token, foreign),
        Err(df_render::ResourceError::WrongResource)
    );
    assert_eq!(owner.work_bytes(), 244);
    assert_eq!(owner.lease_count(), 1);
    let exact = input
        .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
        .unwrap();
    owner.complete(&token, exact).unwrap();
    assert_eq!(owner.work_bytes(), 0);
    assert_eq!(owner.decoded_bytes(), 4);
    assert_eq!(owner.lease_count(), 1);
    owner.release(&key).unwrap();
    assert_eq!(owner.decoded_bytes(), 0);
    assert_eq!(owner.lease_count(), 0);
    owner.dispose().unwrap();
}

#[test]
fn prepared_metadata_refusal_cannot_retain_a_lease_or_reserve_codec_work() {
    // Preflight temporarily acquires the canonical lease; refusal drops it before return.
    let mut owner = support::lifecycle();
    let key = support::key("prepared-refusal");
    owner
        .apply_current(
            support::scope(),
            support::revision(1, 0),
            std::slice::from_ref(&key),
        )
        .unwrap();
    owner
        .update_scene(
            support::label("prepared-scene"),
            geometry::flat(support::revision(1, 0)),
            std::slice::from_ref(&key),
        )
        .unwrap();
    support::install(&mut owner, &key);
    let mut metadata = df_render::PreparedImageMetadata {
        mime: "image/webp",
        width: 1,
        height: 1,
        max_ancillary_bytes: 0,
    };
    assert_eq!(
        owner
            .prepare_image(&key, metadata, support::decode_limits())
            .err(),
        Some(df_render::ImageDecodeError::MimeMismatch)
    );
    metadata.mime = "image/png";
    metadata.height = 2;
    assert_eq!(
        owner
            .prepare_image(&key, metadata, support::decode_limits())
            .err(),
        Some(df_render::ImageDecodeError::WrongDimensions)
    );
    assert_eq!(owner.lease_count(), 0);
    assert_eq!(owner.work_bytes(), 0);
    assert_eq!(owner.decoded_bytes(), 0);
    owner.dispose().unwrap();
}
