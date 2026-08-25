//! Wave 132 residual peels: MapSelectMenu WND residual
//! (ListboxMap + OK/Back/difficulty/filter; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 131 SinglePlayer New, Wave 115 Skirmish map select.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - MapSelectMenu.cpp / MapSelectMenu.wnd
//! - ButtonOK, ButtonBack, ListboxMap, RadioButtonEasy/Medium/HardAI
//! - ButtonSinglePlayer/Multiplayer, RadioButtonSystemMaps/UserMaps
//!
//! Fail-closed:
//! - Not full map list populate / start_game transfer residual
//! - Not full preference write residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Map select residual tables
// ---------------------------------------------------------------------------

/// Retail MapSelectMenu layout filename residual.
pub const MAP_SELECT_MENU_LAYOUT_FILENAME_WAVE132: &str = "Menus/MapSelectMenu.wnd";

/// Retail MapSelectMenu control names residual.
pub const MAP_SELECT_MENU_CONTROL_NAMES_WAVE132: &[&str] = &[
    "MapSelectMenu.wnd:MapSelectMenuParent",
    "MapSelectMenu.wnd:ListboxMap",
    "MapSelectMenu.wnd:ButtonOK",
    "MapSelectMenu.wnd:ButtonBack",
    "MapSelectMenu.wnd:ButtonSinglePlayer",
    "MapSelectMenu.wnd:ButtonMultiplayer",
    "MapSelectMenu.wnd:RadioButtonEasyAI",
    "MapSelectMenu.wnd:RadioButtonMediumAI",
    "MapSelectMenu.wnd:RadioButtonHardAI",
    "MapSelectMenu.wnd:RadioButtonSystemMaps",
    "MapSelectMenu.wnd:RadioButtonUserMaps",
];

/// Ordered MapSelectMenu residual navigation steps.
pub const MAP_SELECT_MENU_NAV_STEPS_WAVE132: &[&str] = &[
    "PUSH_MAP_SELECT_MENU_LAYOUT",
    "POPULATE_LISTBOX_MAP",
    "GBM_SELECTED_RADIO_SYSTEM_OR_USER",
    "GBM_SELECTED_BUTTON_SOLO_OR_MP",
    "GBM_SELECTED_DIFFICULTY_RADIO",
    "SELECT_LISTBOX_MAP_ROW",
    "GBM_SELECTED_BUTTON_OK",
    "START_GAME_TRANSFER",
    "GBM_SELECTED_BUTTON_BACK",
    "SHELL_POP",
];

/// Runtime-host command residual names for MapSelectMenu peels.
pub const RUNTIME_HOST_MAP_SELECT_MENU_CMD_NAMES_WAVE132: &[&str] = &[
    "open_map_select_menu_ok_wnd",
    "open_map_select_menu_ok",
    "click_map_select_menu_ok_wnd_ok",
    "click_map_select_menu_ok_wnd_select",
    "click_map_select_menu_ok_wnd_difficulty",
    "click_map_select_menu_ok_wnd_back",
    "click_map_select_menu_ok_wnd_prepare_ok",
    "click_map_select_menu_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: MapSelectMenu control names residual pack.
pub fn honesty_map_select_menu_control_names_residual_wave132() -> bool {
    MAP_SELECT_MENU_LAYOUT_FILENAME_WAVE132 == "Menus/MapSelectMenu.wnd"
        && MAP_SELECT_MENU_CONTROL_NAMES_WAVE132.len() == 11
        && residual_name_index(
            MAP_SELECT_MENU_CONTROL_NAMES_WAVE132,
            "MapSelectMenu.wnd:ButtonOK",
        ) == Some(2)
        && residual_name_index(
            MAP_SELECT_MENU_CONTROL_NAMES_WAVE132,
            "MapSelectMenu.wnd:ListboxMap",
        ) == Some(1)
        && residual_name_index(
            MAP_SELECT_MENU_CONTROL_NAMES_WAVE132,
            "MapSelectMenu.wnd:RadioButtonHardAI",
        ) == Some(8)
        && MAP_SELECT_MENU_CONTROL_NAMES_WAVE132
            .iter()
            .all(|n| n.starts_with("MapSelectMenu.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_map_select_menu_nav_commands_residual_wave132() -> bool {
    MAP_SELECT_MENU_NAV_STEPS_WAVE132.len() == 10
        && residual_name_index(MAP_SELECT_MENU_NAV_STEPS_WAVE132, "SELECT_LISTBOX_MAP_ROW")
            == Some(5)
        && residual_name_index(MAP_SELECT_MENU_NAV_STEPS_WAVE132, "GBM_SELECTED_BUTTON_OK")
            == Some(6)
        && residual_name_index(MAP_SELECT_MENU_NAV_STEPS_WAVE132, "SHELL_POP") == Some(9)
        && RUNTIME_HOST_MAP_SELECT_MENU_CMD_NAMES_WAVE132.len() == 8
        && residual_name_index(
            RUNTIME_HOST_MAP_SELECT_MENU_CMD_NAMES_WAVE132,
            "open_map_select_menu_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_MAP_SELECT_MENU_CMD_NAMES_WAVE132,
            "click_map_select_menu_ok_wnd_prepare_ok",
        ) == Some(6)
}

/// Wave 132 composite residual honesty pack.
pub fn honesty_map_select_menu_residual_pack_wave132() -> bool {
    honesty_map_select_menu_control_names_residual_wave132()
        && honesty_map_select_menu_nav_commands_residual_wave132()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_map_select_menu_control_names_residual_wave132());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_map_select_menu_nav_commands_residual_wave132());
    }

    #[test]
    fn wave132_composite_pack() {
        assert!(honesty_map_select_menu_residual_pack_wave132());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_map_select_prepare_ok_residual_live() {
        use game_client::gui::callbacks::{
            ResidualMapSelectMenuAction, residual_map_select_menu_difficulty,
            residual_map_select_menu_last_action, residual_map_select_menu_selected_map,
            simulate_map_select_menu_back_button_gadget_selected,
            simulate_map_select_menu_clear_button_pushed, simulate_map_select_menu_prepare_ok,
        };
        assert!(
            simulate_map_select_menu_prepare_ok("Maps/Test/Test.map"),
            "map+difficulty+ok residual must latch"
        );
        assert_eq!(
            residual_map_select_menu_selected_map().as_deref(),
            Some("Maps/Test/Test.map")
        );
        assert_eq!(residual_map_select_menu_difficulty(), 1);
        assert_eq!(
            residual_map_select_menu_last_action(),
            ResidualMapSelectMenuAction::Ok
        );
        assert!(simulate_map_select_menu_clear_button_pushed());
        assert!(simulate_map_select_menu_back_button_gadget_selected());
        assert_eq!(
            residual_map_select_menu_last_action(),
            ResidualMapSelectMenuAction::Back
        );
    }
}
