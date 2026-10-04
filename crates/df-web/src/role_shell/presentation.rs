use df_display::DisplayJoinInput;
use df_player::PlayerJoinInput;
use df_types::SessionRevision;
use df_ui::{
    AftermathSelection, CampaignLimits, CampfireSelection, CampfireView, CharacterDisplayLimits,
    CharacterDisplaySubmission, CharacterDisplayView, CharacterLimits, CharacterPhaseView,
    CharacterSheetView, CharacterSubmission, CombatIntent, CombatLimits, CombatPhaseView,
    EncounterAftermathView, ExplorationInput, ExplorationLimits, ExplorationView, JoinPhaseIntent,
    JoinPhaseView, SceneTransitionView, SessionOverlaySelection, SheetSubmission,
};

/// Borrowed, already audience-filtered presentation. This is not a wire snapshot,
/// credential or authorization boundary. No server/private record is accepted here.
pub enum PlayerPhase<'a> {
    Join {
        view: &'a JoinPhaseView<'a>,
        input: PlayerJoinInput<'a>,
        limits: CampaignLimits,
    },
    Character {
        view: &'a CharacterPhaseView,
        limits: CharacterLimits,
    },
    Sheet(&'a CharacterSheetView<'a>),
    Exploration {
        view: &'a ExplorationView<'a>,
        limits: ExplorationLimits,
    },
    Combat {
        view: &'a CombatPhaseView<'a>,
        limits: CombatLimits,
    },
    Aftermath(&'a EncounterAftermathView<'a>),
    Campfire(&'a CampfireView<'a>),
    Transition {
        view: &'a SceneTransitionView<'a>,
        generation: u64,
    },
}

/// The display owner must supply its separate public projection. The shell never
/// converts player presentation into display presentation or infers host rights.
pub enum DisplayPhase<'a> {
    Join {
        view: &'a JoinPhaseView<'a>,
        input: DisplayJoinInput<'a>,
        limits: CampaignLimits,
    },
    Character {
        view: &'a CharacterDisplayView,
        limits: CharacterDisplayLimits,
    },
    Exploration {
        view: &'a ExplorationView<'a>,
        limits: ExplorationLimits,
    },
    Combat {
        view: &'a CombatPhaseView<'a>,
        limits: CombatLimits,
    },
    Aftermath(&'a EncounterAftermathView<'a>),
    Campfire(&'a CampfireView<'a>),
    Transition {
        view: &'a SceneTransitionView<'a>,
        generation: u64,
    },
}

