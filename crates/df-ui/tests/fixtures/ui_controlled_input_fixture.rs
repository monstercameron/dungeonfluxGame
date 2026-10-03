#[cfg(target_arch = "wasm32")]
mod browser {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    use df_ui::{
        ActionView, ControlError, ControlledAction, ControlledTextInput, DraftError, DraftUpdate,
        InputFeedback, LayoutRole, LayoutRoot, MAX_DRAFT_UTF16_UNITS, TextInputView, UiError,
        panel,
    };
    use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
    use web_sys::{Document, Element, Event};

    const LITERAL_LABEL: &str = "<img src=x onerror=alert(1)> draft";
    const LITERAL_REJECTION: &str = "<svg onload=alert(1)> Rejected; edit your draft.";

    #[derive(Clone, Copy)]
    enum FixtureOperation {
        Reconcile,
        Pending,
        Reject,
        Invalidate,
        Verify,
    }

    struct Fixture {
        root: Element,
        layouts: Vec<LayoutRoot>,
        controls: Vec<ControlledAction>,
        field: Rc<ControlledTextInput>,
        action: Rc<ControlledAction>,
        document: Document,
        status: Element,
        changes: Rc<Cell<u32>>,
        latest_change: Rc<RefCell<Option<Result<String, DraftError>>>>,
        activations: Rc<Cell<u32>>,
        display_field: ControlledTextInput,
        display_action: ControlledAction,
    }

    thread_local! {
        static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
    }

    fn ui_error(error: UiError) -> JsValue {
        JsValue::from_str(&error.to_string())
    }

    fn control_error(error: ControlError) -> JsValue {
        JsValue::from_str(&error.to_string())
    }

    fn require(condition: bool, failure: &str) -> Result<(), JsValue> {
        if !condition {
            return Err(JsValue::from_str(failure));
        }
        Ok(())
    }

    fn view(feedback: InputFeedback<'_>) -> TextInputView<'_> {
        TextInputView {
            label: LITERAL_LABEL,
            enabled: true,
            feedback,
        }
    }

    fn action_view(pending: bool) -> ActionView<'static> {
        ActionView {
            label: "Native keyboard action <b>literal</b>",
            enabled: true,
            pending,
        }
    }

    fn preserve_check(document: &Document, field: &ControlledTextInput) -> Result<(), JsValue> {
        field.input().focus()?;
        field.input().set_selection_range(1, 3)?;
        let draft = field.draft();
        let start = field.input().selection_start()?;
        let end = field.input().selection_end()?;
        let input = field.input().clone();
        for _ in 0..100 {
            field
                .update(view(InputFeedback::None), DraftUpdate::Preserve)
                .map_err(control_error)?;
        }
        require(
            field.input().is_same_node(Some(&input)),
            "input was replaced",
        )?;
        require(
            document
                .active_element()
                .as_ref()
                .is_some_and(|active| active.is_same_node(Some(field.input()))),
            "focus was lost during reconciliation",
        )?;
        require(
            field.draft() == draft,
            "draft was lost during reconciliation",
        )?;
        require(
            field.input().selection_start()? == start && field.input().selection_end()? == end,
            "selection was changed during reconciliation",
        )
    }

