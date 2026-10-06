#![cfg(not(target_arch = "wasm32"))]
#[path = "support/campaign_png.rs"]
mod images;
use df_render::{
    ImageDecodeError, ImageDecodeLimits, PreparedImageMetadata, inspect_png, inspect_prepared_image,
};

fn limits() -> ImageDecodeLimits {
    ImageDecodeLimits {
        max_encoded_bytes: 4096,
        max_dimension: 8,
        max_decoded_bytes: 256,
        max_work_bytes: 16384,
    }
}
fn metadata() -> PreparedImageMetadata<'static> {
    PreparedImageMetadata {
        mime: "image/png",
        width: 1,
        height: 1,
        max_ancillary_bytes: 4096,
    }
}

#[test]
fn existing_gray_palette_sixteen_bit_and_adam7_png_layouts_admit_real_bounded_rgba_work() {
    for sample in images::IMAGES {
        let plan = inspect_prepared_image(sample.bytes, metadata(), limits()).expect(sample.name);
        assert_eq!((plan.width, plan.height), (1, 1));
        assert_eq!(plan.budget.decoded_bytes, 4);
        let expected = sample.bytes.len() * 3
            + 4 * 5
            + sample.source_bytes_per_pixel
            + 1
            + if sample.interlaced { 6 } else { 0 };
        assert_eq!(plan.budget.work_bytes, expected, "{}", sample.name);
        let computed: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(sample.bytes).into();
        assert_eq!(computed, sample.digest);
        assert_eq!(sample.pixel[3], 255);
    }
}
#[test]
fn source_sample_and_adam7_work_bound_refuses_one_byte_below_actual_plan() {
    for sample in images::IMAGES {
        let plan = inspect_prepared_image(sample.bytes, metadata(), limits()).expect(sample.name);
        let exact = ImageDecodeLimits {
            max_work_bytes: plan.budget.work_bytes,
            ..limits()
        };
        assert!(inspect_prepared_image(sample.bytes, metadata(), exact).is_ok());
        assert_eq!(
            inspect_prepared_image(
                sample.bytes,
                metadata(),
                ImageDecodeLimits {
                    max_work_bytes: plan.budget.work_bytes - 1,
                    ..exact
                }
            ),
            Err(ImageDecodeError::ByteCapacity)
        );
    }
}
#[test]
fn compressed_metadata_and_animation_are_explicit_static_png_refusals() {
    for (name, bytes, digest) in images::REFUSED {
        use sha2::{Digest, Sha256};
        let computed: [u8; 32] = Sha256::digest(bytes).into();
        assert_eq!(&computed, digest);
        assert_eq!(
            inspect_prepared_image(bytes, metadata(), limits()),
            Err(ImageDecodeError::UnsupportedPng),
            "{name}"
        );
    }
}
#[test]
fn legacy_strict_png_path_stays_strict_and_mismatched_declared_dimensions_refuse() {
    let gray = &images::IMAGES[0];
    assert_eq!(
        inspect_png(gray.bytes, limits()),
        Err(ImageDecodeError::UnsupportedPng)
    );
    assert_eq!(
        inspect_prepared_image(
            gray.bytes,
            PreparedImageMetadata {
                width: 2,
                ..metadata()
            },
            limits()
        ),
        Err(ImageDecodeError::WrongDimensions)
    );
}

#[test]
fn intact_crc_framing_never_claims_a_real_deflate_decode() {
    let (_, bytes, digest) = images::BAD_DEFLATE;
    let computed: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(bytes).into();
    assert_eq!(computed, digest);
    assert!(inspect_prepared_image(bytes, metadata(), limits()).is_ok());
    // Actual corrupt-Deflate rejection belongs to the mounted browser fixture.
}
