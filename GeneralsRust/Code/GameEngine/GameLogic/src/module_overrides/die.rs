//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Die-module factory wrappers including DamDie (C++ Die modules).
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

fn upgrade_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = UpgradeDieModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse UpgradeDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn upgrade_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<UpgradeDieModuleData>()
        .expect("UpgradeDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("UpgradeDie requires owning object");

    let die_module = UpgradeDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("UpgradeDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DieModuleData at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn destroy_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DieModuleData>()
        .expect("DieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("DestroyDie requires owning object");

    let die_module = DestroyDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("DestroyDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn keep_object_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DieModuleData>()
        .expect("DieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("KeepObjectDie requires owning object");

    let die_module = KeepObjectDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("KeepObjectDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn create_object_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CreateObjectDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CreateObjectDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn create_object_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CreateObjectDieModuleData>()
        .expect("CreateObjectDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("CreateObjectDie requires owning object");

    let die_module = CreateObjectDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("CreateObjectDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn create_crate_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CreateCrateDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CreateCrateDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn create_crate_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CreateCrateDieModuleData>()
        .expect("CreateCrateDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("CreateCrateDie requires owning object");

    let die_module = CreateCrateDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("CreateCrateDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn fx_list_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FXListDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FXListDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn fx_list_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FXListDieModuleData>()
        .expect("FXListDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("FXListDie requires owning object");

    let die_module = FXListDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("FXListDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn crush_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CrushDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CrushDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn crush_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CrushDieModuleData>()
        .expect("CrushDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("CrushDie requires owning object");

    let die_module = CrushDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("CrushDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn eject_pilot_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = EjectPilotDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse EjectPilotDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn eject_pilot_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<EjectPilotDieModuleData>()
        .expect("EjectPilotDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("EjectPilotDie requires owning object");

    let die_module = EjectPilotDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("EjectPilotDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn rebuild_hole_expose_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RebuildHoleExposeDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RebuildHoleExposeDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn rebuild_hole_expose_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RebuildHoleExposeDieModuleData>()
        .expect("RebuildHoleExposeDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("RebuildHoleExposeDie requires owning object");

    let die_module = RebuildHoleExposeDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("RebuildHoleExposeDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn special_power_completion_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpecialPowerCompletionDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpecialPowerCompletionDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn special_power_completion_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpecialPowerCompletionDieModuleData>()
        .expect("SpecialPowerCompletionDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("SpecialPowerCompletionDie requires owning object");

    let die_module = SpecialPowerCompletionDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("SpecialPowerCompletionDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

fn dam_die_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DamDieModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DamDie module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn dam_die_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DamDieModuleData>()
        .expect("DamDieModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let die_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id).expect("DamDie requires owning object");

    let die_module = DamDie::new(Arc::clone(&object), die_data_arc);
    let module_name = AsciiString::from("DamDie");
    let module_data_trait: Arc<dyn ModuleData> = module_data_arc;

    Box::new(DieModuleWrapper::new(
        &module_name,
        module_data_trait,
        object,
        Box::new(die_module),
    ))
}

