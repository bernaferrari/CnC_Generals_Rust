//! Wave 119 residual peels: MainMenu campaign side + difficulty residual
//! (USA/GLA/China + Easy/Normal/Hard; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 118 MainMenu shell buttons, Wave 114 Skirmish,
//! Wave 106 MainMenu faction windows. Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - MainMenu.cpp ButtonUSA/GLA/China → difficulty dropdown
//! - ButtonEasy/Medium/Hard → checkCDBeforeCampaign → prepareCampaignGame
//! - GameDifficulty Easy=0 / Normal=1 / Hard=2
//!
//! Fail-closed:
//! - Not full CD prompt / Options.ini write residual
//! - Not full campaign map resolve / FadeWholeScreen residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Campaign residual tables
// ---------------------------------------------------------------------------

/// Retail campaign side button control names residual.
pub const MAIN_MENU_CAMPAIGN_SIDE_BUTTON_NAMES_WAVE119: &[&str] = &[
    "MainMenu.wnd:ButtonUSA",
    "MainMenu.wnd:ButtonGLA",
    "MainMenu.wnd:ButtonChina",
    "MainMenu.wnd:ButtonChallenge",
];

/// Retail difficulty button control names residual.
pub const MAIN_MENU_DIFFICULTY_BUTTON_NAMES_WAVE119: &[&str] = &[
    "MainMenu.wnd:ButtonEasy",
    "MainMenu.wnd:ButtonMedium",
    "MainMenu.wnd:ButtonHard",
    "MainMenu.wnd:ButtonDiffBack",
];

/// Retail `GameDifficulty` residual names (enum order).
pub const GAME_DIFFICULTY_NAMES_WAVE119: &[&str] = &["Easy", "Normal", "Hard"];

/// Retail `ShowSide` residual names used by campaign peels.
pub const CAMPAIGN_SHOW_SIDE_NAMES_WAVE119: &[&str] = &["USA", "GLA", "China", "Training"];

/// Ordered campaign residual navigation steps.
pub const MAIN_MENU_CAMPAIGN_NAV_STEPS_WAVE119: &[&str] = &[
    "OPEN_SINGLE_PLAYER_DROPDOWN",
    "GBM_SELECTED_BUTTON_CAMPAIGN_SIDE",
    "OPEN_DIFFICULTY_DROPDOWN",
    "LATCH_CAMPAIGN_SELECTED",
    "SET_SHOW_SIDE",
    "GBM_SELECTED_BUTTON_DIFFICULTY",
    "CHECK_CD_BEFORE_CAMPAIGN",
    "PREPARE_CAMPAIGN_GAME",
    "SETUP_GAME_START_OR_CHALLENGE",
    "LATCH_START_GAME",
];

