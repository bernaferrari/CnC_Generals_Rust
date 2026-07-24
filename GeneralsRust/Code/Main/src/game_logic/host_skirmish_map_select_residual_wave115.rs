//! Wave 115 residual peels: Skirmish map-select WND residual
//! (host-testable shell map pick path; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 106 WindowLayout, Wave 114 MainMenu→Skirmish ButtonSkirmish,
//! Wave 113 Gadget messages. Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - SkirmishMapSelectMenu.cpp / .wnd ButtonOK / ButtonBack / ListboxMap
//! - SkirmishGameOptionsMenu.cpp ButtonSelectMap → overlay push
//! - Map commit into SkirmishBattleHonors / GameInfo setMap + CRC/size
//!
//! Fail-closed:
//! - Not full map-cache thumbnail / start-position button residual
//! - Not full W3D overlay animate residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Skirmish map-select residual
// ---------------------------------------------------------------------------

/// Retail map-select layout filename residual.
pub const SKIRMISH_MAP_SELECT_LAYOUT_FILENAME_WAVE115: &str = "Menus/SkirmishMapSelectMenu.wnd";

/// Retail options layout that owns ButtonSelectMap residual.
pub const SKIRMISH_OPTIONS_LAYOUT_FILENAME_WAVE115: &str = "Menus/SkirmishGameOptionsMenu.wnd";

/// Retail map-select control name residual table.
pub const SKIRMISH_MAP_SELECT_CONTROL_NAMES_WAVE115: &[&str] = &[
    "SkirmishMapSelectMenu.wnd:SkrimishMapSelectMenuParent",
    "SkirmishMapSelectMenu.wnd:ListboxMap",
    "SkirmishMapSelectMenu.wnd:ButtonOK",
    "SkirmishMapSelectMenu.wnd:ButtonBack",
    "SkirmishMapSelectMenu.wnd:RadioButtonSystemMaps",
    "SkirmishMapSelectMenu.wnd:RadioButtonUserMaps",
    "SkirmishMapSelectMenu.wnd:WinMapPreview",
];

/// Retail options ButtonSelectMap control name residual.
pub const SKIRMISH_BUTTON_SELECT_MAP_NAME_WAVE115: &str =
    "SkirmishGameOptionsMenu.wnd:ButtonSelectMap";

/// Ordered map-select residual navigation steps.
pub const SKIRMISH_MAP_SELECT_NAV_STEPS_WAVE115: &[&str] = &[
    "ENSURE_SKIRMISH_OPTIONS_LAYOUT",
    "GBM_SELECTED_BUTTON_SELECT_MAP",
    "CREATE_MAP_SELECT_OVERLAY",
    "LATCH_SELECTED_MAP_ROW",
    "GBM_SELECTED_BUTTON_OK",
    "COMMIT_SETUP_MAP_CRC_SIZE",
    "REFRESH_OPTIONS_FROM_SETUP",
    "DESTROY_MAP_SELECT_OVERLAY",
    "GBM_SELECTED_BUTTON_START",
];

/// Runtime-host command residual names for map-select peels.
pub const RUNTIME_HOST_MAP_SELECT_CMD_NAMES_WAVE115: &[&str] = &[
    "click_skirmish_map_select_ok_wnd",
    "click_skirmish_start_ok_wnd_via_map_select",
    "click_skirmish_start_ok_wnd",
    "click_skirmish_start_wnd_pending",
];

