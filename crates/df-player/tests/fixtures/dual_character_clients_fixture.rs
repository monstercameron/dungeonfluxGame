//! Reachable two-role presentation fixture. All projections and offers are
//! synthetic and separately constructed. No production transport, authorization,
//! rules catalog or persistence is connected.

#[cfg(target_arch = "wasm32")]
mod browser {
    use df_display::DisplayCharacterScreen;
    use df_player::{PlayerCharacterConnection, PlayerCharacterScreen};
    use df_ui::{
        ActionView, CharacterAction, CharacterActionKind, CharacterAppearanceDraft,
        CharacterChoice, CharacterDisplayConnection, CharacterDisplayHostOffer,
        CharacterDisplayLimits, CharacterDisplaySubmission, CharacterDisplayView, CharacterFact,
        CharacterGroup, CharacterLimits, CharacterOption, CharacterPhaseView, CharacterPortrait,
        CharacterPublicMember, CharacterPublicReadiness, CharacterStatus, CharacterSubmission,
        ControlledAction,
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use wasm_bindgen::{JsCast, prelude::*};
    use web_sys::{Document, Element, Event, HtmlElement, HtmlInputElement, Node};

    struct Fixture {
        player: PlayerCharacterScreen,
        display: DisplayCharacterScreen,
        player_view: CharacterPhaseView,
        display_view: CharacterDisplayView,
        controls: Vec<ControlledAction>,
    }
    thread_local! { static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) }; }
    fn error(error: impl std::fmt::Display) -> JsValue {
        JsValue::from_str(&error.to_string())
    }
    fn check(condition: bool, message: &str) -> Result<(), JsValue> {
        if condition {
            Ok(())
        } else {
            Err(JsValue::from_str(message))
        }
    }
    fn element(
        document: &Document,
        parent: &Element,
        tag: &str,
        text: &str,
    ) -> Result<Element, JsValue> {
        let node = document.create_element(tag)?;
        node.set_text_content(Some(text));
        parent.append_child(&node)?;
        Ok(node)
    }
    fn required(root: &Element, selector: &str) -> Result<Element, JsValue> {
        root.query_selector(selector)?
            .ok_or_else(|| JsValue::from_str("fixture node missing"))
    }
    fn input(root: &Element, selector: &str) -> Result<HtmlInputElement, JsValue> {
        required(root, selector)?
            .dyn_into()
            .map_err(|_| JsValue::from_str("fixture input missing"))
    }
    fn player_limits() -> CharacterLimits {
        CharacterLimits {
            max_groups: 16,
            max_options: 128,
            max_facts: 128,
            max_actions: 16,
            max_text_bytes: 4096,
        }
    }
    fn display_limits() -> CharacterDisplayLimits {
        CharacterDisplayLimits {
            max_members: 64,
            max_host_offers: 16,
            max_text_bytes: 4096,
        }
    }
    fn player_view() -> CharacterPhaseView {
        CharacterPhaseView {
            generation: 1,
            owner_key: "synthetic-private-player-one".into(),
            revision: 1,
            chapter: "Prologue · Player character creation".into(),
            title: "Who will you become?".into(),
            description:
                "Choose a story, write a name, and carry a spark of courage into the dark.".into(),
            connection: "Synthetic filtered player projection · Production RPC pending".into(),
            status: CharacterStatus::Editing,
            status_message:
                "Fixture offers illustrate input only; server build validation is not connected."
                    .into(),
            editable: true,
            name: "Mara".into(),
            flavor: "PRIVATE-DRAFT-SENTINEL · A lantern that never goes out".into(),
            appearance: Some(CharacterAppearanceDraft {
                features: "PRIVATE-FEATURES-SENTINEL · A scar across one brow".into(),
                outfit: "PRIVATE-OUTFIT-SENTINEL · A green travel cloak".into(),
            }),
            portrait: Some(CharacterPortrait::Narrator),
            groups: vec![CharacterGroup {
                id: "synthetic-story".into(),
                label: "A story calling".into(),
                description: "Synthetic presentation choices, with no mechanical effect.".into(),
                selected: Some("lantern".into()),
                options: vec![
                    CharacterOption {
                        id: "lantern".into(),
                        label: "Lantern keeper".into(),
                        description: "A light to guide the travellers home.".into(),
                        enabled: true,
                        availability: "Synthetic advertised choice".into(),
                        portrait: Some(CharacterPortrait::Narrator),
                    },
                    CharacterOption {
                        id: "harbor".into(),
                        label: "Harbor witness".into(),
                        description: "A hundred stories waiting to be remembered.".into(),
                        enabled: true,
                        availability: "Synthetic advertised choice".into(),
                        portrait: Some(CharacterPortrait::Vell),
                    },
                ],
            }],
            facts: vec![CharacterFact {
                label: "View source".into(),
                value: "Separate synthetic player projection · AI-generated v2 lantern-keeper art"
                    .into(),
            }],
            actions: vec![CharacterAction {
                id: "synthetic-submit".into(),
                kind: CharacterActionKind::SubmitDraft,
                label: "Send draft · fixture only".into(),
                enabled: true,
            }],
        }
    }
    fn display_view() -> CharacterDisplayView {
        CharacterDisplayView {
            generation: 1, public_scope_key: "synthetic-public-room-one".into(), revision: 1,
            chapter: "Prologue · Shared character creation".into(), title: "Every story needs its heroes".into(),
            description: "Your paths are gathering around the fire. A shared adventure waits beyond its light.".into(),
            readiness: CharacterPublicReadiness::Choosing,
            progress_label: "Synthetic server report · Party choosing characters".into(),
            public_notice: "Separate public projection · AI-generated v2 lantern-keeper art · No private form, build choices, or rejection details".into(),
            connection: CharacterDisplayConnection::Connected,
            connection_label: "Synthetic shared display · Production RPC pending".into(),
            members: vec![
                CharacterPublicMember { key: "public-mara".into(), character_name: "Mara".into(),
                    portrait: Some(CharacterPortrait::Narrator), readiness: CharacterPublicReadiness::Choosing,
                    progress_label: "Public report · Choosing".into(), appearance_summary: None },
                CharacterPublicMember { key: "public-elian".into(), character_name: "Elian".into(),
                    portrait: Some(CharacterPortrait::Vell), readiness: CharacterPublicReadiness::Ready,
                    progress_label: "Public report · Ready".into(), appearance_summary: Some("A silver braid and blue travel coat".into()) },
            ],
            host_offers: vec![CharacterDisplayHostOffer { id: "synthetic-host-offer".into(),
                label: "Advertised host offer · fixture only".into(), enabled: true, pending: false }],
        }
    }
    fn retain(node: &Node, nodes: &mut Vec<Node>) {
        nodes.push(node.clone());
        let mut next = node.first_child();
        while let Some(child) = next {
            next = child.next_sibling();
            retain(&child, nodes);
        }
    }
    fn private_nodes(root: &Element) -> Result<Vec<Node>, JsValue> {
        let mut nodes = Vec::new();
        for selector in [
            ".character-hero",
            ".character-groups",
            ".character-facts",
            ".character-actions",
            ".identity-panel",
            ".character-appearance",
            ".character-connection",
        ] {
            if let Some(node) = root.query_selector(selector)? {
                retain(node.as_ref(), &mut nodes);
            }
        }
        Ok(nodes)
    }
    fn scrubbed(nodes: &[Node]) -> bool {
        nodes.iter().all(|node| {
            node.text_content().is_none_or(|text| text.is_empty())
                && node.node_value().is_none_or(|text| text.is_empty())
        })
    }
    // Synthetic fixture evidence only: keep the failing predicate intact and
    // identify up to eight surviving raw Text nodes without dumping a whole view.
    fn remaining_text_diagnostic(nodes: &[Node]) -> String {
        let mut details = String::from("scope replacement retained private text nodes");
        let mut reported = 0;
        for node in nodes {
            let value = node.node_value().unwrap_or_default();
            if value.is_empty() {
                continue;
            }
            if reported == 8 {
                details.push_str("; additional nonempty Text omitted");
                break;
            }
            let parent = node
                .parent_element()
                .map(|element| {
                    let class: String = element.class_name().chars().take(80).collect();
                    format!("{} class={class:?}", element.tag_name())
                })
                .unwrap_or_else(|| "detached Text".into());
            let excerpt: String = value.chars().take(160).collect();
            details.push_str(&format!("; parent={parent}, value={excerpt:?}"));
            reported += 1;
        }
        details
    }

    fn test_player_lifecycle(document: &Document, slot: &Element) -> Result<(), JsValue> {
        let mut data = player_view();
        let deliveries = Rc::new(RefCell::new(Vec::<CharacterSubmission>::new()));
        let captured = Rc::clone(&deliveries);
        let mut screen = PlayerCharacterScreen::mount(
            document,
            slot,
            "player-client-check",
            &data,
            player_limits(),
            PlayerCharacterConnection::Connected,
            move |request| captured.borrow_mut().push(request),
        )
        .map_err(error)?;
        let root = screen.root().clone();
        let surface = required(&root, ".df-character")?;
        let name = input(&root, ".identity-panel input")?;
        let flavor = input(&root, ".identity-panel .df-ui-field:last-child input")?;
        let features = input(
            &root,
            ".character-appearance .df-ui-field:first-of-type input",
        )?;
        let outfit = input(
            &root,
            ".character-appearance .df-ui-field:nth-of-type(2) input",
        )?;
        name.set_value("Local private draft");
        name.dispatch_event(&Event::new("input")?)?;
        flavor.set_value("Local private flavor");
        flavor.dispatch_event(&Event::new("input")?)?;
        features.set_value("Local private features");
        features.dispatch_event(&Event::new("input")?)?;
        outfit.set_value("Local private outfit");
        outfit.dispatch_event(&Event::new("input")?)?;
        name.focus()?;
        required(&root, "[data-option-id='harbor']")?.dispatch_event(&Event::new("click")?)?;
        for _ in 0..64 {
            data.revision += 1;
            screen.update(&data).map_err(error)?;
        }
        check(
            required(&root, ".df-character")?.is_same_node(Some(&surface)),
            "same-phase remounted component",
        )?;
        check(
            input(&root, ".identity-panel input")?.is_same_node(Some(&name)),
            "same-phase replaced input",
        )?;
        check(
            name.value() == "Local private draft"
                && flavor.value() == "Local private flavor"
                && features.value() == "Local private features"
                && outfit.value() == "Local private outfit",
            "same-phase discarded drafts",
        )?;
        check(
            document
                .active_element()
                .is_some_and(|node| node.is_same_node(Some(&name))),
            "same-phase lost focus",
        )?;
        let mut conflict = data.clone();
        conflict.title = "Conflicting equal revision".into();
        check(
            screen.update(&conflict).is_err()
                && root.is_connected()
                && name.value() == "Local private draft",
            "same-scope conflict replaced valid mount",
        )?;
        let action = required(&root, ".character-actions button")?;
        screen
            .set_connection(PlayerCharacterConnection::Offline)
            .map_err(error)?;
        name.blur()?;
        action.dispatch_event(&Event::new("click")?)?;
        check(
            deliveries.borrow().is_empty(),
            "offline player emitted submission",
        )?;
        screen
            .set_connection(PlayerCharacterConnection::Reconnecting)
            .map_err(error)?;
        screen.update(&data).map_err(error)?;
        action.dispatch_event(&Event::new("click")?)?;
        check(
            deliveries.borrow().is_empty(),
            "reconnecting player emitted submission",
        )?;
        screen
            .set_connection(PlayerCharacterConnection::Connected)
            .map_err(error)?;
        check(
            name.value() == "Local private draft"
                && features.value() == "Local private features"
                && outfit.value() == "Local private outfit"
                && document
                    .active_element()
                    .is_some_and(|node| node.is_same_node(Some(&name))),
            "reconnect lost draft or focus",
        )?;
        action.dispatch_event(&Event::new("click")?)?;
        check(
            deliveries.borrow().as_slice()
                == [CharacterSubmission {
                    generation: data.generation,
                    owner_key: data.owner_key.clone(),
                    revision: data.revision,
                    action_id: "synthetic-submit".into(),
                    kind: CharacterActionKind::SubmitDraft,
                    choices: vec![CharacterChoice {
                        group_id: "synthetic-story".into(),
                        option_id: "harbor".into(),
                    }],
                    name: "Local private draft".into(),
                    flavor: "Local private flavor".into(),
                    appearance: Some(CharacterAppearanceDraft {
                        features: "Local private features".into(),
                        outfit: "Local private outfit".into(),
                    }),
                }],
            "player callback changed exact advertised submission",
        )?;
        check(
            surface.get_attribute("data-status").as_deref() == Some("editing"),
            "click inferred readiness",
        )?;
        data.revision += 1;
        data.status = CharacterStatus::Pending;
        screen.update(&data).map_err(error)?;
        check(
            features.read_only()
                && outfit.read_only()
                && features.value() == "Local private features",
            "pending appearance was lost or remained editable",
        )?;
        action.dispatch_event(&Event::new("click")?)?;
        check(
            deliveries.borrow().len() == 1,
            "pending appearance submitted",
        )?;
        data.revision += 1;
        data.status = CharacterStatus::Rejected;
        data.status_message = "Synthetic server rejection · revise the visual draft".into();
        screen.update(&data).map_err(error)?;
        check(
            !features.read_only()
                && !outfit.read_only()
                && features.value() == "Local private features"
                && outfit.value() == "Local private outfit",
            "rejected appearance lost editable drafts",
        )?;
        outfit.set_value("Revised local outfit");
        outfit.dispatch_event(&Event::new("input")?)?;
        action.dispatch_event(&Event::new("click")?)?;
        check(
            deliveries.borrow().len() == 2
                && deliveries.borrow()[1].revision == data.revision
                && deliveries.borrow()[1]
                    .appearance
                    .as_ref()
                    .is_some_and(|appearance| {
                        appearance.features == "Local private features"
                            && appearance.outfit == "Revised local outfit"
                    }),
            "rejected appearance did not resubmit current scoped draft",
        )?;
        data.revision += 1;
        data.status = CharacterStatus::Ready;
        data.name = "Confirmed Mara".into();
        data.flavor = "Confirmed story".into();
        data.appearance = Some(CharacterAppearanceDraft {
            features: "Confirmed silver braid".into(),
            outfit: "Confirmed blue travel coat".into(),
        });
        screen.update(&data).map_err(error)?;
        check(
            name.value() == "Confirmed Mara"
                && flavor.value() == "Confirmed story"
                && features.value() == "Confirmed silver braid"
                && outfit.value() == "Confirmed blue travel coat"
                && features.read_only()
                && outfit.read_only(),
            "ready view did not adopt server-confirmed identity and appearance",
        )?;
        for status in [CharacterStatus::Pending, CharacterStatus::Locked] {
            data.revision += 1;
            data.status = status;
            data.actions.clear();
            screen.update(&data).map_err(error)?;
            action.dispatch_event(&Event::new("click")?)?;
            check(
                deliveries.borrow().len() == 2,
                "retired action submitted in pending/locked",
            )?;
        }
        // Browser05 identified this exact ownership-independent heading Text,
        // constructed literally by df-ui, as the only survivor. Preserve it as
        // persistent UI and continue checking every other retained Text for scrub.
        let static_heading = required(&root, ".identity-panel > h2")?;
        let static_heading_text = static_heading
            .first_child()
            .ok_or_else(|| JsValue::from_str("static identity heading Text missing"))?;
        check(
            static_heading_text.node_type() == Node::TEXT_NODE
                && static_heading_text.next_sibling().is_none()
                && static_heading.text_content().as_deref() == Some("Give your story a name"),
            "identity heading differs from the literal ownership-independent UI",
        )?;
        let captured = private_nodes(&root)?;
        check(
            captured
                .iter()
                .any(|node| node.is_same_node(Some(&static_heading_text))),
            "scope capture did not include the exact static identity heading",
        )?;
        let retained: Vec<_> = captured
            .into_iter()
            .filter(|node| {
                node.node_type() == Node::TEXT_NODE
                    && !node.is_same_node(Some(&static_heading_text))
            })
            .collect();
        data.generation += 1;
        data.owner_key = "synthetic-private-player-two".into();
        data.revision = 1;
        data.status = CharacterStatus::Editing;
        data.name = "New owner".into();
        data.flavor = "New owner flavor".into();
        data.appearance = Some(CharacterAppearanceDraft {
            features: "New owner features".into(),
            outfit: "New owner outfit".into(),
        });
        data.groups.clear();
        screen.update(&data).map_err(error)?;
        check(
            name.value() == "New owner"
                && flavor.value() == "New owner flavor"
                && features.value().is_empty()
                && outfit.value().is_empty()
                && input(
                    &root,
                    ".character-appearance .df-ui-field:first-of-type input",
                )?
                .value()
                    == "New owner features"
                && input(
                    &root,
                    ".character-appearance .df-ui-field:nth-of-type(2) input",
                )?
                .value()
                    == "New owner outfit",
            "scope replacement retained old draft",
        )?;
        let current_heading = required(&root, ".identity-panel > h2")?;
        check(
            static_heading.is_connected()
                && current_heading.is_same_node(Some(&static_heading))
                && current_heading
                    .first_child()
                    .is_some_and(|node| node.is_same_node(Some(&static_heading_text)))
                && static_heading.text_content().as_deref() == Some("Give your story a name"),
            "ownership replacement changed the persistent static identity heading",
        )?;
        check(scrubbed(&retained), &remaining_text_diagnostic(&retained))?;
        let withdrawn_features = input(
            &root,
            ".character-appearance .df-ui-field:first-of-type input",
        )?;
        let withdrawn_outfit = input(
            &root,
            ".character-appearance .df-ui-field:nth-of-type(2) input",
        )?;
        data.revision += 1;
        data.appearance = None;
        data.actions = player_view().actions;
        screen.update(&data).map_err(error)?;
        check(
            root.query_selector(".character-appearance")?.is_none()
                && withdrawn_features.value().is_empty()
                && withdrawn_outfit.value().is_empty(),
            "appearance withdrawal retained private controls",
        )?;
        required(&root, ".character-actions button")?.dispatch_event(&Event::new("click")?)?;
        check(
            deliveries.borrow().len() == 3 && deliveries.borrow()[2].appearance.is_none(),
            "legacy appearance-free submission changed",
        )?;
        screen.revoke().map_err(error)?;
        screen.revoke().map_err(error)?;
        action.dispatch_event(&Event::new("click")?)?;
        check(
            !root.is_connected()
                && name.value().is_empty()
                && flavor.value().is_empty()
                && features.value().is_empty()
                && outfit.value().is_empty(),
            "revoke retained private mount or draft",
        )?;
        check(
            screen.update(&data).is_err()
                && screen
                    .set_connection(PlayerCharacterConnection::Connected)
                    .is_err(),
            "revoked scope reactivated",
        )?;
        check(deliveries.borrow().len() == 3, "revoked callback delivered")
    }
    fn test_invalid_replacement_and_drop(
        document: &Document,
        slot: &Element,
    ) -> Result<(), JsValue> {
        for implicit in [false, true] {
            let mut data = player_view();
            let count = Rc::new(Cell::new(0usize));
            let captured = Rc::clone(&count);
            let mut screen = PlayerCharacterScreen::mount(
                document,
                slot,
                "player-cleanup-check",
                &data,
                player_limits(),
                PlayerCharacterConnection::Connected,
                move |_| captured.set(captured.get() + 1),
            )
            .map_err(error)?;
            let root = screen.root().clone();
            let nodes = private_nodes(&root)?;
            let name = input(&root, ".identity-panel input")?;
            let action = required(&root, ".character-actions button")?;
            if !implicit {
                data.owner_key = "new-owner".into();
                data.title.clear();
                check(
                    screen.update(&data).is_err(),
                    "invalid replacement accepted",
                )?;
            }
            drop(screen);
            action.dispatch_event(&Event::new("click")?)?;
            check(
                !root.is_connected()
                    && name.value().is_empty()
                    && scrubbed(&nodes)
                    && count.get() == 0,
                "Drop/failed replacement retained private state or callback",
            )?;
        }
        Ok(())
    }
    fn test_display_lifecycle(document: &Document, slot: &Element) -> Result<(), JsValue> {
        let mut data = display_view();
        let deliveries = Rc::new(RefCell::new(Vec::<CharacterDisplaySubmission>::new()));
        let captured = Rc::clone(&deliveries);
        let mut screen = DisplayCharacterScreen::mount(
            document,
            slot,
            &data,
            display_limits(),
            CharacterDisplayConnection::Connected,
            move |request| captured.borrow_mut().push(request),
        )
        .map_err(error)?;
        let root = screen.root().clone();
        check(
            root.query_selector("input, .character-choice-group, .character-facts")?
                .is_none(),
            "private form entered public mount",
        )?;
        check(
            !root.text_content().is_some_and(|text| {
                text.contains("PRIVATE-DRAFT-SENTINEL")
                    || text.contains("PRIVATE-FEATURES-SENTINEL")
                    || text.contains("PRIVATE-OUTFIT-SENTINEL")
            }),
            "private identity or appearance entered public mount",
        )?;
        check(
            required(
                &root,
                "[data-member-key='public-elian'] .display-member-appearance:not([hidden])",
            )?
            .text_content()
            .as_deref()
                == Some("A silver braid and blue travel coat"),
            "explicit public appearance summary missing",
        )?;
        let member = required(&root, ".display-member")?;
        let action = required(&root, ".display-host-offers button")?;
        action
            .dyn_ref::<HtmlElement>()
            .ok_or_else(|| JsValue::from_str("host focus unavailable"))?
            .focus()?;
        for _ in 0..64 {
            data.revision += 1;
            screen.update(&data).map_err(error)?;
        }
        check(
            required(&root, ".display-member")?.is_same_node(Some(&member))
                && document
                    .active_element()
                    .is_some_and(|node| node.is_same_node(Some(&action))),
            "display keyed update lost node/focus",
        )?;
        let mut conflict = data.clone();
        conflict.title = "Conflicting equal public revision".into();
        check(
            screen.update(&conflict).is_err() && root.is_connected(),
            "same-scope public conflict replaced valid mount",
        )?;
        data.revision += 1;
        data.members[1].appearance_summary = None;
        screen.update(&data).map_err(error)?;
        check(
            required(
                &root,
                "[data-member-key='public-elian'] .display-member-appearance",
            )?
            .has_attribute("hidden")
                && !root
                    .text_content()
                    .is_some_and(|text| text.contains("A silver braid and blue travel coat")),
            "withdrawn public summary remained visible",
        )?;
        for connection in [
            CharacterDisplayConnection::Offline,
            CharacterDisplayConnection::Reconnecting,
        ] {
            screen.set_connection(connection).map_err(error)?;
            action.dispatch_event(&Event::new("click")?)?;
            check(
                deliveries.borrow().is_empty(),
                "suspended display emitted host offer",
            )?;
        }
        screen
            .set_connection(CharacterDisplayConnection::Connected)
            .map_err(error)?;
        action.dispatch_event(&Event::new("click")?)?;
        check(
            deliveries.borrow().as_slice()
                == [CharacterDisplaySubmission {
                    generation: data.generation,
                    public_scope_key: data.public_scope_key.clone(),
                    revision: data.revision,
                    host_offer_id: "synthetic-host-offer".into(),
                }],
            "display changed advertised host intent",
        )?;
        data.revision += 1;
        data.connection = CharacterDisplayConnection::Offline;
        screen.update(&data).map_err(error)?;
        action.dispatch_event(&Event::new("click")?)?;
        check(
            deliveries.borrow().len() == 1,
            "server-offline host offer submitted",
        )?;
        data.revision += 1;
        data.connection = CharacterDisplayConnection::Connected;
        data.host_offers.clear();
        data.readiness = CharacterPublicReadiness::Locked;
        screen.update(&data).map_err(error)?;
        action.dispatch_event(&Event::new("click")?)?;
        check(
            deliveries.borrow().len() == 1
                && root
                    .query_selector(".display-host-offers button")?
                    .is_none(),
            "removed host offer stayed active",
        )?;
        let old_name = member
            .query_selector("h2")?
            .and_then(|node| node.first_child())
            .ok_or_else(|| JsValue::from_str("roster text missing"))?;
        data.generation += 1;
        data.public_scope_key = "new-public-room".into();
        data.revision = 1;
        data.members.clear();
        screen.update(&data).map_err(error)?;
        check(
            !member.is_connected() && old_name.node_value().is_none_or(|text| text.is_empty()),
            "public scope retained old roster",
        )?;
        screen.revoke().map_err(error)?;
        screen.revoke().map_err(error)?;
        check(
            !root.is_connected() && screen.update(&data).is_err(),
            "display revoke reactivated",
        )?;
        for implicit in [false, true] {
            let data = display_view();
            let count = Rc::new(Cell::new(0usize));
            let captured = Rc::clone(&count);
            let mut screen = DisplayCharacterScreen::mount(
                document,
                slot,
                &data,
                display_limits(),
                CharacterDisplayConnection::Connected,
                move |_| captured.set(captured.get() + 1),
            )
            .map_err(error)?;
            let root = screen.root().clone();
            let action = required(&root, ".display-host-offers button")?;
            let mut nodes = Vec::new();
            retain(required(&root, ".display-roster")?.as_ref(), &mut nodes);
            if !implicit {
                let mut invalid = data;
                invalid.public_scope_key = "other-public-scope".into();
                invalid.title.clear();
                check(
                    screen.update(&invalid).is_err(),
                    "invalid display replacement accepted",
                )?;
            }
            drop(screen);
            action.dispatch_event(&Event::new("click")?)?;
            check(
                !root.is_connected() && scrubbed(&nodes) && count.get() == 0,
                "display Drop/failed replacement retained roster or callback",
            )?;
        }
        Ok(())
    }
    fn run_checks(document: &Document, output: &Element) -> Result<(), JsValue> {
        let test_slot = document.create_element("div")?;
        document
            .body()
            .ok_or_else(|| JsValue::from_str("body unavailable"))?
            .append_child(&test_slot)?;
        let checks = [
            (
                "Player mount, exact offers, 64 updates, drafts, focus, offline/reconnect, pending/locked, replacement and revoke",
                test_player_lifecycle(document, &test_slot),
            ),
            (
                "Player invalid ownership replacement and implicit Drop privacy cleanup",
                test_invalid_replacement_and_drop(document, &test_slot),
            ),
            (
                "Shared-display public-only mount, 64 updates, focus, host offers, offline/reconnect, replacement/revoke and Drop",
                test_display_lifecycle(document, &test_slot),
            ),
        ];
        test_slot.remove();
        let mut failed = false;
        for (name, result) in checks {
            failed |= result.is_err();
            element(
                document,
                output,
                "p",
                &format!("{} · {name}", if result.is_ok() { "PASS" } else { "FAIL" }),
            )?;
            if let Err(detail) = result {
                element(
                    document,
                    output,
                    "p",
                    detail
                        .as_string()
                        .as_deref()
                        .unwrap_or("Browser assertion failed"),
                )?;
            }
        }
        check(!failed, "dual-client mount checks failed")
    }
    fn apply_control(name: &str) -> Result<(), JsValue> {
        FIXTURE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let fixture = slot
                .as_mut()
                .ok_or_else(|| JsValue::from_str("fixture unavailable"))?;
            match name {
                "update" => {
                    fixture.player_view.revision += 1;
                    fixture.display_view.revision += 1;
                    fixture.player.update(&fixture.player_view).map_err(error)?;
                    fixture
                        .display
                        .update(&fixture.display_view)
                        .map_err(error)?;
                }
                "offline" | "reconnecting" | "connected" => {
                    let (player, display) = match name {
                        "offline" => (
                            PlayerCharacterConnection::Offline,
                            CharacterDisplayConnection::Offline,
                        ),
                        "reconnecting" => (
                            PlayerCharacterConnection::Reconnecting,
                            CharacterDisplayConnection::Reconnecting,
                        ),
                        _ => (
                            PlayerCharacterConnection::Connected,
                            CharacterDisplayConnection::Connected,
                        ),
                    };
                    fixture.player.set_connection(player).map_err(error)?;
                    fixture.display.set_connection(display).map_err(error)?;
                }
                "pending" | "locked" => {
                    fixture.player_view.revision += 1;
                    fixture.display_view.revision += 1;
                    fixture.player_view.status = if name == "pending" {
                        CharacterStatus::Pending
                    } else {
                        CharacterStatus::Locked
                    };
                    fixture.player_view.actions.clear();
                    fixture.display_view.readiness = if name == "pending" {
                        CharacterPublicReadiness::Reviewing
                    } else {
                        CharacterPublicReadiness::Locked
                    };
                    fixture.display_view.host_offers.clear();
                    fixture.player.update(&fixture.player_view).map_err(error)?;
                    fixture
                        .display
                        .update(&fixture.display_view)
                        .map_err(error)?;
                }
                "rejected" => {
                    fixture.player_view.revision += 1;
                    fixture.player_view.status = CharacterStatus::Rejected;
                    fixture.player_view.status_message =
                        "Synthetic server rejection · revise your visual draft".into();
                    fixture.player_view.actions = player_view().actions;
                    fixture.player.update(&fixture.player_view).map_err(error)?;
                }
                "ready" => {
                    fixture.player_view.revision += 1;
                    fixture.player_view.status = CharacterStatus::Ready;
                    fixture.player_view.status_message =
                        "Synthetic server confirmation · reference sheet follows creation".into();
                    fixture.player_view.name = "Mara · confirmed".into();
                    fixture.player_view.flavor = "Lantern story · confirmed".into();
                    fixture.player_view.appearance = Some(CharacterAppearanceDraft {
                        features: "Silver braid · confirmed".into(),
                        outfit: "Blue travel coat · confirmed".into(),
                    });
                    fixture.player_view.actions.clear();
                    fixture.display_view.revision += 1;
                    fixture.display_view.readiness = CharacterPublicReadiness::Ready;
                    fixture.display_view.progress_label =
                        "Synthetic server report · Party ready".into();
                    let member = fixture
                        .display_view
                        .members
                        .get_mut(0)
                        .ok_or_else(|| JsValue::from_str("synthetic public member missing"))?;
                    member.character_name = "Mara · confirmed".into();
                    member.readiness = CharacterPublicReadiness::Ready;
                    member.progress_label = "Public report · Ready".into();
                    member.appearance_summary = Some("Silver braid and blue travel coat".into());
                    fixture.player.update(&fixture.player_view).map_err(error)?;
                    fixture
                        .display
                        .update(&fixture.display_view)
                        .map_err(error)?;
                }
                "appearance-withdraw" | "appearance-reoffer" => {
                    fixture.player_view.revision += 1;
                    fixture.player_view.status = CharacterStatus::Editing;
                    fixture.player_view.status_message =
                        "Synthetic editable view · current appearance offer controls the form"
                            .into();
                    fixture.player_view.actions = player_view().actions;
                    fixture.player_view.appearance = if name == "appearance-withdraw" {
                        None
                    } else {
                        player_view().appearance
                    };
                    fixture.player.update(&fixture.player_view).map_err(error)?;
                }
                "summary-withdraw" | "summary-reoffer" => {
                    fixture.display_view.revision += 1;
                    let member = fixture
                        .display_view
                        .members
                        .get_mut(1)
                        .ok_or_else(|| JsValue::from_str("synthetic public member missing"))?;
                    member.appearance_summary = if name == "summary-withdraw" {
                        None
                    } else {
                        Some("A silver braid and blue travel coat".into())
                    };
                    fixture
                        .display
                        .update(&fixture.display_view)
                        .map_err(error)?;
                }
                "replace" => {
                    let generation = fixture.player_view.generation + 1;
                    fixture.player_view = player_view();
                    fixture.display_view = display_view();
                    fixture.player_view.generation = generation;
                    fixture.display_view.generation = generation;
                    fixture.player_view.owner_key =
                        format!("synthetic-private-player-{generation}");
                    fixture.player_view.name = format!("New owner {generation}");
                    fixture.player_view.flavor = "New scope draft".into();
                    fixture.display_view.public_scope_key =
                        format!("synthetic-public-room-{generation}");
                    fixture.player.update(&fixture.player_view).map_err(error)?;
                    fixture
                        .display
                        .update(&fixture.display_view)
                        .map_err(error)?;
                }
                "retry" => {
                    fixture.player.retry_failed_portraits().map_err(error)?;
                    fixture.display.retry_failed_portraits().map_err(error)?;
                }
                "revoke" => {
                    fixture.player.revoke().map_err(error)?;
                    fixture.display.revoke().map_err(error)?;
                }
                _ => return Err(JsValue::from_str("unknown fixture control")),
            }
            Ok(())
        })
    }
    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| JsValue::from_str("document unavailable"))?;
        let body = document
            .body()
            .ok_or_else(|| JsValue::from_str("body unavailable"))?;
        let root = element(&document, &body, "main", "")?;
        root.set_attribute("data-dual-character-fixture", "synthetic")?;
        element(
            &document,
            &root,
            "style",
            r#"
body{margin:0;background:#141310}
[data-dual-character-fixture]{background:#141310!important;color:#f4eddf!important;padding:0!important}
[data-dual-character-fixture] > h1{width:92%;margin:0 auto;padding:22px 0 10px;font:22px/1.3 Georgia,serif;color:#efce91}
[data-dual-character-fixture] > p{width:92%;margin:8px auto 16px;font:11px/1.6 system-ui,sans-serif;color:#b4aca0}
[data-dual-character-fixture] > nav{width:92%;margin:0 auto 12px;display:flex;flex-wrap:wrap;gap:8px}
[data-dual-character-fixture] > nav button{min-height:44px;padding:10px 13px;border:1px solid #a98b574d;border-radius:6px;background:#29251e;color:#ede2cb;font:12px/1.5 system-ui,sans-serif;touch-action:manipulation;cursor:pointer}
[data-dual-character-fixture] > nav button:hover:enabled{background:#42382a;border-color:#e5bd77}
[data-dual-character-fixture] > nav button:focus-visible{outline:3px solid #f1d69b;outline-offset:4px}
[data-dual-character-fixture] > [data-mount-check-results]{width:92%;max-height:110px;overflow:auto;margin:0 auto 20px;padding:8px 12px;border-left:2px solid #a98b574d;background:#1d1c17;font:10px/1.5 system-ui,sans-serif;color:#b4aca0}
[data-dual-character-fixture] > [data-mount-check-results] p{margin:4px 0;overflow-wrap:anywhere}
[data-dual-character-fixture] > section[aria-label] > h2{width:92%;margin:0 auto;padding:18px 0 12px;border-top:1px solid #d9b77a24;font:10px/1.6 system-ui,sans-serif;letter-spacing:2px;text-transform:uppercase;color:#d9b77a}
@media(max-width:700px){[data-dual-character-fixture] > nav{display:grid;grid-template-columns:1fr 1fr}[data-dual-character-fixture] > h1{font-size:19px}}
"#,
        )?;
        root.set_attribute(
            "style",
            "background:#12101a;color:#eee;font:16px system-ui;padding:16px",
        )?;
        element(
            &document,
            &root,
            "h1",
            "DungeonFlux · Both character creation clients",
        )?;
        element(
            &document,
            &root,
            "p",
            "Synthetic caller boundary. Each role receives a separate permitted presentation projection. Production RPC, authorization, catalog, and persistence remain pending.",
        )?;
        let controls_root = element(&document, &root, "nav", "")?;
        controls_root.set_attribute("aria-label", "Synthetic view and connection controls")?;
        let output = element(&document, &root, "section", "")?;
        output.set_attribute("data-mount-check-results", "")?;
        // Execute actual WASM mounts in the document, then remove their temporary
        // slots. Independent browser review must still verify rendered behavior.
        let checks = run_checks(&document, &output);
        output.set_attribute("data-result", if checks.is_ok() { "PASS" } else { "FAIL" })?;
        let player_slot = element(&document, &root, "section", "")?;
        player_slot.set_attribute("aria-label", "Player client · Private character creation")?;
        element(&document, &player_slot, "h2", "Player client")?;
        let display_slot = element(&document, &root, "section", "")?;
        display_slot.set_attribute("aria-label", "Shared display · Public character creation")?;
        element(&document, &display_slot, "h2", "Shared display")?;
        let player_count = element(
            &document,
            &root,
            "p",
            "Player advertised submissions observed: 0",
        )?;
        let display_count = element(
            &document,
            &root,
            "p",
            "Shared-display advertised host submissions observed: 0",
        )?;
        let player_deliveries = Rc::new(Cell::new(0usize));
        let display_deliveries = Rc::new(Cell::new(0usize));
        let player_data = player_view();
        let display_data = display_view();
        let player = PlayerCharacterScreen::mount(
            &document,
            &player_slot,
            "dual-player",
            &player_data,
            player_limits(),
            PlayerCharacterConnection::Connected,
            move |submission| {
                player_deliveries.set(player_deliveries.get().saturating_add(1));
                player_count.set_text_content(Some(&format!(
                    "Player advertised submissions observed: {} · appearance {} · no gameplay action completed",
                    player_deliveries.get(),
                    if submission.appearance.is_some() { "included" } else { "omitted" },
                )));
            },
        )
        .map_err(error)?;
        let display = DisplayCharacterScreen::mount(&document, &display_slot, &display_data, display_limits(),
            CharacterDisplayConnection::Connected, move |_| { display_deliveries.set(display_deliveries.get().saturating_add(1));
                display_count.set_text_content(Some(&format!("Shared-display advertised host submissions observed: {} · no gameplay action completed", display_deliveries.get()))); }).map_err(error)?;
        FIXTURE.with(|slot| {
            *slot.borrow_mut() = Some(Fixture {
                player,
                display,
                player_view: player_data,
                display_view: display_data,
                controls: Vec::new(),
            })
        });
        let report = element(
            &document,
            &root,
            "p",
            "Fixture controls do not mutate a game",
        )?;
        for (id, label) in [
            ("update", "Same-phase update"),
            ("offline", "Offline"),
            ("reconnecting", "Reconnecting"),
            ("connected", "Reconnect"),
            ("pending", "Server reports pending"),
            ("rejected", "Server rejects draft"),
            ("ready", "Server confirms character"),
            ("locked", "Server reports locked"),
            ("appearance-withdraw", "Withdraw appearance offer"),
            ("appearance-reoffer", "Reoffer appearance input"),
            ("summary-withdraw", "Withdraw public appearance"),
            ("summary-reoffer", "Reoffer public appearance"),
            ("replace", "Replace ownership scopes"),
            ("retry", "Retry optional portraits"),
            ("revoke", "Revoke both mounts"),
        ] {
            let control = ControlledAction::create(
                &document,
                ActionView {
                    label,
                    enabled: true,
                    pending: false,
                },
            )
            .map_err(error)?;
            control
                .element()
                .set_attribute("data-fixture-control", id)?;
            let report = report.clone();
            control
                .on_activate(move || {
                    report.set_text_content(Some(if apply_control(id).is_ok() {
                        "Applied synthetic presentation control"
                    } else {
                        "Control rejected · mount unavailable or browser reconciliation failed"
                    }))
                })
                .map_err(error)?;
            controls_root.append_child(control.element())?;
            FIXTURE.with(|slot| {
                if let Some(fixture) = slot.borrow_mut().as_mut() {
                    fixture.controls.push(control);
                }
            });
        }
        Ok(())
    }
    /// Explicit unload for browser acceptance; underlying mounts are dropped and
    /// fence callbacks before releasing private data. Retained nodes can be probed.
    #[wasm_bindgen]
    pub fn dispose_fixture() {
        FIXTURE.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}