    fn verify_contract(fixture: &Fixture) -> Result<(), JsValue> {
        let field = fixture.field.as_ref();
        let original_draft = field.draft();
        field
            .update(view(InputFeedback::None), DraftUpdate::Preserve)
            .map_err(control_error)?;
        fixture
            .action
            .update(action_view(false))
            .map_err(control_error)?;
        require(
            matches!(
                field.on_change(|_| {}),
                Err(ControlError::CallbackAlreadyRegistered)
            ),
            "a second input callback was accepted",
        )?;
        require(
            matches!(
                fixture.action.on_activate(|| {}),
                Err(ControlError::CallbackAlreadyRegistered)
            ),
            "a second activation callback was accepted",
        )?;
        let changes_before = fixture.changes.get();
        field.input().set_value("edited through native input event");
        field.input().dispatch_event(&Event::new("input")?)?;
        require(
            fixture.changes.get() == changes_before + 1
                && field.draft() == "edited through native input event",
            "native input did not deliver one controlled change",
        )?;
        field.input().dispatch_event(&Event::new("input")?)?;
        require(
            fixture.changes.get() == changes_before + 1,
            "same-value event duplicated change notification",
        )?;
        let draft_before = field.draft();
        preserve_check(&fixture.document, field)?;
        field
            .update(
                view(InputFeedback::Pending("Sending draft")),
                DraftUpdate::Preserve,
            )
            .map_err(control_error)?;
        require(field.input().read_only(), "pending draft remains editable")?;
        field.input().set_value("late pending mutation");
        field.input().dispatch_event(&Event::new("input")?)?;
        require(
            field.draft() == draft_before
                && field.input().value() == draft_before
                && fixture.latest_change.borrow().as_ref() == Some(&Err(DraftError::Unavailable)),
            "pending synthetic input mutated the controlled draft",
        )?;
        require(
            fixture
                .document
                .active_element()
                .as_ref()
                .is_some_and(|active| active.is_same_node(Some(field.input()))),
            "pending state removed input focus",
        )?;
        fixture
            .action
            .update(action_view(true))
            .map_err(control_error)?;
        require(
            fixture.action.element().disabled(),
            "pending action remains enabled",
        )?;
        let activations_before = fixture.activations.get();
        fixture.action.element().click();
        fixture
            .action
            .element()
            .dispatch_event(&Event::new("click")?)?;
        require(
            fixture.activations.get() == activations_before,
            "disabled action delivered an activation",
        )?;
        field
            .update(
                view(InputFeedback::Rejected(LITERAL_REJECTION)),
                DraftUpdate::Preserve,
            )
            .map_err(control_error)?;
        require(!field.input().read_only(), "rejected draft is not editable")?;
        field
            .input()
            .set_value(&"x".repeat(MAX_DRAFT_UTF16_UNITS + 1));
        field.input().dispatch_event(&Event::new("input")?)?;
        require(
            field.draft() == draft_before
                && field.input().value() == draft_before
                && fixture.latest_change.borrow().as_ref() == Some(&Err(DraftError::TooLong)),
            "oversized draft escaped the controlled bound",
        )?;
        require(field.draft() == draft_before, "rejection erased the draft")?;
        require(
            field.input().get_attribute("aria-invalid").as_deref() == Some("true"),
            "rejection did not expose aria-invalid",
        )?;
        let feedback_id = field
            .input()
            .get_attribute("aria-describedby")
            .ok_or_else(|| JsValue::from_str("rejection missing description association"))?;
        let feedback = fixture
            .document
            .get_element_by_id(&feedback_id)
            .ok_or_else(|| JsValue::from_str("description is not mounted"))?;
        require(
            feedback.text_content().as_deref() == Some(LITERAL_REJECTION)
                && feedback.child_element_count() == 0,
            "rejection content was interpreted as markup",
        )?;
        let label = field
            .root()
            .first_element_child()
            .and_then(|label| label.first_element_child())
            .ok_or_else(|| JsValue::from_str("native label caption missing"))?;
        require(
            label.text_content().as_deref() == Some(LITERAL_LABEL)
                && label.child_element_count() == 0,
            "label content was interpreted as markup",
        )?;
        require(
            fixture.action.element().text_content().as_deref() == Some(action_view(false).label)
                && fixture.action.element().child_element_count() == 0,
            "action label was interpreted as markup",
        )?;
        require(
            matches!(
                ControlledTextInput::create(
                    &fixture.document,
                    "player-draft",
                    view(InputFeedback::None),
                    ""
                ),
                Err(ControlError::IdAlreadyMounted)
            ),
            "duplicate mounted field identifier was accepted",
        )?;
        require(
            matches!(
                ControlledTextInput::create(
                    &fixture.document,
                    "invalid identifier",
                    view(InputFeedback::None),
                    ""
                ),
                Err(ControlError::InvalidId)
            ),
            "invalid field identifier was accepted",
        )?;
        let blank = TextInputView {
            label: " ",
            ..view(InputFeedback::None)
        };
        require(
            matches!(
                field.update(blank, DraftUpdate::Replace("erased")),
                Err(ControlError::Dom(UiError::EmptyLabel))
            ),
            "blank label was accepted",
        )?;
        require(
            matches!(
                field.update(
                    view(InputFeedback::Rejected("\n")),
                    DraftUpdate::Replace("erased")
                ),
                Err(ControlError::Dom(UiError::EmptyLabel))
            ),
            "blank rejection was accepted",
        )?;
        require(
            field.draft() == draft_before,
            "invalid update mutated draft",
        )?;
        field
            .update(view(InputFeedback::None), DraftUpdate::Replace(""))
            .map_err(control_error)?;
        require(
            field.draft().is_empty(),
            "ownership invalidation did not clear draft",
        )?;
        require(
            field.input().get_attribute("aria-describedby").is_none()
                && field.input().get_attribute("aria-invalid").as_deref() == Some("false"),
            "cleared rejection retained stale error attributes",
        )?;
        field
            .update(
                view(InputFeedback::None),
                DraftUpdate::Replace(&original_draft),
            )
            .map_err(control_error)?;
        fixture
            .action
            .update(action_view(false))
            .map_err(control_error)?;
        require(
            !fixture.action.element().disabled(),
            "action did not recover",
        )?;
        fixture.action.element().click();
        require(
            fixture.activations.get() == activations_before + 1,
            "native action did not deliver exactly one activation after updates",
        )?;
        verify_disposal(&fixture.document, &fixture.root)?;
        verify_callback_disposal(&fixture.document, &fixture.root)
    }

