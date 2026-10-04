//! Reachable synthetic player-sheet client. No RPC, catalog, rules, inventory
//! mutation or progression authority is connected. Every displayed value and
//! offer is a fixed, already filtered fixture input.

#[cfg(target_arch = "wasm32")]
mod browser {
    use df_player::{PlayerSheetConnection, PlayerSheetScreen};
    use df_ui::{
        ActionView, CharacterSheetView, ControlledAction, SheetField, SheetLabels, SheetOffer,
        SheetOwnerGeneration, SheetRow, SheetSection, SheetSubmission, SheetTab,
    };
    use std::{cell::RefCell, rc::Rc};
    use wasm_bindgen::{JsCast, prelude::*};
    use web_sys::{
        Document, Element, Event, HtmlButtonElement, HtmlElement, HtmlInputElement, Node,
    };

    struct Fixture {
        screen: Rc<RefCell<PlayerSheetScreen>>,
        controls: Vec<ControlledAction>,
    }
    thread_local! {
        static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
    }
    fn error(value: impl std::fmt::Display) -> JsValue {
        JsValue::from_str(&value.to_string())
    }
    fn check(condition: bool, message: &str) -> Result<(), JsValue> {
        if condition {
            Ok(())
        } else {
            Err(JsValue::from_str(message))
        }
    }
    fn required(root: &Element, selector: &str) -> Result<Element, JsValue> {
        root.query_selector(selector)?
            .ok_or_else(|| error("fixture node missing"))
    }
    fn html(root: &Element, selector: &str) -> Result<HtmlElement, JsValue> {
        required(root, selector)?
            .dyn_into()
            .map_err(|_| error("fixture HTML node missing"))
    }
    fn child(
        document: &Document,
        parent: &Element,
        tag: &str,
        text: &str,
    ) -> Result<Element, JsValue> {
        let element = document.create_element(tag)?;
        element.set_text_content(Some(text));
        parent.append_child(&element)?;
        Ok(element)
    }

