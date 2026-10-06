#![cfg(not(target_arch = "wasm32"))]
#[path = "support/resource_flat.rs"]
mod geometry;
#[path = "support/image_decode.rs"]
mod support;
use df_client::cache::{CacheError, CacheKey};
use df_render::{
    DecodedImage, ImageDecodeError, ResourceError, ResourceLimits, ResourceSceneRenderer,
    SceneRenderer, VerifiedImage, VerifiedPng, inspect_png,
};
use support::{PNG, PNG_DIGEST, cached, label, limits, owner, revision};

fn renderer(key: &CacheKey) -> ResourceSceneRenderer<DecodedImage<CacheKey>> {
    let mut scene = ResourceSceneRenderer::from_scene(
        SceneRenderer::new(owner()),
        owner(),
        ResourceLimits {
            max_references: 2,
            max_resident: 1,
            max_pending: 1,
            max_decoded_bytes: 4,
            max_work_bytes: 244,
        },
        label("png-rgba8-v1"),
    )
    .unwrap();
    scene
        .update(
            owner(),
            label("scene-one"),
            geometry::flat(revision(1, 0)),
            std::slice::from_ref(key),
        )
        .unwrap();
    scene
}

#[test]
fn real_png_framing_reserves_exact_encoded_pixel_and_scanline_work_before_copying() {
    let plan = inspect_png(PNG, limits()).unwrap();
    assert_eq!((plan.width, plan.height), (1, 1));
    assert_eq!(plan.budget.decoded_bytes, 4);
    assert_eq!(plan.budget.work_bytes, 244);
    let (bytes, key) = cached(PNG, PNG_DIGEST);
    let input = VerifiedPng::acquire(bytes.clone(), &key, limits()).unwrap();
    assert_eq!(input.plan(), plan);
    assert_eq!(bytes.borrow().resident_bytes(), 73);
    assert_eq!(bytes.borrow().lease_count(), 1);
    drop(input);
    assert_eq!(bytes.borrow().lease_count(), 0);
}

#[test]
fn malformed_framing_and_unsupported_formats_refuse_explicitly() {
    assert_eq!(
        inspect_png(&PNG[..72], limits()),
        Err(ImageDecodeError::CorruptPng)
    );
    let mut crc = PNG.to_vec();
    crc[29] ^= 1;
    assert_eq!(
        inspect_png(&crc, limits()),
        Err(ImageDecodeError::CorruptPng)
    );
    assert_eq!(
        inspect_png(support::UNSUPPORTED_PNG, limits()),
        Err(ImageDecodeError::UnsupportedPng)
    );
    // Structurally intact corrupt Deflate is a browser-codec case, never decoded success.
    assert!(inspect_png(support::CORRUPT_PNG, limits()).is_ok());
    let (bytes, key) = cached(support::CORRUPT_PNG, support::CORRUPT_DIGEST);
    let input = VerifiedPng::acquire(bytes.clone(), &key, limits()).unwrap();
    drop(input);
    assert_eq!(bytes.borrow().lease_count(), 0);
    let (bytes, key) = cached(support::TOO_WIDE_PNG, support::WIDE_DIGEST);
    assert_eq!(
        VerifiedPng::acquire(bytes, &key, limits()).err(),
        Some(ImageDecodeError::DimensionCapacity)
    );
}

#[test]
fn encoded_dimensions_decoded_and_work_bounds_refuse_before_allocation() {
    let mut bound = limits();
    bound.max_encoded_bytes = 72;
    assert_eq!(inspect_png(PNG, bound), Err(ImageDecodeError::ByteCapacity));
    assert_eq!(
        inspect_png(support::TOO_WIDE_PNG, limits()),
        Err(ImageDecodeError::DimensionCapacity)
    );
    bound = limits();
    bound.max_decoded_bytes = 3;
    assert_eq!(inspect_png(PNG, bound), Err(ImageDecodeError::ByteCapacity));
    bound = limits();
    bound.max_work_bytes = 243;
    assert_eq!(inspect_png(PNG, bound), Err(ImageDecodeError::ByteCapacity));
    bound = limits();
    bound.max_dimension = 0;
    assert_eq!(
        inspect_png(PNG, bound),
        Err(ImageDecodeError::InvalidLimits)
    );
}