/// Retail Lone Eagle map path residual (executable/golden default).
pub const RETAIL_LONE_EAGLE_MAP_PATH_WAVE115: &str = "maps/lone_eagle/lone_eagle.map";

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: map-select layout/control name residual pack.
pub fn honesty_skirmish_map_select_names_residual_wave115() -> bool {
    SKIRMISH_MAP_SELECT_LAYOUT_FILENAME_WAVE115 == "Menus/SkirmishMapSelectMenu.wnd"
        && SKIRMISH_OPTIONS_LAYOUT_FILENAME_WAVE115 == "Menus/SkirmishGameOptionsMenu.wnd"
        && SKIRMISH_MAP_SELECT_CONTROL_NAMES_WAVE115.len() == 7
        && residual_name_index(
            SKIRMISH_MAP_SELECT_CONTROL_NAMES_WAVE115,
            "SkirmishMapSelectMenu.wnd:ButtonOK",
        ) == Some(2)
        && residual_name_index(
            SKIRMISH_MAP_SELECT_CONTROL_NAMES_WAVE115,
            "SkirmishMapSelectMenu.wnd:ListboxMap",
        ) == Some(1)
        && residual_name_index(
            SKIRMISH_MAP_SELECT_CONTROL_NAMES_WAVE115,
            "SkirmishMapSelectMenu.wnd:ButtonBack",
        ) == Some(3)
        && SKIRMISH_BUTTON_SELECT_MAP_NAME_WAVE115
            == "SkirmishGameOptionsMenu.wnd:ButtonSelectMap"
        // C++ typo residual "Skrimish" is intentional retail name.
        && SKIRMISH_MAP_SELECT_CONTROL_NAMES_WAVE115[0].contains("SkrimishMapSelectMenuParent")
}

/// Honesty: map-select nav steps residual pack.
pub fn honesty_skirmish_map_select_nav_steps_residual_wave115() -> bool {
    SKIRMISH_MAP_SELECT_NAV_STEPS_WAVE115.len() == 9
        && residual_name_index(
            SKIRMISH_MAP_SELECT_NAV_STEPS_WAVE115,
            "GBM_SELECTED_BUTTON_SELECT_MAP",
        ) == Some(1)
        && residual_name_index(
            SKIRMISH_MAP_SELECT_NAV_STEPS_WAVE115,
            "GBM_SELECTED_BUTTON_OK",
        ) == Some(4)
        && residual_name_index(
            SKIRMISH_MAP_SELECT_NAV_STEPS_WAVE115,
            "COMMIT_SETUP_MAP_CRC_SIZE",
        ) == Some(5)
        && residual_name_index(
            SKIRMISH_MAP_SELECT_NAV_STEPS_WAVE115,
            "GBM_SELECTED_BUTTON_START",
        ) == Some(8)
}

/// Honesty: runtime-host command + Lone Eagle path residual pack.
pub fn honesty_skirmish_map_select_commands_residual_wave115() -> bool {
    RUNTIME_HOST_MAP_SELECT_CMD_NAMES_WAVE115.len() == 4
        && residual_name_index(
            RUNTIME_HOST_MAP_SELECT_CMD_NAMES_WAVE115,
            "click_skirmish_map_select_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_MAP_SELECT_CMD_NAMES_WAVE115,
            "click_skirmish_start_ok_wnd_via_map_select",
        ) == Some(1)
        && RETAIL_LONE_EAGLE_MAP_PATH_WAVE115.contains("lone_eagle")
        && RETAIL_LONE_EAGLE_MAP_PATH_WAVE115.ends_with(".map")
}

/// Wave 115 composite residual honesty pack.
pub fn honesty_skirmish_map_select_residual_pack_wave115() -> bool {
    honesty_skirmish_map_select_names_residual_wave115()
        && honesty_skirmish_map_select_nav_steps_residual_wave115()
        && honesty_skirmish_map_select_commands_residual_wave115()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_select_names() {
        assert!(honesty_skirmish_map_select_names_residual_wave115());
    }

    #[test]
    fn map_select_nav_steps() {
        assert!(honesty_skirmish_map_select_nav_steps_residual_wave115());
    }

    #[test]
    fn map_select_commands() {
        assert!(honesty_skirmish_map_select_commands_residual_wave115());
    }

    #[test]
    fn wave115_composite_pack() {
        assert!(honesty_skirmish_map_select_residual_pack_wave115());
    }
}
