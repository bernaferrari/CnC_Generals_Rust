//! Die / slow-death module factories.
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::body::{parse_instant_death_behavior_data, parse_slow_death_behavior_data};
use super::helpers::*;
use super::*;

pub(super) fn parse_die_data(ini: &mut INI, data: &mut DieModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_upgrade_die_data(
    ini: &mut INI,
    data: &mut UpgradeDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_create_object_die_data(
    ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_create_crate_die_data(
    ini: &mut INI,
    data: &mut CreateCrateDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_fx_list_die_data(
    ini: &mut INI,
    data: &mut FXListDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_crush_die_data(
    ini: &mut INI,
    data: &mut CrushDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_eject_pilot_die_data(
    ini: &mut INI,
    data: &mut EjectPilotDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_rebuild_hole_expose_die_data(
    ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_special_power_completion_die_data(
    ini: &mut INI,
    data: &mut SpecialPowerCompletionDieModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_dam_die_data(ini: &mut INI, data: &mut DamDieModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn build_die_module<T>(
    module_name: &str,
    thing: Arc<dyn ModuleThing>,
    data: T,
    create_die: fn(Arc<RwLock<crate::object::Object>>, Arc<T>) -> Box<dyn DieModuleInterface>,
) -> Box<dyn Module>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
{
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module(module_name, Arc::new(data.clone()) as Arc<dyn ModuleData>);
    };
    let typed_data = Arc::new(data);
    let module_data: Arc<dyn ModuleData> = typed_data.clone();
    let die_module = create_die(Arc::clone(&object), typed_data);
    Box::new(DieModuleWrapper::new(
        &AsciiString::from(module_name),
        module_data,
        object,
        die_module,
    ))
}

macro_rules! die_factories {
    (
        $data_factory:ident,
        $module_factory:ident,
        $data_ty:ty,
        $module_name:literal,
        $die_ty:ty,
        $parse_data:expr
    ) => {
        pub(super) fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = $parse_data(ini, &mut data) {
                    warn!("Failed to parse {} module data: {}", $module_name, err);
                }
            }
            Box::new(data)
        }

        pub(super) fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let typed_data = cloned_module_data_or_default::<$data_ty>($module_name, &module_data);
            build_die_module(
                $module_name,
                thing,
                typed_data.as_ref().clone(),
                |object, data| Box::new(<$die_ty>::new(object, data)),
            )
        }
    };
}

die_factories!(
    destroy_die_module_data_factory,
    destroy_die_module_factory,
    DieModuleData,
    "DestroyDie",
    DestroyDie,
    parse_die_data
);
die_factories!(
    keep_object_die_module_data_factory,
    keep_object_die_module_factory,
    DieModuleData,
    "KeepObjectDie",
    KeepObjectDie,
    parse_die_data
);
die_factories!(
    upgrade_die_module_data_factory,
    upgrade_die_module_factory,
    UpgradeDieModuleData,
    "UpgradeDie",
    UpgradeDie,
    parse_upgrade_die_data
);
die_factories!(
    create_object_die_module_data_factory,
    create_object_die_module_factory,
    CreateObjectDieModuleData,
    "CreateObjectDie",
    CreateObjectDie,
    parse_create_object_die_data
);
die_factories!(
    create_crate_die_module_data_factory,
    create_crate_die_module_factory,
    CreateCrateDieModuleData,
    "CreateCrateDie",
    CreateCrateDie,
    parse_create_crate_die_data
);
die_factories!(
    fx_list_die_module_data_factory,
    fx_list_die_module_factory,
    FXListDieModuleData,
    "FXListDie",
    FXListDie,
    parse_fx_list_die_data
);
die_factories!(
    crush_die_module_data_factory,
    crush_die_module_factory,
    CrushDieModuleData,
    "CrushDie",
    CrushDie,
    parse_crush_die_data
);
die_factories!(
    eject_pilot_die_module_data_factory,
    eject_pilot_die_module_factory,
    EjectPilotDieModuleData,
    "EjectPilotDie",
    EjectPilotDie,
    parse_eject_pilot_die_data
);
die_factories!(
    rebuild_hole_expose_die_module_data_factory,
    rebuild_hole_expose_die_module_factory,
    RebuildHoleExposeDieModuleData,
    "RebuildHoleExposeDie",
    RebuildHoleExposeDie,
    parse_rebuild_hole_expose_die_data
);
die_factories!(
    special_power_completion_die_module_data_factory,
    special_power_completion_die_module_factory,
    SpecialPowerCompletionDieModuleData,
    "SpecialPowerCompletionDie",
    SpecialPowerCompletionDie,
    parse_special_power_completion_die_data
);
die_factories!(
    dam_die_module_data_factory,
    dam_die_module_factory,
    DamDieModuleData,
    "DamDie",
    DamDie,
    parse_dam_die_data
);
die_factories!(
    instant_death_behavior_module_data_factory,
    instant_death_behavior_module_factory,
    InstantDeathBehaviorModuleData,
    "InstantDeathBehavior",
    InstantDeathBehavior,
    parse_instant_death_behavior_data
);

pub(super) fn slow_death_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SlowDeathBehaviorModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = parse_slow_death_behavior_data(ini, &mut data) {
            warn!("Failed to parse SlowDeathBehavior module data: {}", err);
        }
    }
    Box::new(data)
}

pub(super) fn slow_death_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = cloned_module_data_or_else::<SlowDeathBehaviorModuleData, _>(
        "SlowDeathBehavior",
        &module_data,
        SlowDeathBehaviorModuleData::new,
    );
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        let data_for_missing: Arc<dyn ModuleData> = typed_data;
        return missing_owner_module("SlowDeathBehavior", data_for_missing);
    };
    let data: Arc<dyn crate::common::ModuleData> = typed_data;
    Box::new(
        SlowDeathBehavior::new(object, data)
            .expect("SlowDeathBehavior failed to initialize from module data"),
    )
}

pub(super) fn helicopter_slow_death_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HelicopterSlowDeathBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HelicopterSlowDeathBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn helicopter_slow_death_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<HelicopterSlowDeathBehaviorModuleData>(
        "HelicopterSlowDeathBehavior",
        &module_data,
    );
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
        return missing_owner_module("HelicopterSlowDeathBehavior", data_for_missing);
    };
    let behavior = HelicopterSlowDeathBehavior::new(object, Arc::clone(&data_arc));
    let module_name = AsciiString::from("HelicopterSlowDeathBehavior");
    Box::new(HelicopterSlowDeathBehaviorModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

pub(super) fn poisoned_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PoisonedBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PoisonedBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn poisoned_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<PoisonedBehaviorModuleData>("PoisonedBehavior", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
        return missing_owner_module("PoisonedBehavior", data_for_missing);
    };
    let behavior = PoisonedBehavior::new(object, Arc::clone(&data_arc));
    let module_name = AsciiString::from("PoisonedBehavior");
    Box::new(PoisonedBehaviorModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

pub(super) fn jet_slow_death_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = JetSlowDeathBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse JetSlowDeathBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn jet_slow_death_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<JetSlowDeathBehaviorModuleData>("JetSlowDeathBehavior", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
        return missing_owner_module("JetSlowDeathBehavior", data_for_missing);
    };
    let behavior = JetSlowDeathBehavior::new(object, Arc::clone(&data_arc));
    let module_name = AsciiString::from("JetSlowDeathBehavior");
    Box::new(JetSlowDeathBehaviorModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}
