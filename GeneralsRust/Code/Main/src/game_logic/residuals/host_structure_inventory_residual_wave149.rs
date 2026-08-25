//! Wave 149 residual peels: ControlBar StructureInventory residual
//! (populate/exit/evacuate/stop; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 148 UnderConstruction residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ControlBarStructureInventory.cpp append inventory commands
//! - Command_StructureExit / Command_Evacuate / Command_Stop
//!
//! Fail-closed:
//! - Not full OBJECT_REGISTRY contain-module residual
//! - Not full per-unit inventory button residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// StructureInventory residual tables
// ---------------------------------------------------------------------------

/// Retail structure inventory command names residual.
pub const STRUCTURE_INVENTORY_COMMAND_NAMES_WAVE149: &[&str] =
    &["Command_StructureExit", "Command_Evacuate", "Command_Stop"];

/// StructureInventory helper method residual names.
pub const STRUCTURE_INVENTORY_METHOD_NAMES_WAVE149: &[&str] = &[
    "appendStructureInventoryCommands",
    "appendStructureInventoryCommandsWithPresentation",
    "lastRecordedInventoryCount",
];

/// Ordered StructureInventory residual navigation steps.
pub const STRUCTURE_INVENTORY_NAV_STEPS_WAVE149: &[&str] = &[
    "SELECT_GARRISON_STRUCTURE",
    "READ_MAX_CAPACITY",
    "READ_CONTAINED_COUNT",
    "PUSH_STRUCTURE_EXIT",
    "PUSH_EVACUATE_IF_OCCUPIED",
    "PUSH_STOP_IF_OCCUPIED",
    "RECORD_INVENTORY_COUNT",
];

/// Runtime-host command residual names for StructureInventory peels.
pub const RUNTIME_HOST_STRUCTURE_INVENTORY_CMD_NAMES_WAVE149: &[&str] = &[
    "click_structure_inventory_ok_wnd_populate",
    "click_structure_inventory_ok_wnd_clear",
    "click_structure_inventory_ok_wnd_exit",
    "click_structure_inventory_ok_wnd_evacuate",
    "click_structure_inventory_ok_wnd_stop",
    "click_structure_inventory_ok_wnd_prepare",
    "click_structure_inventory_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: StructureInventory command/method names residual pack.
pub fn honesty_structure_inventory_command_names_residual_wave149() -> bool {
    STRUCTURE_INVENTORY_COMMAND_NAMES_WAVE149.len() == 3
        && residual_name_index(
            STRUCTURE_INVENTORY_COMMAND_NAMES_WAVE149,
            "Command_StructureExit",
        ) == Some(0)
        && residual_name_index(
            STRUCTURE_INVENTORY_COMMAND_NAMES_WAVE149,
            "Command_Evacuate",
        ) == Some(1)
        && residual_name_index(STRUCTURE_INVENTORY_COMMAND_NAMES_WAVE149, "Command_Stop") == Some(2)
        && STRUCTURE_INVENTORY_METHOD_NAMES_WAVE149.len() == 3
        && residual_name_index(
            STRUCTURE_INVENTORY_METHOD_NAMES_WAVE149,
            "appendStructureInventoryCommandsWithPresentation",
        ) == Some(1)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_structure_inventory_nav_commands_residual_wave149() -> bool {
    STRUCTURE_INVENTORY_NAV_STEPS_WAVE149.len() == 7
        && residual_name_index(STRUCTURE_INVENTORY_NAV_STEPS_WAVE149, "PUSH_STRUCTURE_EXIT")
            == Some(3)
        && residual_name_index(
            STRUCTURE_INVENTORY_NAV_STEPS_WAVE149,
            "PUSH_EVACUATE_IF_OCCUPIED",
        ) == Some(4)
        && residual_name_index(
            STRUCTURE_INVENTORY_NAV_STEPS_WAVE149,
            "RECORD_INVENTORY_COUNT",
        ) == Some(6)
        && RUNTIME_HOST_STRUCTURE_INVENTORY_CMD_NAMES_WAVE149.len() == 7
        && residual_name_index(
            RUNTIME_HOST_STRUCTURE_INVENTORY_CMD_NAMES_WAVE149,
            "click_structure_inventory_ok_wnd_prepare",
        ) == Some(5)
}

/// Wave 149 composite residual honesty pack.
pub fn honesty_structure_inventory_residual_pack_wave149() -> bool {
    honesty_structure_inventory_command_names_residual_wave149()
        && honesty_structure_inventory_nav_commands_residual_wave149()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_residual() {
        assert!(honesty_structure_inventory_command_names_residual_wave149());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_structure_inventory_nav_commands_residual_wave149());
    }

    #[test]
    fn wave149_composite_pack() {
        assert!(honesty_structure_inventory_residual_pack_wave149());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_structure_inventory_prepare_occupied_residual_live() {
        use game_client::gui::control_bar::{
            ResidualStructureInventoryAction, STRUCTURE_INVENTORY_EXIT_COMMAND_NAME,
            residual_structure_inventory_evacuate_visible,
            residual_structure_inventory_exit_visible,
            residual_structure_inventory_garrisoned_count,
            residual_structure_inventory_last_action, residual_structure_inventory_max_garrison,
            residual_structure_inventory_stop_visible, simulate_structure_inventory_clear,
            simulate_structure_inventory_prepare_occupied,
        };
        assert_eq!(
            STRUCTURE_INVENTORY_EXIT_COMMAND_NAME,
            "Command_StructureExit"
        );
        assert!(
            simulate_structure_inventory_prepare_occupied(10, 3),
            "occupied inventory residual must latch exit+evacuate+stop"
        );
        assert_eq!(residual_structure_inventory_max_garrison(), 10);
        assert_eq!(residual_structure_inventory_garrisoned_count(), 3);
        assert!(residual_structure_inventory_exit_visible());
        assert!(residual_structure_inventory_evacuate_visible());
        assert!(residual_structure_inventory_stop_visible());
        assert_eq!(
            residual_structure_inventory_last_action(),
            ResidualStructureInventoryAction::Populate
        );
        assert!(simulate_structure_inventory_clear());
        assert_eq!(
            residual_structure_inventory_last_action(),
            ResidualStructureInventoryAction::Clear
        );
    }
}
