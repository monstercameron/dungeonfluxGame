use super::presentation::{DisplayPhase, PlayerPhase, RoleInput, RolePhase};
use super::{Dispatch, RoleShellError};
use df_display::{
    DisplayCharacterScreen, DisplayCombat, DisplayExploration, DisplayJoinConnection,
    DisplayJoinScreen,
};
use df_player::{
    PlayerCharacterConnection, PlayerCharacterScreen, PlayerCombat, PlayerExploration,
    PlayerJoinConnection, PlayerJoinScreen, PlayerSheetConnection, PlayerSheetScreen,
};
use df_types::{ClientBindingId, SessionRevision};
use df_ui::{CharacterDisplayConnection, SessionConnection};
use std::rc::Rc;
use web_sys::{Document, Element};

pub(super) enum MountedPhase {
    PlayerJoin(PlayerJoinScreen),
    DisplayJoin(DisplayJoinScreen),
    PlayerCharacter(PlayerCharacterScreen),
    DisplayCharacter(DisplayCharacterScreen),
    Sheet(PlayerSheetScreen),
    PlayerExploration(PlayerExploration),
    DisplayExploration(DisplayExploration),
    PlayerCombat(PlayerCombat),
    DisplayCombat(DisplayCombat),
}

pub(super) fn validate(
    phase: &RolePhase<'_>,
    binding: ClientBindingId,
    revision: SessionRevision,
) -> Result<(), RoleShellError> {
    match phase {
        RolePhase::Player(PlayerPhase::Join { view, limits, .. }) => {
            view.validate(*limits).map_err(|error| {
                df_player::PlayerJoinError::from(df_ui::JoinPhaseError::from(error)).into()
            })
        }
        RolePhase::Display(DisplayPhase::Join { view, limits, .. }) => {
            view.validate(*limits).map_err(|error| {
                df_display::DisplayJoinError::from(df_ui::JoinPhaseError::from(error)).into()
            })
        }
        RolePhase::Player(PlayerPhase::Character { view, limits }) => view
            .validate(*limits)
            .map_err(|error| df_ui::CharacterPhaseError::from(error).into()),
        RolePhase::Display(DisplayPhase::Character { view, limits }) => view
            .validate(*limits)
            .map_err(|error| df_ui::CharacterPhaseError::from(error).into()),
        RolePhase::Player(PlayerPhase::Sheet(view)) => view
            .validate()
            .map_err(|error| df_ui::SheetError::from(error).into()),
        RolePhase::Player(PlayerPhase::Exploration { view, limits }) => {
            if view.binding != binding || view.revision != revision {
                return Err(RoleShellError::SnapshotMismatch);
            }
            view.validate(*limits).map_err(|error| {
                df_player::ExplorationMountError::from(df_ui::ExplorationError::from(error)).into()
            })
        }
        RolePhase::Display(DisplayPhase::Exploration { view, limits }) => {
            if view.binding != binding || view.revision != revision {
                return Err(RoleShellError::SnapshotMismatch);
            }
            view.validate(*limits).map_err(|error| {
                df_display::ExplorationMountError::from(df_ui::ExplorationError::from(error)).into()
            })
        }
        RolePhase::Player(PlayerPhase::Combat { view, limits }) => {
            view.validate(*limits).map_err(|error| {
                df_player::CombatMountError::from(df_ui::CombatError::from(error)).into()
            })
        }
        RolePhase::Display(DisplayPhase::Combat { view, limits }) => {
            view.validate(*limits).map_err(|error| {
                df_display::CombatMountError::from(df_ui::CombatError::from(error)).into()
            })
        }
    }
}

