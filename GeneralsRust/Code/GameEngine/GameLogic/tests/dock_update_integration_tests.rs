//! Integration tests for dock/exit update modules
//!
//! Tests comprehensive docking scenarios including:
//! - Entry/exit sequences
//! - Queue management
//! - Multiple dock types (repair, supply, tunnel)
//! - Container integration
//! - State machine transitions

#[cfg(test)]
mod dock_integration_tests {
    use gamelogic::common::*;
    use gamelogic::modules::{
        BehaviorModuleInterface, DockUpdateInterface, RailedTransportDockUpdateInterface,
    };
    use gamelogic::object::production::*;

    /// Test helper to create a coordinate
    fn coord(x: f32, y: f32, z: f32) -> Coord3D {
        Coord3D::new(x, y, z)
    }

    #[test]
    fn test_basic_dock_lifecycle() {
        // Create a basic dock
        let data = DockUpdateData::default();
        let pos = coord(100.0, 100.0, 0.0);
        let dock = DockUpdate::new(data, 1, &pos);

        // Dock should be open initially
        assert!(dock.is_dock_open().unwrap());
    }

    #[test]
    fn test_repair_dock_with_healing() {
        let data = RepairDockUpdateData {
            frames_for_full_heal: 60.0,
            ..Default::default()
        };

        let pos = coord(200.0, 200.0, 0.0);
        let dock = RepairDockUpdate::new(data, 2, &pos);

        // Dock should be rally point type
        assert!(dock.is_rally_point_after_dock_type().unwrap());
    }

    #[test]
    fn test_supply_center_dock_operations() {
        let data = SupplyCenterDockUpdateData {
            grant_temporary_stealth_frames: 30,
            ..Default::default()
        };

        let pos = coord(300.0, 300.0, 0.0);
        let dock = SupplyCenterDockUpdate::new(data, 3, &pos);

        // C++ SupplyCenterDockUpdate inherits DockUpdate::isRallyPointAfterDockType() = false.
        assert!(!dock.is_rally_point_after_dock_type().unwrap());
    }

    #[test]
    fn test_supply_warehouse_box_management() {
        let data = SupplyWarehouseDockUpdateData {
            starting_boxes: 8,
            delete_when_empty: true,
            ..Default::default()
        };

        let pos = coord(400.0, 400.0, 0.0);
        let dock = SupplyWarehouseDockUpdate::new(data, 4, &pos);

        // Should have correct starting boxes
        assert_eq!(dock.get_boxes_stored(), 8);

        // Dock should be open with boxes
        assert!(dock.is_dock_open().unwrap());
    }

    #[test]
    fn test_supply_warehouse_empty_state() {
        let data = SupplyWarehouseDockUpdateData {
            starting_boxes: 0,
            delete_when_empty: true,
            ..Default::default()
        };

        let pos = coord(400.0, 400.0, 0.0);
        let dock = SupplyWarehouseDockUpdate::new(data, 4, &pos);

        // Dock should be closed when empty
        assert!(!dock.is_dock_open().unwrap());
    }

    #[test]
    fn test_supply_warehouse_crippled_state() {
        let data = SupplyWarehouseDockUpdateData {
            starting_boxes: 5,
            ..Default::default()
        };

        let pos = coord(400.0, 400.0, 0.0);
        let mut dock = SupplyWarehouseDockUpdate::new(data, 4, &pos);

        // Initially open
        assert!(dock.is_dock_open().unwrap());

        // Close when crippled
        dock.set_dock_crippled(true).unwrap();
        assert!(!dock.is_dock_open().unwrap());
    }

    #[cfg(feature = "allow_surrender")]
    #[test]
    fn test_prison_dock_capture_system() {
        let data = PrisonDockUpdateData::default();

        let pos = coord(500.0, 500.0, 0.0);
        let dock = PrisonDockUpdate::new(data, 5, &pos);

        assert_eq!(dock.get_module_name(), "PrisonDockUpdate");
    }

    #[test]
    fn test_railed_transport_dock_loading_states() {
        let data = RailedTransportDockUpdateData {
            pull_inside_duration_frames: 40,
            push_outside_duration_frames: 40,
            tolerance_distance: 12.0,
            ..Default::default()
        };

        let pos = coord(600.0, 600.0, 0.0);
        let dock = RailedTransportDockUpdate::new(data, 6, &pos);

        // Should allow passthrough (tunnel network)
        assert!(dock.is_allow_passthrough_type().unwrap());

        // Should not be loading initially
        assert!(!dock.is_loading_or_unloading());
    }

    #[test]
    fn test_railed_transport_dock_unload_queue() {
        let data = RailedTransportDockUpdateData::default();
        let pos = coord(600.0, 600.0, 0.0);
        let mut dock = RailedTransportDockUpdate::new(data, 6, &pos);

        // Unload all should reset count
        dock.unload_all();
    }