#[test]
fn partial_or_hash_mismatched_cache_completion_cannot_become_decoder_input() {
    let (bytes, key) = cached(PNG, PNG_DIGEST);
    bytes.borrow_mut().release(&key).unwrap();
    let fetch = bytes.borrow_mut().fetch(&key).unwrap();
    assert_eq!(
        bytes.borrow_mut().complete(&fetch, PNG[..72].to_vec()),
        Err(CacheError::Incomplete)
    );
    assert_eq!(
        VerifiedPng::acquire(bytes.clone(), &key, limits()).err(),
        Some(ImageDecodeError::MissingBytes)
    );
    let fetch = bytes.borrow_mut().fetch(&key).unwrap();
    assert_eq!(
        bytes
            .borrow_mut()
            .complete(&fetch, support::CORRUPT_PNG.to_vec()),
        Err(CacheError::HashMismatch)
    );
    assert_eq!(
        VerifiedPng::acquire(bytes.clone(), &key, limits()).err(),
        Some(ImageDecodeError::MissingBytes)
    );
    assert_eq!(bytes.borrow().lease_count(), 0);
}

#[test]
fn revoked_lease_and_identical_refetch_cannot_authorize_old_completion() {
    let (bytes, key) = cached(PNG, PNG_DIGEST);
    let input = VerifiedPng::acquire(bytes.clone(), &key, limits()).unwrap();
    bytes.borrow_mut().release(&key).unwrap();
    let fetch = bytes.borrow_mut().fetch(&key).unwrap();
    bytes.borrow_mut().complete(&fetch, PNG.to_vec()).unwrap();
    assert_eq!(
        input
            .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
            .err(),
        Some(ImageDecodeError::Lease(CacheError::StaleLease))
    );
    assert_eq!(bytes.borrow().lease_count(), 0);
}

#[test]
fn wrong_dimensions_and_pixel_lengths_cannot_install_a_surface() {
    let (bytes, key) = cached(PNG, PNG_DIGEST);
    let input = VerifiedPng::acquire(bytes.clone(), &key, limits()).unwrap();
    assert_eq!(
        input.finish_rgba(2, 1, vec![0; 8].into_boxed_slice()).err(),
        Some(ImageDecodeError::WrongDimensions)
    );
    let input = VerifiedPng::acquire(bytes.clone(), &key, limits()).unwrap();
    assert_eq!(
        input.finish_rgba(1, 1, vec![0; 3].into_boxed_slice()).err(),
        Some(ImageDecodeError::ByteCapacity)
    );
    assert_eq!(bytes.borrow().lease_count(), 0);
}

#[test]
fn cancelled_work_stays_reserved_until_terminal_and_duplicate_cannot_release_newer_work() {
    let (bytes, key) = cached(PNG, PNG_DIGEST);
    let mut scene = renderer(&key);
    let input = VerifiedPng::acquire(bytes.clone(), &key, limits()).unwrap();
    let token = scene.begin(&key, input.plan().budget, || {}).unwrap();
    scene.cancel(&token).unwrap();
    assert_eq!(scene.work_bytes(), 244);
    assert_eq!(
        scene.begin(&key, input.plan().budget, || {}).err(),
        Some(ResourceError::AlreadyPending)
    );
    let image = input
        .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
        .unwrap();
    assert_eq!(scene.complete(&token, image), Err(ResourceError::Cancelled));
    assert_eq!(scene.work_bytes(), 0);
    assert_eq!(scene.decoded_bytes(), 0);
    assert_eq!(bytes.borrow().lease_count(), 0);
    let next_input = VerifiedPng::acquire(bytes.clone(), &key, limits()).unwrap();
    let next = scene.begin(&key, next_input.plan().budget, || {}).unwrap();
    let duplicate = VerifiedPng::acquire(bytes.clone(), &key, limits()).unwrap();
    let duplicate = duplicate
        .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
        .unwrap();
    assert_eq!(
        scene.complete(&token, duplicate),
        Err(ResourceError::StaleDecode)
    );
    assert_eq!(scene.work_bytes(), 244);
    scene
        .complete(
            &next,
            next_input
                .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        scene.get(&key).unwrap().unwrap().pixels().unwrap(),
        [16, 32, 48, 255]
    );
    scene.dispose().unwrap();
    scene.dispose().unwrap();
    assert_eq!(bytes.borrow().lease_count(), 0);
}

