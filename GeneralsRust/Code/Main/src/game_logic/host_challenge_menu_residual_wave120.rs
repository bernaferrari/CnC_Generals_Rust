//! Wave 120 residual peels: ChallengeMenu WND residual
//! (general select + ButtonPlay/Back; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 118/119 MainMenu campaign/challenge entry,
//! Wave 106 shell layouts. Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ChallengeMenu.cpp / .wnd ButtonPlay, ButtonBack, GeneralPosition0..N
//! - ChallengeGenerals NUM_GENERALS = 12
//! - startChallengeGame after general selection
//!
//! Fail-closed:
//! - Not full bio portrait / preview audio residual
//! - Not full campaign map resolve / rank points residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Challenge menu residual tables
// ---------------------------------------------------------------------------

/// Retail ChallengeMenu layout filename residual.
pub const CHALLENGE_MENU_LAYOUT_FILENAME_WAVE120: &str = "Menus/ChallengeMenu.wnd";

/// Retail `NUM_GENERALS` residual.
pub const CHALLENGE_NUM_GENERALS_WAVE120: usize = 12;

/// Retail ChallengeMenu primary control names residual.
pub const CHALLENGE_MENU_CONTROL_NAMES_WAVE120: &[&str] = &[
    "ChallengeMenu.wnd:ParentChallengeMenu",
    "ChallengeMenu.wnd:ButtonPlay",
    "ChallengeMenu.wnd:ButtonBack",
    "ChallengeMenu.wnd:GadgetParent",
    "ChallengeMenu.wnd:GeneralsBioParent",
    "ChallengeMenu.wnd:BioPortrait",
    "ChallengeMenu.wnd:BioNameEntry",
    "ChallengeMenu.wnd:BioDOBEntry",
    "ChallengeMenu.wnd:BioBirthplaceEntry",
    "ChallengeMenu.wnd:BioStrategyEntry",
];

/// Ordered ChallengeMenu residual navigation steps.
pub const CHALLENGE_MENU_NAV_STEPS_WAVE120: &[&str] = &[
    "PUSH_CHALLENGE_MENU_LAYOUT",
    "GBM_SELECTED_GENERAL_POSITION",
    "LATCH_LAST_BUTTON_INDEX",
    "SHOW_BIO_AND_PLAY_BUTTON",
    "GBM_SELECTED_BUTTON_PLAY",
    "START_CHALLENGE_GAME",
    "GBM_SELECTED_BUTTON_BACK",
    "SHELL_POP",
];

/// Runtime-host command residual names for challenge peels.
pub const RUNTIME_HOST_CHALLENGE_CMD_NAMES_WAVE120: &[&str] = &[
    "open_challenge_menu_ok_wnd",
    "open_challenge_menu_ok",
    "click_challenge_start_ok_wnd",
    "click_challenge_start_miss",
    "click_campaign_start_ok_wnd",
];

/// Build residual GeneralPosition control name for index.
pub fn challenge_general_position_control_name_wave120(index: usize) -> Option<String> {
    if index >= CHALLENGE_NUM_GENERALS_WAVE120 {
        return None;
    }
    Some(format!("ChallengeMenu.wnd:GeneralPosition{index}"))
}

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: ChallengeMenu control names residual pack.
pub fn honesty_challenge_menu_control_names_residual_wave120() -> bool {
    CHALLENGE_MENU_LAYOUT_FILENAME_WAVE120 == "Menus/ChallengeMenu.wnd"
        && CHALLENGE_NUM_GENERALS_WAVE120 == 12
        && CHALLENGE_MENU_CONTROL_NAMES_WAVE120.len() == 10
        && residual_name_index(
            CHALLENGE_MENU_CONTROL_NAMES_WAVE120,
            "ChallengeMenu.wnd:ButtonPlay",
        ) == Some(1)
        && residual_name_index(
            CHALLENGE_MENU_CONTROL_NAMES_WAVE120,
            "ChallengeMenu.wnd:ButtonBack",
        ) == Some(2)
        && residual_name_index(
            CHALLENGE_MENU_CONTROL_NAMES_WAVE120,
            "ChallengeMenu.wnd:BioPortrait",
        ) == Some(5)
        && challenge_general_position_control_name_wave120(0)
            == Some("ChallengeMenu.wnd:GeneralPosition0".into())
        && challenge_general_position_control_name_wave120(11)
            == Some("ChallengeMenu.wnd:GeneralPosition11".into())
        && challenge_general_position_control_name_wave120(12).is_none()
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_challenge_menu_nav_commands_residual_wave120() -> bool {
    CHALLENGE_MENU_NAV_STEPS_WAVE120.len() == 8
        && residual_name_index(
            CHALLENGE_MENU_NAV_STEPS_WAVE120,
            "GBM_SELECTED_GENERAL_POSITION",
        ) == Some(1)
        && residual_name_index(CHALLENGE_MENU_NAV_STEPS_WAVE120, "GBM_SELECTED_BUTTON_PLAY")
            == Some(4)
        && residual_name_index(CHALLENGE_MENU_NAV_STEPS_WAVE120, "START_CHALLENGE_GAME") == Some(5)
        && RUNTIME_HOST_CHALLENGE_CMD_NAMES_WAVE120.len() == 5
        && residual_name_index(
            RUNTIME_HOST_CHALLENGE_CMD_NAMES_WAVE120,
            "open_challenge_menu_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_CHALLENGE_CMD_NAMES_WAVE120,
            "click_challenge_start_ok_wnd",
        ) == Some(2)
}

/// Wave 120 composite residual honesty pack.
pub fn honesty_challenge_menu_residual_pack_wave120() -> bool {
    honesty_challenge_menu_control_names_residual_wave120()
        && honesty_challenge_menu_nav_commands_residual_wave120()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_challenge_menu_control_names_residual_wave120());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_challenge_menu_nav_commands_residual_wave120());
    }

    #[test]
    fn wave120_composite_pack() {
        assert!(honesty_challenge_menu_residual_pack_wave120());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_challenge_prepare_start_residual_live() {
        assert!(
            game_client::gui::callbacks::simulate_challenge_menu_prepare_start(0),
            "select general 0 + play residual must latch"
        );
        assert_eq!(
            game_client::gui::callbacks::residual_challenge_selected_general(),
            Some(0)
        );
        assert!(game_client::gui::callbacks::residual_challenge_play_requested());
        assert!(game_client::gui::callbacks::simulate_challenge_menu_back_button_gadget_selected());
        assert!(game_client::gui::callbacks::residual_challenge_selected_general().is_none());
        assert!(!game_client::gui::callbacks::residual_challenge_play_requested());
    }
}
