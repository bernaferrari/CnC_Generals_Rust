//! Wave 126 residual peels: OptionsMenu WND residual
//! (Accept/Back/Defaults/Keyboard/Advanced; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 124 KeyboardOptions, Wave 118 ButtonOptions.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - OptionsMenu.cpp / OptionsMenu.wnd
//! - ButtonAccept, ButtonBack, ButtonDefaults, ButtonKeyboardOptions
//! - ButtonAdvanceAccept, ButtonAdvanceBack, ButtonFirewallRefresh
//!
//! Fail-closed:
//! - Not full preference write / resolution dialog residual
//! - Not full advanced checkbox/slider populate residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Options menu residual tables
// ---------------------------------------------------------------------------

/// Retail OptionsMenu layout filename residual.
pub const OPTIONS_MENU_LAYOUT_FILENAME_WAVE126: &str = "Menus/OptionsMenu.wnd";

/// Retail OptionsMenu core control names residual.
pub const OPTIONS_MENU_CONTROL_NAMES_WAVE126: &[&str] = &[
    "OptionsMenu.wnd:OptionsMenuParent",
    "OptionsMenu.wnd:ButtonBack",
    "OptionsMenu.wnd:ButtonDefaults",
    "OptionsMenu.wnd:ButtonAccept",
    "OptionsMenu.wnd:ButtonKeyboardOptions",
    "OptionsMenu.wnd:ButtonAdvanceAccept",
    "OptionsMenu.wnd:ButtonAdvanceBack",
    "OptionsMenu.wnd:ButtonFirewallRefresh",
    "OptionsMenu.wnd:ComboBoxDetail",
    "OptionsMenu.wnd:ComboBoxResolution",
    "OptionsMenu.wnd:SliderMusicVolume",
    "OptionsMenu.wnd:WinAdvancedDisplayOptions",
];

/// Ordered OptionsMenu residual navigation steps.
pub const OPTIONS_MENU_NAV_STEPS_WAVE126: &[&str] = &[
    "PUSH_OPTIONS_MENU_LAYOUT",
    "POPULATE_CONTROLS",
    "GBM_SELECTED_BUTTON_DEFAULTS",
    "APPLY_DEFAULT_CONTROLS",
    "GBM_SELECTED_BUTTON_KEYBOARD_OPTIONS",
    "PUSH_KEYBOARD_OPTIONS",
    "GBM_SELECTED_BUTTON_ACCEPT",
    "APPLY_OPTIONS",
    "CLOSE_OPTIONS_MENU",
    "GBM_SELECTED_BUTTON_BACK",
    "DESTROY_OPTIONS_LAYOUT",
];

/// Runtime-host command residual names for OptionsMenu peels.
pub const RUNTIME_HOST_OPTIONS_MENU_CMD_NAMES_WAVE126: &[&str] = &[
    "open_options_menu_ok_wnd",
    "open_options_menu_ok",
    "click_options_menu_ok_wnd_accept",
    "click_options_menu_ok_wnd_back",
    "click_options_menu_ok_wnd_defaults",
    "click_options_menu_ok_wnd_keyboard",
    "click_options_menu_ok_wnd_advanced_accept",
    "click_options_menu_ok_wnd_advanced_back",
    "click_options_menu_ok_wnd_firewall",
    "click_options_menu_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: OptionsMenu control names residual pack.
pub fn honesty_options_menu_control_names_residual_wave126() -> bool {
    OPTIONS_MENU_LAYOUT_FILENAME_WAVE126 == "Menus/OptionsMenu.wnd"
        && OPTIONS_MENU_CONTROL_NAMES_WAVE126.len() == 12
        && residual_name_index(
            OPTIONS_MENU_CONTROL_NAMES_WAVE126,
            "OptionsMenu.wnd:ButtonAccept",
        ) == Some(3)
        && residual_name_index(
            OPTIONS_MENU_CONTROL_NAMES_WAVE126,
            "OptionsMenu.wnd:ButtonBack",
        ) == Some(1)
        && residual_name_index(
            OPTIONS_MENU_CONTROL_NAMES_WAVE126,
            "OptionsMenu.wnd:ButtonKeyboardOptions",
        ) == Some(4)
        && OPTIONS_MENU_CONTROL_NAMES_WAVE126
            .iter()
            .all(|n| n.starts_with("OptionsMenu.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_options_menu_nav_commands_residual_wave126() -> bool {
    OPTIONS_MENU_NAV_STEPS_WAVE126.len() == 11
        && residual_name_index(OPTIONS_MENU_NAV_STEPS_WAVE126, "POPULATE_CONTROLS") == Some(1)
        && residual_name_index(OPTIONS_MENU_NAV_STEPS_WAVE126, "GBM_SELECTED_BUTTON_ACCEPT")
            == Some(6)
        && residual_name_index(OPTIONS_MENU_NAV_STEPS_WAVE126, "DESTROY_OPTIONS_LAYOUT") == Some(10)
        && RUNTIME_HOST_OPTIONS_MENU_CMD_NAMES_WAVE126.len() == 10
        && residual_name_index(
            RUNTIME_HOST_OPTIONS_MENU_CMD_NAMES_WAVE126,
            "open_options_menu_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_OPTIONS_MENU_CMD_NAMES_WAVE126,
            "click_options_menu_ok_wnd_accept",
        ) == Some(2)
}

/// Wave 126 composite residual honesty pack.
pub fn honesty_options_menu_residual_pack_wave126() -> bool {
    honesty_options_menu_control_names_residual_wave126()
        && honesty_options_menu_nav_commands_residual_wave126()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_options_menu_control_names_residual_wave126());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_options_menu_nav_commands_residual_wave126());
    }

    #[test]
    fn wave126_composite_pack() {
        assert!(honesty_options_menu_residual_pack_wave126());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_options_prepare_accept_residual_live() {
        use game_client::gui::callbacks::{
            ResidualOptionsMenuAction, residual_options_menu_is_bound,
            residual_options_menu_last_action, simulate_options_menu_back_button_gadget_selected,
            simulate_options_menu_prepare_accept,
        };
        assert!(
            simulate_options_menu_prepare_accept(),
            "bind+accept residual must latch"
        );
        assert!(residual_options_menu_is_bound());
        assert_eq!(
            residual_options_menu_last_action(),
            ResidualOptionsMenuAction::Accept
        );
        assert!(simulate_options_menu_back_button_gadget_selected());
        assert_eq!(
            residual_options_menu_last_action(),
            ResidualOptionsMenuAction::Back
        );
    }
}
