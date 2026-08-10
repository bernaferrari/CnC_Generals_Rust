#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::system::xfer_load::XferLoad;
    use game_engine::system::xfer_save::XferSave;
    use std::io::Cursor;

    #[test]
    fn supply_truck_xfer_preserves_cpp_saved_fields() {
        let data = SupplyTruckAIUpdateData {
            max_boxes: 4,
            ..Default::default()
        };
        let mut original = SupplyTruckAIUpdate::new(data.clone(), 42, 3);
        original.state = SupplyTruckState::Docking;
        original.number_boxes = 2;
        original.preferred_dock = Some(9001);
        original.force_wanting_state = true;
        original.force_busy_state = true;

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            original.xfer(&mut save).unwrap();
        }

        let mut loaded = SupplyTruckAIUpdate::new(data, 42, 3);
        loaded.force_busy_state = false;
        {
            let cursor = Cursor::new(bytes.as_slice());
            let mut load = XferLoad::new(cursor, 1);
            loaded.xfer(&mut load).unwrap();
        }

        assert_eq!(loaded.state, SupplyTruckState::Docking);
        assert_eq!(loaded.number_boxes, 2);
        assert_eq!(loaded.preferred_dock, Some(9001));
        assert!(loaded.force_wanting_state);
        assert!(!loaded.force_busy_state);
    }

    #[test]
    fn resource_gathering_manager_xfer_preserves_supply_ids() {
        let mut original = ResourceGatheringManager::new();
        original.add_supply_warehouse(101);
        original.add_supply_warehouse(102);
        original.add_supply_center(201);

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            original.xfer(&mut save).unwrap();
        }

        let mut loaded = ResourceGatheringManager::new();
        {
            let cursor = Cursor::new(bytes.as_slice());
            let mut load = XferLoad::new(cursor, 1);
            loaded.xfer(&mut load).unwrap();
        }

        assert_eq!(loaded.get_supply_warehouses(), &[101, 102]);
        assert_eq!(loaded.get_supply_centers(), &[201]);
    }
}
