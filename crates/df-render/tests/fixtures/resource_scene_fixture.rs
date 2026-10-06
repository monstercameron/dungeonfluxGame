#[cfg(target_arch = "wasm32")]
#[path = "../support/resource_flat.rs"]
mod geometry;

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::geometry;
    use df_assets::AssetManifest;
    use df_client::cache::{AssetCache, CacheError, CacheKey, CacheLimits, CacheScope};
    use df_render::{
        BrowserResourceScene, DecodeBudget, DecodeToken, DecodedImage, ImageSurfaceError,
        PresentationOutcome, ResourceError, ResourceLimits, ResourceSceneError, SceneOwner,
    };
    use df_types::{
        ClientBindingId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
    };
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
    use web_sys::{CanvasRenderingContext2d, Element, HtmlCanvasElement};

    const SOURCE: &[u8] = b"RGBA\x01\0\0\0\x01\0\0\0\x10\x20\x30\xff";
    const DIGEST: [u8; 32] = [
        178, 133, 58, 194, 210, 196, 126, 6, 138, 49, 244, 253, 100, 144, 200, 44, 47, 111, 98, 69,
        91, 141, 223, 21, 179, 133, 216, 31, 143, 254, 21, 172,
    ];
    struct Host(Element);
    impl Drop for Host {
        fn drop(&mut self) {
            self.0.remove();
        }
    }
    fn error(value: impl std::fmt::Debug) -> JsValue {
        JsValue::from_str(&format!("{value:?}"))
    }
    fn require(condition: bool, message: &'static str) -> Result<(), JsValue> {
        if condition {
            Ok(())
        } else {
            Err(JsValue::from_str(message))
        }
    }
    fn label(text: &str) -> RevisionLabel {
        RevisionLabel::new(Some(text)).unwrap()
    }
    fn revision(epoch: u64, sequence: u64) -> SessionRevision {
        SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
    }
    fn owner() -> SceneOwner {
        (
            SessionId::from_bytes(&[1; 16]).unwrap(),
            RunId::from_bytes(&[2; 16]).unwrap(),
            ClientBindingId::from_bytes(&[3; 16]).unwrap(),
        )
    }
    fn limits() -> ResourceLimits {
        ResourceLimits {
            max_references: 2,
            max_resident: 1,
            max_pending: 1,
            max_decoded_bytes: 4,
            max_work_bytes: 24,
        }
    }
    fn budget() -> DecodeBudget {
        DecodeBudget {
            decoded_bytes: 4,
            work_bytes: 24,
        }
    }
    fn query(host: &Element, selector: &str) -> Result<Element, JsValue> {
        host.query_selector(selector)?
            .ok_or_else(|| error("missing actual scene node"))
    }
    fn canvas(host: &Element) -> Result<HtmlCanvasElement, JsValue> {
        query(host, "[data-scene-image-host] canvas")?
            .dyn_into()
            .map_err(error)
    }
    fn pixels(canvas: &HtmlCanvasElement) -> Result<Vec<u8>, JsValue> {
        let context = canvas
            .get_context("2d")?
            .ok_or_else(|| error("missing real canvas context"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(error)?;
        Ok(context.get_image_data(0.0, 0.0, 1.0, 1.0)?.data().0)
    }
    fn decoded(
        bytes: &Rc<RefCell<AssetCache>>,
        key: &CacheKey,
        released: &Rc<Cell<usize>>,
    ) -> Result<DecodedImage<CacheKey>, JsValue> {
        let lease = bytes
            .borrow_mut()
            .acquire(key)
            .map_err(error)?
            .ok_or_else(|| error("verified bytes missing"))?;
        // Test-local RGBA encoding only. Owned decoded surface is allocated separately from
        // compressed/source bytes; no private-model import and no browser auth issuance.
        let owned_pixels = {
            let borrowed = bytes.borrow();
            let input = borrowed.lease_bytes(&lease).map_err(error)?;
            require(
                input.get(..12) == Some(&SOURCE[..12]),
                "invalid local image header",
            )?;
            input
                .get(12..)
                .ok_or_else(|| error("pixel source missing"))?
                .to_vec()
                .into_boxed_slice()
        };
        let current_cache = bytes.clone();
        let current_lease = lease.clone();
        let release_cache = bytes.clone();
        let count = released.clone();
        DecodedImage::new(
            key.clone(),
            1,
            1,
            owned_pixels,
            move |expected| {
                current_lease.key() == expected
                    && current_cache.borrow().lease_bytes(&current_lease).is_ok()
            },
            move || {
                match release_cache.borrow_mut().release_lease(&lease) {
                    Ok(()) | Err(CacheError::StaleLease | CacheError::Closed) => {}
                    Err(value) => panic!("unexpected actual lease retirement: {value:?}"),
                }
                count.set(count.get() + 1);
            },
        )
        .map_err(error)
    }
    fn install_source(bytes: &Rc<RefCell<AssetCache>>, key: &CacheKey) -> Result<(), JsValue> {
        let token = bytes.borrow_mut().fetch(key).map_err(error)?;
        bytes
            .borrow_mut()
            .complete(&token, SOURCE.to_vec())
            .map_err(error)
    }

    /// Root invokes this exported consumer assertion after its WASM compilation/browser gate.
    /// There are no global state slots, timers, listeners, media URLs or simulated backends.
    #[wasm_bindgen]
    pub fn verify_resource_scene() -> Result<(), JsValue> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| error("document unavailable"))?;
        let host = Host(document.create_element("section")?);
        document
            .body()
            .ok_or_else(|| error("body unavailable"))?
            .append_child(&host.0)?;
        let owner = owner();
        let (session, run, binding) = owner;
        let scope = CacheScope {
            session,
            run,
            binding,
        };
        let key = CacheKey {
            version: label("current-rgba-fixture-v1"),
            bytes: AssetManifest {
                byte_len: 16,
                sha256: DIGEST,
            },
        };
        let bytes = Rc::new(RefCell::new(
            AssetCache::new(
                scope,
                CacheLimits {
                    max_assets: 2,
                    max_pending: 1,
                    max_leases: 4,
                    max_bytes: 32,
                },
            )
            .map_err(error)?,
        ));
        bytes
            .borrow_mut()
            .apply_current(scope, revision(1, 0), std::slice::from_ref(&key))
            .map_err(error)?;
        let released = Rc::new(Cell::new(0));
        let mut renderer =
            BrowserResourceScene::mount(&host.0, owner, limits(), label("rgba8-surface-v1"), 8)
                .map_err(error)?;
        renderer
            .update(
                owner,
                label("scene-one"),
                geometry::flat(revision(1, 0)),
                std::slice::from_ref(&key),
            )
            .map_err(error)?;
        let token = renderer.begin(&key, budget(), || {}).map_err(error)?;
        install_source(&bytes, &key)?;
        renderer
            .complete(&token, decoded(&bytes, &key, &released)?)
            .map_err(error)?;
        require(
            renderer.present(&key).map_err(error)?,
            "surface not presented",
        )?;
        let actual_canvas = canvas(&host.0)?;
        let svg = query(&host.0, "svg")?;
        let scene_token = query(&svg, "[data-token-key]")?;
        require(
            pixels(&actual_canvas)? == [16, 32, 48, 255],
            "real canvas pixels differ",
        )?;
        require(
            renderer.decoded_bytes() == 4
                && renderer.work_bytes() == 0
                && renderer.surface_bytes() == 8,
            "decoded/work/display budgets differ",
        )?;
        require(
            bytes.borrow().resident_bytes() == 16 && bytes.borrow().lease_count() == 1,
            "source bytes/lease not separately resident",
        )?;
        let invalid = geometry::flat(revision(1, 1));
        let duplicates = [key.clone(), key.clone()];
        require(
            renderer.update(owner, label("scene-one"), invalid, &duplicates)
                == Err(ImageSurfaceError::Scene(ResourceSceneError::Resource(
                    ResourceError::DuplicateReference,
                ))),
            "invalid refs accepted",
        )?;
        require(
            pixels(&actual_canvas)? == [16, 32, 48, 255],
            "rejected refs erased current pixels",
        )?;
        require(
            renderer.update(
                owner,
                label("changed-label"),
                geometry::flat(revision(1, 0)),
                &[],
            ) == Ok(PresentationOutcome::Duplicate {
                current: revision(1, 0),
            }),
            "duplicate not ignored",
        )?;
        require(
            pixels(&actual_canvas)? == [16, 32, 48, 255],
            "duplicate erased current pixels",
        )?;
        renderer
            .update(
                owner,
                label("scene-one"),
                geometry::flat(revision(1, 1)),
                std::slice::from_ref(&key),
            )
            .map_err(error)?;
        require(
            actual_canvas.is_same_node(Some(&canvas(&host.0)?.into())),
            "canvas replaced on same scene update",
        )?;
        require(
            scene_token.is_same_node(Some(&query(&host.0, "[data-token-key]")?.into())),
            "scene token replaced",
        )?;
        require(
            pixels(&actual_canvas)? == [16, 32, 48, 255] && released.get() == 0,
            "same-generation pixels/lease not retained and redrawn",
        )?;
        // Byte eviction notification triggers current-lease refresh without a scene snapshot.
        bytes.borrow_mut().release(&key).map_err(error)?;
        require(
            renderer.refresh() == Err(ImageSurfaceError::Resource(ResourceError::RevokedResource)),
            "byte-only revocation remained usable",
        )?;
        require(
            actual_canvas.width() == 0
                && renderer.surface_bytes() == 0
                && renderer.decoded_bytes() == 0,
            "revoked canvas/cache surface retained",
        )?;
        require(
            svg.is_connected() && scene_token.is_connected(),
            "optional image revocation removed flat scene",
        )?;
        install_source(&bytes, &key)?;
        let prepare = renderer.begin(&key, budget(), || {}).map_err(error)?;
        renderer
            .complete(&prepare, decoded(&bytes, &key, &released)?)
            .map_err(error)?;
        renderer.present(&key).map_err(error)?;
        renderer
            .update(
                owner,
                label("scene-one"),
                geometry::flat(revision(2, 0)),
                std::slice::from_ref(&key),
            )
            .map_err(error)?;
        require(
            actual_canvas.width() == 0 && renderer.decoded_bytes() == 0 && released.get() == 2,
            "recovery retained old generation pixels",
        )?;
        let aborts = Rc::new(Cell::new(0));
        let observed = aborts.clone();
        let cancelled = renderer
            .begin(&key, budget(), move || observed.set(observed.get() + 1))
            .map_err(error)?;
        renderer.cancel(&cancelled).map_err(error)?;
        require(
            renderer.work_bytes() == 24 && aborts.get() == 1,
            "abort request released running work",
        )?;
        require(
            renderer.begin(&key, budget(), || {}).err() == Some(ResourceError::AlreadyPending),
            "cancelled duplicate admitted",
        )?;
        require(
            renderer.complete(&cancelled, decoded(&bytes, &key, &released)?)
                == Err(ResourceError::Cancelled),
            "cancelled callback published",
        )?;
        let next = renderer.begin(&key, budget(), || {}).map_err(error)?;
        require(
            renderer.complete(&cancelled, decoded(&bytes, &key, &released)?)
                == Err(ResourceError::StaleDecode)
                && renderer.work_bytes() == 24,
            "stale callback altered new job",
        )?;
        renderer.dispose().map_err(error)?;
        require(
            !actual_canvas.is_connected()
                && host.0.query_selector("[data-scene-mount]")?.is_none()
                && renderer.work_bytes() == 24,
            "dispose retained nodes or freed running work",
        )?;
        require(
            renderer.complete(&next, decoded(&bytes, &key, &released)?)
                == Err(ResourceError::Closed)
                && renderer.work_bytes() == 0,
            "disposed callback returned pixels/work",
        )?;
        require(
            renderer.dispose() == Ok(PresentationOutcome::AlreadyDisposed),
            "second dispose not idempotent",
        )?;
        // The separate real canvas allocation bound rejects before any backing pixels exist.
        let mut bounded =
            BrowserResourceScene::mount(&host.0, owner, limits(), label("rgba8-surface-v1"), 7)
                .map_err(error)?;
        bounded
            .update(
                owner,
                label("scene-one"),
                geometry::flat(revision(1, 0)),
                std::slice::from_ref(&key),
            )
            .map_err(error)?;
        let prepare = bounded.begin(&key, budget(), || {}).map_err(error)?;
        bounded
            .complete(&prepare, decoded(&bytes, &key, &released)?)
            .map_err(error)?;
        require(
            bounded.present(&key) == Err(ImageSurfaceError::SurfaceCapacity),
            "display bound ignored",
        )?;
        require(
            canvas(&host.0)?.width() == 0 && query(&host.0, "[data-token-key]")?.is_connected(),
            "display failure erased scene or retained pixels",
        )?;
        drop(bounded);
        require(
            host.0.query_selector("canvas")?.is_none()
                && host.0.query_selector("[data-scene-mount]")?.is_none(),
            "drop retained canvas/mount",
        )?;
        require(
            bytes.borrow().lease_count() == 0,
            "retired consumer left a live lease",
        )?;
        bytes.borrow_mut().dispose();
        Ok(())
    }
    struct ObservedFixture {
        host: Host,
        renderer: BrowserResourceScene<CacheKey>,
        bytes: Rc<RefCell<AssetCache>>,
        key: CacheKey,
        released: Rc<Cell<usize>>,
        canvas: HtmlCanvasElement,
        token_node: Element,
        pending: Option<DecodeToken<CacheKey>>,
        phase: u8,
    }
    thread_local! { static OBSERVED: RefCell<Option<ObservedFixture>> = const {RefCell::new(None)}; }

    fn phase(fixture: &ObservedFixture) -> Result<(), JsValue> {
        fixture
            .host
            .0
            .set_attribute("data-resource-phase", &fixture.phase.to_string())?;
        fixture.host.0.set_attribute(
            "data-decoded-bytes",
            &fixture.renderer.decoded_bytes().to_string(),
        )?;
        fixture.host.0.set_attribute(
            "data-work-bytes",
            &fixture.renderer.work_bytes().to_string(),
        )?;
        fixture.host.0.set_attribute(
            "data-surface-bytes",
            &fixture.renderer.surface_bytes().to_string(),
        )?;
        Ok(())
    }

    /// One bounded fixture owner, retained for independent visual observation between calls.
    /// It is not a production global cache or an authority subscription.
    #[wasm_bindgen]
    pub fn resource_fixture_start() -> Result<(), JsValue> {
        OBSERVED.with(|slot| {
            require(slot.borrow().is_none(), "observed fixture already mounted")?;
            let document = web_sys::window()
                .and_then(|window| window.document())
                .ok_or_else(|| error("document unavailable"))?;
            let host = Host(document.create_element("section")?);
            host.0
                .set_attribute("data-resource-fixture", "current-canvas-consumer")?;
            document
                .body()
                .ok_or_else(|| error("body unavailable"))?
                .append_child(&host.0)?;
            let owner = owner();
            let (session, run, binding) = owner;
            let scope = CacheScope {
                session,
                run,
                binding,
            };
            let key = CacheKey {
                version: label("current-rgba-fixture-v1"),
                bytes: AssetManifest {
                    byte_len: 16,
                    sha256: DIGEST,
                },
            };
            let bytes = Rc::new(RefCell::new(
                AssetCache::new(
                    scope,
                    CacheLimits {
                        max_assets: 2,
                        max_pending: 1,
                        max_leases: 4,
                        max_bytes: 32,
                    },
                )
                .map_err(error)?,
            ));
            bytes
                .borrow_mut()
                .apply_current(scope, revision(1, 0), std::slice::from_ref(&key))
                .map_err(error)?;
            let released = Rc::new(Cell::new(0));
            let mut renderer =
                BrowserResourceScene::mount(&host.0, owner, limits(), label("rgba8-surface-v1"), 8)
                    .map_err(error)?;
            renderer
                .update(
                    owner,
                    label("scene-one"),
                    geometry::flat(revision(1, 0)),
                    std::slice::from_ref(&key),
                )
                .map_err(error)?;
            let work = renderer.begin(&key, budget(), || {}).map_err(error)?;
            install_source(&bytes, &key)?;
            renderer
                .complete(&work, decoded(&bytes, &key, &released)?)
                .map_err(error)?;
            renderer.present(&key).map_err(error)?;
            let canvas = canvas(&host.0)?;
            // Display scale makes one backing pixel inspectable; backing allocation stays 4B.
            canvas.set_attribute("style", "width:96px;height:96px;image-rendering:pixelated")?;
            let token_node = query(&host.0, "[data-token-key]")?;
            require(
                pixels(&canvas)? == [16, 32, 48, 255],
                "observed actual pixels differ",
            )?;
            let fixture = ObservedFixture {
                host,
                renderer,
                bytes,
                key,
                released,
                canvas,
                token_node,
                pending: None,
                phase: 0,
            };
            phase(&fixture)?;
            *slot.borrow_mut() = Some(fixture);
            Ok(())
        })
    }

    /// 0 initial pixels; 1 same-scene redraw; 2 byte-only revocation clears; 3 cancelled
    /// work still counted; 4 disposed owner still counts its actual pending callback;
    /// 5 terminal callback retired and work settled. Explicit dispose then removes fixture.
    #[wasm_bindgen]
    pub fn resource_fixture_advance() -> Result<u8, JsValue> {
        OBSERVED.with(|slot| {
            let mut borrow = slot.borrow_mut();
            let fixture = borrow
                .as_mut()
                .ok_or_else(|| error("observed fixture absent"))?;
            match fixture.phase {
                0 => {
                    fixture
                        .renderer
                        .update(
                            owner(),
                            label("scene-one"),
                            geometry::flat(revision(1, 1)),
                            std::slice::from_ref(&fixture.key),
                        )
                        .map_err(error)?;
                    require(
                        pixels(&fixture.canvas)? == [16, 32, 48, 255]
                            && fixture.released.get() == 0,
                        "observed redraw lost actual pixels/lease",
                    )?;
                    require(
                        fixture.token_node.is_same_node(Some(
                            &query(&fixture.host.0, "[data-token-key]")?.into(),
                        )),
                        "observed redraw replaced token",
                    )?;
                }
                1 => {
                    fixture
                        .bytes
                        .borrow_mut()
                        .release(&fixture.key)
                        .map_err(error)?;
                    require(
                        fixture.renderer.refresh()
                            == Err(ImageSurfaceError::Resource(ResourceError::RevokedResource)),
                        "observed revoked lease disclosed",
                    )?;
                    require(
                        fixture.canvas.width() == 0 && fixture.token_node.is_connected(),
                        "observed revoke did not clear optional pixels alone",
                    )?;
                }
                2 => {
                    install_source(&fixture.bytes, &fixture.key)?;
                    let token = fixture
                        .renderer
                        .begin(&fixture.key, budget(), || {})
                        .map_err(error)?;
                    fixture.renderer.cancel(&token).map_err(error)?;
                    require(
                        fixture.renderer.work_bytes() == 24,
                        "observed abort freed actual work",
                    )?;
                    fixture.pending = Some(token);
                }
                3 => {
                    let token = fixture
                        .pending
                        .as_ref()
                        .cloned()
                        .ok_or_else(|| error("cancelled operation missing"))?;
                    require(
                        fixture.renderer.complete(
                            &token,
                            decoded(&fixture.bytes, &fixture.key, &fixture.released)?,
                        ) == Err(ResourceError::Cancelled),
                        "observed cancelled operation published",
                    )?;
                    fixture.pending = Some(
                        fixture
                            .renderer
                            .begin(&fixture.key, budget(), || {})
                            .map_err(error)?,
                    );
                    fixture.renderer.dispose().map_err(error)?;
                    require(
                        !fixture.canvas.is_connected() && fixture.renderer.work_bytes() == 24,
                        "observed disposal freed live work or retained canvas",
                    )?;
                }
                4 => {
                    let token = fixture
                        .pending
                        .as_ref()
                        .cloned()
                        .ok_or_else(|| error("disposed pending operation missing"))?;
                    require(
                        fixture.renderer.complete(
                            &token,
                            decoded(&fixture.bytes, &fixture.key, &fixture.released)?,
                        ) == Err(ResourceError::Closed),
                        "observed disposed callback disclosed",
                    )?;
                    require(
                        fixture.renderer.work_bytes() == 0,
                        "observed terminal work leaked",
                    )?;
                    fixture.pending = None;
                }
                _ => return Err(error("observed fixture already terminal")),
            }
            fixture.phase += 1;
            phase(fixture)?;
            Ok(fixture.phase)
        })
    }

    #[wasm_bindgen]
    pub fn resource_fixture_dispose() -> Result<(), JsValue> {
        OBSERVED.with(|slot| {
            let mut borrow = slot.borrow_mut();
            if let Some(fixture) = borrow.as_mut() {
                fixture.renderer.dispose().map_err(error)?;
                // Fixture work is synchronous and explicitly controllable: this call settles
                // the confirmed terminal operation, never merely drops an abort reservation.
                if let Some(token) = fixture.pending.as_ref().cloned() {
                    fixture.renderer.finish_failed(&token).map_err(error)?;
                    fixture.pending = None;
                }
                require(
                    fixture.renderer.work_bytes() == 0,
                    "observed disposal left terminal work",
                )?;
                fixture.bytes.borrow_mut().dispose();
            }
            *borrow = None;
            Ok(())
        })
    }
}
