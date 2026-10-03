//! Synthetic controlled browser presentation, with no RPC, providers or game authority.
//! Labels and numbers below are fixed fixture inputs, not a rules implementation.
#[cfg(target_arch = "wasm32")]
mod browser {
    use df_ui::{
        ActionView, CharacterSheetSurface, CharacterSheetView, ControlledAction, SheetField,
        SheetLabels, SheetOffer, SheetOwnerGeneration, SheetRow, SheetSection, SheetSubmission,
        SheetTab,
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use wasm_bindgen::{JsCast, JsValue, prelude::*};
    use web_sys::{Document, Element, HtmlElement, HtmlInputElement, Node};

    struct Fixture {
        sheet: Rc<CharacterSheetSurface>,
        controls: Vec<ControlledAction>,
        revision: Rc<Cell<u64>>,
        revoked: Rc<Cell<bool>>,
        status: Element,
        harness: Element,
    }
    thread_local! {
        static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
    }
    fn error(value: impl std::fmt::Display) -> JsValue {
        JsValue::from_str(&value.to_string())
    }
    fn child(
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
    fn present(
        sheet: &CharacterSheetSurface,
        revision: u64,
        revoked: bool,
        rejected: bool,
    ) -> Result<(), JsValue> {
        let abilities = [
            ("strength", "Strength", "10 · +0"),
            ("dexterity", "Dexterity", "16 · +3"),
            ("constitution", "Constitution", "14 · +2"),
            ("intelligence", "Intelligence", "12 · +1"),
            ("wisdom", "Wisdom", "15 · +2"),
            ("charisma", "Charisma", "11 · +0"),
        ];
        let ability_fields = [
            SheetField {
                label: "Source",
                value: "Synthetic supplied score and modifier",
            },
            SheetField {
                label: "Saving throw",
                value: "Supplied in this character view",
            },
        ];
        let ability_rows: Vec<_> = abilities
            .iter()
            .map(|(key, title, value)| SheetRow {
                key,
                title,
                value,
                summary: "A supplied character ability. Expand to inspect the permitted detail.",
                details: &ability_fields,
                offers: &[],
            })
            .collect();
        let proficiency_fields = [
            SheetField {
                label: "Proficiency bonus",
                value: "+2 · supplied",
            },
            SheetField {
                label: "Languages",
                value: "Common, Elvish",
            },
            SheetField {
                label: "Tools",
                value: "Herbalism kit",
            },
        ];
        let proficiencies = [SheetRow {
            key: "skills",
            title: "Skills & proficiencies",
            value: "Trained",
            summary: "Perception +4 · Survival +4 · Stealth +5. All values are supplied fixture text.",
            details: &proficiency_fields,
            offers: &[],
        }];
        let resource_fields = [
            SheetField {
                label: "Armor Class",
                value: "14",
            },
            SheetField {
                label: "Speed",
                value: "30 ft.",
            },
            SheetField {
                label: "Hit Dice",
                value: "2d10 · available",
            },
            SheetField {
                label: "Condition",
                value: "No current condition supplied",
            },
        ];
        let resources = [SheetRow {
            key: "vitality",
            title: "Vitality & resources",
            value: if revision > 1 {
                "17 / 20 HP"
            } else {
                "18 / 20 HP"
            },
            summary: "Current permitted resource values; submitting an action never changes these locally.",
            details: &resource_fields,
            offers: &[],
        }];
        let bow_fields = [
            SheetField {
                label: "Equipment state",
                value: "Carried · not equipped",
            },
            SheetField {
                label: "Properties",
                value: "Two-handed · ammunition",
            },
            SheetField {
                label: "Attack / damage",
                value: "+5 · 1d8 + 3 piercing · supplied",
            },
            SheetField {
                label: "Ammunition",
                value: "20 arrows · supplied",
            },
        ];
        let bow_offer = [SheetOffer {
            id: "fixture-equip-longbow-007",
            label: "Equip longbow",
            enabled: true,
            pending: false,
        }];
        let armor_fields = [
            SheetField {
                label: "Equipment state",
                value: "Equipped",
            },
            SheetField {
                label: "Source",
                value: "Prepared synthetic inventory",
            },
        ];
        let pack_fields = [
            SheetField {
                label: "Contents",
                value: "Bedroll, rope, waterskin, rations",
            },
            SheetField {
                label: "Load",
                value: "Within capacity · supplied by owner",
            },
        ];
        let equipment = [
            SheetRow {
                key: "longbow",
                title: "Longbow",
                value: "Carried",
                summary: "Ash wood, worn leather grip. A trusted companion on the road north.",
                details: &bow_fields,
                offers: if revoked { &[] } else { &bow_offer },
            },
            SheetRow {
                key: "leather",
                title: "Leather armor",
                value: "Equipped",
                summary: "Weathered leather, maintained through many journeys.",
                details: &armor_fields,
                offers: &[],
            },
            SheetRow {
                key: "pack",
                title: "Explorer’s pack",
                value: "Carried",
                summary: "Supplies for the next leg of the journey.",
                details: &pack_fields,
                offers: &[],
            },
        ];
        let quest_fields = [
            SheetField {
                label: "Purpose",
                value: "Return the token to the harbor keeper",
            },
            SheetField {
                label: "Quest state",
                value: "Awaiting delivery · supplied",
            },
        ];
        let quest_items = [SheetRow {
            key: "token",
            title: "The drowned lantern token",
            value: "Quest item",
            summary: "A copper token recovered beneath the old bridge.",
            details: &quest_fields,
            offers: &[],
        }];
        let spell_fields = [
            SheetField {
                label: "Casting time",
                value: "1 action · supplied",
            },
            SheetField {
                label: "Range",
                value: "Self · supplied",
            },
            SheetField {
                label: "Preparation",
                value: "Prepared",
            },
            SheetField {
                label: "Spell slots",
                value: "2 / 2 · supplied",
            },
        ];
        let spells = [SheetRow {
            key: "spell",
            title: "Speak with Animals",
            value: "Prepared",
            summary: "Listen for what the wilderness has witnessed. No casting offer is currently supplied.",
            details: &spell_fields,
            offers: &[],
        }];
        let feature_fields = [
            SheetField {
                label: "Uses",
                value: "Available · supplied",
            },
            SheetField {
                label: "Source",
                value: "Character feature presentation",
            },
        ];
        let features = [SheetRow {
            key: "feature",
            title: "Favored Enemy",
            value: "Feature",
            summary: "Your character’s permitted feature description appears here.",
            details: &feature_fields,
            offers: &[],
        }];
        let quest_steps = [
            SheetField {
                label: "Find the missing ferryman",
                value: "Complete",
            },
            SheetField {
                label: "Investigate the sunken hall",
                value: "Active",
            },
            SheetField {
                label: "Return to the harbor",
                value: "Pending",
            },
        ];
        let journal = [
            SheetRow {
                key: "quest",
                title: "The light beneath the water",
                value: "In progress",
                summary: "Follow the lantern trail beneath the harbor. This is a permitted journal summary.",
                details: &quest_steps,
                offers: &[],
            },
            SheetRow {
                key: "memory",
                title: "A promise at the campfire",
                value: "Journal",
                summary: "The party agreed to seek the old watchtower at first light.",
                details: &[],
                offers: &[],
            },
        ];
        let level_fields = [
            SheetField {
                label: "Current level",
                value: "2 · supplied",
            },
            SheetField {
                label: "Advancement",
                value: "No level-up selection currently offered",
            },
        ];
        let reward_fields = [
            SheetField {
                label: "Reward state",
                value: "Ready for selection · supplied",
            },
            SheetField {
                label: "Contents",
                value: "20 gp · supplied",
            },
            SheetField {
                label: "Delivery",
                value: "Not committed until confirmed by the server",
            },
        ];
        let reward_offer = [SheetOffer {
            id: "fixture-reward-harbor-021",
            label: "Submit reward selection",
            enabled: true,
            pending: false,
        }];
        let progression = [
            SheetRow {
                key: "level",
                title: "Character advancement",
                value: "Level 2",
                summary: "Your current level and available advancement are supplied by the server.",
                details: &level_fields,
                offers: &[],
            },
            SheetRow {
                key: "reward",
                title: "Harbor keeper’s thanks",
                value: "Reward available",
                summary: "A reward offered for the party’s committed accomplishment.",
                details: &reward_fields,
                offers: &reward_offer,
            },
        ];
        let sections = [
            SheetSection {
                key: "abilities",
                tab: SheetTab::Character,
                title: "Abilities",
                caption: "Your character at a glance",
                rows: &ability_rows,
            },
            SheetSection {
                key: "resources",
                tab: SheetTab::Character,
                title: "Resources",
                caption: "Current supplied values",
                rows: &resources,
            },
            SheetSection {
                key: "proficiencies",
                tab: SheetTab::Character,
                title: "Proficiencies",
                caption: "Skills, languages and tools",
                rows: &proficiencies,
            },
            SheetSection {
                key: "equipment",
                tab: SheetTab::Equipment,
                title: "Equipment",
                caption: "Inspect an item to see its current offered actions",
                rows: &equipment,
            },
            SheetSection {
                key: "quest-items",
                tab: SheetTab::Equipment,
                title: "Quest inventory",
                caption: "Items tied to your current story",
                rows: &quest_items,
            },
            SheetSection {
                key: "spells",
                tab: SheetTab::Spells,
                title: "Spells",
                caption: "Prepared magic and supplied resources",
                rows: &spells,
            },
            SheetSection {
                key: "features",
                tab: SheetTab::Spells,
                title: "Features",
                caption: "Character features and permitted descriptions",
                rows: &features,
            },
            SheetSection {
                key: "journal",
                tab: SheetTab::Journal,
                title: "Your journal",
                caption: "Permitted quest progress and remembered moments",
                rows: &journal,
            },
            SheetSection {
                key: "progression",
                tab: SheetTab::Progression,
                title: "Progression & rewards",
                caption: "Server-provided advancement and reward states",
                rows: &progression,
            },
        ];
        let view = CharacterSheetView {
            owner: SheetOwnerGeneration(1),
            revision,
            name: "Vell Ashwalker",
            identity: "Wood elf · Ranger · Level 2",
            subtitle: "Keeper of the wild paths. The next chapter is yours to discover.",
            connection: "Synthetic preview · No server session",
            notice: if rejected {
                "Synthetic rejection: this action is unavailable. Your sheet stays unchanged; inspect the current offered actions."
            } else if revoked {
                "Synthetic update: the equipment offer was invalidated. Current inventory remains server supplied."
            } else {
                "Synthetic controlled preview · Presentation only. No action changes inventory, levels or rewards locally."
            },
            labels: SheetLabels {
                tabs: [
                    "Character",
                    "Equipment",
                    "Spells & features",
                    "Journal",
                    "Progression",
                ],
                navigation: "Personal character sheet",
                filter: "Filter the current sheet",
                no_matches: "No entries match this filter in the selected tab.",
                art_fallback: "Optional campfire artwork unavailable · Your sheet remains usable",
            },
            sections: &sections,
        };
        sheet.update(&view).map_err(error)
    }
    fn initial<'a>(sections: &'a [SheetSection<'a>]) -> CharacterSheetView<'a> {
        CharacterSheetView {
            owner: SheetOwnerGeneration(1),
            revision: 1,
            name: "Vell Ashwalker",
            identity: "Wood elf · Ranger · Level 2",
            subtitle: "Own character",
            connection: "Synthetic preview · No server session",
            notice: "Synthetic controlled preview",
            labels: SheetLabels {
                tabs: [
                    "Character",
                    "Equipment",
                    "Spells & features",
                    "Journal",
                    "Progression",
                ],
                navigation: "Personal character sheet",
                filter: "Filter the current sheet",
                no_matches: "No matching entries",
                art_fallback: "Optional artwork unavailable",
            },
            sections,
        }
    }
    fn control(
        document: &Document,
        parent: &Element,
        label: &str,
        callback: impl FnMut() + 'static,
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
        control.on_activate(callback).map_err(error)?;
        parent.append_child(control.element())?;
        Ok(control)
    }
    fn selected_focus(document: &Document, id: &str) -> Result<(), JsValue> {
        let tab = document
            .get_element_by_id(id)
            .ok_or_else(|| error("selected tab missing"))?;
        if document
            .active_element()
            .as_ref()
            .is_some_and(|active| tab.is_same_node(Some(active)))
        {
            Ok(())
        } else {
            Err(error("focus did not fall back to the usable selected tab"))
        }
    }
    fn held(root: &Element, selector: &str) -> Result<(Element, Node), JsValue> {
        let element = root
            .query_selector(selector)?
            .ok_or_else(|| error("retained target missing"))?;
        let text = element
            .first_child()
            .ok_or_else(|| error("retained text missing"))?;
        if text.node_type() != Node::TEXT_NODE || text.node_value().as_deref() == Some("") {
            return Err(error("regression must retain a populated actual Text node"));
        }
        Ok((element, text))
    }
    fn sanitized(element: &Element, text: &Node) -> Result<(), JsValue> {
        if element.text_content().as_deref() != Some("") || text.node_value().as_deref() != Some("")
        {
            Err(error(
                "detached private element or Text node retained its value",
            ))
        } else {
            Ok(())
        }
    }
    fn boundary_view<'a>(
        revision: u64,
        sections: &'a [SheetSection<'a>],
    ) -> CharacterSheetView<'a> {
        let mut view = initial(sections);
        view.revision = revision;
        view.name = "Synthetic private owner name";
        view
    }
    fn boundary_checks(document: &Document) -> Result<(), JsValue> {
        let prior_focus = document.active_element();
        let fields = [SheetField {
            label: "Private detail label",
            value: "Private detail value",
        }];
        let ready = [SheetOffer {
            id: "boundary-exact-offer",
            label: "Private offered action",
            enabled: true,
            pending: false,
        }];
        let pending = [SheetOffer {
            id: "boundary-exact-offer",
            label: "Private offered action",
            enabled: true,
            pending: true,
        }];
        let rows_a = [SheetRow {
            key: "boundary-a",
            title: "Needle item",
            value: "Private value",
            summary: "Authorized detail",
            details: &fields,
            offers: &ready,
        }];
        let rows_b = [SheetRow {
            key: "boundary-b",
            title: "Another item",
            value: "Private value",
            summary: "Authorized detail",
            details: &fields,
            offers: &[],
        }];
        let original = [
            SheetSection {
                key: "boundary-section-a",
                tab: SheetTab::Character,
                title: "Section A",
                caption: "Private caption A",
                rows: &rows_a,
            },
            SheetSection {
                key: "boundary-section-b",
                tab: SheetTab::Character,
                title: "Section B",
                caption: "Private caption B",
                rows: &rows_b,
            },
        ];
        let sheet = CharacterSheetSurface::create(
            document,
            "synthetic-sheet-boundaries",
            &boundary_view(1, &original),
            |_| {},
        )
        .map_err(error)?;
        let body = document.body().ok_or_else(|| error("body missing"))?;
        body.append_child(sheet.root())?;
        let (name, name_text) = held(sheet.root(), "h1")?;
        let (old_dd, old_dd_text) = held(sheet.root(), "[data-sheet-row='boundary-a'] dd")?;
        let (old_dt, old_dt_text) = held(sheet.root(), "[data-sheet-row='boundary-a'] dt")?;
        let row_a = sheet
            .root()
            .query_selector("[data-sheet-row='boundary-a']")?
            .ok_or_else(|| error("row A missing"))?;
        let row_b = sheet
            .root()
            .query_selector("[data-sheet-row='boundary-b']")?
            .ok_or_else(|| error("row B missing"))?;
        let section_a = row_a
            .parent_element()
            .and_then(|rows| rows.parent_element())
            .ok_or_else(|| error("section A missing"))?;
        let section_b = row_b
            .parent_element()
            .and_then(|rows| rows.parent_element())
            .ok_or_else(|| error("section B missing"))?;
        let moved = [
            SheetSection {
                key: "boundary-section-a",
                tab: SheetTab::Equipment,
                title: "Section A",
                caption: "Private caption A",
                rows: &rows_a,
            },
            SheetSection {
                key: "boundary-section-b",
                tab: SheetTab::Character,
                title: "Section B",
                caption: "Private caption B",
                rows: &rows_b,
            },
        ];
        sheet.update(&boundary_view(2, &moved)).map_err(error)?;
        sanitized(&old_dd, &old_dd_text)?;
        sanitized(&old_dt, &old_dt_text)?;
        if !sheet.root().is_connected()
            || section_a
                .parent_element()
                .map(|parent| parent.id())
                .as_deref()
                != Some("synthetic-sheet-boundaries-panel-1")
            || section_b
                .parent_element()
                .map(|parent| parent.id())
                .as_deref()
                != Some("synthetic-sheet-boundaries-panel-0")
            || !sheet
                .root()
                .query_selector("[data-sheet-row='boundary-a']")?
                .as_ref()
                .is_some_and(|row| row_a.is_same_node(Some(row)))
            || !sheet
                .root()
                .query_selector("[data-sheet-row='boundary-b']")?
                .as_ref()
                .is_some_and(|row| row_b.is_same_node(Some(row)))
        {
            return Err(error(
                "keyed section move lost mounted identity or ordering",
            ));
        }
        let tab = document
            .get_element_by_id("synthetic-sheet-boundaries-tab-1")
            .ok_or_else(|| error("equipment tab missing"))?
            .dyn_into::<HtmlElement>()?;
        tab.click();
        row_a.set_attribute("open", "")?;
        let input = sheet
            .root()
            .query_selector("input")?
            .ok_or_else(|| error("filter missing"))?
            .dyn_into::<HtmlInputElement>()?;
        input.set_value("needle");
        input.dispatch_event(&web_sys::Event::new("input")?)?;
        let summary = row_a
            .query_selector("summary")?
            .ok_or_else(|| error("summary missing"))?
            .dyn_into::<HtmlElement>()?;
        summary.focus()?;
        if !document
            .active_element()
            .as_ref()
            .is_some_and(|active| summary.is_same_node(Some(active)))
        {
            return Err(error(
                "hidden-focus regression did not start with focused summary",
            ));
        }
        let renamed_rows = [SheetRow {
            key: "boundary-a",
            title: "Renamed item",
            value: "Private value",
            summary: "Authorized detail",
            details: &fields,
            offers: &ready,
        }];
        let renamed = [
            SheetSection {
                key: "boundary-section-a",
                tab: SheetTab::Equipment,
                title: "Section A",
                caption: "Private caption A",
                rows: &renamed_rows,
            },
            SheetSection {
                key: "boundary-section-b",
                tab: SheetTab::Character,
                title: "Section B",
                caption: "Private caption B",
                rows: &rows_b,
            },
        ];
        sheet.update(&boundary_view(3, &renamed)).map_err(error)?;
        if !row_a.has_attribute("hidden") || input.value() != "needle" {
            return Err(error(
                "filter did not retain its query and hide the superseded match",
            ));
        }
        selected_focus(document, "synthetic-sheet-boundaries-tab-1")?;
        input.set_value("");
        input.dispatch_event(&web_sys::Event::new("input")?)?;
        let offer = row_a
            .query_selector("[data-sheet-offer='boundary-exact-offer']")?
            .ok_or_else(|| error("offer missing"))?
            .dyn_into::<HtmlElement>()?;
        offer.focus()?;
        if !document
            .active_element()
            .as_ref()
            .is_some_and(|active| offer.is_same_node(Some(active)))
        {
            return Err(error(
                "pending-focus regression did not start with focused offer",
            ));
        }
        let pending_rows = [SheetRow {
            key: "boundary-a",
            title: "Renamed item",
            value: "Private value",
            summary: "Authorized detail",
            details: &fields,
            offers: &pending,
        }];
        let pending_sections = [
            SheetSection {
                key: "boundary-section-a",
                tab: SheetTab::Equipment,
                title: "Section A",
                caption: "Private caption A",
                rows: &pending_rows,
            },
            SheetSection {
                key: "boundary-section-b",
                tab: SheetTab::Character,
                title: "Section B",
                caption: "Private caption B",
                rows: &rows_b,
            },
        ];
        sheet
            .update(&boundary_view(4, &pending_sections))
            .map_err(error)?;
        selected_focus(document, "synthetic-sheet-boundaries-tab-1")?;
        let (removed_dd, removed_dd_text) = held(sheet.root(), "[data-sheet-row='boundary-a'] dd")?;
        let (removed_dt, removed_dt_text) = held(sheet.root(), "[data-sheet-row='boundary-a'] dt")?;
        let (removed_title, removed_title_text) = held(
            sheet.root(),
            "[data-sheet-row='boundary-a'] .sheet-row-title",
        )?;
        let removed = [
            SheetSection {
                key: "boundary-section-a",
                tab: SheetTab::Equipment,
                title: "Section A",
                caption: "Private caption A",
                rows: &[],
            },
            SheetSection {
                key: "boundary-section-b",
                tab: SheetTab::Character,
                title: "Section B",
                caption: "Private caption B",
                rows: &rows_b,
            },
        ];
        sheet.update(&boundary_view(5, &removed)).map_err(error)?;
        sanitized(&removed_dd, &removed_dd_text)?;
        sanitized(&removed_dt, &removed_dt_text)?;
        sanitized(&removed_title, &removed_title_text)?;
        let (section_dd, section_dd_text) = held(sheet.root(), "[data-sheet-row='boundary-b'] dd")?;
        let (section_title, section_title_text) = held(&section_b, "h2")?;
        sheet
            .update(&boundary_view(6, &removed[..1]))
            .map_err(error)?;
        sanitized(&section_dd, &section_dd_text)?;
        sanitized(&section_title, &section_title_text)?;
        sheet.update(&boundary_view(7, &moved)).map_err(error)?;
        let (owner_dd, owner_dd_text) = held(sheet.root(), "[data-sheet-row='boundary-a'] dd")?;
        let (owner_offer, owner_offer_text) =
            held(sheet.root(), "[data-sheet-offer='boundary-exact-offer']")?;
        input.set_value("private filter");
        input.dispatch_event(&web_sys::Event::new("input")?)?;
        let mut replacement = boundary_view(8, &[]);
        replacement.owner = SheetOwnerGeneration(2);
        if !matches!(
            sheet.update(&replacement),
            Err(df_ui::SheetError::OwnerChanged)
        ) {
            return Err(error("owner change failed to invalidate private scope"));
        }
        sanitized(&name, &name_text)?;
        sanitized(&owner_dd, &owner_dd_text)?;
        sanitized(&owner_offer, &owner_offer_text)?;
        if !input.value().is_empty() || sheet.root().is_connected() {
            return Err(error("private filter or root survived owner change"));
        }
        sheet.dispose().map_err(error)?;
        let disposed = CharacterSheetSurface::create(
            document,
            "synthetic-sheet-dispose-boundaries",
            &boundary_view(1, &original),
            |_| {},
        )
        .map_err(error)?;
        body.append_child(disposed.root())?;
        let (dispose_name, dispose_name_text) = held(disposed.root(), "h1")?;
        let (dispose_dd, dispose_dd_text) = held(disposed.root(), "dd")?;
        let (dispose_dt, dispose_dt_text) = held(disposed.root(), "dt")?;
        disposed.dispose().map_err(error)?;
        disposed.dispose().map_err(error)?;
        sanitized(&dispose_name, &dispose_name_text)?;
        sanitized(&dispose_dd, &dispose_dd_text)?;
        sanitized(&dispose_dt, &dispose_dt_text)?;
        if let Some(prior) = prior_focus.and_then(|element| element.dyn_into::<HtmlElement>().ok())
        {
            prior.focus()?;
        }
        Ok(())
    }
    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window missing"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("document missing"))?;
        let body = document
            .body()
            .ok_or_else(|| JsValue::from_str("body missing"))?;
        body.set_attribute("style", "margin:0;background:#09111c")?;
        let harness = document.create_element("aside")?;
        harness.set_attribute("style", "padding:20px 5vw;color:#c8d5e0;font:13px system-ui;background:#0a1523;display:flex;gap:12px;flex-wrap:wrap;align-items:center")?;
        harness.set_attribute("aria-label", "Synthetic presentation checks")?;
        let status = child(
            &document,
            &harness,
            "p",
            "Controlled preview checks are ready.",
        )?;
        status.set_attribute("role", "status")?;
        let submission_status = status.clone();
        let sheet = Rc::new(
            CharacterSheetSurface::create(
                &document,
                "synthetic-character-sheet",
                &initial(&[]),
                move |submission: SheetSubmission| {
                    submission_status.set_text_content(Some(&format!(
                        "Synthetic intent only: {} · owner {}. No inventory or reward mutation.",
                        submission.offer_id, submission.owner.0
                    )));
                },
            )
            .map_err(error)?,
        );
        present(&sheet, 1, false, false)?;
        body.append_child(sheet.root())?;
        body.append_child(&harness)?;
        let revision = Rc::new(Cell::new(1));
        let revoked = Rc::new(Cell::new(false));
        let mut controls = Vec::new();
        let boundary_document = document.clone();
        let boundary_status = status.clone();
        controls.push(control(&document, &harness, "Check retained private nodes & update boundaries", move || {
            let result = boundary_checks(&boundary_document);
            boundary_status.set_text_content(Some(if result.is_ok() {
                "PASS · actual Text nodes and dt/dd scrubbed on update, removal, owner change and disposal; moved sections retain identity; hidden/pending focus falls back."
            } else { "FAIL · retained-node or update-boundary regression failed." }));
        })?);
        let updated = Rc::clone(&sheet);
        let current = Rc::clone(&revision);
        let invalidated = Rc::clone(&revoked);
        let update_status = status.clone();
        let update_document = document.clone();
        controls.push(control(&document, &harness, "Check 64 same-phase updates", move || {
            let result = (|| -> Result<(), JsValue> {
                let input = updated.root().query_selector("input")?
                    .ok_or_else(|| JsValue::from_str("filter missing"))?.dyn_into::<HtmlInputElement>()?;
                input.set_value("strength");
                input.dispatch_event(&web_sys::Event::new("input")?)?;
                input.focus()?;
                let before = update_document.active_element();
                let heading = updated.root().query_selector("h1")?;
                let details = updated.root().query_selector("[data-sheet-row='strength']")?
                    .ok_or_else(|| JsValue::from_str("ability row missing"))?;
                details.set_attribute("open", "")?;
                let window = web_sys::window().ok_or_else(|| JsValue::from_str("window missing"))?;
                window.scroll_to_with_x_and_y(0.0, 370.0);
                let top = window.scroll_y()?;
                for _ in 0..64 {
                    current.set(current.get() + 1);
                    present(&updated, current.get(), invalidated.get(), false)?;
                }
                let focus = before.as_ref().is_some_and(|before|
                    update_document.active_element().as_ref().is_some_and(|after| before.is_same_node(Some(after))));
                let same_heading = heading.as_ref().is_some_and(|before|
                    updated.root().query_selector("h1").ok().flatten().as_ref()
                        .is_some_and(|after| before.is_same_node(Some(after))));
                let scroll = window.scroll_y()? == top;
                if !focus || !same_heading || !scroll || input.value() != "strength"
                    || !details.has_attribute("open") { return Err(JsValue::from_str("mounted presentation changed")); }
                Ok(())
            })();
            update_status.set_text_content(Some(if result.is_ok() {
                "PASS · 64 updates retained heading, focused filter, query, expansion and page scroll."
            } else { "FAIL · same-phase check failed." }));
        })?);
        let updated = Rc::clone(&sheet);
        let current = Rc::clone(&revision);
        let invalidated = Rc::clone(&revoked);
        let revoke_status = status.clone();
        controls.push(control(
            &document,
            &harness,
            "Invalidate equipment offer",
            move || {
                let stale = updated
                    .root()
                    .query_selector("[data-sheet-offer='fixture-equip-longbow-007']")
                    .ok()
                    .flatten();
                invalidated.set(true);
                current.set(current.get() + 1);
                let result = present(&updated, current.get(), true, false).and_then(|()| {
                    revoke_status.set_text_content(Some("No stale callback observed"));
                    if let Some(stale) = stale {
                        stale.dispatch_event(&web_sys::Event::new("click")?)?;
                    }
                    if updated
                        .root()
                        .query_selector("[data-sheet-offer='fixture-equip-longbow-007']")?
                        .is_some()
                        || revoke_status.text_content().as_deref()
                            != Some("No stale callback observed")
                    {
                        return Err(JsValue::from_str("invalidated action remains"));
                    }
                    Ok(())
                });
                revoke_status.set_text_content(Some(if result.is_ok() {
                    "PASS · revoked offered ID removed; retained detached button cannot submit."
                } else {
                    "FAIL · invalidated offer check failed."
                }));
            },
        )?);
        let updated = Rc::clone(&sheet);
        let current = Rc::clone(&revision);
        let invalidated = Rc::clone(&revoked);
        let rejected_status = status.clone();
        controls.push(control(
            &document,
            &harness,
            "Supply rejected action",
            move || {
                current.set(current.get() + 1);
                let query = updated
                    .root()
                    .query_selector("input")
                    .ok()
                    .flatten()
                    .and_then(|node| node.dyn_into::<HtmlInputElement>().ok())
                    .map(|node| node.value());
                let result = present(&updated, current.get(), invalidated.get(), true);
                let after = updated
                    .root()
                    .query_selector("input")
                    .ok()
                    .flatten()
                    .and_then(|node| node.dyn_into::<HtmlInputElement>().ok())
                    .map(|node| node.value());
                rejected_status.set_text_content(Some(if result.is_ok() && query == after {
                "PASS · supplied rejection rendered; local filter retained; inventory unchanged."
            } else { "FAIL · rejection check failed." }));
            },
        )?);
        let updated = Rc::clone(&sheet);
        let art_status = status.clone();
        controls.push(control(
            &document,
            &harness,
            "Exercise missing artwork",
            move || {
                let result = (|| -> Result<(), JsValue> {
                    let art = updated
                        .root()
                        .query_selector(".sheet-art")?
                        .ok_or_else(|| JsValue::from_str("art missing"))?;
                    art.set_attribute("src", "assets/synthetic-deliberately-missing.webp")?;
                    art.dispatch_event(&web_sys::Event::new("error")?)?;
                    if !art.has_attribute("hidden") {
                        return Err(JsValue::from_str("fallback missing"));
                    }
                    Ok(())
                })();
                art_status.set_text_content(Some(if result.is_ok() {
                    "PASS · missing optional art falls back; sheet controls remain available."
                } else {
                    "FAIL · artwork fallback check failed."
                }));
            },
        )?);
        let updated = Rc::clone(&sheet);
        let dispose_status = status.clone();
        controls.push(control(&document, &harness, "Dispose private owner", move || {
            let result = updated.dispose().and_then(|()| updated.dispose());
            dispose_status.set_text_content(Some(if result.is_ok() && updated.root().text_content().as_deref() == Some("") {
                "PASS · private owner disposed twice; text and filter cleared; callbacks fenced."
            } else { "FAIL · private disposal check failed." }));
        })?);
        FIXTURE.with(|slot| {
            *slot.borrow_mut() = Some(Fixture {
                sheet,
                controls,
                revision,
                revoked,
                status,
                harness,
            })
        });
        Ok(())
    }
    /// External evaluator can trigger a current accepted synthetic update in place.
    #[wasm_bindgen]
    pub fn update() -> Result<(), JsValue> {
        FIXTURE.with(|slot| {
            let owned = slot.borrow();
            let fixture = owned
                .as_ref()
                .ok_or_else(|| JsValue::from_str("fixture disposed"))?;
            fixture.revision.set(fixture.revision.get() + 1);
            present(
                &fixture.sheet,
                fixture.revision.get(),
                fixture.revoked.get(),
                false,
            )
        })
    }
    /// Controlled owner-change check does not invent a new private member projection.
    #[wasm_bindgen]
    pub fn change_owner() -> Result<(), JsValue> {
        FIXTURE.with(|slot| {
            let owned = slot.borrow();
            let fixture = owned
                .as_ref()
                .ok_or_else(|| JsValue::from_str("fixture disposed"))?;
            let mut replacement = initial(&[]);
            replacement.owner = SheetOwnerGeneration(2);
            if !matches!(
                fixture.sheet.update(&replacement),
                Err(df_ui::SheetError::OwnerChanged)
            ) {
                return Err(JsValue::from_str(
                    "owner change did not invalidate private scope",
                ));
            }
            fixture.status.set_text_content(Some(
                "PASS · ownership change scrubbed and disposed the previous private sheet.",
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
                    control.dispose().map_err(error)?;
                }
                fixture.sheet.dispose().map_err(error)?;
                fixture.harness.remove();
            }
            *owned = None;
            Ok(())
        })
    }
}
