//! Wave 148 residual peels: ControlBar UnderConstruction residual
//! (populate/percent/complete/cancel; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 147 ControlBarResizer residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ControlBarUnderConstruction.cpp populate / updateContext
//! - Command_CancelConstruction
//!
//! Fail-closed:
//! - Not full selected-object construction module residual
//! - Not full control-bar portrait/text gadget residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// UnderConstruction residual tables
// ---------------------------------------------------------------------------

/// Retail cancel construction command residual.
pub const UNDER_CONSTRUCTION_CANCEL_COMMAND_WAVE148: &str = "Command_CancelConstruction";

/// UnderConstruction helper method residual names.
pub const UNDER_CONSTRUCTION_METHOD_NAMES_WAVE148: &[&str] = &[
    "populateUnderConstructionCommands",
    "updateContextUnderConstruction",
    "updateConstructionTextDisplay",
    "Command_CancelConstruction",
];

/// Ordered UnderConstruction residual navigation steps.
pub const UNDER_CONSTRUCTION_NAV_STEPS_WAVE148: &[&str] = &[
    "SELECT_UNDER_CONSTRUCTION_OBJECT",
    "POPULATE_CANCEL_COMMAND",
    "READ_CONSTRUCTION_PERCENT",
    "UPDATE_PERCENT_TEXT",
    "SHOULD_REDRAW_PERCENT",
    "CONSTRUCTION_COMPLETE",
    "HIDE_CANCEL_COMMAND",
];

/// Runtime-host command residual names for UnderConstruction peels.
pub const RUNTIME_HOST_UNDER_CONSTRUCTION_CMD_NAMES_WAVE148: &[&str] = &[
    "click_under_construction_ok_wnd_populate",
    "click_under_construction_ok_wnd_update",
    "click_under_construction_ok_wnd_complete",
    "click_under_construction_ok_wnd_cancel",
    "click_under_construction_ok_wnd_prepare",
    "click_under_construction_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: UnderConstruction method/command names residual pack.
pub fn honesty_under_construction_method_names_residual_wave148() -> bool {
    UNDER_CONSTRUCTION_CANCEL_COMMAND_WAVE148 == "Command_CancelConstruction"
        && UNDER_CONSTRUCTION_METHOD_NAMES_WAVE148.len() == 4
        && residual_name_index(
            UNDER_CONSTRUCTION_METHOD_NAMES_WAVE148,
            "populateUnderConstructionCommands",
        ) == Some(0)
        && residual_name_index(
            UNDER_CONSTRUCTION_METHOD_NAMES_WAVE148,
            "Command_CancelConstruction",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_under_construction_nav_commands_residual_wave148() -> bool {
    UNDER_CONSTRUCTION_NAV_STEPS_WAVE148.len() == 7
        && residual_name_index(
            UNDER_CONSTRUCTION_NAV_STEPS_WAVE148,
            "POPULATE_CANCEL_COMMAND",
        ) == Some(1)
        && residual_name_index(
            UNDER_CONSTRUCTION_NAV_STEPS_WAVE148,
            "CONSTRUCTION_COMPLETE",
        ) == Some(5)
        && RUNTIME_HOST_UNDER_CONSTRUCTION_CMD_NAMES_WAVE148.len() == 6
        && residual_name_index(
            RUNTIME_HOST_UNDER_CONSTRUCTION_CMD_NAMES_WAVE148,
            "click_under_construction_ok_wnd_prepare",
        ) == Some(4)
}

/// Wave 148 composite residual honesty pack.
pub fn honesty_under_construction_residual_pack_wave148() -> bool {
    honesty_under_construction_method_names_residual_wave148()
        && honesty_under_construction_nav_commands_residual_wave148()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_under_construction_method_names_residual_wave148());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_under_construction_nav_commands_residual_wave148());
    }

    #[test]
    fn wave148_composite_pack() {
        assert!(honesty_under_construction_residual_pack_wave148());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_under_construction_prepare_cycle_residual_live() {
        use game_client::gui::control_bar::{
            ResidualUnderConstructionAction, UNDER_CONSTRUCTION_CANCEL_COMMAND_NAME,
            residual_under_construction_cancel_visible, residual_under_construction_is_completed,
            residual_under_construction_last_action, residual_under_construction_percent,
            simulate_under_construction_complete, simulate_under_construction_prepare_cycle,
        };
        assert_eq!(
            UNDER_CONSTRUCTION_CANCEL_COMMAND_NAME,
            UNDER_CONSTRUCTION_CANCEL_COMMAND_WAVE148
        );
        assert!(
            simulate_under_construction_prepare_cycle("Strategy Center", 66, 75),
            "populate+update residual must latch"
        );
        assert!(residual_under_construction_cancel_visible());
        assert!(!residual_under_construction_is_completed());
        assert_eq!(residual_under_construction_percent(), 75);
        assert_eq!(
            residual_under_construction_last_action(),
            ResidualUnderConstructionAction::UpdatePercent
        );
        assert!(simulate_under_construction_complete());
        assert!(residual_under_construction_is_completed());
        assert!(!residual_under_construction_cancel_visible());
        assert_eq!(
            residual_under_construction_last_action(),
            ResidualUnderConstructionAction::Complete
        );
    }
}
