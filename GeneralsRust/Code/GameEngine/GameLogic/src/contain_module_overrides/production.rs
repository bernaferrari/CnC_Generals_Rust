//! Production-exit and dock-update module factories.
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::helpers::*;
use super::*;

pub(super) fn default_production_exit_update_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = DefaultProductionExitModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DefaultProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn default_production_exit_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let behavior = DefaultProductionExitBehavior::from_module_thing(thing, module_data.clone())
        .expect("DefaultProductionExitUpdate requires an owning object");
    Box::new(DefaultProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("DefaultProductionExitUpdate"),
        module_data,
    ))
}

pub(super) fn queue_production_exit_update_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = QueueProductionExitModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse QueueProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn queue_production_exit_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let behavior = QueueProductionExitBehavior::from_module_thing(thing, module_data.clone())
        .expect("QueueProductionExitUpdate requires an owning object");
    Box::new(QueueProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("QueueProductionExitUpdate"),
        module_data,
    ))
}

pub(super) fn spawn_point_production_exit_update_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SpawnPointProductionExitModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpawnPointProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn spawn_point_production_exit_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let behavior = SpawnPointProductionExitBehavior::from_module_thing(thing, module_data.clone())
        .expect("SpawnPointProductionExitUpdate requires an owning object");
    Box::new(SpawnPointProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("SpawnPointProductionExitUpdate"),
        module_data,
    ))
}

pub(super) fn supply_center_production_exit_update_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SupplyCenterProductionExitModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SupplyCenterProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn supply_center_production_exit_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let behavior = SupplyCenterProductionExitBehavior::from_module_thing(thing, module_data.clone())
        .expect("SupplyCenterProductionExitUpdate requires an owning object");
    Box::new(SupplyCenterProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("SupplyCenterProductionExitUpdate"),
        module_data,
    ))
}

pub(super) fn flight_deck_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FlightDeckBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FlightDeckBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn flight_deck_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let behavior = FlightDeckBehavior::from_module_thing(thing, module_data.clone())
        .expect("FlightDeckBehavior requires an owning object");
    Box::new(FlightDeckBehaviorModule::new(
        behavior,
        &AsciiString::from("FlightDeckBehavior"),
        module_data,
    ))
}

pub(super) fn production_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ProductionUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ProductionUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn production_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.read().as_any().is::<ProductionUpdateModuleData>(), "ProductionUpdate module data type expected");
    let module_name = AsciiString::from("ProductionUpdate");
    let owner_id = resolve_owner_id(&thing);
    Box::new(ProductionUpdateCompleteModule::new(
        &module_name,
        module_data,
        owner_id,
    ))
}

pub(super) fn repair_dock_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RepairDockUpdateData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RepairDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn repair_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = RepairDockUpdate::from_shared(module_data.clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("RepairDockUpdate");
    Box::new(RepairDockUpdateModule::new(
        behavior,
        &module_name,
        module_data,
    ))
}

pub(super) fn railed_transport_dock_update_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = RailedTransportDockUpdateData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RailedTransportDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn railed_transport_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = RailedTransportDockUpdate::from_shared(module_data.clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("RailedTransportDockUpdate");
    Box::new(RailedTransportDockUpdateModule::new(
        behavior,
        &module_name,
        module_data,
    ))
}

pub(super) fn supply_center_dock_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SupplyCenterDockUpdateData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SupplyCenterDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn supply_center_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = SupplyCenterDockUpdate::from_shared(module_data.clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("SupplyCenterDockUpdate");
    Box::new(SupplyCenterDockUpdateModule::new(
        behavior,
        &module_name,
        module_data,
    ))
}

pub(super) fn supply_warehouse_dock_update_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SupplyWarehouseDockUpdateData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SupplyWarehouseDockUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn supply_warehouse_dock_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = SupplyWarehouseDockUpdate::from_shared(module_data.clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("SupplyWarehouseDockUpdate");
    Box::new(SupplyWarehouseDockUpdateModule::new(
        behavior,
        &module_name,
        module_data,
    ))
}

active_behavior_factories!(
    supply_warehouse_crippling_behavior_data_factory,
    supply_warehouse_crippling_behavior_module_factory,
    SupplyWarehouseCripplingBehaviorModuleData,
    SupplyWarehouseCripplingBehavior,
    "SupplyWarehouseCripplingBehavior"
);
