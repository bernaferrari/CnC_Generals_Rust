//! Production-exit and dock-update module factories.
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::*;
use super::helpers::*;

pub(super) fn default_production_exit_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<DefaultProductionExitModuleData>(
        "DefaultProductionExitUpdate",
        &module_data,
    );
    let behavior = DefaultProductionExitBehavior::from_module_thing(thing, data_arc.clone())
        .expect("DefaultProductionExitUpdate requires an owning object");
    Box::new(DefaultProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("DefaultProductionExitUpdate"),
        data_arc,
    ))
}

pub(super) fn queue_production_exit_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<QueueProductionExitModuleData>(
        "QueueProductionExitUpdate",
        &module_data,
    );
    let behavior = QueueProductionExitBehavior::from_module_thing(thing, data_arc.clone())
        .expect("QueueProductionExitUpdate requires an owning object");
    Box::new(QueueProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("QueueProductionExitUpdate"),
        data_arc,
    ))
}

pub(super) fn spawn_point_production_exit_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SpawnPointProductionExitModuleData>(
        "SpawnPointProductionExitUpdate",
        &module_data,
    );
    let behavior = SpawnPointProductionExitBehavior::from_module_thing(thing, data_arc.clone())
        .expect("SpawnPointProductionExitUpdate requires an owning object");
    Box::new(SpawnPointProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("SpawnPointProductionExitUpdate"),
        data_arc,
    ))
}

pub(super) fn supply_center_production_exit_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SupplyCenterProductionExitModuleData>(
        "SupplyCenterProductionExitUpdate",
        &module_data,
    );
    let behavior = SupplyCenterProductionExitBehavior::from_module_thing(thing, data_arc.clone())
        .expect("SupplyCenterProductionExitUpdate requires an owning object");
    Box::new(SupplyCenterProductionExitBehaviorModule::new(
        behavior,
        &AsciiString::from("SupplyCenterProductionExitUpdate"),
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<FlightDeckBehaviorModuleData>("FlightDeckBehavior", &module_data);
    let behavior = FlightDeckBehavior::from_module_thing(thing, data_arc.clone())
        .expect("FlightDeckBehavior requires an owning object");
    Box::new(FlightDeckBehaviorModule::new(
        behavior,
        &AsciiString::from("FlightDeckBehavior"),
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<ProductionUpdateModuleData>("ProductionUpdate", &module_data);
    let module_name = AsciiString::from("ProductionUpdate");
    let owner_id = resolve_owner_id(&thing);
    Box::new(ProductionUpdateCompleteModule::new(
        &module_name,
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<RepairDockUpdateData>("RepairDockUpdate", &module_data);
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = RepairDockUpdate::new((*data_arc).clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("RepairDockUpdate");
    Box::new(RepairDockUpdateModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

pub(super) fn railed_transport_dock_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<RailedTransportDockUpdateData>(
        "RailedTransportDockUpdate",
        &module_data,
    );
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = RailedTransportDockUpdate::new((*data_arc).clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("RailedTransportDockUpdate");
    Box::new(RailedTransportDockUpdateModule::new(
        behavior,
        &module_name,
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<SupplyCenterDockUpdateData>("SupplyCenterDockUpdate", &module_data);
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = SupplyCenterDockUpdate::new((*data_arc).clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("SupplyCenterDockUpdate");
    Box::new(SupplyCenterDockUpdateModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

pub(super) fn supply_warehouse_dock_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SupplyWarehouseDockUpdateData>(
        "SupplyWarehouseDockUpdate",
        &module_data,
    );
    let (owner_id, owner_pos) = resolve_owner_info(&thing);
    let behavior = SupplyWarehouseDockUpdate::new((*data_arc).clone(), owner_id, &owner_pos);
    let module_name = AsciiString::from("SupplyWarehouseDockUpdate");
    Box::new(SupplyWarehouseDockUpdateModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

active_behavior_factories!(
    supply_warehouse_crippling_behavior_data_factory,
    supply_warehouse_crippling_behavior_module_factory,
    SupplyWarehouseCripplingBehaviorModuleData,
    SupplyWarehouseCripplingBehavior,
    "SupplyWarehouseCripplingBehavior"
);