#[test]
fn generation_replacement_and_disposal_refuse_late_surface_and_cleanup_once() {
    let (bytes, key) = cached(PNG, PNG_DIGEST);
    let mut scene = renderer(&key);
    let input = VerifiedPng::acquire(bytes.clone(), &key, limits()).unwrap();
    let token = scene.begin(&key, input.plan().budget, || {}).unwrap();
    scene
        .update(
            owner(),
            label("scene-two"),
            geometry::flat(revision(2, 0)),
            std::slice::from_ref(&key),
        )
        .unwrap();
    assert_eq!(scene.work_bytes(), 244);
    assert_eq!(
        scene.complete(
            &token,
            input
                .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
                .unwrap()
        ),
        Err(ResourceError::Cancelled)
    );
    let input = VerifiedPng::acquire(bytes.clone(), &key, limits()).unwrap();
    let token = scene.begin(&key, input.plan().budget, || {}).unwrap();
    scene.dispose().unwrap();
    scene.dispose().unwrap();
    assert_eq!(scene.work_bytes(), 244);
    assert_eq!(
        scene.complete(
            &token,
            input
                .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
                .unwrap()
        ),
        Err(ResourceError::Closed)
    );
    assert_eq!(scene.work_bytes(), 0);
    assert_eq!(scene.pending_count(), 0);
    assert_eq!(bytes.borrow().lease_count(), 0);
}

#[test]
fn exact_four_prepared_public_files_bind_mime_dimensions_and_canonical_lease() {
    use df_assets::AssetManifest;
    use df_client::cache::{AssetCache, CacheLimits};
    use df_render::inspect_prepared_image;
    use sha2::{Digest, Sha256};
    for asset in support::PREPARED_ASSETS {
        assert!(asset.route.starts_with("/assets/"));
        assert_eq!(<[u8; 32]>::from(Sha256::digest(asset.bytes)), asset.digest);
        let plan =
            inspect_prepared_image(asset.bytes, asset.metadata(), support::prepared_limits())
                .unwrap();
        assert_eq!((plan.width, plan.height), (asset.width, asset.height));
        assert_eq!(
            plan.budget.decoded_bytes,
            asset.width as usize * asset.height as usize * 4
        );
        let key = asset.key();
        assert_eq!(
            key.bytes,
            AssetManifest {
                byte_len: asset.bytes.len() as u64,
                sha256: asset.digest
            }
        );
        let mut cache = AssetCache::new(
            support::scope(),
            CacheLimits {
                max_assets: 1,
                max_pending: 1,
                max_leases: 4,
                max_bytes: 4 * 1024 * 1024,
            },
        )
        .unwrap();
        cache
            .apply_current(support::scope(), revision(1, 0), std::slice::from_ref(&key))
            .unwrap();
        let fetch = cache.fetch(&key).unwrap();
        cache.complete(&fetch, asset.bytes.to_vec()).unwrap();
        let cache = std::rc::Rc::new(std::cell::RefCell::new(cache));
        let strict_error = if asset.mime == "image/png" {
            ImageDecodeError::UnsupportedPng
        } else {
            ImageDecodeError::CorruptPng
        };
        assert_eq!(
            VerifiedPng::acquire(cache.clone(), &key, support::prepared_limits()).err(),
            Some(strict_error)
        );
        assert_eq!(cache.borrow().lease_count(), 0);
        let input = VerifiedImage::acquire_prepared(
            cache.clone(),
            &key,
            asset.metadata(),
            support::prepared_limits(),
        )
        .unwrap();
        assert_eq!(input.mime(), asset.mime);
        assert_eq!(input.plan(), plan);
        assert_eq!(cache.borrow().lease_count(), 1);
        cache.borrow_mut().release(&key).unwrap();
        assert_eq!(
            input.validate_current(),
            Err(ImageDecodeError::Lease(CacheError::StaleLease))
        );
        drop(input);
        assert_eq!(cache.borrow().lease_count(), 0);
    }
}

