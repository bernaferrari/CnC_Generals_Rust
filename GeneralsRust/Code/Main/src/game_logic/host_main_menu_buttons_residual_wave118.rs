//! Wave 118 residual peels: MainMenu button WND navigation residual
//! (Options/Credits/Load/Single/Multi/Challenge/Replay; never flips `playable_claim`).
//!
//! Orthogonal to Wave 114 ButtonSkirmish, Wave 106 MainMenu name tables.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - MainMenu.cpp GBM_SELECTED handlers for Options/Credits/Load/Single/
//!   Multiplayer/Challenge/Replay
//! - PushShellScreen targets + dropdown residual
//!
//! Fail-closed:
//! - Not full W3D TransitionHandler / animate-window residual
//! - Not full Options layout ShowOptionsLayout device residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// MainMenu button residual tables
// ---------------------------------------------------------------------------

/// Retail MainMenu layout filename residual.
pub const MAIN_MENU_LAYOUT_FILENAME_WAVE118: &str = "Menus/MainMenu.wnd";

/// Retail MainMenu button control names residual (Wave 118 peels).
pub const MAIN_MENU_BUTTON_CONTROL_NAMES_WAVE118: &[&str] = &[
    "MainMenu.wnd:ButtonOptions",
    "MainMenu.wnd:ButtonCredits",
    "MainMenu.wnd:ButtonLoadGame",
    "MainMenu.wnd:ButtonSinglePlayer",
    "MainMenu.wnd:ButtonMultiplayer",
    "MainMenu.wnd:ButtonChallenge",
    "MainMenu.wnd:ButtonReplay",
    "MainMenu.wnd:ButtonSkirmish",
];

/// Retail shell push targets from MainMenu buttons residual.
pub const MAIN_MENU_PUSH_LAYOUT_TARGETS_WAVE118: &[&str] = &[
    "Menus/CreditsMenu.wnd",
    "Menus/SaveLoad.wnd",
    "Menus/ReplayMenu.wnd",
    "Menus/SkirmishGameOptionsMenu.wnd",
    "Menus/LanLobbyMenu.wnd",
    "Menus/ChallengeMenu.wnd",
];

/// Ordered MainMenu button residual navigation steps.
pub const MAIN_MENU_BUTTON_NAV_STEPS_WAVE118: &[&str] = &[
    "ENSURE_MAIN_MENU_LAYOUT",
    "GBM_SELECTED_BUTTON_OPTIONS",
    "SHOW_OPTIONS_LAYOUT",
    "GBM_SELECTED_BUTTON_SINGLE_PLAYER",
    "OPEN_SINGLE_DROPDOWN",
    "GBM_SELECTED_BUTTON_MULTI_PLAYER",
    "OPEN_MULTI_DROPDOWN",
    "GBM_SELECTED_BUTTON_CREDITS",
    "PUSH_CREDITS_MENU",
    "GBM_SELECTED_BUTTON_LOAD_GAME",
    "PUSH_SAVE_LOAD",
    "GBM_SELECTED_BUTTON_REPLAY",
    "PUSH_REPLAY_MENU",
    "GBM_SELECTED_BUTTON_CHALLENGE",
    "LAUNCH_CHALLENGE_DIFFICULTY",
];

