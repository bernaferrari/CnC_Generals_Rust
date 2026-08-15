//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! BattleBus, bridge, parking-place, and dock factories.
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

fn battle_bus_slow_death_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    battle_bus_slow_death_data_factory(ini)
}

fn battle_bus_slow_death_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    battle_bus_slow_death_module_factory(thing, module_data)
}

fn bridge_scaffold_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BridgeScaffoldBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BridgeScaffoldBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn bridge_scaffold_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BridgeScaffoldBehaviorModuleData>()
        .expect("BridgeScaffoldBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        BridgeScaffoldBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("BridgeScaffoldBehavior requires an owning object");

    let module_name = AsciiString::from("BridgeScaffoldBehavior");
    Box::new(BridgeScaffoldBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn bridge_tower_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BridgeTowerBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BridgeTowerBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn bridge_tower_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BridgeTowerBehaviorModuleData>()
        .expect("BridgeTowerBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        BridgeTowerBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("BridgeTowerBehavior requires an owning object");

    let module_name = AsciiString::from("BridgeTowerBehavior");
    Box::new(BridgeTowerBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn bridge_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BridgeBehaviorModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse BridgeBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn bridge_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BridgeBehaviorModuleData>()
        .expect("BridgeBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = BridgeBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
        .expect("BridgeBehavior requires an owning object");

    let module_name = AsciiString::from("BridgeBehavior");
    Box::new(BridgeBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn parking_place_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ParkingPlaceBehaviorModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse ParkingPlaceBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn parking_place_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ParkingPlaceBehaviorModuleData>()
        .expect("ParkingPlaceBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("ParkingPlaceBehavior requires owning object");
    let behavior = ParkingPlaceBehavior::new(object, module_data_arc.clone())
        .expect("Failed to create ParkingPlaceBehavior");

    let module_name = AsciiString::from("ParkingPlaceBehavior");
    Box::new(ParkingPlaceBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn repair_dock_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RepairDockUpdateData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse RepairDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn repair_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RepairDockUpdateData>()
        .expect("RepairDockUpdateData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = RepairDockUpdate::new(typed_data.clone(), owner_id, &owner_pos);

    let module_name = AsciiString::from("RepairDockUpdate");
    Box::new(RepairDockUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

#[cfg(feature = "allow_surrender")]
fn prison_dock_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(PrisonDockUpdateData::default())
}

#[cfg(feature = "allow_surrender")]
fn prison_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PrisonDockUpdateData>()
        .expect("PrisonDockUpdateData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = PrisonDockUpdate::new(typed_data.clone(), owner_id, &owner_pos);

    let module_name = AsciiString::from("PrisonDockUpdate");
    Box::new(PrisonDockUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn railed_transport_dock_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RailedTransportDockUpdateData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse RailedTransportDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn railed_transport_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_data_arc = cloned_module_data::<RailedTransportDockUpdateData>(
        "RailedTransportDockUpdate",
        &module_data,
    );
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = RailedTransportDockUpdate::new((*module_data_arc).clone(), owner_id, &owner_pos);

    let module_name = AsciiString::from("RailedTransportDockUpdate");
    Box::new(RailedTransportDockUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn supply_center_dock_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SupplyCenterDockUpdateData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse SupplyCenterDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn supply_center_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_data_arc =
        cloned_module_data::<SupplyCenterDockUpdateData>("SupplyCenterDockUpdate", &module_data);
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = SupplyCenterDockUpdate::new((*module_data_arc).clone(), owner_id, &owner_pos);

    let module_name = AsciiString::from("SupplyCenterDockUpdate");
    Box::new(SupplyCenterDockUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn supply_warehouse_dock_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SupplyWarehouseDockUpdateData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse SupplyWarehouseDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn supply_warehouse_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_data_arc = cloned_module_data::<SupplyWarehouseDockUpdateData>(
        "SupplyWarehouseDockUpdate",
        &module_data,
    );
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = SupplyWarehouseDockUpdate::new((*module_data_arc).clone(), owner_id, &owner_pos);

    let module_name = AsciiString::from("SupplyWarehouseDockUpdate");
    Box::new(SupplyWarehouseDockUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