#[test]
fn prepared_mime_dimensions_payload_and_allocation_limits_refuse_explicitly() {
    use df_render::inspect_prepared_image;
    for asset in support::PREPARED_ASSETS {
        let mut metadata = asset.metadata();
        metadata.mime = if asset.mime == "image/png" {
            "image/webp"
        } else {
            "image/png"
        };
        assert_eq!(
            inspect_prepared_image(asset.bytes, metadata, support::prepared_limits()),
            Err(ImageDecodeError::MimeMismatch)
        );
        metadata.mime = "image/gif";
        assert_eq!(
            inspect_prepared_image(asset.bytes, metadata, support::prepared_limits()),
            Err(ImageDecodeError::UnsupportedMime)
        );
        metadata = asset.metadata();
        metadata.width += 1;
        assert_eq!(
            inspect_prepared_image(asset.bytes, metadata, support::prepared_limits()),
            Err(ImageDecodeError::WrongDimensions)
        );
        let mut limits = support::prepared_limits();
        limits.max_encoded_bytes = asset.bytes.len() - 1;
        assert_eq!(
            inspect_prepared_image(asset.bytes, asset.metadata(), limits),
            Err(ImageDecodeError::ByteCapacity)
        );
        limits = support::prepared_limits();
        limits.max_dimension = asset.width - 1;
        assert_eq!(
            inspect_prepared_image(asset.bytes, asset.metadata(), limits),
            Err(ImageDecodeError::DimensionCapacity)
        );
        limits = support::prepared_limits();
        limits.max_decoded_bytes = asset.width as usize * asset.height as usize * 4 - 1;
        assert_eq!(
            inspect_prepared_image(asset.bytes, asset.metadata(), limits),
            Err(ImageDecodeError::ByteCapacity)
        );
        let plan =
            inspect_prepared_image(asset.bytes, asset.metadata(), support::prepared_limits())
                .unwrap();
        limits = support::prepared_limits();
        limits.max_work_bytes = plan.budget.work_bytes - 1;
        assert_eq!(
            inspect_prepared_image(asset.bytes, asset.metadata(), limits),
            Err(ImageDecodeError::ByteCapacity)
        );
    }
    let harbor = &support::PREPARED_ASSETS[0];
    let mut metadata = harbor.metadata();
    metadata.max_ancillary_bytes = 23_653;
    assert_eq!(
        inspect_prepared_image(harbor.bytes, metadata, support::prepared_limits()),
        Err(ImageDecodeError::AncillaryCapacity)
    );
    assert_eq!(
        inspect_png(harbor.bytes, support::prepared_limits()),
        Err(ImageDecodeError::UnsupportedPng)
    );
}

