use std::{collections::BTreeSet, fmt};

/// Closed artwork selection. Baked interface references are never scene backgrounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConceptScene {
    Harbor,
    Tavern,
    Mountain,
    SunkenHall,
    Campfire,
}

impl ConceptScene {
    pub const fn asset_path(self) -> &'static str {
        match self {
            Self::Harbor => "assets/concept-art/establishing-harbor-canal-rowboat-bridge.webp",
            Self::Tavern => "assets/concept-art/scene-tavern-barkeep-talk-rain.webp",
            Self::Mountain => "assets/concept-art/scene-mountain-ruins-snowy-ridge-trek.webp",
            Self::SunkenHall => "assets/concept-art/scene-flooded-hall-party-wading-torchlit.webp",
            Self::Campfire => "assets/concept-art/scene-campfire-under-stars.webp",
        }
    }
    pub const fn description(self) -> &'static str {
        match self {
            Self::Harbor => "Moonlit harbor with lanterns, a stone bridge and a distant castle",
            Self::Tavern => "Adventurers talking to a barkeeper in a candlelit tavern",
            Self::Mountain => "Adventurers trekking toward snowy mountain ruins",
            Self::SunkenHall => "A torchlit party wading through a flooded hall",
            Self::Campfire => "A campfire beneath the stars",
        }
    }
}

/// Resource policy chosen by the mounting owner, separate from gameplay party limits.
#[derive(Clone, Copy, Debug)]
pub struct CampaignLimits {
    pub max_members: usize,
    pub max_objectives: usize,
    pub max_text_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectiveState {
    Pending,
    Active,
    Complete,
}

pub struct CampaignMember<'a> {
    pub key: &'a str,
    pub name: &'a str,
    pub role: &'a str,
    pub sigil: &'a str,
}
pub struct CampaignObjective<'a> {
    pub label: &'a str,
    pub state: ObjectiveState,
}

/// Already localized, audience-safe presentation from the caller. No mechanical
/// resolution, permission filtering, or connection inference happens in this model.
pub struct CampaignView<'a> {
    pub scene: ConceptScene,
    pub chapter: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub location: &'a str,
    pub scene_label: &'a str,
    pub narration: &'a str,
    pub connection: &'a str,
    pub notice: &'a str,
    pub members: &'a [CampaignMember<'a>],
    pub objectives: &'a [CampaignObjective<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CampaignValidationError {
    ResourceLimit,
    EmptyText,
    DuplicateMemberKey,
}
impl fmt::Display for CampaignValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ResourceLimit => "campaign presentation exceeds owner limits",
            Self::EmptyText => "campaign presentation text is empty",
            Self::DuplicateMemberKey => "campaign member presentation keys repeat",
        })
    }
}
impl std::error::Error for CampaignValidationError {}

