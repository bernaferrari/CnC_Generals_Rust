//! Wave 117 residual peels: Skirmish rules residual
//! (starting cash / superweapons / game speed; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 115 map-select, Wave 116 slot/AI config,
//! Wave 114 MainMenu→Skirmish. Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - SkirmishGameOptionsMenu.cpp ComboBoxStartingCash / CheckboxLimitSuperweapons /
//!   SliderGameSpeed / StaticTextGameSpeed
//! - GUIUtil PopulateStartingCashComboBox multiplayer money list
//! - Money::default 10000 starting cash
//!
//! Fail-closed:
//! - Not full SkirmishPreferences disk write residual
//! - Not full live combo/slider gadget residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Rules residual tables
// ---------------------------------------------------------------------------

/// Retail starting-cash combo amounts residual.
pub const SKIRMISH_STARTING_CASH_AMOUNTS_WAVE117: &[u32] = &[10000, 20000, 30000, 40000, 50000];

/// Retail default starting cash residual.
pub const SKIRMISH_DEFAULT_STARTING_CASH_WAVE117: u32 = 10000;

/// Retail game-speed slider residual.
pub const SKIRMISH_GAME_SPEED_SLIDER_MAX_WAVE117: i32 = 60;
pub const SKIRMISH_GAME_SPEED_NO_LIMIT_WAVE117: i32 = 61;
pub const SKIRMISH_DEFAULT_GAME_SPEED_FPS_WAVE117: i32 = 30;

/// Retail rules control names residual.
pub const SKIRMISH_RULES_CONTROL_NAMES_WAVE117: &[&str] = &[
    "SkirmishGameOptionsMenu.wnd:ComboBoxStartingCash",
    "SkirmishGameOptionsMenu.wnd:CheckboxLimitSuperweapons",
    "SkirmishGameOptionsMenu.wnd:SliderGameSpeed",
    "SkirmishGameOptionsMenu.wnd:StaticTextGameSpeed",
];

/// Ordered rules residual navigation steps.
pub const SKIRMISH_RULES_NAV_STEPS_WAVE117: &[&str] = &[
    "POPULATE_STARTING_CASH_COMBO",
    "SELECT_STARTING_CASH",
    "SET_LIMIT_SUPERWEAPONS",
    "SET_GAME_SPEED_SLIDER",
    "UPDATE_STATIC_TEXT_GAME_SPEED",
    "PREPARE_MATCH_OPTIONS",
    "GBM_SELECTED_BUTTON_START",
];

/// Runtime-host command residual names including rules peels.
pub const RUNTIME_HOST_RULES_CMD_NAMES_WAVE117: &[&str] = &[
    "click_skirmish_start_ok_wnd_via_slots_rules",
    "click_skirmish_start_ok_wnd_via_map_select_slots_rules",
    "click_skirmish_start_ok_wnd_via_map_select_slots",
    "click_skirmish_start_ok_wnd_via_slots",
    "click_skirmish_start_ok_wnd",
];

/// Clamp residual FPS like SliderGameSpeed (1..=60, or 61 = no limit).
pub fn skirmish_clamp_game_speed_fps_wave117(fps: i32) -> Option<i32> {
    if fps < 1 || fps > SKIRMISH_GAME_SPEED_NO_LIMIT_WAVE117 {
        return None;
    }
    if fps > SKIRMISH_GAME_SPEED_SLIDER_MAX_WAVE117 {
        Some(SKIRMISH_GAME_SPEED_NO_LIMIT_WAVE117)
    } else {
        Some(fps)
    }
}

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: starting cash residual pack.
pub fn honesty_skirmish_starting_cash_residual_wave117() -> bool {
    SKIRMISH_STARTING_CASH_AMOUNTS_WAVE117.len() == 5
        && SKIRMISH_STARTING_CASH_AMOUNTS_WAVE117[0] == 10000
        && SKIRMISH_STARTING_CASH_AMOUNTS_WAVE117[4] == 50000
        && SKIRMISH_DEFAULT_STARTING_CASH_WAVE117 == 10000
        && SKIRMISH_STARTING_CASH_AMOUNTS_WAVE117.contains(&SKIRMISH_DEFAULT_STARTING_CASH_WAVE117)
        && SKIRMISH_STARTING_CASH_AMOUNTS_WAVE117
            .windows(2)
            .all(|w| w[0] < w[1])
}

