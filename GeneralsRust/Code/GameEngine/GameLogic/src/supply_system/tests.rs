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
    fn supply_truck_reads_both_shared_definition_types_after_xfer() {
        use crate::object::update::ai_update::chinook_ai_update::ChinookAIUpdateModuleData;
        use crate::object::update::ai_update::supply_truck_ai_update::SupplyTruckAIUpdateModuleData;
        use game_engine::common::thing::module::ModuleDataHandle;

        let mut chinook = ChinookAIUpdateModuleData::default();
        chinook.max_boxes_data = 5;
        chinook.center_delay = 17;
        chinook.warehouse_scan_distance = 48.0;
        let mut supply = SupplyTruckAIUpdateModuleData::default();
        supply.max_boxes_data = 7;
        supply.warehouse_delay = 23;
        supply.warehouse_scan_distance = 64.0;

        for (handle, mut changed, capacity, delay, distance, warehouse) in [
            (ModuleDataHandle::new(ChinookAIUpdateModuleData::default()), Box::new(chinook) as Box<dyn game_engine::common::thing::module::ModuleData>, 5, 17, 48.0, false),
            (ModuleDataHandle::new(SupplyTruckAIUpdateModuleData::default()), Box::new(supply) as Box<dyn game_engine::common::thing::module::ModuleData>, 7, 23, 64.0, true),
        ] {
            let truck = SupplyTruckAIUpdate::new_with_module_data(handle.clone(), 42, 3);
            let peer = SupplyTruckAIUpdate::new_with_module_data(handle.clone(), 43, 3);
            let separate = SupplyTruckAIUpdate::new(SupplyTruckAIUpdateData::default(), 44, 3);
            assert_eq!(truck.get_max_boxes(), 0);

            let mut bytes = Vec::new();
            changed.xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1)).unwrap();
            handle.write().xfer(&mut XferLoad::new(Cursor::new(bytes.as_slice()), 1)).unwrap();

            for owner in [&truck, &peer] {
                assert_eq!(owner.get_max_boxes(), capacity);
                assert_eq!(owner.get_action_delay_for_dock(warehouse), delay);
                assert_eq!(owner.get_warehouse_scan_distance(false), distance);
            }
            assert_eq!(separate.get_max_boxes(), 0);
        }
    }

    #[test]
    fn worker_snapshot_updates_shared_definition_and_not_independent_owner() {
        use crate::object::update::ai_update::worker_ai_update::WorkerAIUpdateModuleData;
        use game_engine::common::thing::module::ModuleDataHandle;

        let mut source_data = WorkerAIUpdateModuleData::default();
        source_data.bored_time = 3.5;
        source_data.center_delay = 12;
        source_data.warehouse_scan_distance = 90.0;
        let source = ModuleDataHandle::new(source_data);
        let target = ModuleDataHandle::new(WorkerAIUpdateModuleData::default());
        let independent = ModuleDataHandle::new(WorkerAIUpdateModuleData::default());
        let mut writer = WorkerAIUpdate::new_with_module_data(source, 1, 0);
        let mut reader = WorkerAIUpdate::new_with_module_data(target.clone(), 2, 0);
        let peer = WorkerAIUpdate::new_with_module_data(target, 3, 0);
        let other = WorkerAIUpdate::new_with_module_data(independent, 4, 0);

        let mut bytes = Vec::new();
        writer.xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1)).unwrap();
        reader.xfer(&mut XferLoad::new(Cursor::new(bytes.as_slice()), 1)).unwrap();

        for worker in [&reader, &peer] {
            assert_eq!(worker.get_bored_time(), 3.5);
            assert_eq!(worker.get_action_delay_for_dock(false), 12);
            assert_eq!(worker.get_warehouse_scan_distance(true), 180.0);
        }
        assert_eq!(other.get_bored_time(), 0.0);
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
