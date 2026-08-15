//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Upgrade factories plus TransitionDamageFX and StealthUpdate.
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

fn status_bits_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StatusBitsUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StatusBitsUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn status_bits_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StatusBitsUpgradeModuleData>()
        .expect("StatusBitsUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("StatusBitsUpgrade");
    Box::new(StatusBitsUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn passengers_fire_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PassengersFireUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PassengersFireUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn passengers_fire_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PassengersFireUpgradeModuleData>()
        .expect("PassengersFireUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("PassengersFireUpgrade");

    Box::new(PassengersFireUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn subobjects_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SubObjectsUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SubObjectsUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn subobjects_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SubObjectsUpgradeModuleData>()
        .expect("SubObjectsUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("SubObjectsUpgrade");

    Box::new(SubObjectsUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn grant_science_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GrantScienceUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GrantScienceUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn grant_science_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<GrantScienceUpgradeModuleData>()
        .expect("GrantScienceUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("GrantScienceUpgrade");

    Box::new(GrantScienceUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn object_creation_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ObjectCreationUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ObjectCreationUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn object_creation_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ObjectCreationUpgradeModuleData>()
        .expect("ObjectCreationUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ObjectCreationUpgrade");

    Box::new(ObjectCreationUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn active_shroud_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ActiveShroudUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ActiveShroudUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn active_shroud_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ActiveShroudUpgrade");

    Box::new(
        ActiveShroudUpgrade::from_module_data(module_name_key, module_data, object_id)
            .expect("ActiveShroudUpgradeModuleData expected"),
    )
}

fn armor_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ArmorUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ArmorUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn armor_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ArmorUpgradeModuleData>()
        .expect("ArmorUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ArmorUpgrade");

    Box::new(ArmorUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn command_set_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CommandSetUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CommandSetUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn command_set_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CommandSetUpgradeModuleData>()
        .expect("CommandSetUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("CommandSetUpgrade");

    Box::new(CommandSetUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn cost_modifier_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CostModifierUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CostModifierUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn cost_modifier_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CostModifierUpgradeModuleData>()
        .expect("CostModifierUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("CostModifierUpgrade");

    Box::new(CostModifierUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn experience_scalar_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ExperienceScalarUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ExperienceScalarUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn experience_scalar_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ExperienceScalarUpgradeModuleData>()
        .expect("ExperienceScalarUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ExperienceScalarUpgrade");

    Box::new(ExperienceScalarUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn locomotor_set_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = LocomotorSetUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LocomotorSetUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn locomotor_set_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<LocomotorSetUpgradeModuleData>()
        .expect("LocomotorSetUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("LocomotorSetUpgrade");

    Box::new(LocomotorSetUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn max_health_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MaxHealthUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MaxHealthUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn max_health_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<MaxHealthUpgradeModuleData>()
        .expect("MaxHealthUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("MaxHealthUpgrade");

    Box::new(MaxHealthUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn model_condition_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ModelConditionUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ModelConditionUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn model_condition_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ModelConditionUpgradeModuleData>()
        .expect("ModelConditionUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ModelConditionUpgrade");

    Box::new(ModelConditionUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn power_plant_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PowerPlantUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PowerPlantUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn power_plant_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PowerPlantUpgradeModuleData>()
        .expect("PowerPlantUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("PowerPlantUpgrade");

    Box::new(PowerPlantUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn radar_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RadarUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RadarUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn radar_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("RadarUpgrade");

    Box::new(
        RadarUpgrade::from_module_data(module_name_key, module_data, object_id)
            .expect("RadarUpgradeModuleData expected"),
    )
}

fn replace_object_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ReplaceObjectUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ReplaceObjectUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn replace_object_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ReplaceObjectUpgradeModuleData>()
        .expect("ReplaceObjectUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("ReplaceObjectUpgrade");

    Box::new(ReplaceObjectUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn stealth_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StealthUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StealthUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn stealth_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StealthUpgradeModuleData>()
        .expect("StealthUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("StealthUpgrade");

    Box::new(StealthUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn unpause_special_power_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = UnpauseSpecialPowerUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse UnpauseSpecialPowerUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn unpause_special_power_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<UnpauseSpecialPowerUpgradeModuleData>()
        .expect("UnpauseSpecialPowerUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("UnpauseSpecialPowerUpgrade");

    Box::new(UnpauseSpecialPowerUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn weapon_bonus_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WeaponBonusUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WeaponBonusUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn weapon_bonus_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WeaponBonusUpgradeModuleData>()
        .expect("WeaponBonusUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("WeaponBonusUpgrade");

    Box::new(WeaponBonusUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn weapon_set_upgrade_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WeaponSetUpgradeModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WeaponSetUpgrade module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn weapon_set_upgrade_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WeaponSetUpgradeModuleData>()
        .expect("WeaponSetUpgradeModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();
    let module_name_key = NameKeyGenerator::name_to_key("WeaponSetUpgrade");

    Box::new(WeaponSetUpgrade::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

fn transition_damage_fx_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TransitionDamageFXModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TransitionDamageFX module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn transition_damage_fx_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<TransitionDamageFXModuleData>()
        .expect("TransitionDamageFXModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        TransitionDamageFX::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("TransitionDamageFX requires an owning object");

    let module_name = AsciiString::from("TransitionDamageFX");
    Box::new(TransitionDamageFXModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn stealth_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StealthUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StealthUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn stealth_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StealthUpdateModuleData>()
        .expect("StealthUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();

    let module_name_key = NameKeyGenerator::name_to_key("StealthUpdate");
    Box::new(StealthUpdateModule::new(
        module_name_key,
        module_data_arc,
        object_id,
    ))
}