impl CampaignView<'_> {
    pub fn validate(&self, limits: CampaignLimits) -> Result<(), CampaignValidationError> {
        if self.members.len() > limits.max_members || self.objectives.len() > limits.max_objectives
        {
            return Err(CampaignValidationError::ResourceLimit);
        }
        let validate = |text: &str| {
            if text.trim().is_empty() {
                Err(CampaignValidationError::EmptyText)
            } else if text.len() > limits.max_text_bytes {
                Err(CampaignValidationError::ResourceLimit)
            } else {
                Ok(())
            }
        };
        for text in [
            self.chapter,
            self.title,
            self.description,
            self.location,
            self.scene_label,
            self.narration,
            self.connection,
            self.notice,
        ] {
            validate(text)?;
        }
        let mut keys = BTreeSet::new();
        for member in self.members {
            for text in [member.key, member.name, member.role, member.sigil] {
                validate(text)?;
            }
            if !keys.insert(member.key) {
                return Err(CampaignValidationError::DuplicateMemberKey);
            }
        }
        for objective in self.objectives {
            validate(objective.label)?;
        }
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use crate::UiError;
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeMap,
    };
    use web_sys::{Document, Element};

    #[derive(Debug)]
    pub enum CampaignError {
        InvalidView(CampaignValidationError),
        Dom(UiError),
        Disposed,
    }
    impl fmt::Display for CampaignError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::InvalidView(e) => fmt::Display::fmt(e, f),
                Self::Dom(e) => fmt::Display::fmt(e, f),
                Self::Disposed => f.write_str("campaign surface is disposed"),
            }
        }
    }
    impl std::error::Error for CampaignError {}
    impl From<wasm_bindgen::JsValue> for CampaignError {
        fn from(e: wasm_bindgen::JsValue) -> Self {
            Self::Dom(UiError::Browser(e))
        }
    }
    impl From<CampaignValidationError> for CampaignError {
        fn from(e: CampaignValidationError) -> Self {
            Self::InvalidView(e)
        }
    }

    struct MemberNodes {
        root: Element,
        name: Element,
        role: Element,
        sigil: Element,
    }
    /// One mounted cinematic shell. Updates retain root, navigation controls and
    /// keyed party nodes. The caller owns navigation listeners and disposes them
    /// before disposing the shell. There is no detached work or audio playback.
    pub struct CampaignSurface {
        document: Document,
        root: Element,
        art: Element,
        toolbar: Element,
        chapter: Element,
        title: Element,
        description: Element,
        location: Element,
        scene_label: Element,
        narration: Element,
        connection: Element,
        notice: Element,
        party: Element,
        objectives: Element,
        members: RefCell<BTreeMap<String, MemberNodes>>,
        objective_nodes: RefCell<Vec<Element>>,
        limits: CampaignLimits,
        disposed: Cell<bool>,
    }
    fn element(
        doc: &Document,
        tag: &str,
        class: &str,
        text: Option<&str>,
    ) -> Result<Element, CampaignError> {
        let node = doc.create_element(tag)?;
        node.set_class_name(class);
        node.set_text_content(text);
        Ok(node)
    }
    fn child(
        doc: &Document,
        parent: &Element,
        tag: &str,
        class: &str,
        text: Option<&str>,
    ) -> Result<Element, CampaignError> {
        let node = element(doc, tag, class, text)?;
        parent.append_child(&node)?;
        Ok(node)
    }
    impl CampaignSurface {
        pub fn create(
            document: &Document,
            view: &CampaignView<'_>,
            limits: CampaignLimits,
        ) -> Result<Self, CampaignError> {
            view.validate(limits)?;
            let root = element(document, "section", "df-campaign", None)?;
            let style = child(
                document,
                &root,
                "style",
                "",
                Some(crate::campaign_theme::STYLES),
            )?;
            style.set_attribute("data-campaign-theme", "")?;
            let art = child(document, &root, "img", "scene-art", None)?;
            art.set_attribute("fetchpriority", "high")?;
            let header = child(document, &root, "header", "topbar", None)?;
            let brand = child(document, &header, "div", "brand", Some("Dungeon"))?;
            child(document, &brand, "span", "", Some("Flux"))?;
            let toolbar = child(document, &header, "nav", "toolbar", None)?;
            toolbar.set_attribute("aria-label", "Presentation scenes")?;
            let notice = child(document, &header, "div", "preview-badge", None)?;
            let stage = child(document, &root, "div", "stage", None)?;
            let hero = child(document, &stage, "div", "hero", None)?;
            let chapter = child(document, &hero, "p", "chapter", None)?;
            let title = child(document, &hero, "h1", "", None)?;
            let description = child(document, &hero, "p", "description", None)?;
            let location = child(document, &hero, "p", "location", None)?;
            let panel = child(document, &stage, "aside", "scene-panel", None)?;
            child(document, &panel, "div", "overline", Some("Current scene"))?;
            let scene_label = child(document, &panel, "h2", "", None)?;
            let objectives = child(document, &panel, "ol", "objectives", None)?;
            let lower = child(document, &root, "div", "lower", None)?;
            let narration_panel = child(document, &lower, "section", "narration", None)?;
            narration_panel.set_attribute("aria-label", "Narration")?;
            let avatar = child(document, &narration_panel, "img", "narrator", None)?;
            avatar.set_attribute("src", "assets/concept-art/dm-avatar.webp")?;
            avatar.set_attribute("alt", "Hooded narrator portrait")?;
            let narrative = child(document, &narration_panel, "div", "", None)?;
            child(
                document,
                &narrative,
                "div",
                "overline",
                Some("Dungeon Master · Narration"),
            )?;
            let narration = child(document, &narrative, "blockquote", "", None)?;
            let party_section = child(document, &lower, "section", "party-section", None)?;
            let party_heading = child(document, &party_section, "div", "party-heading", None)?;
            child(
                document,
                &party_heading,
                "div",
                "overline",
                Some("Your party"),
            )?;
            child(
                document,
                &party_heading,
                "span",
                "overline",
                Some("Together, a story"),
            )?;
            let party = child(document, &party_section, "div", "party", None)?;
            let connection = child(document, &root, "footer", "connection", None)?;
            let surface = Self {
                document: document.clone(),
                root,
                art,
                toolbar,
                chapter,
                title,
                description,
                location,
                scene_label,
                narration,
                connection,
                notice,
                party,
                objectives,
                members: RefCell::new(BTreeMap::new()),
                objective_nodes: RefCell::new(Vec::new()),
                limits,
                disposed: Cell::new(false),
            };
            surface.update(view)?;
            Ok(surface)
        }
        pub fn root(&self) -> &Element {
            &self.root
        }
        pub fn navigation(&self) -> &Element {
            &self.toolbar
        }
        pub fn update(&self, view: &CampaignView<'_>) -> Result<(), CampaignError> {
            if self.disposed.get() {
                return Err(CampaignError::Disposed);
            }
            view.validate(self.limits)?;
            self.art.set_attribute("src", view.scene.asset_path())?;
            self.art.set_attribute("alt", view.scene.description())?;
            for (node, text) in [
                (&self.chapter, view.chapter),
                (&self.title, view.title),
                (&self.description, view.description),
                (&self.location, view.location),
                (&self.scene_label, view.scene_label),
                (&self.narration, view.narration),
                (&self.connection, view.connection),
                (&self.notice, view.notice),
            ] {
                node.set_text_content(Some(text));
            }
            let mut members = self.members.borrow_mut();
            let obsolete: Vec<_> = members
                .keys()
                .filter(|key| !view.members.iter().any(|member| member.key == key.as_str()))
                .cloned()
                .collect();
            for key in obsolete {
                if let Some(nodes) = members.remove(&key) {
                    self.party.remove_child(&nodes.root)?;
                }
            }
            for member in view.members {
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    members.entry(member.key.to_owned())
                {
                    let root = element(&self.document, "article", "party-card", None)?;
                    let seal = child(&self.document, &root, "div", "sigil", None)?;
                    let sigil = child(&self.document, &seal, "span", "", None)?;
                    let content = child(&self.document, &root, "div", "", None)?;
                    let name = child(&self.document, &content, "div", "member-name", None)?;
                    let role = child(&self.document, &content, "div", "member-role", None)?;
                    entry.insert(MemberNodes {
                        root,
                        name,
                        role,
                        sigil,
                    });
                }
                if let Some(nodes) = members.get(member.key) {
                    nodes.name.set_text_content(Some(member.name));
                    nodes.role.set_text_content(Some(member.role));
                    nodes.sigil.set_text_content(Some(member.sigil));
                    self.party.append_child(&nodes.root)?;
                }
            }
            let mut objectives = self.objective_nodes.borrow_mut();
            while objectives.len() > view.objectives.len() {
                if let Some(node) = objectives.pop() {
                    self.objectives.remove_child(&node)?;
                }
            }
            while objectives.len() < view.objectives.len() {
                objectives.push(child(
                    &self.document,
                    &self.objectives,
                    "li",
                    "objective",
                    None,
                )?);
            }
            for (node, objective) in objectives.iter().zip(view.objectives) {
                node.set_text_content(Some(objective.label));
                node.set_attribute(
                    "data-state",
                    match objective.state {
                        ObjectiveState::Pending => "pending",
                        ObjectiveState::Active => "active",
                        ObjectiveState::Complete => "complete",
                    },
                )?;
            }
            Ok(())
        }
        /// Terminal, repeatable removal. No callbacks or timers are owned here.
        pub fn dispose(&self) -> Result<(), CampaignError> {
            self.disposed.set(true);
            if let Some(parent) = self.root.parent_node() {
                parent.remove_child(&self.root)?;
            }
            self.art.remove_attribute("src")?;
            self.members.borrow_mut().clear();
            self.objective_nodes.borrow_mut().clear();
            self.root.set_text_content(None);
            Ok(())
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub use browser::{CampaignError, CampaignSurface};

#[cfg(test)]
mod tests {
    use super::*;
    fn view<'a>(members: &'a [CampaignMember<'a>]) -> CampaignView<'a> {
        CampaignView {
            scene: ConceptScene::Harbor,
            chapter: "I",
            title: "<img src=x onerror=alert(1)>",
            description: "Description",
            location: "Harbor",
            scene_label: "Scene",
            narration: "Narration",
            connection: "Offline",
            notice: "Preview",
            members,
            objectives: &[],
        }
    }
    #[test]
    fn owner_limits_bound_presentation_without_a_four_player_rule() {
        let keys: Vec<_> = (0..8).map(|index| format!("member-{index}")).collect();
        let mut members: Vec<_> = keys
            .iter()
            .map(|key| CampaignMember {
                key,
                name: "Name",
                role: "Role",
                sigil: "N",
            })
            .collect();
        let limits = CampaignLimits {
            max_members: 8,
            max_objectives: 10,
            max_text_bytes: 100,
        };
        assert_eq!(view(&members).validate(limits), Ok(()));
        members[1].key = members[0].key;
        assert_eq!(
            view(&members).validate(limits),
            Err(CampaignValidationError::DuplicateMemberKey)
        );
        assert_eq!(view(&[]).validate(limits), Ok(()));
        assert_eq!(
            view(&members).validate(CampaignLimits {
                max_members: 7,
                ..limits
            }),
            Err(CampaignValidationError::ResourceLimit)
        );
    }
    #[test]
    fn empty_or_overlong_text_is_rejected_before_dom_publication() {
        let limits = CampaignLimits {
            max_members: 10,
            max_objectives: 10,
            max_text_bytes: 100,
        };
        let mut data = view(&[]);
        data.narration = " ";
        assert_eq!(
            data.validate(limits),
            Err(CampaignValidationError::EmptyText)
        );
        data.narration = "Narration";
        assert_eq!(
            data.validate(CampaignLimits {
                max_text_bytes: 3,
                ..limits
            }),
            Err(CampaignValidationError::ResourceLimit)
        );
    }
}
