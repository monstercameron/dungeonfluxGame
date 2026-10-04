use super::{Dispatch, RoleInput, RoleShellError};
use df_display::DisplaySessionOverlays;
use df_player::PlayerSessionOverlays;
use df_ui::SessionOverlayView;
use std::rc::Rc;
use web_sys::{Document, Element};

pub(super) enum MountedOverlay {
    Player(PlayerSessionOverlays),
    Display(DisplaySessionOverlays),
}
impl MountedOverlay {
    pub(super) fn mount(
        document: &Document,
        slot: &Element,
        essential: &Element,
        player: bool,
        view: &SessionOverlayView<'_>,
        dispatch: Rc<Dispatch>,
        generation: u64,
    ) -> Result<Self, RoleShellError> {
        Ok(if player {
            Self::Player(PlayerSessionOverlays::mount(
                document,
                slot,
                essential,
                view,
                move |input| dispatch.emit(generation, RoleInput::PlayerOverlay(input)),
            )?)
        } else {
            Self::Display(DisplaySessionOverlays::mount(
                document,
                slot,
                essential,
                view,
                move |input| dispatch.emit(generation, RoleInput::DisplayOverlay(input)),
            )?)
        })
    }
    pub(super) fn update(&mut self, view: &SessionOverlayView<'_>) -> Result<(), RoleShellError> {
        match self {
            Self::Player(mount) => mount.update(view).map_err(Into::into),
            Self::Display(mount) => mount.update(view).map_err(Into::into),
        }
    }
    pub(super) fn revoke(&mut self) -> Result<(), RoleShellError> {
        match self {
            Self::Player(mount) => mount.revoke().map_err(Into::into),
            Self::Display(mount) => mount.revoke().map_err(Into::into),
        }
    }
}