    fn with_view<T>(
        owner: u64,
        revision: u64,
        replacement: bool,
        operation: impl FnOnce(&CharacterSheetView<'_>) -> T,
    ) -> T {
        let fields = [SheetField {
            label: "Private fixture detail",
            value: "PLAYER-SHEET-PRIVATE-SENTINEL",
        }];
        let equip = [SheetOffer {
            id: if replacement {
                "fixture-equip-current-002"
            } else {
                "fixture-equip-original-001"
            },
            label: "Send advertised equipment selection · fixture",
            enabled: true,
            pending: false,
        }];
        let reward = [SheetOffer {
            id: "fixture-reward-pending-003",
            label: "Reward request pending · supplied",
            enabled: true,
            pending: true,
        }];
        let entries = [
            (
                "character",
                SheetTab::Character,
                "Character",
                "Fixture abilities",
                "Supplied values",
            ),
            (
                "equipment",
                SheetTab::Equipment,
                "Equipment",
                "Fixture lantern",
                "Carried · supplied",
            ),
            (
                "spells",
                SheetTab::Spells,
                "Spells",
                "Fixture prepared magic",
                "Prepared · supplied",
            ),
            (
                "journal",
                SheetTab::Journal,
                "Journal",
                "Fixture harbor journal",
                "Permitted memory",
            ),
            (
                "progression",
                SheetTab::Progression,
                "Progression",
                "Fixture advancement",
                "Awaiting server confirmation",
            ),
        ];
        let rows: Vec<_> = entries
            .iter()
            .map(|(key, tab, _, title, value)| SheetRow {
                key,
                title,
                value,
                summary: "Fixture private description; navigation changes no game state.",
                details: &fields,
                offers: match tab {
                    SheetTab::Equipment => &equip,
                    SheetTab::Progression => &reward,
                    _ => &[],
                },
            })
            .collect();
        let sections: Vec<_> = entries
            .iter()
            .zip(rows.iter())
            .map(|((key, tab, title, _, _), row)| SheetSection {
                key,
                tab: *tab,
                title,
                caption: "Synthetic filtered player view · Production RPC pending",
                rows: std::slice::from_ref(row),
            })
            .collect();
        operation(&CharacterSheetView {
            owner: SheetOwnerGeneration(owner),
            revision,
            name: "Vell Ashwalker · synthetic personal sheet",
            identity: "Character identity supplied by owner",
            subtitle: "Inspect your pack, remembered stories and the next chapter.",
            connection: "Synthetic preview · Production RPC pending",
            notice: "No equipment, spell, reward or advancement resolves in this fixture.",
            labels: SheetLabels {
                tabs: ["Character", "Equipment", "Spells", "Journal", "Progression"],
                navigation: "Personal sheet navigation",
                filter: "Filter your current sheet",
                no_matches: "No matching permitted entries",
                art_fallback: "Optional artwork unavailable · Sheet remains usable",
            },
            sections: &sections,
        })
    }

    fn select_tab(root: &Element, index: usize) -> Result<(), JsValue> {
        let id = required(root, ".df-sheet")?.id();
        let selector = format!("#{id}-tab-{index}");
        html(root, &selector)?.click();
        check(
            required(root, &selector)?
                .get_attribute("aria-selected")
                .as_deref()
                == Some("true"),
            "local tab selection failed",
        )?;
        let panel = required(root, &format!("#{id}-panel-{index}"))?;
        check(
            !panel.has_attribute("hidden"),
            "selected panel remained hidden",
        )
    }
    fn offer(root: &Element) -> Result<HtmlButtonElement, JsValue> {
        required(root, "[data-sheet-offer^='fixture-equip']")?
            .dyn_into()
            .map_err(|_| error("equipment offer button missing"))
    }
    fn sanitized(element: &Element, text: &Node) -> Result<(), JsValue> {
        check(
            element.text_content().as_deref() == Some("")
                && text.node_value().as_deref() == Some(""),
            "retained private node was not scrubbed",
        )
    }

    /// Actual mounted role/component transitions, rerunnable without a timer,
    /// network operation, synthetic rules engine or production authority claim.
    fn boundary_checks(document: &Document, parent: &Element) -> Result<(), JsValue> {
        let slot = child(document, parent, "div", "")?;
        let submissions = Rc::new(RefCell::new(Vec::<SheetSubmission>::new()));
        let received = Rc::clone(&submissions);
        let mut screen = with_view(41, 1, false, |view| {
            PlayerSheetScreen::mount(
                document,
                &slot,
                "sheet-client-boundary",
                view,
                PlayerSheetConnection::Connected,
                move |submission| received.borrow_mut().push(submission),
            )
        })
        .map_err(error)?;
        let root = screen.root().clone();
        let sheet = required(&root, ".df-sheet")?;
        for index in 0..5 {
            select_tab(&root, index)?;
        }
        check(
            submissions.borrow().is_empty(),
            "local navigation emitted a game selection",
        )?;
        select_tab(&root, 1)?;
        let details = required(&root, "[data-sheet-row='equipment']")?;
        details.set_attribute("open", "")?;
        let filter: HtmlInputElement = required(&root, ".sheet-filter input")?
            .dyn_into()
            .map_err(|_| error("sheet filter missing"))?;
        filter.set_value("fixture");
        filter.dispatch_event(&Event::new("input")?)?;
        filter.focus()?;
        for revision in 2..22 {
            with_view(41, revision, false, |view| screen.update(view)).map_err(error)?;
            check(
                required(&root, ".df-sheet")?.is_same_node(Some(&sheet)),
                "update remounted sheet",
            )?;
            check(
                required(&root, "[data-sheet-row='equipment']")?.is_same_node(Some(&details)),
                "update replaced keyed expandable row",
            )?;
            check(
                details.has_attribute("open") && filter.value() == "fixture",
                "update lost local inspection state",
            )?;
            check(
                document
                    .active_element()
                    .as_ref()
                    .is_some_and(|node| filter.is_same_node(Some(node))),
                "update lost valid filter focus",
            )?;
        }
        let original = offer(&root)?;
        original.click();
        check(
            submissions.borrow().as_slice()
                == [SheetSubmission {
                    owner: SheetOwnerGeneration(41),
                    offer_id: "fixture-equip-original-001".into(),
                }],
            "submission changed the supplied owner or exact offer identity",
        )?;
        check(
            details
                .text_content()
                .as_deref()
                .is_some_and(|text| text.contains("Carried · supplied")),
            "submission mutated supplied inventory",
        )?;
        original.focus()?;
        screen
            .set_connection(PlayerSheetConnection::Offline)
            .map_err(error)?;
        check(
            original.disabled(),
            "offline action remained visibly enabled",
        )?;
        original.click();
        original.dispatch_event(&Event::new("click")?)?;
        check(
            submissions.borrow().len() == 1,
            "offline offer callback escaped fence",
        )?;
        for index in 0..5 {
            select_tab(&root, index)?;
        }
        screen
            .set_connection(PlayerSheetConnection::Reconnecting)
            .map_err(error)?;
        with_view(41, 22, true, |view| screen.update(view)).map_err(error)?;
        let current = offer(&root)?;
        check(
            current.disabled(),
            "offline update reenabled supplied offer",
        )?;
        original.dispatch_event(&Event::new("click")?)?;
        check(
            submissions.borrow().len() == 1,
            "obsolete detached offer emitted a submission",
        )?;
        screen
            .set_connection(PlayerSheetConnection::Connected)
            .map_err(error)?;
        check(
            !current.disabled(),
            "reconnect did not restore current supplied offer",
        )?;
        let pending: HtmlButtonElement =
            required(&root, "[data-sheet-offer='fixture-reward-pending-003']")?
                .dyn_into()
                .map_err(|_| error("pending offer missing"))?;
        check(
            pending.disabled(),
            "reconnect enabled server-pending reward",
        )?;
        select_tab(&root, 1)?;
        current.click();
        check(
            submissions.borrow().last()
                == Some(&SheetSubmission {
                    owner: SheetOwnerGeneration(41),
                    offer_id: "fixture-equip-current-002".into(),
                }),
            "reconnect emitted obsolete offer",
        )?;
        filter.focus()?;
        screen.set_visible(false).map_err(error)?;
        current.dispatch_event(&Event::new("click")?)?;
        check(
            submissions.borrow().len() == 2,
            "hidden panel emitted a game selection",
        )?;
        screen.set_visible(true).map_err(error)?;
        check(
            filter.value() == "fixture" && details.has_attribute("open"),
            "navigation discarded valid local state",
        )?;
        check(
            document
                .active_element()
                .as_ref()
                .is_some_and(|node| filter.is_same_node(Some(node))),
            "return navigation failed to restore valid focus",
        )?;
        with_view(41, 21, false, |view| screen.update(view)).map_or_else(
            |error| {
                check(
                    matches!(error, df_ui::SheetError::StaleView),
                    "stale view returned unexpected error",
                )
            },
            |_| Err(error("stale view was accepted")),
        )?;
        current.click();
        check(
            submissions.borrow().len() == 3,
            "stale update disabled current valid offer",
        )?;
        let private = required(&root, ".sheet-fields dd")?;
        let text = private
            .first_child()
            .ok_or_else(|| error("private Text node missing"))?;
        let owner_change = with_view(42, 23, false, |view| screen.update(view));
        check(
            matches!(owner_change, Err(df_ui::SheetError::OwnerChanged)),
            "owner replacement did not revoke",
        )?;
        sanitized(&private, &text)?;
        check(!root.is_connected(), "revoked role root remained mounted")?;
        current.dispatch_event(&Event::new("click")?)?;
        check(
            submissions.borrow().len() == 3,
            "revoked offer callback escaped fence",
        )?;
        screen.revoke().map_err(error)?;
        check(
            screen.set_visible(true).is_err(),
            "navigation revived revoked authorization",
        )?;
        drop(screen);
        slot.remove();

        let slot = child(document, parent, "div", "")?;
        let mut malformed = with_view(43, 1, false, |view| {
            PlayerSheetScreen::mount(
                document,
                &slot,
                "sheet-client-malformed",
                view,
                PlayerSheetConnection::Connected,
                |_| {},
            )
        })
        .map_err(error)?;
        let private = required(malformed.root(), ".sheet-fields dd")?;
        let text = private
            .first_child()
            .ok_or_else(|| error("failure Text node missing"))?;
        let result = with_view(43, 2, false, |view| {
            let invalid = CharacterSheetView {
                name: "",
                labels: SheetLabels {
                    tabs: view.labels.tabs,
                    navigation: view.labels.navigation,
                    filter: view.labels.filter,
                    no_matches: view.labels.no_matches,
                    art_fallback: view.labels.art_fallback,
                },
                ..*view
            };
            malformed.update(&invalid)
        });
        check(
            matches!(result, Err(df_ui::SheetError::InvalidView(_))),
            "malformed update did not fail closed",
        )?;
        sanitized(&private, &text)?;
        drop(malformed);
        slot.remove();

        let slot = child(document, parent, "div", "")?;
        let drop_submissions = Rc::new(RefCell::new(Vec::<SheetSubmission>::new()));
        let received = Rc::clone(&drop_submissions);
        let dropped = with_view(44, 1, false, |view| {
            PlayerSheetScreen::mount(
                document,
                &slot,
                "sheet-client-drop",
                view,
                PlayerSheetConnection::Connected,
                move |submission| received.borrow_mut().push(submission),
            )
        })
        .map_err(error)?;
        let private = required(dropped.root(), ".sheet-fields dd")?;
        let text = private
            .first_child()
            .ok_or_else(|| error("Drop Text node missing"))?;
        let held_offer = offer(dropped.root())?;
        drop(dropped);
        sanitized(&private, &text)?;
        held_offer.dispatch_event(&Event::new("click")?)?;
        check(
            drop_submissions.borrow().is_empty(),
            "Drop did not fence the retained offer callback",
        )?;
        slot.remove();
        Ok(())
    }

    fn control(
        document: &Document,
        parent: &Element,
        label: &str,
        mut action: impl FnMut() -> Result<(), JsValue> + 'static,
        status: &Element,
    ) -> Result<ControlledAction, JsValue> {
        let control = ControlledAction::create(
            document,
            ActionView {
                label,
                enabled: true,
                pending: false,
            },
        )
        .map_err(error)?;
        let status = status.clone();
        control
            .on_activate(move || match action() {
                Ok(()) => status.set_text_content(Some("PASS · synthetic mounted transition")),
                Err(error) => status.set_text_content(Some(&format!("FAIL · {error:?}"))),
            })
            .map_err(error)?;
        parent.append_child(control.element())?;
        Ok(control)
    }

    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| error("browser document unavailable"))?;
        let body = document
            .body()
            .ok_or_else(|| error("browser body unavailable"))?;
        let root = child(&document, &body, "main", "")?;
        root.set_attribute("data-fixture", "player-sheet-client")?;
        child(
            &document,
            &root,
            "style",
            r#"
html,body{margin:0;background:#141310;color:#f4eddf}
[data-fixture="player-sheet-client"]{font:13px/1.6 system-ui,sans-serif}
[data-fixture="player-sheet-client"]>h2{font:400 22px/1.3 Georgia,serif;color:#e8d7b3;margin:22px 5% 8px;overflow-wrap:anywhere}
[data-fixture="player-sheet-client"]>p{margin:8px 5% 14px;max-width:900px;color:#b4aca0;font-size:12px;overflow-wrap:anywhere}
[data-fixture="player-sheet-client"]>div:first-of-type{display:flex;flex-wrap:wrap;gap:8px;margin:0 5% 14px;padding:14px 0;border-top:1px solid #d9b77a24;border-bottom:1px solid #d9b77a24}
[data-fixture="player-sheet-client"]>div:first-of-type button{min-height:44px;max-width:100%;padding:10px 13px;border:1px solid #a98b574d;border-radius:6px;background:#1d1c17;color:#c5b99e;font:12px/1.5 system-ui,sans-serif;white-space:normal;overflow-wrap:anywhere;cursor:pointer;touch-action:manipulation}
[data-fixture="player-sheet-client"]>div:first-of-type button:hover:enabled{background:#30291f;border-color:#cba466;color:#f4eddf}
[data-fixture="player-sheet-client"] button:focus-visible,[data-fixture="player-sheet-client"] [tabindex]:focus-visible{outline:3px solid #f1d69b;outline-offset:4px}
[data-fixture="player-sheet-client"] [data-player-client="character-sheet"]>p{margin:0;padding:10px 5%;background:#1c1a15;color:#d8cbb2;font:12px/1.6 system-ui,sans-serif;border-top:1px solid #d9b77a24;overflow-wrap:anywhere}
@media(max-width:700px){[data-fixture="player-sheet-client"]>h2{font-size:20px;margin-top:18px}[data-fixture="player-sheet-client"]>div:first-of-type{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:7px}[data-fixture="player-sheet-client"]>div:first-of-type button{width:100%;font-size:11px}}
@media(prefers-reduced-motion:reduce){[data-fixture="player-sheet-client"] *{animation:none!important;transition:none!important;scroll-behavior:auto!important}}
"#,
        )?;
        child(
            &document,
            &root,
            "h2",
            "Player client · Persistent personal sheet",
        )?;
        child(
            &document,
            &root,
            "p",
            "Synthetic filtered presentation. All five tabs are interactive. Production RPC and full rules catalog remain pending.",
        )?;
        let controls_root = child(&document, &root, "div", "")?;
        let status = child(&document, &root, "p", "Ready · controlled fixture")?;
        status.set_id("player-sheet-fixture-result");
        status.set_attribute("role", "status")?;
        let slot = child(&document, &root, "div", "")?;
        let submission_status = status.clone();
        let screen = Rc::new(RefCell::new(
            with_view(1, 1, false, |view| {
                PlayerSheetScreen::mount(
                    &document,
                    &slot,
                    "player-sheet-client",
                    view,
                    PlayerSheetConnection::Connected,
                    move |submission| {
                        submission_status.set_text_content(Some(&format!(
                            "Exact fixture selection · owner {} · {} · No local game mutation",
                            submission.owner.0, submission.offer_id
                        )))
                    },
                )
            })
            .map_err(error)?,
        ));
        let mut controls = Vec::new();
        for (label, connection) in [
            ("Offline", PlayerSheetConnection::Offline),
            ("Reconnecting", PlayerSheetConnection::Reconnecting),
            ("Connected", PlayerSheetConnection::Connected),
        ] {
            let screen = Rc::clone(&screen);
            controls.push(control(
                &document,
                &controls_root,
                label,
                move || {
                    screen
                        .borrow_mut()
                        .set_connection(connection)
                        .map_err(error)
                },
                &status,
            )?);
        }
        for (label, visible) in [
            ("Close personal sheet", false),
            ("Open personal sheet", true),
        ] {
            let screen = Rc::clone(&screen);
            controls.push(control(
                &document,
                &controls_root,
                label,
                move || screen.borrow_mut().set_visible(visible).map_err(error),
                &status,
            )?);
        }
        let updated = Rc::clone(&screen);
        controls.push(control(
            &document,
            &controls_root,
            "Current permitted update",
            move || with_view(1, 2, true, |view| updated.borrow_mut().update(view)).map_err(error),
            &status,
        )?);
        let check_document = document.clone();
        let check_root = root.clone();
        controls.push(control(
            &document,
            &controls_root,
            "Run mounted client boundary checks",
            move || boundary_checks(&check_document, &check_root),
            &status,
        )?);
        let revoked = Rc::clone(&screen);
        controls.push(control(
            &document,
            &controls_root,
            "Revoke private sheet",
            move || revoked.borrow_mut().revoke().map_err(error),
            &status,
        )?);
        FIXTURE.with(|fixture| *fixture.borrow_mut() = Some(Fixture { screen, controls }));
        Ok(())
    }

    #[wasm_bindgen]
    pub fn dispose() -> Result<(), JsValue> {
        FIXTURE.with(|fixture| {
            let Some(mut fixture) = fixture.borrow_mut().take() else {
                return Ok(());
            };
            let mut first_error = fixture.screen.borrow_mut().revoke().map_err(error).err();
            for control in fixture.controls.drain(..) {
                if let Err(error) = control.dispose() {
                    first_error.get_or_insert_with(|| self::error(error));
                }
            }
            match first_error {
                Some(error) => Err(error),
                None => Ok(()),
            }
        })
    }
}
