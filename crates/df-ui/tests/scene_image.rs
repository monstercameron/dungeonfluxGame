use df_client::cache::CacheLimits;
use df_ui::{SceneImageError, SceneImageLimits};

fn limits() -> SceneImageLimits {
    SceneImageLimits {
        cache: CacheLimits {
            max_assets: 2,
            max_pending: 1,
            max_leases: 1,
            max_bytes: 1024,
        },
        max_width: 64,
        max_height: 64,
        max_pixels: 4096,
    }
}

#[test]
fn zero_image_or_cache_bounds_reject_without_a_browser() {
    assert_eq!(limits().validate(), Ok(()));
    for invalid in [
        SceneImageLimits {
            max_width: 0,
            ..limits()
        },
        SceneImageLimits {
            max_height: 0,
            ..limits()
        },
        SceneImageLimits {
            max_pixels: 0,
            ..limits()
        },
        SceneImageLimits {
            cache: CacheLimits {
                max_bytes: 0,
                ..limits().cache
            },
            ..limits()
        },
        SceneImageLimits {
            cache: CacheLimits {
                max_leases: 0,
                ..limits().cache
            },
            ..limits()
        },
        SceneImageLimits {
            cache: CacheLimits {
                max_pending: 0,
                ..limits().cache
            },
            ..limits()
        },
        SceneImageLimits {
            cache: CacheLimits {
                max_assets: 0,
                ..limits().cache
            },
            ..limits()
        },
    ] {
        assert_eq!(invalid.validate(), Err(SceneImageError::InvalidLimits));
    }
}