/// Runtime-host command residual names for campaign peels.
pub const RUNTIME_HOST_CAMPAIGN_CMD_NAMES_WAVE119: &[&str] = &[
    "open_difficulty_menu_ok_wnd",
    "open_difficulty_menu_ok",
    "click_campaign_start_ok_wnd",
    "click_campaign_start_miss",
    "open_single_player_menu_ok_wnd",
    "open_challenge_menu_ok_wnd",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: campaign side + difficulty button names residual pack.
pub fn honesty_main_menu_campaign_button_names_residual_wave119() -> bool {
    MAIN_MENU_CAMPAIGN_SIDE_BUTTON_NAMES_WAVE119.len() == 4
        && residual_name_index(
            MAIN_MENU_CAMPAIGN_SIDE_BUTTON_NAMES_WAVE119,
            "MainMenu.wnd:ButtonUSA",
        ) == Some(0)
        && residual_name_index(
            MAIN_MENU_CAMPAIGN_SIDE_BUTTON_NAMES_WAVE119,
            "MainMenu.wnd:ButtonChina",
        ) == Some(2)
        && MAIN_MENU_DIFFICULTY_BUTTON_NAMES_WAVE119.len() == 4
        && residual_name_index(
            MAIN_MENU_DIFFICULTY_BUTTON_NAMES_WAVE119,
            "MainMenu.wnd:ButtonEasy",
        ) == Some(0)
        && residual_name_index(
            MAIN_MENU_DIFFICULTY_BUTTON_NAMES_WAVE119,
            "MainMenu.wnd:ButtonHard",
        ) == Some(2)
        && residual_name_index(
            MAIN_MENU_DIFFICULTY_BUTTON_NAMES_WAVE119,
            "MainMenu.wnd:ButtonDiffBack",
        ) == Some(3)
}

/// Honesty: difficulty / show-side enum residual pack.
pub fn honesty_main_menu_campaign_enums_residual_wave119() -> bool {
    GAME_DIFFICULTY_NAMES_WAVE119.len() == 3
        && residual_name_index(GAME_DIFFICULTY_NAMES_WAVE119, "Easy") == Some(0)
        && residual_name_index(GAME_DIFFICULTY_NAMES_WAVE119, "Normal") == Some(1)
        && residual_name_index(GAME_DIFFICULTY_NAMES_WAVE119, "Hard") == Some(2)
        && CAMPAIGN_SHOW_SIDE_NAMES_WAVE119.len() == 4
        && residual_name_index(CAMPAIGN_SHOW_SIDE_NAMES_WAVE119, "USA") == Some(0)
        && residual_name_index(CAMPAIGN_SHOW_SIDE_NAMES_WAVE119, "Training") == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_main_menu_campaign_nav_commands_residual_wave119() -> bool {
    MAIN_MENU_CAMPAIGN_NAV_STEPS_WAVE119.len() == 10
        && residual_name_index(
            MAIN_MENU_CAMPAIGN_NAV_STEPS_WAVE119,
            "GBM_SELECTED_BUTTON_CAMPAIGN_SIDE",
        ) == Some(1)
        && residual_name_index(
            MAIN_MENU_CAMPAIGN_NAV_STEPS_WAVE119,
            "GBM_SELECTED_BUTTON_DIFFICULTY",
        ) == Some(5)
        && residual_name_index(MAIN_MENU_CAMPAIGN_NAV_STEPS_WAVE119, "LATCH_START_GAME") == Some(9)
        && RUNTIME_HOST_CAMPAIGN_CMD_NAMES_WAVE119.len() == 6
        && residual_name_index(
            RUNTIME_HOST_CAMPAIGN_CMD_NAMES_WAVE119,
            "open_difficulty_menu_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_CAMPAIGN_CMD_NAMES_WAVE119,
            "click_campaign_start_ok_wnd",
        ) == Some(2)
}

/// Wave 119 composite residual honesty pack.
pub fn honesty_main_menu_campaign_residual_pack_wave119() -> bool {
    honesty_main_menu_campaign_button_names_residual_wave119()
        && honesty_main_menu_campaign_enums_residual_wave119()
        && honesty_main_menu_campaign_nav_commands_residual_wave119()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_button_names_residual() {
        assert!(honesty_main_menu_campaign_button_names_residual_wave119());
    }

    #[test]
    fn campaign_enums_residual() {
        assert!(honesty_main_menu_campaign_enums_residual_wave119());
    }

    #[test]
    fn campaign_nav_commands_residual() {
        assert!(honesty_main_menu_campaign_nav_commands_residual_wave119());
    }

    #[test]
    fn wave119_composite_pack() {
        assert!(honesty_main_menu_campaign_residual_pack_wave119());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_campaign_start_residual_live() {
        use game_client::gui::{GameDifficulty, ShowSide};
        assert!(
            game_client::gui::simulate_main_menu_campaign_start_residual(
                ShowSide::USA,
                GameDifficulty::Normal
            ),
            "USA+Normal campaign residual must latch start_game"
        );
        assert_eq!(
            game_client::gui::residual_last_campaign_difficulty(),
            Some(GameDifficulty::Normal)
        );
    }
}