/// Honesty: game speed + control names residual pack.
pub fn honesty_skirmish_game_speed_controls_residual_wave117() -> bool {
    SKIRMISH_GAME_SPEED_SLIDER_MAX_WAVE117 == 60
        && SKIRMISH_GAME_SPEED_NO_LIMIT_WAVE117 == 61
        && SKIRMISH_DEFAULT_GAME_SPEED_FPS_WAVE117 == 30
        && skirmish_clamp_game_speed_fps_wave117(30) == Some(30)
        && skirmish_clamp_game_speed_fps_wave117(60) == Some(60)
        && skirmish_clamp_game_speed_fps_wave117(61) == Some(61)
        && skirmish_clamp_game_speed_fps_wave117(0).is_none()
        && skirmish_clamp_game_speed_fps_wave117(62).is_none()
        && SKIRMISH_RULES_CONTROL_NAMES_WAVE117.len() == 4
        && residual_name_index(
            SKIRMISH_RULES_CONTROL_NAMES_WAVE117,
            "SkirmishGameOptionsMenu.wnd:ComboBoxStartingCash",
        ) == Some(0)
        && residual_name_index(
            SKIRMISH_RULES_CONTROL_NAMES_WAVE117,
            "SkirmishGameOptionsMenu.wnd:CheckboxLimitSuperweapons",
        ) == Some(1)
        && residual_name_index(
            SKIRMISH_RULES_CONTROL_NAMES_WAVE117,
            "SkirmishGameOptionsMenu.wnd:SliderGameSpeed",
        ) == Some(2)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_skirmish_rules_nav_commands_residual_wave117() -> bool {
    SKIRMISH_RULES_NAV_STEPS_WAVE117.len() == 7
        && residual_name_index(SKIRMISH_RULES_NAV_STEPS_WAVE117, "SELECT_STARTING_CASH") == Some(1)
        && residual_name_index(SKIRMISH_RULES_NAV_STEPS_WAVE117, "SET_LIMIT_SUPERWEAPONS")
            == Some(2)
        && residual_name_index(SKIRMISH_RULES_NAV_STEPS_WAVE117, "PREPARE_MATCH_OPTIONS") == Some(5)
        && RUNTIME_HOST_RULES_CMD_NAMES_WAVE117.len() == 5
        && residual_name_index(
            RUNTIME_HOST_RULES_CMD_NAMES_WAVE117,
            "click_skirmish_start_ok_wnd_via_slots_rules",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_RULES_CMD_NAMES_WAVE117,
            "click_skirmish_start_ok_wnd_via_map_select_slots_rules",
        ) == Some(1)
}

/// Wave 117 composite residual honesty pack.
pub fn honesty_skirmish_rules_residual_pack_wave117() -> bool {
    honesty_skirmish_starting_cash_residual_wave117()
        && honesty_skirmish_game_speed_controls_residual_wave117()
        && honesty_skirmish_rules_nav_commands_residual_wave117()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_cash_residual() {
        assert!(honesty_skirmish_starting_cash_residual_wave117());
    }

    #[test]
    fn game_speed_controls_residual() {
        assert!(honesty_skirmish_game_speed_controls_residual_wave117());
    }

    #[test]
    fn rules_nav_commands_residual() {
        assert!(honesty_skirmish_rules_nav_commands_residual_wave117());
    }

    #[test]
    fn wave117_composite_pack() {
        assert!(honesty_skirmish_rules_residual_pack_wave117());
    }

    // Live GameClient rules simulate is covered by runtime-host click_skirmish_start
    // (prepare_match_options). Keep this pack pure/host-testable only.
}