    fn verify_disposal(document: &Document, root: &Element) -> Result<(), JsValue> {
        let field = ControlledTextInput::create(
            document,
            "disposal-draft",
            view(InputFeedback::None),
            "sensitive test draft",
        )
        .map_err(control_error)?;
        let changed = Rc::new(Cell::new(0_u32));
        let change_count = Rc::clone(&changed);
        field
            .on_change(move |_| change_count.set(change_count.get() + 1))
            .map_err(control_error)?;
        root.append_child(field.root())?;
        let input = field.input().clone();
        field.dispose().map_err(control_error)?;
        field.dispose().map_err(control_error)?;
        input.set_value("stale input");
        input.dispatch_event(&Event::new("input")?)?;
        require(
            changed.get() == 0 && field.draft().is_empty(),
            "disposed field delivered private input",
        )?;
        require(
            matches!(
                field.update(view(InputFeedback::None), DraftUpdate::Replace("revive")),
                Err(ControlError::Draft(DraftError::Disposed))
            ),
            "disposed input accepted reconciliation",
        )?;
        let action =
            ControlledAction::create(document, action_view(false)).map_err(control_error)?;
        let called = Rc::new(Cell::new(0_u32));
        let call_count = Rc::clone(&called);
        action
            .on_activate(move || call_count.set(call_count.get() + 1))
            .map_err(control_error)?;
        root.append_child(action.element())?;
        let button = action.element().clone();
        action.dispose().map_err(control_error)?;
        action.dispose().map_err(control_error)?;
        button.dispatch_event(&Event::new("click")?)?;
        require(called.get() == 0, "disposed action delivered a callback")?;
        require(
            matches!(
                action.update(action_view(false)),
                Err(ControlError::Draft(DraftError::Disposed))
            ),
            "disposed action accepted reconciliation",
        )
    }

