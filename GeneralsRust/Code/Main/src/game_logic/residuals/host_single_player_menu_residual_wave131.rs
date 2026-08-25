//! Wave 131 residual peels: SinglePlayerMenu WND residual
//! (New/Load/Back; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 118 ButtonSinglePlayer, Wave 121 SaveLoad.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - SinglePlayerMenu.cpp / SinglePlayerMenu.wnd
//! - ButtonNew, ButtonLoad, ButtonBack
//!
//! Fail-closed:
//! - Not full MapSelectMenu push residual
//! - Not full LoadGame menu residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Single player menu residual tables
// ---------------------------------------------------------------------------

/// Retail SinglePlayerMenu layout filename residual.
pub const SINGLE_PLAYER_MENU_LAYOUT_FILENAME_WAVE131: &str = "Menus/SinglePlayerMenu.wnd";

/// Retail SinglePlayerMenu control names residual.
pub const SINGLE_PLAYER_MENU_CONTROL_NAMES_WAVE131: &[&str] = &[
    "SinglePlayerMenu.wnd:SinglePlayerMenuParent",
    "SinglePlayerMenu.wnd:ButtonNew",
    "SinglePlayerMenu.wnd:ButtonLoad",
    "SinglePlayerMenu.wnd:ButtonBack",
];

/// Ordered SinglePlayerMenu residual navigation steps.
pub const SINGLE_PLAYER_MENU_NAV_STEPS_WAVE131: &[&str] = &[
    "PUSH_SINGLE_PLAYER_MENU_LAYOUT",
    "GBM_SELECTED_BUTTON_NEW",
    "PUSH_MAP_SELECT_MENU",
    "GBM_SELECTED_BUTTON_LOAD",
    "OPEN_SAVE_LOAD_MENU",
    "GBM_SELECTED_BUTTON_BACK",
    "SHELL_POP",
];

/// Runtime-host command residual names for SinglePlayerMenu peels.
pub const RUNTIME_HOST_SINGLE_PLAYER_MENU_CMD_NAMES_WAVE131: &[&str] = &[
    "open_single_player_menu_ok_wnd",
    "open_single_player_menu_ok",
    "click_single_player_menu_ok_wnd_new",
    "click_single_player_menu_ok_wnd_load",
    "click_single_player_menu_ok_wnd_back",
    "click_single_player_menu_ok_wnd_prepare_new",
    "click_single_player_menu_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: SinglePlayerMenu control names residual pack.
pub fn honesty_single_player_menu_control_names_residual_wave131() -> bool {
    SINGLE_PLAYER_MENU_LAYOUT_FILENAME_WAVE131 == "Menus/SinglePlayerMenu.wnd"
        && SINGLE_PLAYER_MENU_CONTROL_NAMES_WAVE131.len() == 4
        && residual_name_index(
            SINGLE_PLAYER_MENU_CONTROL_NAMES_WAVE131,
            "SinglePlayerMenu.wnd:ButtonNew",
        ) == Some(1)
        && residual_name_index(
            SINGLE_PLAYER_MENU_CONTROL_NAMES_WAVE131,
            "SinglePlayerMenu.wnd:ButtonLoad",
        ) == Some(2)
        && residual_name_index(
            SINGLE_PLAYER_MENU_CONTROL_NAMES_WAVE131,
            "SinglePlayerMenu.wnd:ButtonBack",
        ) == Some(3)
        && SINGLE_PLAYER_MENU_CONTROL_NAMES_WAVE131
            .iter()
            .all(|n| n.starts_with("SinglePlayerMenu.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_single_player_menu_nav_commands_residual_wave131() -> bool {
    SINGLE_PLAYER_MENU_NAV_STEPS_WAVE131.len() == 7
        && residual_name_index(
            SINGLE_PLAYER_MENU_NAV_STEPS_WAVE131,
            "GBM_SELECTED_BUTTON_NEW",
        ) == Some(1)
        && residual_name_index(SINGLE_PLAYER_MENU_NAV_STEPS_WAVE131, "PUSH_MAP_SELECT_MENU")
            == Some(2)
        && residual_name_index(SINGLE_PLAYER_MENU_NAV_STEPS_WAVE131, "SHELL_POP") == Some(6)
        && RUNTIME_HOST_SINGLE_PLAYER_MENU_CMD_NAMES_WAVE131.len() == 7
        && residual_name_index(
            RUNTIME_HOST_SINGLE_PLAYER_MENU_CMD_NAMES_WAVE131,
            "open_single_player_menu_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_SINGLE_PLAYER_MENU_CMD_NAMES_WAVE131,
            "click_single_player_menu_ok_wnd_prepare_new",
        ) == Some(5)
}

/// Wave 131 composite residual honesty pack.
pub fn honesty_single_player_menu_residual_pack_wave131() -> bool {
    honesty_single_player_menu_control_names_residual_wave131()
        && honesty_single_player_menu_nav_commands_residual_wave131()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_single_player_menu_control_names_residual_wave131());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_single_player_menu_nav_commands_residual_wave131());
    }

    #[test]
    fn wave131_composite_pack() {
        assert!(honesty_single_player_menu_residual_pack_wave131());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_single_player_prepare_new_residual_live() {
        use game_client::gui::callbacks::{
            ResidualSinglePlayerMenuAction, residual_single_player_menu_button_pushed,
            residual_single_player_menu_last_action,
            simulate_single_player_menu_back_button_gadget_selected,
            simulate_single_player_menu_clear_button_pushed,
            simulate_single_player_menu_prepare_new,
        };
        assert!(
            simulate_single_player_menu_prepare_new(),
            "bind+new residual must latch"
        );
        assert!(residual_single_player_menu_button_pushed());
        assert_eq!(
            residual_single_player_menu_last_action(),
            ResidualSinglePlayerMenuAction::New
        );
        // Second New while button_pushed should fail closed.
        assert!(
            !game_client::gui::callbacks::simulate_single_player_menu_new_button_gadget_selected()
        );
        assert!(simulate_single_player_menu_clear_button_pushed());
        assert!(simulate_single_player_menu_back_button_gadget_selected());
        assert_eq!(
            residual_single_player_menu_last_action(),
            ResidualSinglePlayerMenuAction::Back
        );
    }
}
