//! Wave 125 residual peels: ScoreScreen WND residual
//! (Ok/Continue/SaveReplay/Buddy/Emote; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 122 ReplayMenu, Wave 123 QuitMenu exit.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ScoreScreen.cpp / ScoreScreen.wnd
//! - ButtonOk, ButtonContinue, ButtonSaveReplay, ButtonBuddy, ButtonEmote
//!
//! Fail-closed:
//! - Not full score gather / portrait / chat residual
//! - Not full next-campaign map start residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Score screen residual tables
// ---------------------------------------------------------------------------

/// Retail ScoreScreen layout filename residual.
pub const SCORE_SCREEN_LAYOUT_FILENAME_WAVE125: &str = "Menus/ScoreScreen.wnd";

/// Retail ScoreScreen control names residual (core buttons + parent).
pub const SCORE_SCREEN_CONTROL_NAMES_WAVE125: &[&str] = &[
    "ScoreScreen.wnd:ParentScoreScreen",
    "ScoreScreen.wnd:ButtonOk",
    "ScoreScreen.wnd:ButtonContinue",
    "ScoreScreen.wnd:ButtonSaveReplay",
    "ScoreScreen.wnd:ButtonBuddy",
    "ScoreScreen.wnd:ButtonEmote",
    "ScoreScreen.wnd:TextEntryChat",
    "ScoreScreen.wnd:ListboxChatWindowScoreScreen",
    "ScoreScreen.wnd:MainBackdrop",
];

/// Ordered ScoreScreen residual navigation steps.
pub const SCORE_SCREEN_NAV_STEPS_WAVE125: &[&str] = &[
    "PUSH_SCORE_SCREEN_LAYOUT",
    "GATHER_SCORE_STATS",
    "SHOW_WIN_LOSS",
    "GBM_SELECTED_BUTTON_SAVE_REPLAY",
    "POPUP_REPLAY",
    "GBM_SELECTED_BUTTON_CONTINUE",
    "START_NEXT_CAMPAIGN_OR_FINISH",
    "GBM_SELECTED_BUTTON_OK",
    "SHELL_POP_CLEAR_CAMPAIGN",
];

/// Runtime-host command residual names for ScoreScreen peels.
pub const RUNTIME_HOST_SCORE_SCREEN_CMD_NAMES_WAVE125: &[&str] = &[
    "open_score_screen_ok_wnd",
    "open_score_screen_ok",
    "click_score_screen_ok_wnd_ok",
    "click_score_screen_ok_wnd_continue",
    "click_score_screen_ok_wnd_finish",
    "click_score_screen_ok_wnd_save_replay",
    "click_score_screen_ok_wnd_buddy",
    "click_score_screen_ok_wnd_emote",
    "click_score_screen_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: ScoreScreen control names residual pack.
pub fn honesty_score_screen_control_names_residual_wave125() -> bool {
    SCORE_SCREEN_LAYOUT_FILENAME_WAVE125 == "Menus/ScoreScreen.wnd"
        && SCORE_SCREEN_CONTROL_NAMES_WAVE125.len() == 9
        && residual_name_index(
            SCORE_SCREEN_CONTROL_NAMES_WAVE125,
            "ScoreScreen.wnd:ButtonOk",
        ) == Some(1)
        && residual_name_index(
            SCORE_SCREEN_CONTROL_NAMES_WAVE125,
            "ScoreScreen.wnd:ButtonContinue",
        ) == Some(2)
        && residual_name_index(
            SCORE_SCREEN_CONTROL_NAMES_WAVE125,
            "ScoreScreen.wnd:ButtonSaveReplay",
        ) == Some(3)
        && SCORE_SCREEN_CONTROL_NAMES_WAVE125
            .iter()
            .all(|n| n.starts_with("ScoreScreen.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_score_screen_nav_commands_residual_wave125() -> bool {
    SCORE_SCREEN_NAV_STEPS_WAVE125.len() == 9
        && residual_name_index(SCORE_SCREEN_NAV_STEPS_WAVE125, "GATHER_SCORE_STATS") == Some(1)
        && residual_name_index(
            SCORE_SCREEN_NAV_STEPS_WAVE125,
            "GBM_SELECTED_BUTTON_CONTINUE",
        ) == Some(5)
        && residual_name_index(SCORE_SCREEN_NAV_STEPS_WAVE125, "SHELL_POP_CLEAR_CAMPAIGN")
            == Some(8)
        && RUNTIME_HOST_SCORE_SCREEN_CMD_NAMES_WAVE125.len() == 9
        && residual_name_index(
            RUNTIME_HOST_SCORE_SCREEN_CMD_NAMES_WAVE125,
            "open_score_screen_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_SCORE_SCREEN_CMD_NAMES_WAVE125,
            "click_score_screen_ok_wnd_finish",
        ) == Some(4)
}

/// Wave 125 composite residual honesty pack.
pub fn honesty_score_screen_residual_pack_wave125() -> bool {
    honesty_score_screen_control_names_residual_wave125()
        && honesty_score_screen_nav_commands_residual_wave125()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_score_screen_control_names_residual_wave125());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_score_screen_nav_commands_residual_wave125());
    }

    #[test]
    fn wave125_composite_pack() {
        assert!(honesty_score_screen_residual_pack_wave125());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_score_prepare_finish_residual_live() {
        use game_client::gui::callbacks::{
            ResidualScoreScreenAction, residual_score_screen_is_finish_campaign,
            residual_score_screen_last_action, simulate_score_screen_prepare_finish,
            simulate_score_screen_prepare_ok,
        };
        assert!(
            simulate_score_screen_prepare_finish(),
            "finish+continue residual must latch"
        );
        assert!(residual_score_screen_is_finish_campaign());
        assert_eq!(
            residual_score_screen_last_action(),
            ResidualScoreScreenAction::Continue
        );
        assert!(simulate_score_screen_prepare_ok());
        assert_eq!(
            residual_score_screen_last_action(),
            ResidualScoreScreenAction::Ok
        );
    }
}
