//! Wave 151 residual peels: Credits manager roll residual
//! (init/reset/add/update; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 150 MultiSelect residual and credits menu skip residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Credits.cpp init/reset/add/update/isFinished
//! - CreditStyle TITLE/MINORTITLE/NORMAL/COLUMN
//!
//! Fail-closed:
//! - Not full Credits.ini parse residual
//! - Not full DisplayString/font draw residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Credits residual tables
// ---------------------------------------------------------------------------

/// CreditStyle residual names (Credits.cpp parse).
pub const CREDIT_STYLE_NAMES_WAVE151: &[&str] = &["TITLE", "MINORTITLE", "NORMAL", "COLUMN"];

/// Credits manager method residual names.
pub const CREDITS_METHOD_NAMES_WAVE151: &[&str] = &[
    "init",
    "reset",
    "loadFromPath",
    "addText",
    "addBlank",
    "update",
    "draw",
    "isFinished",
];

/// Ordered Credits residual navigation steps.
pub const CREDITS_NAV_STEPS_WAVE151: &[&str] = &[
    "CREDITS_RESET",
    "CREDITS_INIT",
    "ADD_TITLE_LINE",
    "ADD_BLANK_LINE",
    "ADD_NORMAL_LINE",
    "UPDATE_SCROLL",
    "PROBE_IS_FINISHED",
];

/// Runtime-host command residual names for Credits roll peels.
pub const RUNTIME_HOST_CREDITS_ROLL_CMD_NAMES_WAVE151: &[&str] = &[
    "click_credits_roll_ok_wnd_init",
    "click_credits_roll_ok_wnd_reset",
    "click_credits_roll_ok_wnd_add",
    "click_credits_roll_ok_wnd_blank",
    "click_credits_roll_ok_wnd_update",
    "click_credits_roll_ok_wnd_finished",
    "click_credits_roll_ok_wnd_prepare",
    "click_credits_roll_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: CreditStyle + method names residual pack.
pub fn honesty_credits_style_method_names_residual_wave151() -> bool {
    CREDIT_STYLE_NAMES_WAVE151.len() == 4
        && residual_name_index(CREDIT_STYLE_NAMES_WAVE151, "TITLE") == Some(0)
        && residual_name_index(CREDIT_STYLE_NAMES_WAVE151, "MINORTITLE") == Some(1)
        && residual_name_index(CREDIT_STYLE_NAMES_WAVE151, "COLUMN") == Some(3)
        && CREDITS_METHOD_NAMES_WAVE151.len() == 8
        && residual_name_index(CREDITS_METHOD_NAMES_WAVE151, "addText") == Some(3)
        && residual_name_index(CREDITS_METHOD_NAMES_WAVE151, "isFinished") == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_credits_nav_commands_residual_wave151() -> bool {
    CREDITS_NAV_STEPS_WAVE151.len() == 7
        && residual_name_index(CREDITS_NAV_STEPS_WAVE151, "CREDITS_INIT") == Some(1)
        && residual_name_index(CREDITS_NAV_STEPS_WAVE151, "ADD_BLANK_LINE") == Some(3)
        && residual_name_index(CREDITS_NAV_STEPS_WAVE151, "PROBE_IS_FINISHED") == Some(6)
        && RUNTIME_HOST_CREDITS_ROLL_CMD_NAMES_WAVE151.len() == 8
        && residual_name_index(
            RUNTIME_HOST_CREDITS_ROLL_CMD_NAMES_WAVE151,
            "click_credits_roll_ok_wnd_prepare",
        ) == Some(6)
}

/// Wave 151 composite residual honesty pack.
pub fn honesty_credits_roll_residual_pack_wave151() -> bool {
    honesty_credits_style_method_names_residual_wave151()
        && honesty_credits_nav_commands_residual_wave151()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_method_names_residual() {
        assert!(honesty_credits_style_method_names_residual_wave151());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_credits_nav_commands_residual_wave151());
    }

    #[test]
    fn wave151_composite_pack() {
        assert!(honesty_credits_roll_residual_pack_wave151());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_credits_prepare_short_roll_residual_live() {
        use game_client::credits::{
            ResidualCreditsAction, residual_credits_is_finished, residual_credits_last_action,
            residual_credits_line_count, simulate_credits_prepare_short_roll,
        };
        assert!(
            simulate_credits_prepare_short_roll(),
            "reset+init+lines residual must latch"
        );
        assert!(residual_credits_line_count() >= 3);
        assert!(!residual_credits_is_finished());
        assert_eq!(
            residual_credits_last_action(),
            ResidualCreditsAction::AddText
        );
    }
}
