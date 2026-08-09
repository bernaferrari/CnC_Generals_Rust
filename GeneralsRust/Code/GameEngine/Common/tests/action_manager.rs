#![cfg(feature = "internal")]
//! Common ActionManager API-parity tests for the three C++ methods that were
//! missing from the handle-wrapper port (canBribeUnit / canCutBuildingPower /
//! canOverrideSpecialPowerDestination).
//!
//! `game_engine` disables lib tests (`[lib] test = false`), so these live here.

use game_engine::common::rts::action_manager::{
    ActionManager, CommandSourceType, Coord3D, Object, SpecialPowerType,
};

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
