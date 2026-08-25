//! Wave 134 residual peels: DifficultySelect WND residual
//! (Easy/Medium/Hard + Ok/Cancel; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 119 campaign entry, Wave 132 MapSelect difficulty.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - DifficultySelect.cpp / DifficultySelect.wnd
//! - RadioButtonEasy/Medium/Hard, ButtonOk, ButtonCancel
//!
//! Fail-closed:
//! - Not full start_campaign_game residual
//! - Not full side-specific difficulty layout residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Difficulty select residual tables
// ---------------------------------------------------------------------------

/// Retail DifficultySelect layout filename residual.
pub const DIFFICULTY_SELECT_LAYOUT_FILENAME_WAVE134: &str = "Menus/DifficultySelect.wnd";

/// Retail DifficultySelect control names residual.
pub const DIFFICULTY_SELECT_CONTROL_NAMES_WAVE134: &[&str] = &[
    "DifficultySelect.wnd:DifficultySelectParent",
    "DifficultySelect.wnd:ButtonOk",
    "DifficultySelect.wnd:ButtonCancel",
    "DifficultySelect.wnd:RadioButtonEasy",
    "DifficultySelect.wnd:RadioButtonMedium",
    "DifficultySelect.wnd:RadioButtonHard",
];

/// Ordered DifficultySelect residual navigation steps.
pub const DIFFICULTY_SELECT_NAV_STEPS_WAVE134: &[&str] = &[
    "PUSH_DIFFICULTY_SELECT_LAYOUT",
    "GBM_SELECTED_RADIO_EASY",
    "GBM_SELECTED_RADIO_MEDIUM",
    "GBM_SELECTED_RADIO_HARD",
    "GBM_SELECTED_BUTTON_OK",
    "START_CAMPAIGN_GAME",
    "GBM_SELECTED_BUTTON_CANCEL",
    "SHELL_POP",
];

/// Runtime-host command residual names for DifficultySelect peels.
pub const RUNTIME_HOST_DIFFICULTY_SELECT_CMD_NAMES_WAVE134: &[&str] = &[
    "open_difficulty_menu_ok_wnd",
    "open_difficulty_menu_ok",
    "click_difficulty_select_ok_wnd_easy",
    "click_difficulty_select_ok_wnd_medium",
    "click_difficulty_select_ok_wnd_hard",
    "click_difficulty_select_ok_wnd_ok",
    "click_difficulty_select_ok_wnd_cancel",
    "click_difficulty_select_ok_wnd_prepare_ok",
    "click_difficulty_select_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: DifficultySelect control names residual pack.
pub fn honesty_difficulty_select_control_names_residual_wave134() -> bool {
    DIFFICULTY_SELECT_LAYOUT_FILENAME_WAVE134 == "Menus/DifficultySelect.wnd"
        && DIFFICULTY_SELECT_CONTROL_NAMES_WAVE134.len() == 6
        && residual_name_index(
            DIFFICULTY_SELECT_CONTROL_NAMES_WAVE134,
            "DifficultySelect.wnd:ButtonOk",
        ) == Some(1)
        && residual_name_index(
            DIFFICULTY_SELECT_CONTROL_NAMES_WAVE134,
            "DifficultySelect.wnd:RadioButtonEasy",
        ) == Some(3)
        && residual_name_index(
            DIFFICULTY_SELECT_CONTROL_NAMES_WAVE134,
            "DifficultySelect.wnd:RadioButtonHard",
        ) == Some(5)
        && DIFFICULTY_SELECT_CONTROL_NAMES_WAVE134
            .iter()
            .all(|n| n.starts_with("DifficultySelect.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_difficulty_select_nav_commands_residual_wave134() -> bool {
    DIFFICULTY_SELECT_NAV_STEPS_WAVE134.len() == 8
        && residual_name_index(
            DIFFICULTY_SELECT_NAV_STEPS_WAVE134,
            "GBM_SELECTED_RADIO_MEDIUM",
        ) == Some(2)
        && residual_name_index(
            DIFFICULTY_SELECT_NAV_STEPS_WAVE134,
            "GBM_SELECTED_BUTTON_OK",
        ) == Some(4)
        && residual_name_index(DIFFICULTY_SELECT_NAV_STEPS_WAVE134, "SHELL_POP") == Some(7)
        && RUNTIME_HOST_DIFFICULTY_SELECT_CMD_NAMES_WAVE134.len() == 9
        && residual_name_index(
            RUNTIME_HOST_DIFFICULTY_SELECT_CMD_NAMES_WAVE134,
            "click_difficulty_select_ok_wnd_prepare_ok",
        ) == Some(7)
        && residual_name_index(
            RUNTIME_HOST_DIFFICULTY_SELECT_CMD_NAMES_WAVE134,
            "click_difficulty_select_ok_wnd_hard",
        ) == Some(4)
}

/// Wave 134 composite residual honesty pack.
pub fn honesty_difficulty_select_residual_pack_wave134() -> bool {
    honesty_difficulty_select_control_names_residual_wave134()
        && honesty_difficulty_select_nav_commands_residual_wave134()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_difficulty_select_control_names_residual_wave134());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_difficulty_select_nav_commands_residual_wave134());
    }

    #[test]
    fn wave134_composite_pack() {
        assert!(honesty_difficulty_select_residual_pack_wave134());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_difficulty_prepare_ok_residual_live() {
        use game_client::gui::callbacks::{
            ResidualDifficultySelectAction, residual_difficulty_select_last_action,
            residual_difficulty_select_level,
            simulate_difficulty_select_cancel_button_gadget_selected,
            simulate_difficulty_select_prepare_ok,
        };
        assert!(
            simulate_difficulty_select_prepare_ok(2),
            "hard+ok residual must latch"
        );
        assert_eq!(residual_difficulty_select_level(), 2);
        assert_eq!(
            residual_difficulty_select_last_action(),
            ResidualDifficultySelectAction::Ok
        );
        assert!(simulate_difficulty_select_cancel_button_gadget_selected());
        assert_eq!(
            residual_difficulty_select_last_action(),
            ResidualDifficultySelectAction::Cancel
        );
    }
}