    fn verify_callback_disposal(document: &Document, root: &Element) -> Result<(), JsValue> {
        let field = Rc::new(
            ControlledTextInput::create(
                document,
                "callback-disposal-draft",
                view(InputFeedback::None),
                "private callback draft",
            )
            .map_err(control_error)?,
        );
        let weak_field = Rc::downgrade(&field);
        let changes = Rc::new(Cell::new(0_u32));
        let change_count = Rc::clone(&changes);
        let field_result = Rc::new(RefCell::new(None));
        let change_result = Rc::clone(&field_result);
        field
            .on_change(move |_| {
                change_count.set(change_count.get() + 1);
                *change_result.borrow_mut() = Some(match weak_field.upgrade() {
                    Some(owner) => owner.dispose().map_err(control_error),
                    None => Err(JsValue::from_str("input callback owner missing")),
                });
            })
            .map_err(control_error)?;
        root.append_child(field.root())?;
        let input = field.input().clone();
        input.set_value("dispose from callback");
        input.dispatch_event(&Event::new("input")?)?;
        require(
            changes.get() == 1
                && field_result.borrow().as_ref().is_some_and(Result::is_ok)
                && field.draft().is_empty()
                && field.root().parent_node().is_none(),
            "input callback disposal failed to clear and detach owned state",
        )?;
        input.set_value("stale callback input");
        input.dispatch_event(&Event::new("input")?)?;
        require(changes.get() == 1, "disposed input callback fired again")?;
        require(
            matches!(
                field.on_change(|_| {}),
                Err(ControlError::Draft(DraftError::Disposed))
            ),
            "disposed input callback registration was accepted",
        )?;

        let action =
            Rc::new(ControlledAction::create(document, action_view(false)).map_err(control_error)?);
        let weak_action = Rc::downgrade(&action);
        let activations = Rc::new(Cell::new(0_u32));
        let activation_count = Rc::clone(&activations);
        let action_result = Rc::new(RefCell::new(None));
        let activation_result = Rc::clone(&action_result);
        action
            .on_activate(move || {
                activation_count.set(activation_count.get() + 1);
                *activation_result.borrow_mut() = Some(match weak_action.upgrade() {
                    Some(owner) => owner.dispose().map_err(control_error),
                    None => Err(JsValue::from_str("action callback owner missing")),
                });
            })
            .map_err(control_error)?;
        root.append_child(action.element())?;
        let button = action.element().clone();
        button.click();
        require(
            activations.get() == 1
                && action_result.borrow().as_ref().is_some_and(Result::is_ok)
                && button.disabled()
                && button.parent_node().is_none(),
            "action callback disposal failed to disable and detach owned state",
        )?;
        button.dispatch_event(&Event::new("click")?)?;
        require(
            activations.get() == 1,
            "disposed action callback fired again",
        )?;
        require(
            matches!(
                action.on_activate(|| {}),
                Err(ControlError::Draft(DraftError::Disposed))
            ),
            "disposed action callback registration was accepted",
        )
    }

    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        require(
            !FIXTURE.with(|slot| slot.borrow().is_some()),
            "fixture already mounted",
        )?;
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window missing"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("document missing"))?;
        let body = document
            .body()
            .ok_or_else(|| JsValue::from_str("body missing"))?;
        let root = document.create_element("main")?;
        let title = document.create_element("h1")?;
        title.set_text_content(Some("DungeonFlux controlled input fixture"));
        root.append_child(&title)?;
        let status = document.create_element("p")?;
        status.set_attribute("role", "status")?;
        status.set_text_content(Some(
            "Synthetic fixture; keyboard/touch and visual review pending.",
        ));
        root.append_child(&status)?;
        let player = LayoutRoot::create(&document, LayoutRole::Player).map_err(ui_error)?;
        let player_panel = panel(&document, "Player controls").map_err(ui_error)?;
        let field = Rc::new(
            ControlledTextInput::create(
                &document,
                "player-draft",
                view(InputFeedback::None),
                "draft",
            )
            .map_err(control_error)?,
        );
        let action = Rc::new(
            ControlledAction::create(&document, action_view(false)).map_err(control_error)?,
        );
        let changes = Rc::new(Cell::new(0_u32));
        let latest_change = Rc::new(RefCell::new(None));
        let change_count = Rc::clone(&changes);
        let change_value = Rc::clone(&latest_change);
        field
            .on_change(move |event| {
                change_count.set(change_count.get().saturating_add(1));
                *change_value.borrow_mut() = Some(event);
            })
            .map_err(control_error)?;
        player_panel.append_child(field.root())?;
        player_panel.append_child(action.element())?;
        player.content().append_child(&player_panel)?;
        let display = LayoutRoot::create(&document, LayoutRole::SharedDisplay).map_err(ui_error)?;
        let display_panel = panel(&document, "Shared display controls").map_err(ui_error)?;
        let display_field = ControlledTextInput::create(
            &document,
            "display-draft",
            TextInputView {
                label: "A long localized label remains readable on a narrow display viewport",
                enabled: false,
                feedback: InputFeedback::None,
            },
            "Unavailable draft remains selectable",
        )
        .map_err(control_error)?;
        display_panel.append_child(display_field.root())?;
        let unavailable = ControlledAction::create(
            &document,
            ActionView {
                label: "Unavailable native action",
                enabled: false,
                pending: false,
            },
        )
        .map_err(control_error)?;
        display_panel.append_child(unavailable.element())?;
        display.content().append_child(&display_panel)?;
        root.append_child(player.root())?;
        root.append_child(display.root())?;
        let mut controls = Vec::new();
        for (label, operation) in [
            ("Reconcile 100 updates", FixtureOperation::Reconcile),
            ("Show pending", FixtureOperation::Pending),
            ("Reject and preserve draft", FixtureOperation::Reject),
            ("Invalidate draft ownership", FixtureOperation::Invalidate),
            ("Run DOM contract checks", FixtureOperation::Verify),
        ] {
            let button = ControlledAction::create(
                &document,
                ActionView {
                    label,
                    enabled: true,
                    pending: false,
                },
            )
            .map_err(control_error)?;
            player.content().append_child(button.element())?;
            let click_field = Rc::clone(&field);
            let click_action = Rc::clone(&action);
            let click_status = status.clone();
            let click_document = document.clone();
            button
                .on_activate(move || {
                    let result = (|| -> Result<(), JsValue> {
                        match operation {
                            FixtureOperation::Reconcile => {
                                preserve_check(&click_document, &click_field)?;
                            }
                            FixtureOperation::Pending => {
                                click_field.input().focus()?;
                                click_field
                                    .update(
                                        view(InputFeedback::Pending("Sending draft")),
                                        DraftUpdate::Preserve,
                                    )
                                    .map_err(control_error)?;
                                click_action
                                    .update(action_view(true))
                                    .map_err(control_error)?;
                            }
                            FixtureOperation::Reject => {
                                click_field
                                    .update(
                                        view(InputFeedback::Rejected(LITERAL_REJECTION)),
                                        DraftUpdate::Preserve,
                                    )
                                    .map_err(control_error)?;
                                click_action
                                    .update(action_view(false))
                                    .map_err(control_error)?;
                            }
                            FixtureOperation::Invalidate => {
                                click_field
                                    .update(view(InputFeedback::None), DraftUpdate::Replace(""))
                                    .map_err(control_error)?;
                                click_action
                                    .update(action_view(false))
                                    .map_err(control_error)?;
                            }
                            FixtureOperation::Verify => verify_widget_contract()?,
                        }
                        Ok(())
                    })();
                    match result {
                        Ok(()) => click_status.set_text_content(Some(label)),
                        Err(_) => click_status.set_text_content(Some("Control operation failed")),
                    }
                })
                .map_err(control_error)?;
            controls.push(button);
        }
        let action_status = status.clone();
        let activations = Rc::new(Cell::new(0_u32));
        let action_count = Rc::clone(&activations);
        action
            .on_activate(move || {
                action_count.set(action_count.get().saturating_add(1));
                action_status.set_text_content(Some(&format!(
                    "Native action activations: {}",
                    action_count.get()
                )));
            })
            .map_err(control_error)?;
        body.append_child(&root)?;
        FIXTURE.with(|slot| {
            *slot.borrow_mut() = Some(Fixture {
                root,
                layouts: vec![player, display],
                controls,
                field,
                action,
                document,
                status,
                changes,
                latest_change,
                activations,
                display_field,
                display_action: unavailable,
            });
        });
        Ok(())
    }

    /// Executable DOM assertions; a passing result alone does not prove visual review.
    #[wasm_bindgen]
    pub fn verify_widget_contract() -> Result<(), JsValue> {
        FIXTURE.with(|slot| {
            let owned = slot.borrow();
            let fixture = owned
                .as_ref()
                .ok_or_else(|| JsValue::from_str("fixture is not mounted"))?;
            verify_contract(fixture)?;
            fixture.status.set_text_content(Some(
                "DOM contract checks passed; independent browser review pending.",
            ));
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn shutdown() -> Result<(), JsValue> {
        FIXTURE.with(|slot| {
            let mut owned = slot.borrow_mut();
            if let Some(fixture) = owned.as_ref() {
                for control in &fixture.controls {
                    control.dispose().map_err(control_error)?;
                }
                fixture.field.dispose().map_err(control_error)?;
                fixture.action.dispose().map_err(control_error)?;
                fixture.display_field.dispose().map_err(control_error)?;
                fixture.display_action.dispose().map_err(control_error)?;
                for layout in &fixture.layouts {
                    layout.unmount().map_err(ui_error)?;
                    layout.unmount().map_err(ui_error)?;
                }
                if let Some(parent) = fixture.root.parent_node() {
                    parent.remove_child(&fixture.root)?;
                }
            }
            *owned = None;
            Ok(())
        })
    }
}
