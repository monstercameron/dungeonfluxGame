#[cfg(target_arch = "wasm32")]
mod browser {
    use std::{cell::RefCell, rc::Rc};

    use df_ui::{LayoutRole, LayoutRoot, UiError, action_button, panel, text_input};
    use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
    use web_sys::{Document, Element, Event, HtmlButtonElement, HtmlInputElement};

    struct Click {
        button: HtmlButtonElement,
        callback: Closure<dyn FnMut(Event)>,
    }

    impl Click {
        fn bind(
            button: HtmlButtonElement,
            callback: impl FnMut(Event) + 'static,
        ) -> Result<Self, JsValue> {
            let callback = Closure::wrap(Box::new(callback) as Box<dyn FnMut(Event)>);
            button.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
            Ok(Self { button, callback })
        }

        fn dispose(&self) -> Result<(), JsValue> {
            self.button.remove_event_listener_with_callback(
                "click",
                self.callback.as_ref().unchecked_ref(),
            )
        }
    }

    struct Fixture {
        root: Element,
        player: Rc<LayoutRoot>,
        display: Rc<LayoutRoot>,
        controls: Vec<Click>,
    }

    thread_local! {
        static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
    }

    fn error(error: UiError) -> JsValue {
        JsValue::from_str(&error.to_string())
    }

    fn build_role(
        document: &Document,
        role: LayoutRole,
        name: &str,
    ) -> Result<(Rc<LayoutRoot>, HtmlInputElement), UiError> {
        let layout = Rc::new(LayoutRoot::create(document, role)?);
        let content = panel(document, name)?;
        let (field, input) = text_input(document, &format!("{name} draft"))?;
        content.append_child(&field)?;
        let native_action = action_button(document, "Native keyboard action", true)?;
        let unavailable_action = action_button(document, "Unavailable action", false)?;
        content.append_child(&native_action)?;
        content.append_child(&unavailable_action)?;
        layout.content().append_child(&content)?;
        Ok((layout, input))
    }

    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        if FIXTURE.with(|slot| slot.borrow().is_some()) {
            return Err(JsValue::from_str("fixture already mounted"));
        }
        let window =
            web_sys::window().ok_or_else(|| JsValue::from_str("browser window missing"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("browser document missing"))?;
        let body = document
            .body()
            .ok_or_else(|| JsValue::from_str("browser body missing"))?;
        let root = document.create_element("main")?;
        let heading = document.create_element("h1")?;
        heading.set_text_content(Some("DungeonFlux shared UI foundation fixture"));
        root.append_child(&heading)?;
        let notice = document.create_element("p")?;
        notice.set_text_content(Some(
            "Synthetic presentation fixture. Server authorization and gameplay are untested.",
        ));
        root.append_child(&notice)?;
        let status = document.create_element("p")?;
        status.set_attribute("role", "status")?;
        let failures_pass = matches!(panel(&document, " "), Err(UiError::EmptyLabel))
            && matches!(action_button(&document, "", true), Err(UiError::EmptyLabel))
            && matches!(text_input(&document, "\n"), Err(UiError::EmptyLabel));
        if !failures_pass {
            return Err(JsValue::from_str("empty-label rejection fixture failed"));
        }
        status.set_text_content(Some("Empty labels rejected. Ready for browser operation."));
        root.append_child(&status)?;
        let (player, player_input) =
            build_role(&document, LayoutRole::Player, "Player").map_err(error)?;
        let (display, _) =
            build_role(&document, LayoutRole::SharedDisplay, "Shared display").map_err(error)?;
        let literal_heading = panel(&document, "<img src=x onerror=alert(1)>").map_err(error)?;
        display.content().append_child(&literal_heading)?;
        let long_heading = panel(&document, "A deliberately long presentation heading keeps every word readable when this shared foundation is viewed on a narrow screen").map_err(error)?;
        display.content().append_child(&long_heading)?;
        let motion_sample =
            action_button(&document, "Reduced motion sample", true).map_err(error)?;
        motion_sample.set_attribute("style", "transition: opacity 2s linear")?;
        display.content().append_child(&motion_sample)?;
        let update = action_button(&document, "Update layout and preserve draft focus", true)
            .map_err(error)?;
        let remove = action_button(&document, "Unmount layouts", true).map_err(error)?;
        root.append_child(&update)?;
        root.append_child(&remove)?;
        root.append_child(player.root())?;
        root.append_child(display.root())?;
        let update_player = Rc::clone(&player);
        let update_display = Rc::clone(&display);
        let update_status = status.clone();
        let update_document = document.clone();
        let update_click = Click::bind(update, move |_| {
            let result = (|| -> Result<(), UiError> {
                player_input.focus()?;
                let active_before = update_document.active_element();
                let draft_before = player_input.value();
                update_player.set_role(LayoutRole::SharedDisplay)?;
                update_display.set_role(LayoutRole::Player)?;
                update_player.set_role(LayoutRole::Player)?;
                update_display.set_role(LayoutRole::SharedDisplay)?;
                let focused = active_before.as_ref().is_some_and(|before| {
                    update_document
                        .active_element()
                        .as_ref()
                        .is_some_and(|after| before.is_same_node(Some(after)))
                });
                if !focused || player_input.value() != draft_before {
                    return Err(UiError::Browser(JsValue::from_str(
                        "focus or draft changed during update",
                    )));
                }
                Ok(())
            })();
            match result {
                Ok(()) => update_status.set_text_content(Some(
                    "Update passed: same focused input and draft retained.",
                )),
                Err(error) => {
                    update_status.set_text_content(Some(&format!("Update failed: {error}")))
                }
            }
        })?;
        let remove_player = Rc::clone(&player);
        let remove_display = Rc::clone(&display);
        let remove_status = status;
        let remove_click = Click::bind(remove, move |_| {
            let result = remove_player
                .unmount()
                .and_then(|()| remove_display.unmount())
                .and_then(|()| remove_player.unmount())
                .and_then(|()| remove_display.unmount());
            match result {
                Ok(()) => remove_status.set_text_content(Some(
                    "Both layouts unmounted twice; no scoped layout styles remain.",
                )),
                Err(error) => {
                    remove_status.set_text_content(Some(&format!("Unmount failed: {error}")))
                }
            }
        })?;
        body.append_child(&root)?;
        FIXTURE.with(|slot| {
            *slot.borrow_mut() = Some(Fixture {
                root,
                player,
                display,
                controls: vec![update_click, remove_click],
            });
        });
        Ok(())
    }

    #[wasm_bindgen]
    pub fn shutdown() -> Result<(), JsValue> {
        FIXTURE.with(|slot| {
            let mut owned = slot.borrow_mut();
            if let Some(fixture) = owned.as_ref() {
                for control in &fixture.controls {
                    control.dispose()?;
                }
                fixture.player.unmount().map_err(error)?;
                fixture.display.unmount().map_err(error)?;
                if let Some(parent) = fixture.root.parent_node() {
                    parent.remove_child(&fixture.root)?;
                }
            }
            *owned = None;
            Ok(())
        })
    }
}
