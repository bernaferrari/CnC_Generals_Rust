//! Common ActionManager API-parity tests for the three C++ methods that were
//! missing from the handle-wrapper port (canBribeUnit / canCutBuildingPower /
//! canOverrideSpecialPowerDestination), plus ObjectDataProvider install tests.
//!
//! `game_engine` disables lib tests (`[lib] test = false`), so these live here.

use game_engine::common::rts::action_manager::{
    ActionManager, CommandSourceType, Coord3D, Object, ObjectDataProvider, ObjectShroudStatus,
    PlayerType, Relationship, SpecialPowerType, clear_object_data_provider, kind_of_bit,
    object_data_provider_is_set, set_object_data_provider,
};
use game_engine::common::rts::handles::ObjectHandle;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

fn provider_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct MockObjectDataProvider;

impl ObjectDataProvider for MockObjectDataProvider {
    fn is_valid_object(&self, id: ObjectHandle) -> bool {
        id.value() == 42 || id.value() == 43
    }
    fn get_relationship(&self, _source: ObjectHandle, _target: ObjectHandle) -> Relationship {
        Relationship::Allies
    }
    fn get_team_relationship(&self, _source: ObjectHandle, _player_id: u32) -> Relationship {
        Relationship::Allies
    }
    fn is_effectively_dead(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn is_mobile(&self, _id: ObjectHandle) -> bool {
        true
    }
    fn test_status(&self, _id: ObjectHandle, _status_bit: u32) -> bool {
        false
    }
    fn is_kind_of(&self, id: ObjectHandle, kind_of: u32) -> bool {
        match (id.value(), kind_of) {
            (42, kind_of_bit::VEHICLE) => true,
            (43, kind_of_bit::REPAIR_PAD) => true,
            _ => false,
        }
    }
    fn is_above_terrain(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn get_health(&self, _id: ObjectHandle) -> f32 {
        50.0
    }
    fn get_max_health(&self, _id: ObjectHandle) -> f32 {
        100.0
    }
    fn get_controlling_player_id(&self, _id: ObjectHandle) -> Option<u32> {
        Some(0)
    }
    fn get_controlling_player_type(&self, _id: ObjectHandle) -> PlayerType {
        PlayerType::Computer
    }
    fn get_shrouded_status(
        &self,
        _target: ObjectHandle,
        _viewer_player_id: u32,
    ) -> ObjectShroudStatus {
        ObjectShroudStatus::Clear
    }
    fn has_supply_truck_ai(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn is_supply_warehouse(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn is_supply_center(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn get_warehouse_boxes(&self, _id: ObjectHandle) -> u32 {
        0
    }
    fn get_supply_boxes(&self, _id: ObjectHandle) -> u32 {
        0
    }
    fn is_available_for_supplying(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn has_dock_update_interface(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn is_railed_transport_dock(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn is_contained(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn is_surrendered(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn get_surrendered_player_index(&self, _id: ObjectHandle) -> Option<u32> {
        None
    }
    fn has_contain_module(&self, _id: ObjectHandle) -> bool {
        false
    }
    fn get_apparent_controlling_player(
        &self,
        _id: ObjectHandle,
        _viewer_player_id: u32,
    ) -> Option<u32> {
        None
    }
}

#[test]
fn can_bribe_unit_always_returns_false() {
    let manager = ActionManager::new();
    let obj = Object::from_id(1);
    let target = Object::from_id(2);
    assert!(!manager.can_bribe_unit(&obj, &target, CommandSourceType::FromPlayer));
    assert!(!manager.can_bribe_unit(&obj, &target, CommandSourceType::FromScript));
    assert!(!manager.can_bribe_unit(&obj, &target, CommandSourceType::FromAi));
}

#[test]
fn can_cut_building_power_always_returns_false() {
    let manager = ActionManager::new();
    let obj = Object::from_id(3);
    let building = Object::from_id(4);
    assert!(!manager.can_cut_building_power(&obj, &building, CommandSourceType::FromPlayer));
    assert!(!manager.can_cut_building_power(&obj, &building, CommandSourceType::FromAi));
    assert!(!manager.can_cut_building_power(&obj, &building, CommandSourceType::FromScript));
}

#[test]
fn can_override_special_power_destination_false_without_overridable_interface() {
    let manager = ActionManager::new();
    let obj = Object::from_id(5);
    let loc = Coord3D {
        x: 100.0,
        y: 100.0,
        z: 0.0,
    };
    // Common Object is a handle wrapper: no SpecialPowerUpdateInterface and no
    // PartitionManager, so the C++ shroud check cannot be performed honestly.
    assert!(!manager.can_override_special_power_destination(
        &obj,
        &loc,
        SpecialPowerType::None,
        CommandSourceType::FromPlayer,
    ));
    assert!(!manager.can_override_special_power_destination(
        &obj,
        &loc,
        SpecialPowerType::InfantryCaptureBuilding,
        CommandSourceType::FromScript,
    ));
}

#[test]
fn can_override_special_power_destination_false_for_null_object() {
    let manager = ActionManager::new();
    // Without an ObjectDataProvider, Object::is_null() is true (fail-closed).
    let obj = Object::from_id(0);
    let loc = Coord3D {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    assert!(!manager.can_override_special_power_destination(
        &obj,
        &loc,
        SpecialPowerType::None,
        CommandSourceType::FromPlayer,
    ));
}

#[test]
fn unset_provider_fail_closed_defaults_block_repair() {
    let _guard = provider_test_lock();
    clear_object_data_provider();
    assert!(!object_data_provider_is_set());

    let manager = ActionManager::new();
    let obj = Object::from_id(42);
    let pad = Object::from_id(43);
    // No provider → is_null true (fail-closed), so repair is refused.
    assert!(!manager.can_get_repaired_at(&obj, &pad, CommandSourceType::FromPlayer));
}

#[test]
fn installed_provider_queries_reach_mock() {
    let _guard = provider_test_lock();
    clear_object_data_provider();
    set_object_data_provider(Arc::new(MockObjectDataProvider));
    assert!(object_data_provider_is_set());

    let manager = ActionManager::new();
    let obj = Object::from_id(42);
    let pad = Object::from_id(43);
    // Mock returns live allied vehicle + repair pad with missing health.
    assert!(manager.can_get_repaired_at(&obj, &pad, CommandSourceType::FromPlayer));
    // Unknown handle is invalid via the mock → still fail-closed.
    assert!(!manager.can_get_repaired_at(
        &Object::from_id(99),
        &pad,
        CommandSourceType::FromPlayer
    ));

    clear_object_data_provider();
    assert!(!object_data_provider_is_set());
    assert!(!manager.can_get_repaired_at(&obj, &pad, CommandSourceType::FromPlayer));
}
