#[cfg(target_arch = "wasm32")]
#[path = "../support/resource_flat.rs"]
mod geometry;
#[cfg(target_arch = "wasm32")]
#[path = "../support/image_decode.rs"]
mod support;

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::{geometry, support};
    use df_client::cache::{AssetCache, CacheError, CacheKey, CacheLimits};
    use df_render::{
        BrowserDecodeStatus, BrowserImageDecode, BrowserResourceScene, ImageDecodeError,
        ImageSurfaceError, ResourceError, ResourceLifecycle, ResourceLimits, inspect_png,
        inspect_prepared_image,
    };
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
    use web_sys::{CanvasRenderingContext2d, Element, HtmlCanvasElement};

    struct Host(Element);
    impl Drop for Host {
        fn drop(&mut self) {
            self.0.remove();
        }
    }
    struct Fixture {
        host: Host,
        scene: Rc<RefCell<BrowserResourceScene<CacheKey>>>,
        cache: Rc<RefCell<AssetCache>>,
        key: CacheKey,
        canvas: HtmlCanvasElement,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.cache.borrow_mut().dispose();
            // Scene's own Drop releases canvas/mount; pending job retains scene to terminal.
        }
    }
    fn error(value: impl std::fmt::Debug) -> JsValue {
        JsValue::from_str(&format!("{value:?}"))
    }
    fn require(value: bool, message: &'static str) -> Result<(), JsValue> {
        if value {
            Ok(())
        } else {
            Err(JsValue::from_str(message))
        }
    }
    fn fixture(source: &[u8], digest: [u8; 32]) -> Result<Fixture, JsValue> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| error("document unavailable"))?;
        let host = Host(document.create_element("section")?);
        host.0
            .set_attribute("data-image-decode-fixture", "verified-png-canvas")?;
        document
            .body()
            .ok_or_else(|| error("body unavailable"))?
            .append_child(&host.0)?;
        let (cache, key) = support::cached(source, digest);
        let mut scene = BrowserResourceScene::mount(
            &host.0,
            support::owner(),
            ResourceLimits {
                max_references: 2,
                max_resident: 1,
                max_pending: 1,
                max_decoded_bytes: 4,
                max_work_bytes: 244,
            },
            support::label("png-rgba8-v1"),
            8,
        )
        .map_err(error)?;
        scene
            .update(
                support::owner(),
                support::label("scene-one"),
                geometry::flat(support::revision(1, 0)),
                std::slice::from_ref(&key),
            )
            .map_err(error)?;
        let canvas = host
            .0
            .query_selector("[data-scene-image-host] canvas")?
            .ok_or_else(|| error("connected canvas absent"))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(error)?;
        canvas.set_attribute("style", "width:96px;height:96px;image-rendering:pixelated")?;
        Ok(Fixture {
            host,
            scene: Rc::new(RefCell::new(scene)),
            cache,
            key,
            canvas,
        })
    }
    fn start(fixture: &Fixture) -> Result<BrowserImageDecode, JsValue> {
        BrowserImageDecode::start(
            fixture.scene.clone(),
            fixture.cache.clone(),
            &fixture.key,
            support::limits(),
            5000,
        )
        .map_err(error)
    }
    fn pixels(canvas: &HtmlCanvasElement) -> Result<Vec<u8>, JsValue> {
        let context = canvas
            .get_context("2d")?
            .ok_or_else(|| error("canvas context absent"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(error)?;
        Ok(context.get_image_data(0.0, 0.0, 1.0, 1.0)?.data().0)
    }
    fn flat_survives(fixture: &Fixture) -> Result<(), JsValue> {
        require(
            fixture
                .host
                .0
                .query_selector("[data-token-key]")?
                .is_some_and(|token| token.is_connected()),
            "optional image failure removed flat scene",
        )
    }

    /// Real browser codec + actual mounted consumer assertion, no synthetic decoded bytes.
    #[wasm_bindgen]
    pub async fn verify_browser_image_decode() -> Result<(), JsValue> {
        let fixture = fixture(support::PNG, support::PNG_DIGEST)?;
        let mut job = start(&fixture)?;
        require(
            fixture.scene.borrow().work_bytes() == 244,
            "input work not reserved",
        )?;
        require(
            job.wait().await.map_err(error)?,
            "decoded PNG not presented",
        )?;
        require(
            pixels(&fixture.canvas)? == [16, 32, 48, 255],
            "actual PNG canvas RGBA differs",
        )?;
        require(
            fixture.scene.borrow().decoded_bytes() == 4
                && fixture.scene.borrow().surface_bytes() == 8
                && fixture.scene.borrow().work_bytes() == 0
                && fixture.cache.borrow().lease_count() == 1,
            "terminal cache budgets/lease differ",
        )?;
        require(
            job.wait().await.map_err(error)?,
            "duplicate terminal observation changed outcome",
        )?;
        require(
            fixture.cache.borrow().lease_count() == 1,
            "duplicate terminal observation leaked",
        )?;
        fixture
            .cache
            .borrow_mut()
            .release(&fixture.key)
            .map_err(error)?;
        require(
            fixture.scene.borrow_mut().refresh()
                == Err(ImageSurfaceError::Resource(ResourceError::RevokedResource)),
            "revoked lease disclosed canvas",
        )?;
        require(
            fixture.canvas.width() == 0 && fixture.scene.borrow().decoded_bytes() == 0,
            "revocation retained surface",
        )?;
        flat_survives(&fixture)?;
        fixture.scene.borrow_mut().dispose().map_err(error)?;
        require(
            !fixture.canvas.is_connected() && fixture.cache.borrow().lease_count() == 0,
            "successful consumer disposal retained canvas/lease",
        )?;

        let corrupt = fixture_new_corrupt()?;
        let mut job = start(&corrupt)?;
        require(
            job.wait().await == Err(ImageDecodeError::BrowserDecode),
            "real codec accepted corrupt Deflate PNG",
        )?;
        require(
            corrupt.canvas.width() == 0
                && corrupt.scene.borrow().work_bytes() == 0
                && corrupt.scene.borrow().decoded_bytes() == 0
                && corrupt.cache.borrow().lease_count() == 0,
            "corrupt decode retained resource/work",
        )?;
        flat_survives(&corrupt)?;
        corrupt.scene.borrow_mut().dispose().map_err(error)?;

        let wide = fixture_new_wide()?;
        require(
            BrowserImageDecode::start(
                wide.scene.clone(),
                wide.cache.clone(),
                &wide.key,
                support::limits(),
                5000,
            )
            .err()
                == Some(ImageDecodeError::DimensionCapacity),
            "oversized PNG admitted browser allocation",
        )?;
        require(
            wide.scene.borrow().work_bytes() == 0 && wide.cache.borrow().lease_count() == 0,
            "rejected preflight retained work/lease",
        )?;
        require(
            inspect_png(support::UNSUPPORTED_PNG, support::limits())
                == Err(ImageDecodeError::UnsupportedPng),
            "unsupported format not typed",
        )?;
        flat_survives(&wide)?;
        wide.scene.borrow_mut().dispose().map_err(error)?;

        let wrong_owner = fixture_new_valid()?;
        let mut wrong_scope = support::scope();
        wrong_scope.binding = df_types::ClientBindingId::from_bytes(&[9; 16]).unwrap();
        let mut wrong_cache = AssetCache::new(
            wrong_scope,
            CacheLimits {
                max_assets: 2,
                max_pending: 1,
                max_leases: 4,
                max_bytes: 1024,
            },
        )
        .map_err(error)?;
        wrong_cache
            .apply_current(
                wrong_scope,
                support::revision(1, 0),
                std::slice::from_ref(&wrong_owner.key),
            )
            .map_err(error)?;
        let fetch = wrong_cache.fetch(&wrong_owner.key).map_err(error)?;
        wrong_cache
            .complete(&fetch, support::PNG.to_vec())
            .map_err(error)?;
        let wrong_cache = Rc::new(RefCell::new(wrong_cache));
        require(
            BrowserImageDecode::start(
                wrong_owner.scene.clone(),
                wrong_cache.clone(),
                &wrong_owner.key,
                support::limits(),
                5000,
            )
            .err()
                == Some(ImageDecodeError::WrongOwner),
            "same key from another canonical scope admitted",
        )?;
        require(
            wrong_owner.scene.borrow().work_bytes() == 0 && wrong_cache.borrow().lease_count() == 0,
            "owner refusal allocated work/lease",
        )?;
        wrong_owner.scene.borrow_mut().dispose().map_err(error)?;

        let cancelled = fixture_new_valid()?;
        let mut job = start(&cancelled)?;
        job.cancel().map_err(error)?;
        require(
            job.status() == BrowserDecodeStatus::Draining(ImageDecodeError::Cancelled)
                && cancelled.scene.borrow().work_bytes() == 244,
            "abort freed unterminal work",
        )?;
        require(
            start(&cancelled).is_err(),
            "cancelled pending duplicate admitted",
        )?;
        require(
            job.wait().await == Err(ImageDecodeError::Cancelled),
            "cancelled callback installed",
        )?;
        require(
            cancelled.scene.borrow().work_bytes() == 0
                && cancelled.canvas.width() == 0
                && cancelled.cache.borrow().lease_count() == 0,
            "cancelled terminal work leaked",
        )?;
        flat_survives(&cancelled)?;
        cancelled.scene.borrow_mut().dispose().map_err(error)?;

        let revoked = fixture_new_valid()?;
        let mut job = start(&revoked)?;
        revoked
            .cache
            .borrow_mut()
            .release(&revoked.key)
            .map_err(error)?;
        let fetch = revoked
            .cache
            .borrow_mut()
            .fetch(&revoked.key)
            .map_err(error)?;
        revoked
            .cache
            .borrow_mut()
            .complete(&fetch, support::PNG.to_vec())
            .map_err(error)?;
        require(
            job.wait().await == Err(ImageDecodeError::Lease(CacheError::StaleLease)),
            "identical refetch revived old async lease",
        )?;
        require(
            revoked.canvas.width() == 0 && revoked.scene.borrow().work_bytes() == 0,
            "revoked async completion installed",
        )?;
        flat_survives(&revoked)?;
        revoked.scene.borrow_mut().dispose().map_err(error)?;

        verify_generation_fences().await?;
        #[cfg(feature = "image-decode-fixture")]
        verify_deadline_fence().await?;

        let disposed = fixture_new_valid()?;
        let mut job = start(&disposed)?;
        disposed.scene.borrow_mut().dispose().map_err(error)?;
        disposed.scene.borrow_mut().dispose().map_err(error)?;
        require(
            disposed.scene.borrow().work_bytes() == 244 && !disposed.canvas.is_connected(),
            "dispose released live decode work or retained canvas",
        )?;
        require(
            job.wait().await == Err(ImageDecodeError::Cancelled),
            "disposed callback published",
        )?;
        require(
            disposed.scene.borrow().work_bytes() == 0 && disposed.cache.borrow().lease_count() == 0,
            "disposed terminal callback leaked",
        )?;
        Ok(())
    }
    async fn verify_generation_fences() -> Result<(), JsValue> {
        // A still-current byte lease isolates ResourceCache's scene generation fence.
        let scene_changed = fixture_new_valid()?;
        let mut job = start(&scene_changed)?;
        scene_changed
            .scene
            .borrow_mut()
            .update(
                support::owner(),
                support::label("scene-two"),
                geometry::flat(support::revision(2, 0)),
                std::slice::from_ref(&scene_changed.key),
            )
            .map_err(error)?;
        require(
            scene_changed.scene.borrow().work_bytes() == 244
                && scene_changed.cache.borrow().lease_count() == 1
                && job.status() == BrowserDecodeStatus::Draining(ImageDecodeError::Cancelled),
            "scene replacement released real work or revoked unrelated byte lease",
        )?;
        require(
            job.wait().await == Err(ImageDecodeError::Cancelled),
            "scene generation accepted stale actual codec completion",
        )?;
        require(
            scene_changed.scene.borrow().work_bytes() == 0
                && scene_changed.scene.borrow().decoded_bytes() == 0
                && scene_changed.canvas.width() == 0
                && scene_changed.cache.borrow().lease_count() == 0,
            "scene generation terminal retained work/pixels/lease",
        )?;
        flat_survives(&scene_changed)?;
        scene_changed.scene.borrow_mut().dispose().map_err(error)?;

        // An unchanged scene isolates the canonical byte-cache epoch/lease fence.
        let cache_changed = fixture_new_valid()?;
        let mut job = start(&cache_changed)?;
        cache_changed
            .cache
            .borrow_mut()
            .apply_current(
                support::scope(),
                support::revision(2, 0),
                std::slice::from_ref(&cache_changed.key),
            )
            .map_err(error)?;
        require(
            cache_changed.scene.borrow().work_bytes() == 244
                && job.status() == BrowserDecodeStatus::Running,
            "cache epoch falsely settled real codec or cancelled scene operation",
        )?;
        require(
            job.wait().await == Err(ImageDecodeError::Lease(CacheError::StaleLease)),
            "cache epoch replacement revived old actual decode lease",
        )?;
        require(
            cache_changed.scene.borrow().work_bytes() == 0
                && cache_changed.scene.borrow().decoded_bytes() == 0
                && cache_changed.canvas.width() == 0
                && cache_changed.cache.borrow().lease_count() == 0,
            "cache epoch terminal retained work/pixels/lease",
        )?;
        flat_survives(&cache_changed)?;
        cache_changed.scene.borrow_mut().dispose().map_err(error)?;

        // The mounted scope's complete reconnect replacement updates both owners before await.
        let replaced = fixture_new_valid()?;
        let mut job = start(&replaced)?;
        replaced
            .cache
            .borrow_mut()
            .apply_current(
                support::scope(),
                support::revision(2, 0),
                std::slice::from_ref(&replaced.key),
            )
            .map_err(error)?;
        replaced
            .scene
            .borrow_mut()
            .update(
                support::owner(),
                support::label("scene-two"),
                geometry::flat(support::revision(2, 0)),
                std::slice::from_ref(&replaced.key),
            )
            .map_err(error)?;
        require(
            replaced.scene.borrow().work_bytes() == 244
                && replaced.canvas.width() == 0
                && job.status() == BrowserDecodeStatus::Draining(ImageDecodeError::Cancelled),
            "combined generation replacement freed pending work or disclosed stale pixels",
        )?;
        require(
            job.wait().await == Err(ImageDecodeError::Cancelled),
            "combined generation callback installed actual stale pixels",
        )?;
        require(
            replaced.scene.borrow().work_bytes() == 0
                && replaced.scene.borrow().decoded_bytes() == 0
                && replaced.cache.borrow().lease_count() == 0
                && replaced.canvas.width() == 0,
            "combined replacement terminal cleanup differs",
        )?;
        flat_survives(&replaced)?;
        replaced.scene.borrow_mut().dispose().map_err(error)?;
        Ok(())
    }

    #[cfg(feature = "image-decode-fixture")]
    async fn verify_deadline_fence() -> Result<(), JsValue> {
        let expired = fixture_new_valid()?;
        let mut job = start(&expired)?;
        require(
            job.fixture_fire_deadline(),
            "first owned deadline fence not accepted",
        )?;
        require(
            !job.fixture_fire_deadline(),
            "duplicate deadline changed fence",
        )?;
        require(
            job.status() == BrowserDecodeStatus::Draining(ImageDecodeError::Deadline)
                && expired.scene.borrow().work_bytes() == 244
                && expired.cache.borrow().lease_count() == 1
                && expired.scene.borrow().decoded_bytes() == 0
                && expired.canvas.width() == 0,
            "deadline fence settled actual codec or published pixels",
        )?;
        require(
            BrowserImageDecode::start(
                expired.scene.clone(),
                expired.cache.clone(),
                &expired.key,
                support::limits(),
                5000,
            )
            .err()
                == Some(ImageDecodeError::Resource(ResourceError::AlreadyPending)),
            "deadline-draining operation admitted replacement before actual terminal",
        )?;
        require(
            job.wait().await == Err(ImageDecodeError::Deadline),
            "deadline actual terminal callback installed pixels",
        )?;
        require(
            job.status() == BrowserDecodeStatus::Terminal(Err(ImageDecodeError::Deadline))
                && expired.scene.borrow().work_bytes() == 0
                && expired.scene.borrow().decoded_bytes() == 0
                && expired.canvas.width() == 0
                && expired.cache.borrow().lease_count() == 0,
            "deadline terminal work/pixel/lease cleanup differs",
        )?;
        flat_survives(&expired)?;
        let mut next = start(&expired)?;
        require(
            !job.fixture_fire_deadline() && expired.scene.borrow().work_bytes() == 244,
            "old terminal deadline affected newer actual codec work",
        )?;
        require(
            next.wait().await.map_err(error)? && pixels(&expired.canvas)? == [16, 32, 48, 255],
            "newer decode failed after old deadline terminal",
        )?;
        expired.scene.borrow_mut().dispose().map_err(error)?;
        require(
            expired.cache.borrow().lease_count() == 0,
            "deadline fixture disposal leaked lease",
        )?;
        Ok(())
    }

    fn fixture_new_valid() -> Result<Fixture, JsValue> {
        fixture(support::PNG, support::PNG_DIGEST)
    }
    fn fixture_new_corrupt() -> Result<Fixture, JsValue> {
        fixture(support::CORRUPT_PNG, support::CORRUPT_DIGEST)
    }
    fn fixture_new_wide() -> Result<Fixture, JsValue> {
        fixture(support::TOO_WIDE_PNG, support::WIDE_DIGEST)
    }

    thread_local! { static OBSERVED: RefCell<Option<Fixture>> = const { RefCell::new(None) }; }

    /// Retains the actual decoded/mounted PNG for independent visual observation.
    #[wasm_bindgen]
    pub async fn browser_image_fixture_start() -> Result<(), JsValue> {
        OBSERVED
            .with(|slot| require(slot.borrow().is_none(), "observed fixture already mounted"))?;
        let fixture = fixture_new_valid()?;
        let mut job = start(&fixture)?;
        job.wait().await.map_err(error)?;
        require(
            pixels(&fixture.canvas)? == [16, 32, 48, 255],
            "observed actual PNG differs",
        )?;
        fixture
            .host
            .0
            .set_attribute("data-image-decode-phase", "decoded")?;
        OBSERVED.with(|slot| {
            require(
                slot.borrow().is_none(),
                "observed fixture replaced during decode",
            )?;
            *slot.borrow_mut() = Some(fixture);
            Ok(())
        })
    }
    #[wasm_bindgen]
    pub fn browser_image_fixture_revoke() -> Result<(), JsValue> {
        OBSERVED.with(|slot| {
            let mut slot = slot.borrow_mut();
            let fixture = slot
                .as_mut()
                .ok_or_else(|| error("observed fixture absent"))?;
            fixture
                .cache
                .borrow_mut()
                .release(&fixture.key)
                .map_err(error)?;
            require(
                fixture.scene.borrow_mut().refresh()
                    == Err(ImageSurfaceError::Resource(ResourceError::RevokedResource)),
                "observed revoked PNG remained visible",
            )?;
            require(
                fixture.canvas.width() == 0,
                "observed revoke retained canvas backing",
            )?;
            flat_survives(fixture)?;
            fixture
                .host
                .0
                .set_attribute("data-image-decode-phase", "revoked")?;
            Ok(())
        })
    }
    #[wasm_bindgen]
    pub fn browser_image_fixture_dispose() -> Result<(), JsValue> {
        OBSERVED.with(|slot| {
            let mut slot = slot.borrow_mut();
            if let Some(fixture) = slot.as_mut() {
                fixture.scene.borrow_mut().dispose().map_err(error)?;
                require(
                    fixture.cache.borrow().lease_count() == 0,
                    "observed dispose leaked lease",
                )?;
            }
            *slot = None;
            Ok(())
        })
    }
    struct PreparedFixture {
        host: Host,
        owner: Rc<RefCell<ResourceLifecycle>>,
        asset: &'static support::PreparedAsset,
        key: CacheKey,
        canvas: HtmlCanvasElement,
        busy: bool,
    }
    fn prepared_fixture(
        asset: &'static support::PreparedAsset,
    ) -> Result<PreparedFixture, JsValue> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| error("document unavailable"))?;
        let host = Host(document.create_element("section")?);
        host.0
            .set_attribute("data-prepared-asset-route", asset.route)?;
        host.0
            .set_attribute("data-prepared-asset-mime", asset.mime)?;
        let caption = document.create_element("p")?;
        caption.set_text_content(Some(asset.route));
        host.0.append_child(&caption)?;
        document
            .body()
            .ok_or_else(|| error("body unavailable"))?
            .append_child(&host.0)?;
        // Fixture capacities only, deliberately finite; no production/device capacity claim.
        let mut owner = ResourceLifecycle::mount(
            &host.0,
            support::scope(),
            CacheLimits {
                max_assets: 1,
                max_pending: 1,
                max_leases: 4,
                max_bytes: 4 * 1024 * 1024,
            },
            ResourceLimits {
                max_references: 1,
                max_resident: 1,
                max_pending: 1,
                max_decoded_bytes: 8 * 1024 * 1024,
                max_work_bytes: 64 * 1024 * 1024,
            },
            support::label("prepared-png-vp8-v1"),
            16 * 1024 * 1024,
        )
        .map_err(error)?;
        let key = asset.key();
        owner
            .apply_current(
                support::scope(),
                support::revision(1, 0),
                std::slice::from_ref(&key),
            )
            .map_err(error)?;
        owner
            .update_scene(
                support::label("prepared-scene"),
                geometry::flat(support::revision(1, 0)),
                std::slice::from_ref(&key),
            )
            .map_err(error)?;
        let fetch = owner.fetch(&key).map_err(error)?;
        owner
            .complete_fetch(&fetch, asset.bytes.to_vec())
            .map_err(error)?;
        let canvas = host
            .0
            .query_selector("[data-scene-image-host] canvas")?
            .ok_or_else(|| error("prepared canvas unavailable"))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(error)?;
        canvas.set_attribute("style", "max-width:720px;width:100%;height:auto")?;
        Ok(PreparedFixture {
            host,
            owner: Rc::new(RefCell::new(owner)),
            asset,
            key,
            canvas,
            busy: false,
        })
    }
    fn prepared_start(fixture: &PreparedFixture) -> Result<BrowserImageDecode, JsValue> {
        BrowserImageDecode::start_owned_prepared(
            fixture.owner.clone(),
            &fixture.key,
            fixture.asset.metadata(),
            support::prepared_limits(),
            10_000,
        )
        .map_err(error)
    }
    fn prepared_flat_survives(fixture: &PreparedFixture) -> Result<(), JsValue> {
        require(
            fixture
                .host
                .0
                .query_selector("[data-token-key]")?
                .is_some_and(|node| node.is_connected()),
            "prepared failure removed flat token",
        )
    }
    fn prepared_install(fixture: &PreparedFixture) -> Result<(), JsValue> {
        let mut owner = fixture.owner.borrow_mut();
        let fetch = owner.fetch(&fixture.key).map_err(error)?;
        owner
            .complete_fetch(&fetch, fixture.asset.bytes.to_vec())
            .map_err(error)
    }
    fn prepared_canvas_matches(fixture: &PreparedFixture) -> Result<(), JsValue> {
        require(
            (fixture.canvas.width(), fixture.canvas.height())
                == (fixture.asset.width, fixture.asset.height),
            "prepared canvas dimensions differ",
        )?;
        let context = fixture
            .canvas
            .get_context("2d")?
            .ok_or_else(|| error("canvas context absent"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(error)?;
        let rgba = context
            .get_image_data(
                0.0,
                0.0,
                f64::from(fixture.asset.width),
                f64::from(fixture.asset.height),
            )?
            .data()
            .0;
        let mut owner = fixture.owner.borrow_mut();
        let resource = owner
            .get(&fixture.key)
            .map_err(error)?
            .ok_or_else(|| error("selected prepared lease/surface absent"))?;
        require(
            resource.dimensions() == (fixture.asset.width, fixture.asset.height),
            "decoded prepared dimensions differ",
        )?;
        require(
            resource.pixels().map_err(error)? == rgba,
            "mounted prepared RGBA differs from owned codec output",
        )?;
        require(
            rgba.as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel.get(3) == Some(&255)),
            "opaque prepared RGB/VP8 produced unexpected alpha",
        )?;
        Ok(())
    }

    /// Every exact local public file passes the real codec and canonical mounted lifecycle.
    /// Corrupt framing cases remain native preflight tests; no duplicate decoder or URL fetch.
    #[wasm_bindgen]
    pub async fn verify_prepared_public_assets() -> Result<(), JsValue> {
        for asset in support::PREPARED_ASSETS {
            let fixture = prepared_fixture(asset)?;
            let plan =
                inspect_prepared_image(asset.bytes, asset.metadata(), support::prepared_limits())
                    .map_err(error)?;
            let mut wrong = asset.metadata();
            wrong.mime = if asset.mime == "image/png" {
                "image/webp"
            } else {
                "image/png"
            };
            require(
                BrowserImageDecode::start_owned_prepared(
                    fixture.owner.clone(),
                    &fixture.key,
                    wrong,
                    support::prepared_limits(),
                    10_000,
                )
                .err()
                    == Some(ImageDecodeError::MimeMismatch),
                "prepared MIME mismatch admitted codec",
            )?;
            wrong = asset.metadata();
            wrong.width += 1;
            require(
                BrowserImageDecode::start_owned_prepared(
                    fixture.owner.clone(),
                    &fixture.key,
                    wrong,
                    support::prepared_limits(),
                    10_000,
                )
                .err()
                    == Some(ImageDecodeError::WrongDimensions),
                "prepared dimension mismatch admitted codec",
            )?;
            let mut insufficient = support::prepared_limits();
            insufficient.max_work_bytes = plan.budget.work_bytes - 1;
            require(
                BrowserImageDecode::start_owned_prepared(
                    fixture.owner.clone(),
                    &fixture.key,
                    asset.metadata(),
                    insufficient,
                    10_000,
                )
                .err()
                    == Some(ImageDecodeError::ByteCapacity),
                "prepared work-cap refusal admitted codec",
            )?;
            if asset.mime == "image/png" {
                let mut too_small = asset.metadata();
                too_small.max_ancillary_bytes = 23_653;
                require(
                    BrowserImageDecode::start_owned_prepared(
                        fixture.owner.clone(),
                        &fixture.key,
                        too_small,
                        support::prepared_limits(),
                        10_000,
                    )
                    .err()
                        == Some(ImageDecodeError::AncillaryCapacity),
                    "prepared ancillary cap admitted codec",
                )?;
            }
            require(
                fixture.owner.borrow().work_bytes() == 0
                    && fixture.owner.borrow().lease_count() == 0,
                "prepared metadata/budget refusal retained work/lease",
            )?;
            let mut job = prepared_start(&fixture)?;
            require(
                fixture.owner.borrow().work_bytes() == plan.budget.work_bytes
                    && fixture.owner.borrow().lease_count() == 1,
                "prepared actual work not exactly reserved",
            )?;
            require(
                job.wait().await.map_err(error)?,
                "prepared actual codec failed to mount",
            )?;
            prepared_canvas_matches(&fixture)?;
            require(
                fixture.owner.borrow().decoded_bytes() == plan.budget.decoded_bytes
                    && fixture.owner.borrow().surface_bytes() == plan.budget.decoded_bytes * 2
                    && fixture.owner.borrow().resident_bytes() == asset.bytes.len()
                    && fixture.owner.borrow().work_bytes() == 0
                    && fixture.owner.borrow().lease_count() == 1,
                "prepared terminal encoded/decoded/display accounting differs",
            )?;
            fixture
                .owner
                .borrow_mut()
                .release(&fixture.key)
                .map_err(error)?;
            require(
                fixture.canvas.width() == 0
                    && fixture.owner.borrow().decoded_bytes() == 0
                    && fixture.owner.borrow().lease_count() == 0,
                "prepared byte-only revoke retained pixels",
            )?;
            prepared_flat_survives(&fixture)?;

            prepared_install(&fixture)?;
            let mut cancelled = prepared_start(&fixture)?;
            cancelled.cancel().map_err(error)?;
            require(
                cancelled.status() == BrowserDecodeStatus::Draining(ImageDecodeError::Cancelled)
                    && fixture.owner.borrow().work_bytes() == plan.budget.work_bytes,
                "prepared abort request freed actual running codec work",
            )?;
            require(
                BrowserImageDecode::start_owned_prepared(
                    fixture.owner.clone(),
                    &fixture.key,
                    asset.metadata(),
                    support::prepared_limits(),
                    10_000,
                )
                .err()
                    == Some(ImageDecodeError::Resource(ResourceError::AlreadyPending)),
                "prepared cancelled duplicate admitted",
            )?;
            require(
                cancelled.wait().await == Err(ImageDecodeError::Cancelled)
                    && fixture.owner.borrow().work_bytes() == 0
                    && fixture.canvas.width() == 0,
                "prepared cancelled terminal installed or leaked work",
            )?;

            let mut revoked = prepared_start(&fixture)?;
            fixture
                .owner
                .borrow_mut()
                .release(&fixture.key)
                .map_err(error)?;
            prepared_install(&fixture)?;
            require(
                fixture.owner.borrow().work_bytes() == plan.budget.work_bytes,
                "prepared revoke/refetch freed running codec",
            )?;
            require(
                revoked.wait().await == Err(ImageDecodeError::Cancelled)
                    && fixture.owner.borrow().work_bytes() == 0
                    && fixture.canvas.width() == 0,
                "prepared identical refetch revived obsolete lease",
            )?;

            #[cfg(feature = "image-decode-fixture")]
            {
                let mut expired = prepared_start(&fixture)?;
                require(
                    expired.fixture_fire_deadline()
                        && fixture.owner.borrow().work_bytes() == plan.budget.work_bytes,
                    "prepared actual deadline fence released work",
                )?;
                require(
                    expired.wait().await == Err(ImageDecodeError::Deadline)
                        && fixture.owner.borrow().work_bytes() == 0
                        && fixture.canvas.width() == 0,
                    "prepared deadline actual terminal installed pixels",
                )?;
                let mut next = prepared_start(&fixture)?;
                require(
                    !expired.fixture_fire_deadline()
                        && fixture.owner.borrow().work_bytes() == plan.budget.work_bytes,
                    "old prepared deadline released newer work",
                )?;
                next.cancel().map_err(error)?;
                require(
                    next.wait().await == Err(ImageDecodeError::Cancelled),
                    "new prepared job fence lost",
                )?;
            }

            let mut generation = prepared_start(&fixture)?;
            fixture
                .owner
                .borrow_mut()
                .apply_current(
                    support::scope(),
                    support::revision(2, 0),
                    std::slice::from_ref(&fixture.key),
                )
                .map_err(error)?;
            fixture
                .owner
                .borrow_mut()
                .update_scene(
                    support::label("prepared-next"),
                    geometry::flat(support::revision(2, 0)),
                    std::slice::from_ref(&fixture.key),
                )
                .map_err(error)?;
            require(
                fixture.owner.borrow().work_bytes() == plan.budget.work_bytes,
                "prepared generation replacement freed pending codec",
            )?;
            require(
                generation.wait().await == Err(ImageDecodeError::Cancelled)
                    && fixture.owner.borrow().work_bytes() == 0
                    && fixture.canvas.width() == 0,
                "prepared stale generation installed",
            )?;
            prepared_flat_survives(&fixture)?;
            let mut replaced = prepared_start(&fixture)?;
            let mut scope = support::scope();
            scope.binding = df_types::ClientBindingId::from_bytes(&[9; 16]).unwrap();
            fixture
                .owner
                .borrow_mut()
                .replace_scope(scope)
                .map_err(error)?;
            require(
                !fixture.canvas.is_connected()
                    && fixture.owner.borrow().work_bytes() == plan.budget.work_bytes,
                "prepared scope replacement retained mount or released actual work",
            )?;
            require(
                replaced.wait().await == Err(ImageDecodeError::Cancelled)
                    && fixture.owner.borrow().work_bytes() == 0
                    && fixture.owner.borrow().lease_count() == 0,
                "prepared replaced-scope terminal published or leaked",
            )?;
            fixture.owner.borrow_mut().dispose().map_err(error)?;
            fixture.owner.borrow_mut().dispose().map_err(error)?;
        }
        Ok(())
    }

    thread_local! { static PREPARED_OBSERVED: RefCell<Option<PreparedFixture>> = const { RefCell::new(None) }; }
    /// One retained real prepared image for independent visual inspection; index is 0..4.
    #[wasm_bindgen]
    pub async fn prepared_assets_fixture_start(index: u32) -> Result<(), JsValue> {
        PREPARED_OBSERVED
            .with(|slot| require(slot.borrow().is_none(), "prepared fixture already owned"))?;
        let asset = support::PREPARED_ASSETS
            .get(index as usize)
            .ok_or_else(|| error("prepared asset index outside 0..4"))?;
        let mut fixture = prepared_fixture(asset)?;
        fixture.busy = true;
        let owner = fixture.owner.clone();
        let mut job = prepared_start(&fixture)?;
        PREPARED_OBSERVED.with(|slot| {
            *slot.borrow_mut() = Some(fixture);
        });
        let terminal = job.wait().await;
        if let Err(failure) = terminal {
            owner.borrow_mut().dispose().map_err(error)?;
            PREPARED_OBSERVED.with(|slot| {
                *slot.borrow_mut() = None;
            });
            return Err(error(failure));
        }
        PREPARED_OBSERVED.with(|slot| {
            let mut slot = slot.borrow_mut();
            let fixture = slot
                .as_mut()
                .ok_or_else(|| error("prepared fixture absent"))?;
            fixture.busy = false;
            prepared_canvas_matches(fixture)?;
            fixture
                .host
                .0
                .set_attribute("data-prepared-phase", "decoded")?;
            Ok(())
        })
    }
    #[wasm_bindgen]
    pub fn prepared_assets_fixture_revoke() -> Result<(), JsValue> {
        PREPARED_OBSERVED.with(|slot| {
            let mut slot = slot.borrow_mut();
            let fixture = slot
                .as_mut()
                .ok_or_else(|| error("prepared fixture absent"))?;
            require(!fixture.busy, "await actual prepared decode terminal")?;
            fixture
                .owner
                .borrow_mut()
                .release(&fixture.key)
                .map_err(error)?;
            require(
                fixture.canvas.width() == 0,
                "prepared observed revoke retained canvas",
            )?;
            prepared_flat_survives(fixture)?;
            fixture
                .host
                .0
                .set_attribute("data-prepared-phase", "revoked")?;
            Ok(())
        })
    }
    #[wasm_bindgen]
    pub fn prepared_assets_fixture_dispose() -> Result<(), JsValue> {
        PREPARED_OBSERVED.with(|slot| {
            let mut slot = slot.borrow_mut();
            if let Some(fixture) = slot.as_mut() {
                require(!fixture.busy, "await actual prepared decode terminal")?;
                fixture.owner.borrow_mut().dispose().map_err(error)?;
                require(
                    fixture.owner.borrow().lease_count() == 0
                        && fixture.owner.borrow().work_bytes() == 0,
                    "prepared observed disposal leaked",
                )?;
            }
            *slot = None;
            Ok(())
        })
    }
}