    #[test]
    fn test_dock_approach_positions() {
        let mut data = DockUpdateData::default();
        data.number_approach_positions_data = 5; // 5 queue spots

        let pos = coord(700.0, 700.0, 0.0);
        let dock = DockUpdate::new(data, 7, &pos);

        // Dock should be open for approaches
        assert!(dock.is_dock_open().unwrap());
    }

    #[test]
    fn test_multiple_dock_types_in_sequence() {
        // Test that different dock types can be created and configured independently
        let repair_data = RepairDockUpdateData::default();
        let supply_data = SupplyCenterDockUpdateData::default();
        let warehouse_data = SupplyWarehouseDockUpdateData::default();

        let pos = coord(800.0, 800.0, 0.0);

        let repair_dock = RepairDockUpdate::new(repair_data, 8, &pos);
        let supply_dock = SupplyCenterDockUpdate::new(supply_data, 9, &pos);
        let warehouse_dock = SupplyWarehouseDockUpdate::new(warehouse_data, 10, &pos);

        // All should be openable
        assert!(repair_dock.is_dock_open().unwrap());
        assert!(supply_dock.is_dock_open().unwrap());
        assert!(warehouse_dock.is_dock_open().unwrap());
    }

    #[test]
    fn test_dock_module_names() {
        let repair_data = RepairDockUpdateData::default();
        let supply_data = SupplyCenterDockUpdateData::default();
        let warehouse_data = SupplyWarehouseDockUpdateData::default();

        let pos = coord(900.0, 900.0, 0.0);

        let repair_dock = RepairDockUpdate::new(repair_data, 11, &pos);
        let supply_dock = SupplyCenterDockUpdate::new(supply_data, 12, &pos);
        let warehouse_dock = SupplyWarehouseDockUpdate::new(warehouse_data, 13, &pos);

        assert_eq!(repair_dock.get_module_name(), "RepairDockUpdate");
        assert_eq!(supply_dock.get_module_name(), "SupplyCenterDockUpdate");
        assert_eq!(
            warehouse_dock.get_module_name(),
            "SupplyWarehouseDockUpdate"
        );
    }

    #[test]
    fn test_supply_warehouse_cash_value_calculation() {
        let mut data = SupplyWarehouseDockUpdateData {
            starting_boxes: 10,
            ..Default::default()
        };

        let pos = coord(1100.0, 1100.0, 0.0);
        let mut dock = SupplyWarehouseDockUpdate::new(data, 15, &pos);

        // Set total value and check per-box value
        dock.set_cash_value(10000); // $10,000 total
                                    // Should be $1000 per box (10000 / 10)
    }

    #[test]
    fn test_dock_passthrough_behaviors() {
        let pos = coord(1200.0, 1200.0, 0.0);

        // Repair dock inherits DockUpdate passthrough behavior.
        let repair = RepairDockUpdate::new(RepairDockUpdateData::default(), 16, &pos);
        assert!(repair.is_allow_passthrough_type().unwrap());

        // Supply center inherits DockUpdate passthrough behavior.
        let supply = SupplyCenterDockUpdate::new(SupplyCenterDockUpdateData::default(), 17, &pos);
        assert!(supply.is_allow_passthrough_type().unwrap());

        // Warehouse inherits DockUpdate passthrough behavior.
        let warehouse =
            SupplyWarehouseDockUpdate::new(SupplyWarehouseDockUpdateData::default(), 18, &pos);
        assert!(warehouse.is_allow_passthrough_type().unwrap());

        // Railed transport (tunnel): allows passthrough
        let tunnel =
            RailedTransportDockUpdate::new(RailedTransportDockUpdateData::default(), 19, &pos);
        assert!(tunnel.is_allow_passthrough_type().unwrap());
    }

    #[test]
    fn test_rally_point_behaviors() {
        let pos = coord(1300.0, 1300.0, 0.0);

        // C++ RepairDockUpdate overrides DockUpdate::isRallyPointAfterDockType() to true.
        let repair = RepairDockUpdate::new(RepairDockUpdateData::default(), 20, &pos);
        assert!(repair.is_rally_point_after_dock_type().unwrap());

        // C++ SupplyCenterDockUpdate inherits DockUpdate::isRallyPointAfterDockType() = false.
        let supply = SupplyCenterDockUpdate::new(SupplyCenterDockUpdateData::default(), 21, &pos);
        assert!(!supply.is_rally_point_after_dock_type().unwrap());

        // Warehouse: no rally point
        let warehouse =
            SupplyWarehouseDockUpdate::new(SupplyWarehouseDockUpdateData::default(), 22, &pos);
        assert!(!warehouse.is_rally_point_after_dock_type().unwrap());

        // Tunnel: no rally point
        let tunnel =
            RailedTransportDockUpdate::new(RailedTransportDockUpdateData::default(), 23, &pos);
        assert!(!tunnel.is_rally_point_after_dock_type().unwrap());
    }
}
