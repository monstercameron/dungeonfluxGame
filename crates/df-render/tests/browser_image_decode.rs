#![cfg(not(target_arch = "wasm32"))]
#[path = "support/resource_flat.rs"]
mod geometry;
#[path = "support/image_decode.rs"]
mod support;
use df_client::cache::{CacheError, CacheKey};
use df_render::{
    DecodedImage, ImageDecodeError, ResourceError, ResourceLimits, ResourceSceneRenderer,
    SceneRenderer, VerifiedPng, inspect_png,
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
