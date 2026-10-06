#[cfg(target_arch = "wasm32")]
#[path = "locale_transition_catalog.rs"]
mod catalog_fixture;

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::{cell::RefCell, collections::BTreeMap};

    use df_locale::{CatalogLoadError, FormatError, MessagePart, TextKey, VersionedCatalog};
    use df_ui::{
        ConceptScene, SceneTransitionError, SceneTransitionPhase, SceneTransitionValidationError,
        SceneTransitionView,
    };
    use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
    use web_sys::{Element, Event, HtmlButtonElement};

    use super::catalog_fixture::CatalogFixture;

    struct ButtonControl {
        button: HtmlButtonElement,
        listener: Closure<dyn FnMut(Event)>,
    }

    struct Fixture {
        root: Element,
        status: Element,
        phase: SceneTransitionPhase,
        source: CatalogFixture,
        catalog: VersionedCatalog<u64>,
        french: bool,
        controls: Vec<ButtonControl>,
    }

    thread_local! {
        static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
    }

    fn view() -> SceneTransitionView<'static> {
        SceneTransitionView {
            scene: ConceptScene::SunkenHall,
            location: "Caller location",
            title: "Caller title",
            narration: "Caller narration",
            continue_label: "Caller action",
            continue_enabled: true,
            pending: false,
        }
    }

    fn failure() -> JsValue {
        JsValue::from_str("locale transition fixture assertion failed")
    }

    fn check(condition: bool) -> Result<(), JsValue> {
        if condition { Ok(()) } else { Err(failure()) }
    }

    fn literal_sink(phase: &SceneTransitionPhase) -> Result<(), JsValue> {
        check(phase.root().query_selector("b,script")?.is_none())
    }

    fn request(fixture: &Fixture) -> &[df_types::LocaleTag] {
        if fixture.french {
            &fixture.source.locales[1..]
        } else {
            &fixture.source.locales[..1]
        }
    }

    fn update(fixture: &Fixture) -> Result<(), JsValue> {
        let messages = fixture
            .source
            .messages(&fixture.catalog, request(fixture), 1)
            .map_err(|_| failure())?;
        fixture
            .phase
            .update_localized(&view(), fixture.catalog.version(), messages.each_ref())
            .map_err(|_| failure())?;
        literal_sink(&fixture.phase)?;
        let action = fixture
            .phase
            .root()
            .query_selector("button")?
            .ok_or_else(failure)?;
        let expected = if fixture.french {
            "Entrer dans les salles"
        } else {
            "Enter the halls"
        };
        check(action.text_content().as_deref() == Some(expected))
    }

    #[wasm_bindgen]
    pub fn locale_transition_step(step: &str) -> Result<(), JsValue> {
        FIXTURE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let fixture = slot.as_mut().ok_or_else(failure)?;
            let before = fixture.phase.root().text_content();
            match step {
                "english" | "french" => {
                    fixture.french = step == "french";
                    let action = fixture
                        .phase
                        .root()
                        .query_selector("button")?
                        .ok_or_else(failure)?;
                    update(fixture)?;
                    let after = fixture
                        .phase
                        .root()
                        .query_selector("button")?
                        .ok_or_else(failure)?;
                    check(action.is_same_node(Some(&after)))?;
                }
                "failed_reload" => {
                    let version = *fixture.catalog.version();
                    let result = fixture.catalog.reload(
                        version + 1,
                        &fixture.source.locales,
                        &fixture.source.declarations,
                        &fixture.source.entries(Some((1, 3))),
                    );
                    check(matches!(result, Err(CatalogLoadError::MissingEntry { .. })))?;
                    check(fixture.catalog.version() == &version)?;
                    update(fixture)?;
                    check(fixture.phase.root().text_content() == before)?;
                }
                "mixed_version" => {
                    let new = fixture
                        .source
                        .load(*fixture.catalog.version() + 1)
                        .map_err(|_| failure())?;
                    let old_messages = fixture
                        .source
                        .messages(&fixture.catalog, request(fixture), 1)
                        .map_err(|_| failure())?;
                    let new_messages = fixture
                        .source
                        .messages(&new, request(fixture), 1)
                        .map_err(|_| failure())?;
                    let result = fixture.phase.update_localized(
                        &view(),
                        fixture.catalog.version(),
                        [
                            &old_messages[0],
                            &new_messages[1],
                            &old_messages[2],
                            &old_messages[3],
                        ],
                    );
                    check(matches!(
                        result,
                        Err(SceneTransitionError::InvalidView(
                            SceneTransitionValidationError::CatalogVersionMismatch
                        ))
                    ))?;
                    check(fixture.phase.root().text_content() == before)?;
                }
                "invalid_text" => {
                    let mut source = CatalogFixture::new();
                    for language in &mut source.parts {
                        language[1] = vec![MessagePart::Literal(String::new())];
                    }
                    let invalid = source
                        .load(*fixture.catalog.version() + 1)
                        .map_err(|_| failure())?;
                    let messages = source
                        .messages(&invalid, request(fixture), 1)
                        .map_err(|_| failure())?;
                    let result = fixture.phase.update_localized(
                        &view(),
                        invalid.version(),
                        messages.each_ref(),
                    );
                    check(matches!(
                        result,
                        Err(SceneTransitionError::InvalidView(
                            SceneTransitionValidationError::EmptyText
                        ))
                    ))?;
                    check(fixture.phase.root().text_content() == before)?;
                }
                "missing_key" => {
                    let key = TextKey::parse("transition.missing").map_err(|_| failure())?;
                    let result = fixture
                        .catalog
                        .format(&key, request(fixture), &BTreeMap::new());
                    check(matches!(result, Err(FormatError::MissingKey { .. })))?;
                    check(fixture.phase.root().text_content() == before)?;
                }
                "replacement" => {
                    let version = *fixture.catalog.version() + 1;
                    for language in &mut fixture.source.parts {
                        language[0] =
                            vec![MessagePart::Literal(format!("Revised location {version}"))];
                    }
                    fixture
                        .catalog
                        .reload(
                            version,
                            &fixture.source.locales,
                            &fixture.source.declarations,
                            &fixture.source.entries(None),
                        )
                        .map_err(|_| failure())?;
                    update(fixture)?;
                    let location = fixture
                        .phase
                        .root()
                        .query_selector(".transition-location")?
                        .ok_or_else(failure)?;
                    check(
                        location.text_content().as_deref()
                            == Some(format!("Revised location {version}").as_str()),
                    )?;
                }
                _ => return Err(JsValue::from_str("unknown locale fixture step")),
            }
            fixture.status.set_text_content(Some(&format!(
                "Passed {step}; complete catalog revision {}",
                fixture.catalog.version()
            )));
            Ok(())
        })
    }

    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        check(FIXTURE.with(|slot| slot.borrow().is_none()))?;
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(failure)?;
        let body = document.body().ok_or_else(failure)?;
        let root = document.create_element("main")?;
        let status = document.create_element("p")?;
        status.set_attribute("role", "status")?;
        status.set_text_content(Some(
            "Loaded complete catalog revision 1; synthetic presentation fixture",
        ));
        root.append_child(&status)?;
        let source = CatalogFixture::new();
        let catalog = source.load(1).map_err(|_| failure())?;
        let messages = source
            .messages(&catalog, &source.locales[..1], 1)
            .map_err(|_| failure())?;
        let phase =
            SceneTransitionPhase::create_localized(&document, &view(), &1, messages.each_ref())
                .map_err(|_| failure())?;
        literal_sink(&phase)?;
        let mut controls = Vec::new();
        for step in [
            "english",
            "french",
            "failed_reload",
            "mixed_version",
            "invalid_text",
            "missing_key",
            "replacement",
        ] {
            let button: HtmlButtonElement = document.create_element("button")?.dyn_into()?;
            button.set_text_content(Some(step));
            button.set_attribute("data-locale-step", step)?;
            let callback = Closure::wrap(Box::new(move |_event: Event| {
                if locale_transition_step(step).is_err() {
                    FIXTURE.with(|slot| {
                        if let Some(fixture) = slot.borrow().as_ref() {
                            fixture
                                .status
                                .set_text_content(Some("Failed locale fixture step"));
                        }
                    });
                }
            }) as Box<dyn FnMut(Event)>);
            button.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
            root.append_child(&button)?;
            controls.push(ButtonControl {
                button,
                listener: callback,
            });
        }
        root.append_child(phase.root())?;
        body.append_child(&root)?;
        FIXTURE.with(|slot| {
            *slot.borrow_mut() = Some(Fixture {
                root,
                status,
                phase,
                source,
                catalog,
                french: false,
                controls,
            })
        });
        Ok(())
    }

    #[wasm_bindgen]
    pub fn dispose_locale_transition_fixture() -> Result<(), JsValue> {
        let fixture = FIXTURE
            .with(|slot| slot.borrow_mut().take())
            .ok_or_else(failure)?;
        let mut failed = false;
        for control in fixture.controls {
            if control
                .button
                .remove_event_listener_with_callback(
                    "click",
                    control.listener.as_ref().unchecked_ref(),
                )
                .is_err()
            {
                failed = true;
            }
        }
        if fixture.phase.dispose().is_err() {
            failed = true;
        }
        fixture.root.remove();
        check(!failed)
    }
}
