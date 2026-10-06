#[cfg(target_arch = "wasm32")]
#[path = "../support/mod.rs"]
mod support;

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::RefCell;

    use df_assets::AssetManifest;
    use df_client::cache::{CacheKey, CacheLimits, CacheScope};
    use df_client::revisions::{ViewAcceptance, ViewStore};
    use df_render::{FlatScene, PresentationOutcome, RenderError, SceneOwner, SceneRenderer};
    use df_types::{ClientBindingId, RevisionLabel};
    use df_ui::{
        CampaignError, CampaignLimits, CampaignSceneAssets, CampaignSurface, CampaignView,
        ConceptScene, LayoutRole, LayoutRoot, SceneImageError, SceneImageLimits, action_button,
        text_input,
    };
    use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
    use web_sys::{Element, Event, HtmlButtonElement, HtmlInputElement};

    use super::support;

    struct MountRollback {
        root: Element,
        armed: bool,
    }
    impl Drop for MountRollback {
        fn drop(&mut self) {
            if self.armed {
                self.root.set_text_content(None);
                self.root.remove();
            }
        }
    }

    struct Control {
        button: HtmlButtonElement,
        callback: Closure<dyn FnMut(Event)>,
    }

    struct Fixture {
        layout: LayoutRoot,
        renderer: SceneRenderer,
        campaign: CampaignSurface,
        views: ViewStore<FlatScene>,
        owner: SceneOwner,
        scene: FlatScene,
        svg: Element,
        token: Element,
        retained_label: Element,
        draft: HtmlInputElement,
        status: Element,
        controls: Vec<Control>,
        closed: bool,
    }

    thread_local! {
        static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
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

    fn query(root: &Element, selector: &str) -> Result<Element, JsValue> {
        root.query_selector(selector)?
            .ok_or_else(|| error("missing mounted element"))
    }

    fn campaign_view() -> CampaignView<'static> {
        CampaignView {
            scene: ConceptScene::Harbor,
            chapter: "Synthetic scene-layer fixture",
            title: "Supplied positions",
            description: "Audience-safe input only. No gameplay session or rules resolution.",
            location: "Fixture chamber",
            scene_label: "Current illustration",
            narration: "Optional media failure must preserve the flat scene and usable input.",
            connection: "Fixture connected",
            notice: "Source and browser boundary fixture",
            members: &[],
            objectives: &[],
        }
    }

    fn apply(fixture: &mut Fixture, scene: FlatScene) -> Result<(), JsValue> {
        let revision = scene.revision;
        let admission = fixture
            .views
            .accept(fixture.owner.2, revision, scene.clone());
        require(
            admission == ViewAcceptance::Applied,
            "client did not admit newer view",
        )?;
        let accepted = fixture
            .views
            .current()
            .ok_or_else(|| error("accepted view absent"))?
            .1
            .clone();
        let outcome = fixture
            .renderer
            .update(fixture.owner, accepted.clone())
            .map_err(error)?;
        require(
            matches!(outcome, PresentationOutcome::Applied { .. }),
            "renderer did not apply view",
        )?;
        fixture.scene = accepted;
        Ok(())
    }

    fn next_scene(fixture: &Fixture, x: f64) -> Result<FlatScene, JsValue> {
        let mut scene = fixture.scene.clone();
        scene.revision = scene.revision.next_sequence().map_err(error)?;
        let token = scene
            .tokens
            .first_mut()
            .ok_or_else(|| error("fixture token missing"))?;
        token.position.x = x;
        Ok(scene)
    }

    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        require(
            FIXTURE.with(|slot| slot.borrow().is_none()),
            "fixture already mounted",
        )?;
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| error("document unavailable"))?;
        let body = document.body().ok_or_else(|| error("body unavailable"))?;
        let layout = LayoutRoot::create(&document, LayoutRole::Player).map_err(error)?;
        body.append_child(layout.root())?;
        let mut rollback = MountRollback {
            root: layout.root().clone(),
            armed: true,
        };
        let scene_host = document.create_element("section")?;
        scene_host.set_attribute("aria-label", "Flat scene and token layers")?;
        layout.content().append_child(&scene_host)?;
        let owner = support::owner();
        let mut renderer = SceneRenderer::mount(&scene_host, owner).map_err(error)?;
        let scene = support::scene(1, 0, -3.125);
        let mut views = ViewStore::new(owner.2);
        require(
            views.accept(owner.2, scene.revision, scene.clone()) == ViewAcceptance::Applied,
            "initial view refused",
        )?;
        let accepted = views
            .current()
            .ok_or_else(|| error("initial accepted view absent"))?
            .1
            .clone();
        renderer.update(owner, accepted).map_err(error)?;
        let svg = query(&scene_host, "svg")?;
        let token = query(&svg, "[data-token-key]")?;
        let retained_label = query(&token, "text")?;
        require(
            svg.get_attribute("viewBox").as_deref() == Some("-10 -20 100 80"),
            "supplied viewport changed",
        )?;
        let floor = query(&svg, "[data-layer-key]")?;
        require(
            floor.get_attribute("x").as_deref() == Some("-4.25"),
            "supplied layer x changed",
        )?;
        require(
            floor.get_attribute("y").as_deref() == Some("7.5"),
            "supplied layer y changed",
        )?;
        require(
            token.get_attribute("transform").as_deref() == Some("translate(-3.125 12.5)"),
            "supplied token position changed",
        )?;
        require(
            retained_label.text_content().as_deref() == Some("Ari <script> & friends"),
            "token label is not exact plain text",
        )?;
        require(
            svg.query_selector("script")?.is_none(),
            "token label became markup",
        )?;
        let (field, draft) = text_input(&document, "Local presentation draft").map_err(error)?;
        draft.set_value("Retain this valid draft");
        layout.content().append_child(&field)?;
        let status = document.create_element("p")?;
        status.set_attribute("role", "status")?;
        status.set_text_content(Some(
            "Synthetic source fixture: exact scene positions, persistent nodes and owned disposal.",
        ));
        layout.content().append_child(&status)?;
        let campaign = CampaignSurface::create(
            &document,
            &campaign_view(),
            CampaignLimits {
                max_members: 4,
                max_objectives: 4,
                max_text_bytes: 512,
            },
        )
        .map_err(error)?;
        layout.content().append_child(campaign.root())?;
        campaign
            .enable_scene_assets(
                CacheScope {
                    session: owner.0,
                    run: owner.1,
                    binding: owner.2,
                },
                SceneImageLimits {
                    cache: CacheLimits {
                        max_assets: 2,
                        max_pending: 1,
                        max_leases: 1,
                        max_bytes: 128,
                    },
                    max_width: 8,
                    max_height: 8,
                    max_pixels: 64,
                },
            )
            .map_err(error)?;
        let fixture = Fixture {
            layout,
            renderer,
            campaign,
            views,
            owner,
            scene,
            svg,
            token,
            retained_label,
            draft,
            status,
            controls: Vec::new(),
            closed: false,
        };
        FIXTURE.with(|slot| *slot.borrow_mut() = Some(fixture));
        rollback.armed = false;
        let setup = FIXTURE.with(|slot| {
            let mut owned = slot.borrow_mut();
            let fixture = owned.as_mut().ok_or_else(|| error("fixture unavailable"))?;
            for (case, label) in [
                (0, "128 same-phase updates"),
                (1, "Reject old owner"),
                (2, "Reject stale and duplicate views"),
                (3, "Recover next epoch"),
                (4, "Reject invalid geometry"),
                (5, "Fail actual optional image"),
                (6, "Omit removed token"),
                (7, "Dispose and scrub twice"),
            ] {
                let button = action_button(&document, label, true).map_err(error)?;
                button.set_attribute("data-fixture-case", &case.to_string())?;
                let status = fixture.status.clone();
                let callback = Closure::wrap(Box::new(move |_| {
                    if let Err(failure) = run_fixture_case(case) {
                        status.set_text_content(Some(&format!("Fixture failure: {failure:?}")));
                    }
                }) as Box<dyn FnMut(Event)>);
                button
                    .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
                fixture.layout.content().append_child(&button)?;
                fixture.controls.push(Control { button, callback });
            }
            Ok::<_, JsValue>(())
        });
        if let Err(failure) = setup {
            shutdown()?;
            return Err(failure);
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub fn run_fixture_case(case: u32) -> Result<(), JsValue> {
        FIXTURE.with(|slot| {
            let mut owned = slot.borrow_mut();
            let fixture = owned.as_mut().ok_or_else(|| error("fixture unavailable"))?;
            require(!fixture.closed, "fixture disposed")?;
            match case {
                0 => {
                    fixture.draft.focus()?;
                    let draft = fixture.draft.value();
                    for index in 0..128 {
                        let next = next_scene(fixture, index as f64)?;
                        apply(fixture, next)?;
                        require(
                            fixture
                                .svg
                                .is_same_node(Some(&query(fixture.layout.root(), "svg")?.into())),
                            "SVG remounted",
                        )?;
                        require(
                            fixture.token.is_same_node(Some(
                                &query(&fixture.svg, "[data-token-key]")?.into(),
                            )),
                            "token remounted",
                        )?;
                        require(
                            query(&fixture.svg, "[data-scene-tokens]")?.child_element_count() == 1,
                            "token nodes accumulated",
                        )?;
                    }
                    fixture
                        .layout
                        .set_role(LayoutRole::SharedDisplay)
                        .map_err(error)?;
                    fixture.layout.set_role(LayoutRole::Player).map_err(error)?;
                    require(
                        fixture
                            .svg
                            .is_same_node(Some(&query(fixture.layout.root(), "svg")?.into())),
                        "role switch remounted scene",
                    )?;
                    require(fixture.draft.value() == draft, "valid draft changed")?;
                    let document = fixture
                        .layout
                        .root()
                        .owner_document()
                        .ok_or_else(|| error("document unavailable"))?;
                    require(
                        document
                            .active_element()
                            .is_some_and(|active| active.is_same_node(Some(&fixture.draft))),
                        "focus changed",
                    )?;
                }
                1 => {
                    let wrong = (
                        fixture.owner.0,
                        fixture.owner.1,
                        ClientBindingId::from_bytes(&[8; 16]).map_err(error)?,
                    );
                    let next = next_scene(fixture, 999.0)?;
                    require(
                        fixture.renderer.update(wrong, next.clone())
                            == Err(RenderError::WrongOwner),
                        "old owner rendered",
                    )?;
                    require(
                        fixture.views.accept(wrong.2, next.revision, next)
                            == ViewAcceptance::WrongBinding,
                        "old binding admitted",
                    )?;
                }
                2 => {
                    let mut duplicate = fixture.scene.clone();
                    duplicate
                        .tokens
                        .first_mut()
                        .ok_or_else(|| error("token missing"))?
                        .position
                        .x = 999.0;
                    require(
                        matches!(
                            fixture.renderer.update(fixture.owner, duplicate.clone()),
                            Ok(PresentationOutcome::Duplicate { .. })
                        ),
                        "duplicate replaced scene",
                    )?;
                    require(
                        matches!(
                            fixture
                                .views
                                .accept(fixture.owner.2, duplicate.revision, duplicate),
                            ViewAcceptance::Duplicate { .. }
                        ),
                        "client duplicate admitted",
                    )?;
                    let stale = support::scene(1, 0, 999.0);
                    let result = fixture.renderer.update(fixture.owner, stale);
                    require(
                        matches!(
                            result,
                            Ok(PresentationOutcome::Stale { .. }
                                | PresentationOutcome::Duplicate { .. })
                        ),
                        "stale view rendered",
                    )?;
                }
                3 => {
                    let epoch = fixture
                        .scene
                        .revision
                        .epoch()
                        .get()
                        .checked_add(1)
                        .ok_or_else(|| error("epoch exhausted"))?;
                    apply(fixture, support::scene(epoch, 0, -8.5))?;
                    require(
                        fixture
                            .svg
                            .is_same_node(Some(&query(fixture.layout.root(), "svg")?.into())),
                        "recovery remounted scene",
                    )?;
                    require(
                        fixture.token.get_attribute("transform").as_deref()
                            == Some("translate(-8.5 12.5)"),
                        "recovery position changed",
                    )?;
                }
                4 => {
                    let invalid = next_scene(fixture, f64::NAN)?;
                    require(
                        fixture.renderer.update(fixture.owner, invalid)
                            == Err(RenderError::InvalidGeometry),
                        "nonfinite geometry rendered",
                    )?;
                    require(
                        fixture.renderer.current() == Some(&fixture.scene),
                        "invalid input replaced current snapshot",
                    )?;
                }
                5 => {
                    // Exact existing byte identity, verified by the actual client cache;
                    // bytes are deliberately not PNG. No replacement image is invented.
                    let next = next_scene(fixture, 7.0)?;
                    apply(fixture, next)?;
                    let key = CacheKey {
                        version: RevisionLabel::new(Some("fixture-invalid-png-v1"))
                            .map_err(error)?,
                        bytes: AssetManifest {
                            byte_len: 3,
                            sha256: [
                                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40,
                                0xde, 0x5d, 0xae, 0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17,
                                0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
                            ],
                        },
                    };
                    let token = fixture
                        .campaign
                        .update_with_scene_assets(
                            &campaign_view(),
                            CampaignSceneAssets {
                                scope: CacheScope {
                                    session: fixture.owner.0,
                                    run: fixture.owner.1,
                                    binding: fixture.owner.2,
                                },
                                revision: fixture.scene.revision,
                                references: std::slice::from_ref(&key),
                                selected: Some(&key),
                            },
                        )
                        .map_err(error)?
                        .ok_or_else(|| error("expected fresh image request"))?;
                    require(
                        matches!(
                            fixture
                                .campaign
                                .complete_scene_asset(&token, b"abc".to_vec()),
                            Err(CampaignError::SceneImage(SceneImageError::InvalidPng))
                        ),
                        "invalid bytes became image",
                    )?;
                    require(
                        fixture.svg.is_connected(),
                        "asset failure removed flat renderer",
                    )?;
                    require(
                        fixture.token.get_attribute("transform").as_deref()
                            == Some("translate(7 12.5)"),
                        "asset failure changed geometry",
                    )?;
                    require(fixture.draft.is_connected(), "asset failure removed input")?;
                }
                6 => {
                    let mut empty = next_scene(fixture, 0.0)?;
                    empty.tokens.clear();
                    apply(fixture, empty)?;
                    require(
                        fixture.svg.query_selector("[data-token-key]")?.is_none(),
                        "omitted token retained",
                    )?;
                    require(
                        fixture.retained_label.text_content().as_deref() == Some(""),
                        "omitted label retained",
                    )?;
                    require(
                        fixture.token.get_attribute("transform").is_none(),
                        "omitted coordinates retained",
                    )?;
                }
                7 => {
                    require(
                        fixture.renderer.dispose().map_err(error)? == PresentationOutcome::Disposed,
                        "first dispose mismatch",
                    )?;
                    require(
                        fixture.renderer.dispose().map_err(error)?
                            == PresentationOutcome::AlreadyDisposed,
                        "repeat disposal changed owner",
                    )?;
                    require(
                        fixture.retained_label.text_content().as_deref() == Some(""),
                        "old token label retained",
                    )?;
                    require(
                        fixture.token.get_attribute("transform").is_none(),
                        "old token position retained",
                    )?;
                    require(
                        fixture.svg.get_attribute("aria-label").is_none(),
                        "old scene label retained",
                    )?;
                    require(
                        !fixture.renderer.capabilities().flat_svg,
                        "disposed readiness fabricated",
                    )?;
                    require(
                        fixture.draft.is_connected(),
                        "scene disposal removed shell input",
                    )?;
                    fixture.closed = true;
                    fixture.scene.tokens.clear();
                    fixture.scene.layers.clear();
                    fixture.scene.label.clear();
                    fixture.views = ViewStore::new(fixture.owner.2);
                }
                _ => return Err(error("unknown fixture case")),
            }
            if !fixture.closed {
                require(
                    fixture.renderer.current() == Some(&fixture.scene),
                    "accepted snapshot lost after fixture boundary",
                )?;
            }
            fixture.status.set_text_content(Some(&format!(
                "Fixture case {case} passed; supplied scene positions and owner lifecycle checked."
            )));
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn shutdown() -> Result<(), JsValue> {
        FIXTURE.with(|slot| {
            let mut owned = slot.borrow_mut();
            if let Some(fixture) = owned.as_mut() {
                for control in &fixture.controls {
                    control.button.remove_event_listener_with_callback(
                        "click",
                        control.callback.as_ref().unchecked_ref(),
                    )?;
                }
                fixture.renderer.dispose().map_err(error)?;
                fixture.campaign.dispose().map_err(error)?;
                fixture.draft.set_value("");
                fixture.layout.unmount().map_err(error)?;
            }
            *owned = None;
            Ok(())
        })
    }
}