/// Local composition only. Selecting a role does not create an audience grant.
pub enum RolePhase<'a> {
    Player(PlayerPhase<'a>),
    Display(DisplayPhase<'a>),
}

/// Exact existing advertised selection values, forwarded without game resolution.
/// The connection owner maps these through its authorized generated RPC boundary.
pub enum RoleInput {
    PlayerJoin(JoinPhaseIntent),
    DisplayJoin(JoinPhaseIntent),
    PlayerCharacter(CharacterSubmission),
    DisplayCharacter(CharacterDisplaySubmission),
    Sheet(SheetSubmission),
    PlayerExploration(ExplorationInput),
    DisplayExploration(ExplorationInput),
    PlayerCombat(CombatIntent),
    DisplayCombat(CombatIntent),
    PlayerAftermath(AftermathSelection),
    DisplayAftermath(AftermathSelection),
    PlayerCampfire(CampfireSelection),
    DisplayCampfire(CampfireSelection),
    PlayerTransition(SessionRevision),
    DisplayTransition(SessionRevision),
    PlayerOverlay(SessionOverlaySelection),
    DisplayOverlay(SessionOverlaySelection),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ScopeKey {
    PlayerJoin(u64),
    DisplayJoin(u64),
    PlayerCharacter(u64, String),
    DisplayCharacter(u64, String),
    Sheet(u64),
    PlayerExploration,
    DisplayExploration,
    PlayerCombat,
    DisplayCombat,
    PlayerAftermath(u64),
    DisplayAftermath(u64),
    PlayerCampfire(u64),
    DisplayCampfire(u64),
    PlayerTransition(u64),
    DisplayTransition(u64),
}

impl RolePhase<'_> {
    pub(super) fn key(&self) -> ScopeKey {
        match self {
            Self::Player(PlayerPhase::Join { view, .. }) => {
                ScopeKey::PlayerJoin(view.stamp.generation)
            }
            Self::Display(DisplayPhase::Join { view, .. }) => {
                ScopeKey::DisplayJoin(view.stamp.generation)
            }
            Self::Player(PlayerPhase::Character { view, .. }) => {
                ScopeKey::PlayerCharacter(view.generation, view.owner_key.clone())
            }
            Self::Display(DisplayPhase::Character { view, .. }) => {
                ScopeKey::DisplayCharacter(view.generation, view.public_scope_key.clone())
            }
            Self::Player(PlayerPhase::Sheet(view)) => ScopeKey::Sheet(view.owner.0),
            Self::Player(PlayerPhase::Exploration { .. }) => ScopeKey::PlayerExploration,
            Self::Display(DisplayPhase::Exploration { .. }) => ScopeKey::DisplayExploration,
            Self::Player(PlayerPhase::Combat { .. }) => ScopeKey::PlayerCombat,
            Self::Display(DisplayPhase::Combat { .. }) => ScopeKey::DisplayCombat,
            Self::Player(PlayerPhase::Aftermath(view)) => {
                ScopeKey::PlayerAftermath(view.generation)
            }
            Self::Player(PlayerPhase::Campfire(view)) => {
                ScopeKey::PlayerCampfire(view.owner_generation)
            }
            Self::Player(PlayerPhase::Transition { generation, .. }) => {
                ScopeKey::PlayerTransition(*generation)
            }
            Self::Display(DisplayPhase::Aftermath(view)) => {
                ScopeKey::DisplayAftermath(view.generation)
            }
            Self::Display(DisplayPhase::Campfire(view)) => {
                ScopeKey::DisplayCampfire(view.owner_generation)
            }
            Self::Display(DisplayPhase::Transition { generation, .. }) => {
                ScopeKey::DisplayTransition(*generation)
            }
        }
    }
    pub(super) fn matches_key(&self, current: Option<&ScopeKey>) -> bool {
        match (self, current) {
            (
                Self::Player(PlayerPhase::Join { view, .. }),
                Some(ScopeKey::PlayerJoin(generation)),
            ) => view.stamp.generation == *generation,
            (
                Self::Display(DisplayPhase::Join { view, .. }),
                Some(ScopeKey::DisplayJoin(generation)),
            ) => view.stamp.generation == *generation,
            (
                Self::Player(PlayerPhase::Character { view, .. }),
                Some(ScopeKey::PlayerCharacter(generation, owner)),
            ) => view.generation == *generation && view.owner_key == *owner,
            (
                Self::Display(DisplayPhase::Character { view, .. }),
                Some(ScopeKey::DisplayCharacter(generation, owner)),
            ) => view.generation == *generation && view.public_scope_key == *owner,
            (Self::Player(PlayerPhase::Sheet(view)), Some(ScopeKey::Sheet(owner))) => {
                view.owner.0 == *owner
            }
            (Self::Player(PlayerPhase::Exploration { .. }), Some(ScopeKey::PlayerExploration))
            | (
                Self::Display(DisplayPhase::Exploration { .. }),
                Some(ScopeKey::DisplayExploration),
            )
            | (Self::Player(PlayerPhase::Combat { .. }), Some(ScopeKey::PlayerCombat))
            | (Self::Display(DisplayPhase::Combat { .. }), Some(ScopeKey::DisplayCombat)) => true,
            (
                Self::Player(PlayerPhase::Aftermath(view)),
                Some(ScopeKey::PlayerAftermath(generation)),
            ) => view.generation == *generation,
            (
                Self::Player(PlayerPhase::Campfire(view)),
                Some(ScopeKey::PlayerCampfire(generation)),
            ) => view.owner_generation == *generation,
            (
                Self::Player(PlayerPhase::Transition { generation, .. }),
                Some(ScopeKey::PlayerTransition(current)),
            ) => generation == current,
            (
                Self::Display(DisplayPhase::Aftermath(view)),
                Some(ScopeKey::DisplayAftermath(generation)),
            ) => view.generation == *generation,
            (
                Self::Display(DisplayPhase::Campfire(view)),
                Some(ScopeKey::DisplayCampfire(generation)),
            ) => view.owner_generation == *generation,
            (
                Self::Display(DisplayPhase::Transition { generation, .. }),
                Some(ScopeKey::DisplayTransition(current)),
            ) => generation == current,
            _ => false,
        }
    }
    pub(super) fn is_player(&self) -> bool {
        matches!(self, Self::Player(_))
    }
}
