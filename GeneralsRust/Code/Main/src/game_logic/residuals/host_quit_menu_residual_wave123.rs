//! Wave 123 residual peels: QuitMenu WND residual
//! (Exit/Return/Options/Restart/SaveLoad + confirm; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 121 SaveLoad, Wave 122 ReplayMenu.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - QuitMenu.cpp / QuitMenu.wnd
//! - ButtonExit, ButtonReturn, ButtonOptions, ButtonRestart, ButtonSaveLoad
//!
//! Fail-closed:
//! - Not full layout create / ClearGameData residual
//! - Not multiplayer SelfDestruct residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Quit menu residual tables
// ---------------------------------------------------------------------------

/// Retail QuitMenu layout filename residual (full save path).
pub const QUIT_MENU_LAYOUT_FILENAME_WAVE123: &str = "Menus/QuitMenu.wnd";

/// Retail no-save QuitMenu layout filename residual.
pub const QUIT_MENU_NO_SAVE_LAYOUT_FILENAME_WAVE123: &str = "Menus/QuitNoSave.wnd";

/// Retail QuitMenu control names residual.
pub const QUIT_MENU_CONTROL_NAMES_WAVE123: &[&str] = &[
    "QuitMenu.wnd:ButtonExit",
    "QuitMenu.wnd:ButtonRestart",
    "QuitMenu.wnd:ButtonReturn",
    "QuitMenu.wnd:ButtonOptions",
    "QuitMenu.wnd:ButtonSaveLoad",
];

/// Ordered QuitMenu residual navigation steps.
pub const QUIT_MENU_NAV_STEPS_WAVE123: &[&str] = &[
    "ESC_OR_TOGGLE_QUIT_MENU",
    "SHOW_QUIT_MENU_LAYOUT",
    "GBM_SELECTED_BUTTON_OPTIONS",
    "GBM_SELECTED_BUTTON_SAVE_LOAD",
    "GBM_SELECTED_BUTTON_RESTART",
    "CONFIRM_RESTART_OR_SURRENDER",
    "GBM_SELECTED_BUTTON_EXIT",
    "CONFIRM_EXIT_YES",
    "CLEAR_GAME_DATA",
    "GBM_SELECTED_BUTTON_RETURN",
    "HIDE_QUIT_MENU",
];

/// Runtime-host command residual names for QuitMenu peels.
pub const RUNTIME_HOST_QUIT_MENU_CMD_NAMES_WAVE123: &[&str] = &[
    "toggle_quit_menu_ok_wnd",
    "toggle_quit_menu_miss",
    "click_quit_menu_ok_wnd_exit",
    "click_quit_menu_ok_wnd_return",
    "click_quit_menu_ok_wnd_options",
    "click_quit_menu_ok_wnd_restart",
    "click_quit_menu_ok_wnd_save_load",
    "click_quit_menu_ok_wnd_confirm_exit",
    "click_quit_menu_ok_wnd_prepare_exit",
    "click_quit_menu_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: QuitMenu control names residual pack.
pub fn honesty_quit_menu_control_names_residual_wave123() -> bool {
    QUIT_MENU_LAYOUT_FILENAME_WAVE123 == "Menus/QuitMenu.wnd"
        && QUIT_MENU_NO_SAVE_LAYOUT_FILENAME_WAVE123 == "Menus/QuitNoSave.wnd"
        && QUIT_MENU_CONTROL_NAMES_WAVE123.len() == 5
        && residual_name_index(QUIT_MENU_CONTROL_NAMES_WAVE123, "QuitMenu.wnd:ButtonExit")
            == Some(0)
        && residual_name_index(QUIT_MENU_CONTROL_NAMES_WAVE123, "QuitMenu.wnd:ButtonReturn")
            == Some(2)
        && residual_name_index(
            QUIT_MENU_CONTROL_NAMES_WAVE123,
            "QuitMenu.wnd:ButtonSaveLoad",
        ) == Some(4)
        && QUIT_MENU_CONTROL_NAMES_WAVE123
            .iter()
            .all(|n| n.starts_with("QuitMenu.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_quit_menu_nav_commands_residual_wave123() -> bool {
    QUIT_MENU_NAV_STEPS_WAVE123.len() == 11
        && residual_name_index(QUIT_MENU_NAV_STEPS_WAVE123, "ESC_OR_TOGGLE_QUIT_MENU") == Some(0)
        && residual_name_index(QUIT_MENU_NAV_STEPS_WAVE123, "GBM_SELECTED_BUTTON_EXIT") == Some(6)
        && residual_name_index(QUIT_MENU_NAV_STEPS_WAVE123, "HIDE_QUIT_MENU") == Some(10)
        && RUNTIME_HOST_QUIT_MENU_CMD_NAMES_WAVE123.len() == 10
        && residual_name_index(
            RUNTIME_HOST_QUIT_MENU_CMD_NAMES_WAVE123,
            "toggle_quit_menu_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_QUIT_MENU_CMD_NAMES_WAVE123,
            "click_quit_menu_ok_wnd_prepare_exit",
        ) == Some(8)
}

/// Wave 123 composite residual honesty pack.
pub fn honesty_quit_menu_residual_pack_wave123() -> bool {
    honesty_quit_menu_control_names_residual_wave123()
        && honesty_quit_menu_nav_commands_residual_wave123()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_quit_menu_control_names_residual_wave123());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_quit_menu_nav_commands_residual_wave123());
    }

    #[test]
    fn wave123_composite_pack() {
        assert!(honesty_quit_menu_residual_pack_wave123());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_quit_prepare_exit_residual_live() {
        use game_client::gui::callbacks::{
            ResidualQuitMenuAction, residual_quit_menu_is_visible, residual_quit_menu_last_action,
            simulate_quit_menu_prepare_exit, simulate_quit_menu_toggle_show,
        };
        assert!(simulate_quit_menu_toggle_show());
        assert!(residual_quit_menu_is_visible());
        assert_eq!(
            residual_quit_menu_last_action(),
            ResidualQuitMenuAction::ToggleShow
        );
        assert!(
            simulate_quit_menu_prepare_exit(),
            "show+exit+confirm residual must latch"
        );
        assert!(!residual_quit_menu_is_visible());
        assert_eq!(
            residual_quit_menu_last_action(),
            ResidualQuitMenuAction::ConfirmExit
        );
    }
}