/// Runtime-host command residual names for MainMenu button peels.
pub const RUNTIME_HOST_MAIN_MENU_BUTTON_CMD_NAMES_WAVE118: &[&str] = &[
    "open_options_ok_wnd",
    "options_ok",
    "open_credits_ok_wnd",
    "open_credits_ok",
    "open_single_player_menu_ok_wnd",
    "open_single_player_menu_ok",
    "open_multiplayer_menu_ok_wnd",
    "open_multiplayer_menu_ok",
    "open_load_game_ok_wnd",
    "open_load_game_ok",
    "open_load_replay_menu_ok_wnd",
    "open_load_replay_menu_ok",
    "open_challenge_menu_ok_wnd",
    "open_challenge_menu_ok",
    "open_skirmish_menu_ok_wnd",
    "open_skirmish_menu_ok",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: MainMenu button control names residual pack.
pub fn honesty_main_menu_button_names_residual_wave118() -> bool {
    MAIN_MENU_LAYOUT_FILENAME_WAVE118 == "Menus/MainMenu.wnd"
        && MAIN_MENU_BUTTON_CONTROL_NAMES_WAVE118.len() == 8
        && residual_name_index(
            MAIN_MENU_BUTTON_CONTROL_NAMES_WAVE118,
            "MainMenu.wnd:ButtonOptions",
        ) == Some(0)
        && residual_name_index(
            MAIN_MENU_BUTTON_CONTROL_NAMES_WAVE118,
            "MainMenu.wnd:ButtonLoadGame",
        ) == Some(2)
        && residual_name_index(
            MAIN_MENU_BUTTON_CONTROL_NAMES_WAVE118,
            "MainMenu.wnd:ButtonChallenge",
        ) == Some(5)
        && residual_name_index(
            MAIN_MENU_BUTTON_CONTROL_NAMES_WAVE118,
            "MainMenu.wnd:ButtonSkirmish",
        ) == Some(7)
        && MAIN_MENU_BUTTON_CONTROL_NAMES_WAVE118
            .iter()
            .all(|n| n.starts_with("MainMenu.wnd:Button"))
}

/// Honesty: push layout targets residual pack.
pub fn honesty_main_menu_push_targets_residual_wave118() -> bool {
    MAIN_MENU_PUSH_LAYOUT_TARGETS_WAVE118.len() == 6
        && residual_name_index(
            MAIN_MENU_PUSH_LAYOUT_TARGETS_WAVE118,
            "Menus/CreditsMenu.wnd",
        ) == Some(0)
        && residual_name_index(MAIN_MENU_PUSH_LAYOUT_TARGETS_WAVE118, "Menus/SaveLoad.wnd")
            == Some(1)
        && residual_name_index(
            MAIN_MENU_PUSH_LAYOUT_TARGETS_WAVE118,
            "Menus/ChallengeMenu.wnd",
        ) == Some(5)
        && MAIN_MENU_PUSH_LAYOUT_TARGETS_WAVE118
            .iter()
            .all(|n| n.starts_with("Menus/") && n.ends_with(".wnd"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_main_menu_button_nav_commands_residual_wave118() -> bool {
    MAIN_MENU_BUTTON_NAV_STEPS_WAVE118.len() == 15
        && residual_name_index(
            MAIN_MENU_BUTTON_NAV_STEPS_WAVE118,
            "GBM_SELECTED_BUTTON_OPTIONS",
        ) == Some(1)
        && residual_name_index(MAIN_MENU_BUTTON_NAV_STEPS_WAVE118, "SHOW_OPTIONS_LAYOUT") == Some(2)
        && residual_name_index(
            MAIN_MENU_BUTTON_NAV_STEPS_WAVE118,
            "LAUNCH_CHALLENGE_DIFFICULTY",
        ) == Some(14)
        && RUNTIME_HOST_MAIN_MENU_BUTTON_CMD_NAMES_WAVE118.len() == 16
        && residual_name_index(
            RUNTIME_HOST_MAIN_MENU_BUTTON_CMD_NAMES_WAVE118,
            "open_options_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_MAIN_MENU_BUTTON_CMD_NAMES_WAVE118,
            "open_challenge_menu_ok_wnd",
        ) == Some(12)
        && residual_name_index(
            RUNTIME_HOST_MAIN_MENU_BUTTON_CMD_NAMES_WAVE118,
            "open_skirmish_menu_ok_wnd",
        ) == Some(14)
}

/// Wave 118 composite residual honesty pack.
pub fn honesty_main_menu_buttons_residual_pack_wave118() -> bool {
    honesty_main_menu_button_names_residual_wave118()
        && honesty_main_menu_push_targets_residual_wave118()
        && honesty_main_menu_button_nav_commands_residual_wave118()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_names_residual() {
        assert!(honesty_main_menu_button_names_residual_wave118());
    }

    #[test]
    fn push_targets_residual() {
        assert!(honesty_main_menu_push_targets_residual_wave118());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_main_menu_button_nav_commands_residual_wave118());
    }

    #[test]
    fn wave118_composite_pack() {
        assert!(honesty_main_menu_buttons_residual_pack_wave118());
    }
}
