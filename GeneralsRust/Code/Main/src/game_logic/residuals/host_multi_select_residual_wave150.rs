//! Wave 150 residual peels: ControlBar MultiSelect residual
//! (intersect OK_FOR_MULTI_SELECT / AttackMove keep; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 149 StructureInventory residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ControlBarMultiSelect.cpp populate / intersect slots
//! - CommandOption::OK_FOR_MULTI_SELECT = 0x100
//! - MAX_COMMANDS_PER_SET = 18
//!
//! Fail-closed:
//! - Not full OBJECT_REGISTRY multi-select residual
//! - Not full control-bar WND command grid residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// MultiSelect residual tables
// ---------------------------------------------------------------------------

/// Retail max commands per set residual.
pub const MULTI_SELECT_MAX_COMMANDS_PER_SET_WAVE150: usize = 18;

/// Retail OK_FOR_MULTI_SELECT option bit residual.
pub const MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT_WAVE150: u32 = 0x00000100;

/// MultiSelect helper method residual names.
pub const MULTI_SELECT_METHOD_NAMES_WAVE150: &[&str] = &[
    "populateMultiSelectCommands",
    "populateMultiSelectCommandsFromSets",
    "intersectCommandSetIntoSlots",
    "OK_FOR_MULTI_SELECT",
    "AttackMoveKeep",
];

/// Ordered MultiSelect residual navigation steps.
pub const MULTI_SELECT_NAV_STEPS_WAVE150: &[&str] = &[
    "SELECT_MULTIPLE_OBJECTS",
    "FILTER_SOLD_IGNORED",
    "SEED_FIRST_OK_FOR_MULTI",
    "INTERSECT_SUBSEQUENT_SLOTS",
    "KEEP_ATTACK_MOVE_IF_ANY",
    "CLEAR_DIVERGENT_PORTRAIT",
    "PUSH_COMMON_COMMANDS",
];

/// Runtime-host command residual names for MultiSelect peels.
pub const RUNTIME_HOST_MULTI_SELECT_CMD_NAMES_WAVE150: &[&str] = &[
    "click_multi_select_ok_wnd_prepare",
    "click_multi_select_ok_wnd_divergent",
    "click_multi_select_ok_wnd_clear",
    "click_multi_select_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: MultiSelect method names + constants residual pack.
pub fn honesty_multi_select_method_names_residual_wave150() -> bool {
    MULTI_SELECT_MAX_COMMANDS_PER_SET_WAVE150 == 18
        && MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT_WAVE150 == 0x00000100
        && MULTI_SELECT_METHOD_NAMES_WAVE150.len() == 5
        && residual_name_index(
            MULTI_SELECT_METHOD_NAMES_WAVE150,
            "populateMultiSelectCommands",
        ) == Some(0)
        && residual_name_index(MULTI_SELECT_METHOD_NAMES_WAVE150, "OK_FOR_MULTI_SELECT") == Some(3)
        && residual_name_index(MULTI_SELECT_METHOD_NAMES_WAVE150, "AttackMoveKeep") == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_multi_select_nav_commands_residual_wave150() -> bool {
    MULTI_SELECT_NAV_STEPS_WAVE150.len() == 7
        && residual_name_index(MULTI_SELECT_NAV_STEPS_WAVE150, "SEED_FIRST_OK_FOR_MULTI") == Some(2)
        && residual_name_index(MULTI_SELECT_NAV_STEPS_WAVE150, "KEEP_ATTACK_MOVE_IF_ANY") == Some(4)
        && residual_name_index(MULTI_SELECT_NAV_STEPS_WAVE150, "PUSH_COMMON_COMMANDS") == Some(6)
        && RUNTIME_HOST_MULTI_SELECT_CMD_NAMES_WAVE150.len() == 4
        && residual_name_index(
            RUNTIME_HOST_MULTI_SELECT_CMD_NAMES_WAVE150,
            "click_multi_select_ok_wnd_prepare",
        ) == Some(0)
}

/// Wave 150 composite residual honesty pack.
pub fn honesty_multi_select_residual_pack_wave150() -> bool {
    honesty_multi_select_method_names_residual_wave150()
        && honesty_multi_select_nav_commands_residual_wave150()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_multi_select_method_names_residual_wave150());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_multi_select_nav_commands_residual_wave150());
    }

    #[test]
    fn wave150_composite_pack() {
        assert!(honesty_multi_select_residual_pack_wave150());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_multi_select_prepare_same_and_divergent_residual_live() {
        use game_client::gui::control_bar::{
            MULTI_SELECT_MAX_COMMANDS_PER_SET, MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT,
            ResidualMultiSelectAction, residual_multi_select_attack_move_kept,
            residual_multi_select_common_command_count, residual_multi_select_last_action,
            residual_multi_select_portrait_agrees, residual_multi_select_selected_count,
            simulate_multi_select_clear, simulate_multi_select_prepare_divergent,
            simulate_multi_select_prepare_same_commands,
        };
        assert_eq!(
            MULTI_SELECT_MAX_COMMANDS_PER_SET,
            MULTI_SELECT_MAX_COMMANDS_PER_SET_WAVE150
        );
        assert_eq!(
            MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT,
            MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT_WAVE150
        );
        assert!(
            simulate_multi_select_prepare_same_commands(),
            "same-command multi-select residual must latch"
        );
        assert_eq!(residual_multi_select_selected_count(), 2);
        assert_eq!(residual_multi_select_common_command_count(), 3);
        assert!(residual_multi_select_portrait_agrees());
        assert!(residual_multi_select_attack_move_kept());
        assert_eq!(
            residual_multi_select_last_action(),
            ResidualMultiSelectAction::AttackMoveKeep
        );
        assert!(simulate_multi_select_prepare_divergent());
        assert_eq!(residual_multi_select_common_command_count(), 1);
        assert!(!residual_multi_select_portrait_agrees());
        assert!(residual_multi_select_attack_move_kept());
        assert_eq!(
            residual_multi_select_last_action(),
            ResidualMultiSelectAction::Intersect
        );
        assert!(simulate_multi_select_clear());
        assert_eq!(
            residual_multi_select_last_action(),
            ResidualMultiSelectAction::Clear
        );
    }
}
