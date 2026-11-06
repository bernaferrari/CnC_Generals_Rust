//! Observable ownership for ControlBar structure-inventory residuals.
//!
//! C++ ControlBarStructureInventory keeps garrison UI state on the bar; the
//! Rust latch lives on ControlBarContext so two clients never share atomics.

use game_client_rust::gui::control_bar::{
    ResidualStructureInventoryAction, STRUCTURE_INVENTORY_EXIT_COMMAND_NAME,
    StructureInventoryLatch, residual_structure_inventory_exit_visible,
    residual_structure_inventory_garrisoned_count, residual_structure_inventory_last_action,
    residual_structure_inventory_max_garrison, simulate_structure_inventory_clear,
    simulate_structure_inventory_exit_command_name, simulate_structure_inventory_populate,
};

#[test]
fn structure_inventory_latch_is_instance_owned() {
    let mut a = StructureInventoryLatch::default();
    let mut b = StructureInventoryLatch::default();
    assert!(simulate_structure_inventory_populate(&mut a, 4, 2));
    assert_eq!(residual_structure_inventory_max_garrison(&a), 4);
    assert_eq!(residual_structure_inventory_garrisoned_count(&a), 2);
    assert!(residual_structure_inventory_exit_visible(&a));
    assert_eq!(
        residual_structure_inventory_last_action(&a),
        ResidualStructureInventoryAction::Populate
    );
    assert_eq!(residual_structure_inventory_max_garrison(&b), 0);
    assert!(simulate_structure_inventory_clear(&mut a));
    assert_eq!(
        residual_structure_inventory_last_action(&a),
        ResidualStructureInventoryAction::Clear
    );
    assert!(!residual_structure_inventory_exit_visible(&a));
    assert_eq!(
        simulate_structure_inventory_exit_command_name(&mut b),
        STRUCTURE_INVENTORY_EXIT_COMMAND_NAME
    );
    assert_eq!(
        residual_structure_inventory_last_action(&b),
        ResidualStructureInventoryAction::Exit
    );
    assert_eq!(
        residual_structure_inventory_last_action(&a),
        ResidualStructureInventoryAction::Clear
    );
}