impl MountedPhase {
    pub(super) fn mount(
        document: &Document,
        slot: &Element,
        identifier: &str,
        binding_revision: (ClientBindingId, SessionRevision),
        phase: RolePhase<'_>,
        dispatch: Rc<Dispatch>,
        generation: u64,
    ) -> Result<Self, RoleShellError> {
        let (binding, revision) = binding_revision;
        Ok(match phase {
            RolePhase::Player(PlayerPhase::Join {
                view,
                input,
                limits,
            }) => Self::PlayerJoin(PlayerJoinScreen::mount(
                document,
                slot,
                view,
                input,
                limits,
                PlayerJoinConnection::Connected,
                move |input| dispatch.emit(generation, RoleInput::PlayerJoin(input)),
            )?),
            RolePhase::Display(DisplayPhase::Join {
                view,
                input,
                limits,
            }) => Self::DisplayJoin(DisplayJoinScreen::mount(
                document,
                slot,
                view,
                input,
                limits,
                DisplayJoinConnection::Connected,
                move |input| dispatch.emit(generation, RoleInput::DisplayJoin(input)),
            )?),
            RolePhase::Player(PlayerPhase::Character { view, limits }) => {
                Self::PlayerCharacter(PlayerCharacterScreen::mount(
                    document,
                    slot,
                    identifier,
                    view,
                    limits,
                    PlayerCharacterConnection::Connected,
                    move |input| dispatch.emit(generation, RoleInput::PlayerCharacter(input)),
                )?)
            }
            RolePhase::Display(DisplayPhase::Character { view, limits }) => {
                Self::DisplayCharacter(DisplayCharacterScreen::mount(
                    document,
                    slot,
                    view,
                    limits,
                    CharacterDisplayConnection::Connected,
                    move |input| dispatch.emit(generation, RoleInput::DisplayCharacter(input)),
                )?)
            }
            RolePhase::Player(PlayerPhase::Sheet(view)) => Self::Sheet(PlayerSheetScreen::mount(
                document,
                slot,
                identifier,
                view,
                PlayerSheetConnection::Connected,
                move |input| dispatch.emit(generation, RoleInput::Sheet(input)),
            )?),
            RolePhase::Player(PlayerPhase::Exploration { view, limits }) => {
                Self::PlayerExploration(PlayerExploration::mount(
                    document,
                    slot,
                    identifier,
                    view,
                    limits,
                    move |input| dispatch.emit(generation, RoleInput::PlayerExploration(input)),
                )?)
            }
            RolePhase::Display(DisplayPhase::Exploration { view, limits }) => {
                Self::DisplayExploration(DisplayExploration::mount(
                    document,
                    slot,
                    identifier,
                    view,
                    limits,
                    move |input| dispatch.emit(generation, RoleInput::DisplayExploration(input)),
                )?)
            }
            RolePhase::Player(PlayerPhase::Combat { view, limits }) => {
                Self::PlayerCombat(PlayerCombat::mount(
                    document,
                    slot,
                    identifier,
                    (binding, revision),
                    view,
                    limits,
                    move |input| dispatch.emit(generation, RoleInput::PlayerCombat(input)),
                )?)
            }
            RolePhase::Display(DisplayPhase::Combat { view, limits }) => {
                Self::DisplayCombat(DisplayCombat::mount(
                    document,
                    slot,
                    identifier,
                    (binding, revision),
                    view,
                    limits,
                    move |input| dispatch.emit(generation, RoleInput::DisplayCombat(input)),
                )?)
            }
        })
    }
    pub(super) fn root(&self) -> &Element {
        match self {
            Self::PlayerJoin(mount) => mount.root(),
            Self::DisplayJoin(mount) => mount.root(),
            Self::PlayerCharacter(mount) => mount.root(),
            Self::DisplayCharacter(mount) => mount.root(),
            Self::Sheet(mount) => mount.root(),
            Self::PlayerExploration(mount) => mount.root(),
            Self::DisplayExploration(mount) => mount.root(),
            Self::PlayerCombat(mount) => mount.root(),
            Self::DisplayCombat(mount) => mount.root(),
        }
    }
    pub(super) fn update(
        &mut self,
        binding: ClientBindingId,
        revision: SessionRevision,
        phase: RolePhase<'_>,
    ) -> Result<(), RoleShellError> {
        match (self, phase) {
            (Self::PlayerJoin(mount), RolePhase::Player(PlayerPhase::Join { view, .. })) => mount
                .update(view)
                .map_err(RoleShellError::from)
                .and_then(|update| {
                    if update == df_ui::JoinUpdate::Applied {
                        Ok(())
                    } else {
                        Err(RoleShellError::SnapshotMismatch)
                    }
                }),
            (Self::DisplayJoin(mount), RolePhase::Display(DisplayPhase::Join { view, .. })) => {
                mount
                    .update(view)
                    .map_err(RoleShellError::from)
                    .and_then(|update| {
                        if update == df_ui::JoinUpdate::Applied {
                            Ok(())
                        } else {
                            Err(RoleShellError::SnapshotMismatch)
                        }
                    })
            }
            (
                Self::PlayerCharacter(mount),
                RolePhase::Player(PlayerPhase::Character { view, .. }),
            ) => mount.update(view).map_err(Into::into),
            (
                Self::DisplayCharacter(mount),
                RolePhase::Display(DisplayPhase::Character { view, .. }),
            ) => mount.update(view).map_err(Into::into),
            (Self::Sheet(mount), RolePhase::Player(PlayerPhase::Sheet(view))) => {
                mount.update(view).map_err(Into::into)
            }
            (
                Self::PlayerExploration(mount),
                RolePhase::Player(PlayerPhase::Exploration { view, .. }),
            ) => mount.update(view).map_err(Into::into),
            (
                Self::DisplayExploration(mount),
                RolePhase::Display(DisplayPhase::Exploration { view, .. }),
            ) => mount.update(view).map_err(Into::into),
            (Self::PlayerCombat(mount), RolePhase::Player(PlayerPhase::Combat { view, .. })) => {
                mount.update(binding, revision, view).map_err(Into::into)
            }
            (Self::DisplayCombat(mount), RolePhase::Display(DisplayPhase::Combat { view, .. })) => {
                mount.update(binding, revision, view).map_err(Into::into)
            }
            _ => Err(RoleShellError::SnapshotMismatch),
        }
    }
    pub(super) fn connection(
        &mut self,
        connection: SessionConnection,
    ) -> Result<(), RoleShellError> {
        let connected = connection == SessionConnection::Connected;
        let offline = connection == SessionConnection::Offline;
        match self {
            Self::PlayerJoin(mount) => mount
                .set_connection(if connected {
                    PlayerJoinConnection::Connected
                } else if offline {
                    PlayerJoinConnection::Offline
                } else {
                    PlayerJoinConnection::Reconnecting
                })
                .map_err(Into::into),
            Self::DisplayJoin(mount) => mount
                .set_connection(if connected {
                    DisplayJoinConnection::Connected
                } else if offline {
                    DisplayJoinConnection::Offline
                } else {
                    DisplayJoinConnection::Reconnecting
                })
                .map_err(Into::into),
            Self::PlayerCharacter(mount) => mount
                .set_connection(if connected {
                    PlayerCharacterConnection::Connected
                } else if offline {
                    PlayerCharacterConnection::Offline
                } else {
                    PlayerCharacterConnection::Reconnecting
                })
                .map_err(Into::into),
            Self::DisplayCharacter(mount) => mount
                .set_connection(if connected {
                    CharacterDisplayConnection::Connected
                } else if offline {
                    CharacterDisplayConnection::Offline
                } else {
                    CharacterDisplayConnection::Reconnecting
                })
                .map_err(Into::into),
            Self::Sheet(mount) => mount
                .set_connection(if connected {
                    PlayerSheetConnection::Connected
                } else if offline {
                    PlayerSheetConnection::Offline
                } else {
                    PlayerSheetConnection::Reconnecting
                })
                .map_err(Into::into),
            Self::PlayerExploration(mount) if !connected => {
                mount.suspend_input().map_err(Into::into)
            }
            Self::DisplayExploration(mount) if !connected => {
                mount.suspend_input().map_err(Into::into)
            }
            Self::PlayerCombat(mount) if !connected => mount.suspend_input().map_err(Into::into),
            Self::DisplayCombat(mount) if !connected => mount.suspend_input().map_err(Into::into),
            _ => Ok(()),
        }
    }
    pub(super) fn revoke(&mut self) -> Result<(), RoleShellError> {
        match self {
            Self::PlayerJoin(mount) => mount.revoke().map_err(Into::into),
            Self::DisplayJoin(mount) => mount.revoke().map_err(Into::into),
            Self::PlayerCharacter(mount) => mount.revoke().map_err(Into::into),
            Self::DisplayCharacter(mount) => mount.revoke().map_err(Into::into),
            Self::Sheet(mount) => mount.revoke().map_err(Into::into),
            Self::PlayerExploration(mount) => mount.revoke().map_err(Into::into),
            Self::DisplayExploration(mount) => mount.revoke().map_err(Into::into),
            Self::PlayerCombat(mount) => mount.revoke().map_err(Into::into),
            Self::DisplayCombat(mount) => mount.revoke().map_err(Into::into),
        }
    }
}
