//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! AI / ProductionUpdate / SpecialPowerModule factory wrappers.
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

fn ai_update_interface_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AIUpdateInterface module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn ai_update_interface_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AIUpdateModuleData>()
        .expect("AIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("AIUpdateInterface");
    Box::new(AIUpdateInterfaceModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn railed_transport_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RailedTransportAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RailedTransportAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn railed_transport_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RailedTransportAIUpdateModuleData>()
        .expect("RailedTransportAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("RailedTransportAIUpdate");
    Box::new(RailedTransportAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn railroad_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RailroadBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RailroadBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn railroad_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RailroadBehaviorModuleData>()
        .expect("RailroadBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("RailroadBehavior");
    let object_id = thing
        .as_object()
        .map(|obj| obj.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("RailroadBehavior requires valid object handle");
    Box::new(
        RailroadBehaviorModule::new(module_name_key, module_data_arc, object)
            .expect("Failed to create RailroadBehaviorModule"),
    )
}

fn special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpecialPowerModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpecialPowerModule data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpecialPowerModuleData>()
        .expect("SpecialPowerModuleData expected");

    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    let module = SpecialPowerModule::new(owner_id, typed_data.clone());
    Box::new(module)
}

fn production_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ProductionUpdateCompleteModuleData::default();

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

fn production_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ProductionUpdateCompleteModuleData>()
        .expect("ProductionUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name = AsciiString::from("ProductionUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    Box::new(ProductionUpdateCompleteModule::new(
        &module_name,
        module_data_arc,
        owner_id,
    ))
}

fn assault_transport_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AssaultTransportAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AssaultTransportAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn transport_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TransportAIUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TransportAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn transport_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<TransportAIUpdateModuleData>()
        .expect("TransportAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("TransportAIUpdate");
    Box::new(TransportAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn assault_transport_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AssaultTransportAIUpdateModuleData>()
        .expect("AssaultTransportAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("AssaultTransportAIUpdate");
    Box::new(AssaultTransportAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn deliver_payload_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DeliverPayloadAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DeliverPayloadAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn deploy_style_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DeployStyleAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DeployStyleAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn deploy_style_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DeployStyleAIUpdateModuleData>()
        .expect("DeployStyleAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("DeployStyleAIUpdate");
    Box::new(DeployStyleAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn wander_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WanderAIUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WanderAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn wander_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WanderAIUpdateModuleData>()
        .expect("WanderAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("WanderAIUpdate");
    Box::new(WanderAIUpdateModule::new(module_name_key, module_data_arc))
}

fn deliver_payload_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DeliverPayloadAIUpdateModuleData>()
        .expect("DeliverPayloadAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("DeliverPayloadAIUpdate");
    Box::new(DeliverPayloadAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn hack_internet_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HackInternetAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HackInternetAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn hack_internet_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HackInternetAIUpdateModuleData>()
        .expect("HackInternetAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("HackInternetAIUpdate");
    Box::new(HackInternetAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn supply_truck_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SupplyTruckAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SupplyTruckAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn supply_truck_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SupplyTruckAIUpdateModuleData>()
        .expect("SupplyTruckAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("SupplyTruckAIUpdate");
    Box::new(SupplyTruckAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}

fn chinook_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ChinookAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ChinookAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn chinook_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ChinookAIUpdateModuleData>()
        .expect("ChinookAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("ChinookAIUpdate");
    Box::new(ChinookAIUpdateModule::new(module_name_key, module_data_arc))
}

fn jet_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = JetAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse JetAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn jet_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<JetAIUpdateModuleData>()
        .expect("JetAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("JetAIUpdate");
    Box::new(JetAIUpdateModule::new(module_name_key, module_data_arc))
}

fn worker_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WorkerAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WorkerAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn worker_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WorkerAIUpdateModuleData>()
        .expect("WorkerAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("WorkerAIUpdate");
    Box::new(WorkerAIUpdateModule::new(module_name_key, module_data_arc))
}

fn dozer_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DozerAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DozerAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn dozer_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DozerAIUpdateModuleData>()
        .expect("DozerAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("DozerAIUpdate");
    Box::new(DozerAIUpdateModule::new(module_name_key, module_data_arc))
}

#[cfg(feature = "allow_surrender")]
fn pow_truck_ai_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = POWTruckAIUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse POWTruckAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

#[cfg(feature = "allow_surrender")]
fn pow_truck_ai_update_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<POWTruckAIUpdateModuleData>()
        .expect("POWTruckAIUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("POWTruckAIUpdate");
    Box::new(POWTruckAIUpdateModule::new(
        module_name_key,
        module_data_arc,
    ))
}
