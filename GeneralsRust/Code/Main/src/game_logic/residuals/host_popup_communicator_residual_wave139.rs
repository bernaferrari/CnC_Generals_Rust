//! Wave 139 residual peels: PopupCommunicator WND residual
//! (show/Ok/ESC; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 133 ControlBar PopupCommunicator, Wave 129 Diplomacy.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - PopupCommunicator.cpp / PopupCommunicator.wnd
//! - ButtonOk, ESC → Ok
//!
//! Fail-closed:
//! - Not full buddy list / WOL residual
//! - Not full modal layout create residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Popup communicator residual tables
// ---------------------------------------------------------------------------

/// Retail PopupCommunicator layout filename residual.
pub const POPUP_COMMUNICATOR_LAYOUT_FILENAME_WAVE139: &str = "Menus/PopupCommunicator.wnd";

/// Retail PopupCommunicator control names residual.
pub const POPUP_COMMUNICATOR_CONTROL_NAMES_WAVE139: &[&str] = &[
    "PopupCommunicator.wnd:PopupCommunicator",
    "PopupCommunicator.wnd:ButtonOk",
];

/// Ordered PopupCommunicator residual navigation steps.
pub const POPUP_COMMUNICATOR_NAV_STEPS_WAVE139: &[&str] = &[
    "OPEN_POPUP_COMMUNICATOR",
    "SET_MODAL",
    "GBM_SELECTED_BUTTON_OK",
    "UNSET_MODAL",
    "DESTROY_LAYOUT",
    "ESC_MAPS_TO_OK",
];

/// Runtime-host command residual names for PopupCommunicator peels.
pub const RUNTIME_HOST_POPUP_COMMUNICATOR_CMD_NAMES_WAVE139: &[&str] = &[
    "open_popup_communicator_ok_wnd",
    "open_popup_communicator_miss",
    "click_popup_communicator_ok_wnd_ok",
    "click_popup_communicator_ok_wnd_esc",
    "click_popup_communicator_ok_wnd_prepare_ok",
    "click_popup_communicator_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: PopupCommunicator control names residual pack.
pub fn honesty_popup_communicator_control_names_residual_wave139() -> bool {
    POPUP_COMMUNICATOR_LAYOUT_FILENAME_WAVE139 == "Menus/PopupCommunicator.wnd"
        && POPUP_COMMUNICATOR_CONTROL_NAMES_WAVE139.len() == 2
        && residual_name_index(
            POPUP_COMMUNICATOR_CONTROL_NAMES_WAVE139,
            "PopupCommunicator.wnd:ButtonOk",
        ) == Some(1)
        && POPUP_COMMUNICATOR_CONTROL_NAMES_WAVE139
            .iter()
            .all(|n| n.starts_with("PopupCommunicator.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_popup_communicator_nav_commands_residual_wave139() -> bool {
    POPUP_COMMUNICATOR_NAV_STEPS_WAVE139.len() == 6
        && residual_name_index(
            POPUP_COMMUNICATOR_NAV_STEPS_WAVE139,
            "GBM_SELECTED_BUTTON_OK",
        ) == Some(2)
        && residual_name_index(POPUP_COMMUNICATOR_NAV_STEPS_WAVE139, "ESC_MAPS_TO_OK") == Some(5)
        && RUNTIME_HOST_POPUP_COMMUNICATOR_CMD_NAMES_WAVE139.len() == 6
        && residual_name_index(
            RUNTIME_HOST_POPUP_COMMUNICATOR_CMD_NAMES_WAVE139,
            "click_popup_communicator_ok_wnd_prepare_ok",
        ) == Some(4)
}

/// Wave 139 composite residual honesty pack.
pub fn honesty_popup_communicator_residual_pack_wave139() -> bool {
    honesty_popup_communicator_control_names_residual_wave139()
        && honesty_popup_communicator_nav_commands_residual_wave139()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_popup_communicator_control_names_residual_wave139());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_popup_communicator_nav_commands_residual_wave139());
    }

    #[test]
    fn wave139_composite_pack() {
        assert!(honesty_popup_communicator_residual_pack_wave139());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_popup_communicator_prepare_ok_residual_live() {
        use game_client::gui::callbacks::{
            ResidualPopupCommunicatorAction, residual_popup_communicator_is_visible,
            residual_popup_communicator_last_action, simulate_popup_communicator_prepare_ok,
        };
        assert!(
            simulate_popup_communicator_prepare_ok(),
            "show+ok residual must latch"
        );
        assert!(!residual_popup_communicator_is_visible());
        assert_eq!(
            residual_popup_communicator_last_action(),
            ResidualPopupCommunicatorAction::Ok
        );
    }
}
