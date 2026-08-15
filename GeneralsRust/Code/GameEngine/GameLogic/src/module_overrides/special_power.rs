//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Special-power factory wrappers (Demoralize/CashHack/SpyVision/Defector/OCL/…).
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

fn demoralize_special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DemoralizeSpecialPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DemoralizeSpecialPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn demoralize_special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DemoralizeSpecialPowerModuleData>()
        .expect("DemoralizeSpecialPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("DemoralizeSpecialPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(DemoralizeSpecialPower::new(
        module_name_key,
        owner_id,
        data_arc,
    ))
}

fn cash_hack_special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CashHackSpecialPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CashHackSpecialPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn cash_hack_special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CashHackSpecialPowerModuleData>()
        .expect("CashHackSpecialPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("CashHackSpecialPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(CashHackSpecialPower::new(
        module_name_key,
        owner_id,
        data_arc,
    ))
}

fn spy_vision_special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpyVisionSpecialPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpyVisionSpecialPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn spy_vision_special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpyVisionSpecialPowerModuleData>()
        .expect("SpyVisionSpecialPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("SpyVisionSpecialPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(SpyVisionSpecialPower::new(
        module_name_key,
        owner_id,
        data_arc,
    ))
}

fn defector_special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DefectorSpecialPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DefectorSpecialPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn defector_special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DefectorSpecialPowerModuleData>()
        .expect("DefectorSpecialPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("DefectorSpecialPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(DefectorSpecialPower::new(
        module_name_key,
        owner_id,
        data_arc,
    ))
}

fn cash_bounty_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CashBountyPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CashBountyPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn cash_bounty_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CashBountyPowerModuleData>()
        .expect("CashBountyPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("CashBountyPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(CashBountyPower::new(module_name_key, owner_id, data_arc))
}

fn cleanup_area_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CleanupAreaPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CleanupAreaPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn cleanup_area_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CleanupAreaPowerModuleData>()
        .expect("CleanupAreaPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("CleanupAreaPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(CleanupAreaPower::new(module_name_key, owner_id, data_arc))
}

fn fire_weapon_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FireWeaponPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn fire_weapon_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireWeaponPowerModuleData>()
        .expect("FireWeaponPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("FireWeaponPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(FireWeaponPower::new(module_name_key, owner_id, data_arc))
}

fn special_ability_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpecialAbilityModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpecialAbility module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn special_ability_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpecialAbilityModuleData>()
        .expect("SpecialAbilityModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("SpecialAbility");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(SpecialAbility::new(module_name_key, owner_id, data_arc))
}

fn baikonur_launch_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BaikonurLaunchPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BaikonurLaunchPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn baikonur_launch_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BaikonurLaunchPowerModuleData>()
        .expect("BaikonurLaunchPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("BaikonurLaunchPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(BaikonurLaunchPower::new(
        module_name_key,
        owner_id,
        data_arc,
    ))
}

fn ocl_special_power_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = OclSpecialPowerModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse OCLSpecialPower module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn ocl_special_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<OclSpecialPowerModuleData>()
        .expect("OclSpecialPowerModuleData expected");
    let data_arc = Arc::new(typed_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("OCLSpecialPower");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);

    Box::new(OclSpecialPower::new(module_name_key, owner_id, data_arc))
}

