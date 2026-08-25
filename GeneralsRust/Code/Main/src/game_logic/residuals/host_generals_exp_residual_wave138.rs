//! Wave 138 residual peels: GeneralsExpPoints / purchase-science residual
//! (show/Exit/ESC/science click; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 133 ControlBar ButtonGeneral.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - GeneralsExpPoints.cpp / GeneralsExpPoints.wnd
//! - ButtonExit, context-sensitive science buttons
//!
//! Fail-closed:
//! - Not full science purchase apply residual
//! - Not full rank/points UI residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Generals exp residual tables
// ---------------------------------------------------------------------------

/// Retail GeneralsExpPoints layout filename residual.
pub const GENERALS_EXP_LAYOUT_FILENAME_WAVE138: &str = "GeneralsExpPoints.wnd";

/// Retail GeneralsExpPoints control names residual.
pub const GENERALS_EXP_CONTROL_NAMES_WAVE138: &[&str] = &["GeneralsExpPoints.wnd:ButtonExit"];

/// Ordered GeneralsExp residual navigation steps.
pub const GENERALS_EXP_NAV_STEPS_WAVE138: &[&str] = &[
    "SHOW_PURCHASE_SCIENCE",
    "GBM_SELECTED_SCIENCE_BUTTON",
    "PROCESS_CONTEXT_SENSITIVE_CLICK",
    "GBM_SELECTED_BUTTON_EXIT",
    "HIDE_PURCHASE_SCIENCE",
    "ESC_HIDE",
];

/// Runtime-host command residual names for GeneralsExp peels.
pub const RUNTIME_HOST_GENERALS_EXP_CMD_NAMES_WAVE138: &[&str] = &[
    "open_generals_exp_ok_wnd",
    "open_generals_exp_miss",
    "click_generals_exp_ok_wnd_exit",
    "click_generals_exp_ok_wnd_esc",
    "click_generals_exp_ok_wnd_science",
    "click_generals_exp_ok_wnd_prepare_exit",
    "click_generals_exp_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: GeneralsExp control names residual pack.
pub fn honesty_generals_exp_control_names_residual_wave138() -> bool {
    GENERALS_EXP_LAYOUT_FILENAME_WAVE138 == "GeneralsExpPoints.wnd"
        && GENERALS_EXP_CONTROL_NAMES_WAVE138.len() == 1
        && residual_name_index(
            GENERALS_EXP_CONTROL_NAMES_WAVE138,
            "GeneralsExpPoints.wnd:ButtonExit",
        ) == Some(0)
        && GENERALS_EXP_CONTROL_NAMES_WAVE138
            .iter()
            .all(|n| n.starts_with("GeneralsExpPoints.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_generals_exp_nav_commands_residual_wave138() -> bool {
    GENERALS_EXP_NAV_STEPS_WAVE138.len() == 6
        && residual_name_index(GENERALS_EXP_NAV_STEPS_WAVE138, "SHOW_PURCHASE_SCIENCE") == Some(0)
        && residual_name_index(GENERALS_EXP_NAV_STEPS_WAVE138, "GBM_SELECTED_BUTTON_EXIT")
            == Some(3)
        && residual_name_index(GENERALS_EXP_NAV_STEPS_WAVE138, "ESC_HIDE") == Some(5)
        && RUNTIME_HOST_GENERALS_EXP_CMD_NAMES_WAVE138.len() == 7
        && residual_name_index(
            RUNTIME_HOST_GENERALS_EXP_CMD_NAMES_WAVE138,
            "click_generals_exp_ok_wnd_prepare_exit",
        ) == Some(5)
}

/// Wave 138 composite residual honesty pack.
pub fn honesty_generals_exp_residual_pack_wave138() -> bool {
    honesty_generals_exp_control_names_residual_wave138()
        && honesty_generals_exp_nav_commands_residual_wave138()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_generals_exp_control_names_residual_wave138());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_generals_exp_nav_commands_residual_wave138());
    }

    #[test]
    fn wave138_composite_pack() {
        assert!(honesty_generals_exp_residual_pack_wave138());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_generals_exp_prepare_exit_residual_live() {
        use game_client::gui::callbacks::{
            ResidualGeneralsExpAction, residual_generals_exp_is_visible,
            residual_generals_exp_last_action, simulate_generals_exp_prepare_exit,
        };
        assert!(
            simulate_generals_exp_prepare_exit(),
            "show+exit residual must latch"
        );
        assert!(!residual_generals_exp_is_visible());
        assert_eq!(
            residual_generals_exp_last_action(),
            ResidualGeneralsExpAction::Exit
        );
    }
}
