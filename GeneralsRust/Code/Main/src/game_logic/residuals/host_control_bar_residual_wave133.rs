//! Wave 133 residual peels: ControlBar WND residual
//! (show/hide + Options/Idle/General/Beacon; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 126 OptionsMenu, Wave 129 Diplomacy communicator.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ControlBarCallback.cpp / ControlBar.wnd
//! - ButtonOptions, ButtonIdleWorker, ButtonGeneral, ButtonLarge
//! - ButtonPlaceBeacon, ButtonDeleteBeacon, ButtonClearBeaconText, PopupCommunicator
//!
//! Fail-closed:
//! - Not full command button cameo residual
//! - Not full beacon place/delete multiplayer residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Control bar residual tables
// ---------------------------------------------------------------------------

/// Retail ControlBar layout filename residual.
pub const CONTROL_BAR_LAYOUT_FILENAME_WAVE133: &str = "ControlBar.wnd";

/// Retail ControlBar control names residual.
pub const CONTROL_BAR_CONTROL_NAMES_WAVE133: &[&str] = &[
    "ControlBar.wnd:ControlBarParent",
    "ControlBar.wnd:ButtonOptions",
    "ControlBar.wnd:ButtonIdleWorker",
    "ControlBar.wnd:ButtonGeneral",
    "ControlBar.wnd:ButtonLarge",
    "ControlBar.wnd:ButtonPlaceBeacon",
    "ControlBar.wnd:ButtonDeleteBeacon",
    "ControlBar.wnd:ButtonClearBeaconText",
    "ControlBar.wnd:EditBeaconText",
    "ControlBar.wnd:PopupCommunicator",
];

/// Ordered ControlBar residual navigation steps.
pub const CONTROL_BAR_NAV_STEPS_WAVE133: &[&str] = &[
    "SHOW_CONTROL_BAR",
    "GBM_SELECTED_BUTTON_OPTIONS",
    "OPEN_OPTIONS_MENU",
    "GBM_SELECTED_BUTTON_IDLE_WORKER",
    "SELECT_NEXT_IDLE_WORKER",
    "GBM_SELECTED_BUTTON_GENERAL",
    "OPEN_GENERALS_EXP",
    "GBM_SELECTED_BUTTON_LARGE",
    "GBM_SELECTED_POPUP_COMMUNICATOR",
    "TOGGLE_DIPLOMACY",
    "HIDE_CONTROL_BAR",
];

/// Runtime-host command residual names for ControlBar peels.
pub const RUNTIME_HOST_CONTROL_BAR_CMD_NAMES_WAVE133: &[&str] = &[
    "toggle_control_bar_ok_wnd",
    "toggle_control_bar_miss",
    "click_control_bar_ok_wnd_options",
    "click_control_bar_ok_wnd_idle",
    "click_control_bar_ok_wnd_general",
    "click_control_bar_ok_wnd_show",
    "click_control_bar_ok_wnd_hide",
    "click_control_bar_ok_wnd_prepare_options",
    "click_control_bar_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: ControlBar control names residual pack.
pub fn honesty_control_bar_control_names_residual_wave133() -> bool {
    CONTROL_BAR_LAYOUT_FILENAME_WAVE133 == "ControlBar.wnd"
        && CONTROL_BAR_CONTROL_NAMES_WAVE133.len() == 10
        && residual_name_index(
            CONTROL_BAR_CONTROL_NAMES_WAVE133,
            "ControlBar.wnd:ButtonOptions",
        ) == Some(1)
        && residual_name_index(
            CONTROL_BAR_CONTROL_NAMES_WAVE133,
            "ControlBar.wnd:ButtonIdleWorker",
        ) == Some(2)
        && residual_name_index(
            CONTROL_BAR_CONTROL_NAMES_WAVE133,
            "ControlBar.wnd:PopupCommunicator",
        ) == Some(9)
        && CONTROL_BAR_CONTROL_NAMES_WAVE133
            .iter()
            .all(|n| n.starts_with("ControlBar.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_control_bar_nav_commands_residual_wave133() -> bool {
    CONTROL_BAR_NAV_STEPS_WAVE133.len() == 11
        && residual_name_index(CONTROL_BAR_NAV_STEPS_WAVE133, "SHOW_CONTROL_BAR") == Some(0)
        && residual_name_index(CONTROL_BAR_NAV_STEPS_WAVE133, "GBM_SELECTED_BUTTON_OPTIONS")
            == Some(1)
        && residual_name_index(CONTROL_BAR_NAV_STEPS_WAVE133, "HIDE_CONTROL_BAR") == Some(10)
        && RUNTIME_HOST_CONTROL_BAR_CMD_NAMES_WAVE133.len() == 9
        && residual_name_index(
            RUNTIME_HOST_CONTROL_BAR_CMD_NAMES_WAVE133,
            "toggle_control_bar_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_CONTROL_BAR_CMD_NAMES_WAVE133,
            "click_control_bar_ok_wnd_prepare_options",
        ) == Some(7)
}

/// Wave 133 composite residual honesty pack.
pub fn honesty_control_bar_residual_pack_wave133() -> bool {
    honesty_control_bar_control_names_residual_wave133()
        && honesty_control_bar_nav_commands_residual_wave133()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_control_bar_control_names_residual_wave133());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_control_bar_nav_commands_residual_wave133());
    }

    #[test]
    fn wave133_composite_pack() {
        assert!(honesty_control_bar_residual_pack_wave133());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_control_bar_prepare_options_residual_live() {
        use game_client::gui::callbacks::{
            ResidualControlBarAction, residual_control_bar_is_visible,
            residual_control_bar_last_action, simulate_control_bar_hide,
            simulate_control_bar_prepare_options,
        };
        assert!(
            simulate_control_bar_prepare_options(),
            "show+options residual must latch"
        );
        assert!(residual_control_bar_is_visible());
        assert_eq!(
            residual_control_bar_last_action(),
            ResidualControlBarAction::Options
        );
        assert!(simulate_control_bar_hide());
        assert!(!residual_control_bar_is_visible());
        assert_eq!(
            residual_control_bar_last_action(),
            ResidualControlBarAction::Hide
        );
    }
}
