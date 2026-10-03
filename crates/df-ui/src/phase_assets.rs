//! Build-owned, public UI artwork. This catalog supplies static identities only;
//! private media authorization, fetching and caches remain with their existing owners.

/// A closed selection of original static SVG assets shipped with this client build.
/// Paths are deployment-relative, matching `ConceptScene::asset_path`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseAsset {
    D20,
    Sword,
    Shield,
    Backpack,
    Spellbook,
    Compass,
    Party,
    Lantern,
    Connection,
    PortraitFrame,
    SectionOrnament,
}

/// Required presentation behavior when optional artwork cannot be displayed.
/// The mounting owner handles the failed load once, without retries or replacing
/// an authorized asset with unrelated artwork. Text and input remain available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseAssetFallback {
    /// Hide the failed icon; retain the adjacent localized control/section label.
    KeepLabel,
    /// Hide the failed ornament; preserve its content and layout bounds.
    OmitDecoration,
}

/// Complete finite catalog. No server-provided path or arbitrary URL is accepted.
pub const PHASE_ASSETS: [PhaseAsset; 11] = [
    PhaseAsset::D20,
    PhaseAsset::Sword,
    PhaseAsset::Shield,
    PhaseAsset::Backpack,
    PhaseAsset::Spellbook,
    PhaseAsset::Compass,
    PhaseAsset::Party,
    PhaseAsset::Lantern,
    PhaseAsset::Connection,
    PhaseAsset::PortraitFrame,
    PhaseAsset::SectionOrnament,
];

/// Maximum shipped SVG size, in bytes. This bounds catalog assets, not private media.
pub const MAX_PHASE_ASSET_BYTES: usize = 2048;

impl PhaseAsset {
    pub const fn name(self) -> &'static str {
        match self {
            Self::D20 => "d20",
            Self::Sword => "sword",
            Self::Shield => "shield",
            Self::Backpack => "backpack",
            Self::Spellbook => "spellbook",
            Self::Compass => "compass",
            Self::Party => "party",
            Self::Lantern => "lantern",
            Self::Connection => "connection",
            Self::PortraitFrame => "portrait-frame",
            Self::SectionOrnament => "section-ornament",
        }
    }

    /// Exact allowlisted name lookup; unknown names remain explicitly absent.
    pub fn from_name(name: &str) -> Option<Self> {
        PHASE_ASSETS.into_iter().find(|asset| asset.name() == name)
    }

    pub const fn asset_path(self) -> &'static str {
        match self {
            Self::D20 => "assets/ui/d20.svg",
            Self::Sword => "assets/ui/sword.svg",
            Self::Shield => "assets/ui/shield.svg",
            Self::Backpack => "assets/ui/backpack.svg",
            Self::Spellbook => "assets/ui/spellbook.svg",
            Self::Compass => "assets/ui/compass.svg",
            Self::Party => "assets/ui/party.svg",
            Self::Lantern => "assets/ui/lantern.svg",
            Self::Connection => "assets/ui/connection.svg",
            Self::PortraitFrame => "assets/ui/portrait-frame.svg",
            Self::SectionOrnament => "assets/ui/section-ornament.svg",
        }
    }

    /// English catalog description, not a gameplay state or a localized control label.
    /// Icons beside equivalent visible text should use an empty `img` alt instead.
    /// Decorations always use empty alt and are hidden from assistive technology.
    pub const fn accessible_label(self) -> Option<&'static str> {
        match self {
            Self::D20 => Some("Twenty-sided die"),
            Self::Sword => Some("Sword"),
            Self::Shield => Some("Shield"),
            Self::Backpack => Some("Backpack"),
            Self::Spellbook => Some("Spellbook"),
            Self::Compass => Some("Compass"),
            Self::Party => Some("Adventuring party"),
            Self::Lantern => Some("Lantern"),
            Self::Connection => Some("Connection"),
            Self::PortraitFrame | Self::SectionOrnament => None,
        }
    }

    /// Intrinsic SVG width and height in CSS pixels. Icons support 24, 32 and 48 px;
    /// the portrait frame uses 120 × 144 and the divider uses 240 × 16 proportions.
    pub const fn intrinsic_size(self) -> (u16, u16) {
        match self {
            Self::PortraitFrame => (120, 144),
            Self::SectionOrnament => (240, 16),
            _ => (24, 24),
        }
    }

    pub const fn fallback(self) -> PhaseAssetFallback {
        match self {
            Self::PortraitFrame | Self::SectionOrnament => PhaseAssetFallback::OmitDecoration,
            _ => PhaseAssetFallback::KeepLabel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untrusted_paths_and_unknown_names_are_not_catalog_assets() {
        for name in [
            "../sword",
            "assets/ui/sword.svg",
            "https://example.com/sword.svg",
            "Sword",
            "unknown",
            "",
        ] {
            assert_eq!(PhaseAsset::from_name(name), None);
        }
        assert_eq!(PhaseAsset::from_name("sword"), Some(PhaseAsset::Sword));
    }

    #[test]
    fn artwork_failure_preserves_labels_and_omits_only_decorations() {
        for asset in PHASE_ASSETS {
            assert_eq!(
                asset.fallback(),
                if asset.accessible_label().is_some() {
                    PhaseAssetFallback::KeepLabel
                } else {
                    PhaseAssetFallback::OmitDecoration
                },
            );
        }
    }
}