#[test]
fn actual_prepared_png_crc_and_webp_riff_truncation_overflow_and_frame_header_refuse() {
    use df_render::inspect_prepared_image;
    for asset in support::PREPARED_ASSETS {
        let expected = if asset.mime == "image/png" {
            ImageDecodeError::CorruptPng
        } else {
            ImageDecodeError::CorruptWebp
        };
        assert_eq!(
            inspect_prepared_image(
                &asset.bytes[..asset.bytes.len() - 1],
                asset.metadata(),
                support::prepared_limits()
            ),
            Err(expected)
        );
        let mut broken = asset.bytes.to_vec();
        if asset.mime == "image/png" {
            broken[29] ^= 1;
        } else {
            broken[23] ^= 1;
        }
        assert_eq!(
            inspect_prepared_image(&broken, asset.metadata(), support::prepared_limits()),
            Err(expected)
        );
        broken = asset.bytes.to_vec();
        if asset.mime == "image/png" {
            broken[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        } else {
            broken[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        }
        assert_eq!(
            inspect_prepared_image(&broken, asset.metadata(), support::prepared_limits()),
            Err(expected)
        );
        if asset.mime == "image/webp" {
            broken = asset.bytes.to_vec();
            broken[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
            assert_eq!(
                inspect_prepared_image(&broken, asset.metadata(), support::prepared_limits()),
                Err(expected)
            );
            broken = asset.bytes.to_vec();
            broken[12..16].copy_from_slice(b"VP8X");
            assert_eq!(
                inspect_prepared_image(&broken, asset.metadata(), support::prepared_limits()),
                Err(ImageDecodeError::UnsupportedWebp)
            );
            broken = asset.bytes.to_vec();
            broken[20] |= 1;
            assert_eq!(
                inspect_prepared_image(&broken, asset.metadata(), support::prepared_limits()),
                Err(ImageDecodeError::UnsupportedWebp)
            );
        }
    }
}

#[test]
fn exact_prepared_cache_refuses_partial_and_hash_mismatched_files_before_decoder_acquisition() {
    use df_client::cache::{AssetCache, CacheLimits};
    for asset in support::PREPARED_ASSETS {
        let key = asset.key();
        let mut cache = AssetCache::new(
            support::scope(),
            CacheLimits {
                max_assets: 1,
                max_pending: 1,
                max_leases: 4,
                max_bytes: 4 * 1024 * 1024,
            },
        )
        .unwrap();
        cache
            .apply_current(support::scope(), revision(1, 0), std::slice::from_ref(&key))
            .unwrap();
        let partial = cache.fetch(&key).unwrap();
        assert!(
            cache
                .complete(&partial, asset.bytes[..asset.bytes.len() - 1].to_vec())
                .is_err()
        );
        let cache = std::rc::Rc::new(std::cell::RefCell::new(cache));
        assert_eq!(
            VerifiedImage::acquire_prepared(
                cache.clone(),
                &key,
                asset.metadata(),
                support::prepared_limits()
            )
            .err(),
            Some(ImageDecodeError::MissingBytes)
        );
        let altered = cache.borrow_mut().fetch(&key).unwrap();
        let mut bytes = asset.bytes.to_vec();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(cache.borrow_mut().complete(&altered, bytes).is_err());
        assert_eq!(
            VerifiedImage::acquire_prepared(
                cache.clone(),
                &key,
                asset.metadata(),
                support::prepared_limits()
            )
            .err(),
            Some(ImageDecodeError::MissingBytes)
        );
        assert_eq!(cache.borrow().resident_bytes(), 0);
        assert_eq!(cache.borrow().lease_count(), 0);
        let complete = cache.borrow_mut().fetch(&key).unwrap();
        cache
            .borrow_mut()
            .complete(&complete, asset.bytes.to_vec())
            .unwrap();
        let input = VerifiedImage::acquire_prepared(
            cache.clone(),
            &key,
            asset.metadata(),
            support::prepared_limits(),
        )
        .unwrap();
        assert_eq!(cache.borrow().lease_count(), 1);
        drop(input);
        assert_eq!(cache.borrow().lease_count(), 0);
    }
}

#[test]
fn strict_png_wrapper_and_neutral_prepared_carrier_keep_distinct_plan_contracts() {
    use df_render::{
        ImageDecodePlan, PngDecodePlan, PreparedImageMetadata, inspect_prepared_image,
    };
    let metadata = PreparedImageMetadata {
        mime: "image/png",
        width: 1,
        height: 1,
        max_ancillary_bytes: 0,
    };
    let strict_plan: PngDecodePlan = inspect_png(PNG, limits()).unwrap();
    let neutral_plan: ImageDecodePlan = inspect_prepared_image(PNG, metadata, limits()).unwrap();
    assert_eq!(
        (strict_plan.width, strict_plan.height, strict_plan.budget),
        (neutral_plan.width, neutral_plan.height, neutral_plan.budget)
    );
    let (cache, key) = cached(PNG, PNG_DIGEST);
    let strict = VerifiedPng::acquire(cache.clone(), &key, limits()).unwrap();
    let neutral = VerifiedImage::acquire_prepared(cache.clone(), &key, metadata, limits()).unwrap();
    assert_eq!(strict.plan(), strict_plan);
    assert_eq!(neutral.plan(), neutral_plan);
    assert_eq!(strict.mime(), "image/png");
    assert_eq!(neutral.mime(), "image/png");
    assert_eq!(cache.borrow().lease_count(), 2);
    let strict = strict
        .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
        .unwrap();
    let neutral = neutral
        .finish_rgba(1, 1, vec![16, 32, 48, 255].into_boxed_slice())
        .unwrap();
    assert_eq!(strict.pixels().unwrap(), neutral.pixels().unwrap());
    cache.borrow_mut().release(&key).unwrap();
    assert!(strict.pixels().is_err());
    assert!(neutral.pixels().is_err());
    drop(strict);
    drop(neutral);
    assert_eq!(cache.borrow().lease_count(), 0);
}
