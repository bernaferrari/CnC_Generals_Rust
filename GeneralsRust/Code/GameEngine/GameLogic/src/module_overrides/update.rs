//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Remaining update/behavior factory wrappers.
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

fn radar_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RadarUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RadarUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn radar_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("RadarUpdate requires a valid object");

    let module_name = AsciiString::from("RadarUpdate");
    Box::new(
        RadarUpdateModule::from_module_data(object, &module_name, module_data)
            .expect("RadarUpdateModuleData expected"),
    )
}

fn stealth_detector_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StealthDetectorUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StealthDetectorUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn stealth_detector_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StealthDetectorUpdateModuleData>()
        .expect("StealthDetectorUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("StealthDetectorUpdate requires a valid object");
    let behavior = StealthDetectorUpdate::new(object, module_data_arc.clone())
        .expect("StealthDetectorUpdate failed to initialize");

    let module_name = AsciiString::from("StealthDetectorUpdate");
    Box::new(StealthDetectorUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn radius_decal_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(RadiusDecalUpdateModuleData::default())
}

fn radius_decal_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RadiusDecalUpdateModuleData>()
        .expect("RadiusDecalUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("RadiusDecalUpdate requires a valid object");
    let behavior = RadiusDecalUpdate::new(object, module_data_arc.clone())
        .expect("RadiusDecalUpdate failed to initialize");

    let module_name = AsciiString::from("RadiusDecalUpdate");
    Box::new(RadiusDecalUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn sticky_bomb_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StickyBombUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StickyBombUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn sticky_bomb_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StickyBombUpdateModuleData>()
        .expect("StickyBombUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("StickyBombUpdate requires a valid object");
    let behavior = StickyBombUpdate::new(object, module_data_arc.clone())
        .expect("StickyBombUpdate failed to initialize");

    let module_name = AsciiString::from("StickyBombUpdate");
    Box::new(StickyBombUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn prone_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ProneUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ProneUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn prone_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ProneUpdateModuleData>()
        .expect("ProneUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("ProneUpdate requires a valid object");
    let behavior = ProneUpdate::new(object, module_data_arc.clone())
        .expect("ProneUpdate failed to initialize");

    let module_name = AsciiString::from("ProneUpdate");
    Box::new(ProneUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn projectile_stream_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ProjectileStreamUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ProjectileStreamUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn projectile_stream_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ProjectileStreamUpdateModuleData>()
        .expect("ProjectileStreamUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("ProjectileStreamUpdate requires a valid object");
    let behavior = ProjectileStreamUpdate::new(object, module_data_arc.clone())
        .expect("ProjectileStreamUpdate failed to initialize");

    let module_name = AsciiString::from("ProjectileStreamUpdate");
    Box::new(ProjectileStreamUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn point_defense_laser_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PointDefenseLaserUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PointDefenseLaserUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn point_defense_laser_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PointDefenseLaserUpdateModuleData>()
        .expect("PointDefenseLaserUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("PointDefenseLaserUpdate requires a valid object");
    let behavior = PointDefenseLaserUpdate::new(object, module_data_arc.clone())
        .expect("PointDefenseLaserUpdate failed to initialize");

    let module_name = AsciiString::from("PointDefenseLaserUpdate");
    Box::new(PointDefenseLaserUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}


fn bone_fx_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BoneFXUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BoneFXUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn bone_fx_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BoneFXUpdateModuleData>()
        .expect("BoneFXUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let behavior = BoneFXUpdate::new(owner_id, module_data_arc.clone());

    let module_name = AsciiString::from("BoneFXUpdate");
    Box::new(BoneFXUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn demo_trap_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DemoTrapUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DemoTrapUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn demo_trap_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DemoTrapUpdateModuleData>()
        .expect("DemoTrapUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("DemoTrapUpdate requires a valid object");
    let behavior = DemoTrapUpdate::new(object, module_data_arc.clone())
        .expect("DemoTrapUpdate failed to initialize");

    let module_name = AsciiString::from("DemoTrapUpdate");
    Box::new(DemoTrapUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn smart_bomb_target_homing_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SmartBombTargetHomingUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SmartBombTargetHomingUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn smart_bomb_target_homing_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SmartBombTargetHomingUpdateModuleData>()
        .expect("SmartBombTargetHomingUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("SmartBombTargetHomingUpdate requires a valid object");
    let behavior = SmartBombTargetHomingUpdate::new(object, module_data_arc.clone())
        .expect("SmartBombTargetHomingUpdate failed to initialize");

    let module_name = AsciiString::from("SmartBombTargetHomingUpdate");
    Box::new(SmartBombTargetHomingUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn tensile_formation_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TensileFormationUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TensileFormationUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn tensile_formation_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<TensileFormationUpdateModuleData>()
        .expect("TensileFormationUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("TensileFormationUpdate requires a valid object");
    let behavior = TensileFormationUpdate::new(object, module_data_arc.clone())
        .expect("TensileFormationUpdate failed to initialize");

    let module_name = AsciiString::from("TensileFormationUpdate");
    Box::new(TensileFormationUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn generate_minefield_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GenerateMinefieldBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GenerateMinefieldBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn generate_minefield_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<GenerateMinefieldBehaviorModuleData>()
        .expect("GenerateMinefieldBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("GenerateMinefieldBehavior requires a valid object");
    let behavior = GenerateMinefieldBehavior::new(object, module_data_arc.clone())
        .expect("GenerateMinefieldBehavior failed to initialize");

    let module_name = AsciiString::from("GenerateMinefieldBehavior");
    Box::new(GenerateMinefieldBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn special_ability_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpecialAbilityUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpecialAbilityUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn special_ability_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpecialAbilityUpdateModuleData>()
        .expect("SpecialAbilityUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("SpecialAbilityUpdate requires object");
    let object_ptr = Arc::downgrade(&object);
    let behavior = SpecialAbilityUpdate::new(object_ptr, module_data_arc.clone());
    let module_name = AsciiString::from("SpecialAbilityUpdate");
    Box::new(SpecialAbilityUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn spectre_gunship_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpectreGunshipUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpectreGunshipUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn spectre_gunship_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpectreGunshipUpdateModuleData>()
        .expect("SpectreGunshipUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("SpectreGunshipUpdate requires object");
    let behavior = SpectreGunshipUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create SpectreGunshipUpdate");
    let module_name = AsciiString::from("SpectreGunshipUpdate");
    Box::new(SpectreGunshipUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn spectre_gunship_deployment_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SpectreGunshipDeploymentUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpectreGunshipDeploymentUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn spectre_gunship_deployment_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpectreGunshipDeploymentUpdateModuleData>()
        .expect("SpectreGunshipDeploymentUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("SpectreGunshipDeploymentUpdate requires object");
    let behavior = SpectreGunshipDeploymentUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create SpectreGunshipDeploymentUpdate");
    let module_name = AsciiString::from("SpectreGunshipDeploymentUpdate");
    Box::new(SpectreGunshipDeploymentUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn particle_uplink_cannon_update_module_data_factory(
    _ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    Box::new(ParticleUplinkCannonUpdateModuleData::default())
}

fn particle_uplink_cannon_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ParticleUplinkCannonUpdateModuleData>()
        .expect("ParticleUplinkCannonUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("ParticleUplinkCannonUpdate requires object");
    let behavior = ParticleUplinkCannonUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create ParticleUplinkCannonUpdate");
    let module_name = AsciiString::from("ParticleUplinkCannonUpdate");
    Box::new(ParticleUplinkCannonUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn battle_plan_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(BattlePlanUpdateModuleData::default())
}

fn battle_plan_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BattlePlanUpdateModuleData>()
        .expect("BattlePlanUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("BattlePlanUpdate requires object");
    let behavior = BattlePlanUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create BattlePlanUpdate");
    let module_name = AsciiString::from("BattlePlanUpdate");
    Box::new(BattlePlanUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn lifetime_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = LifetimeUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LifetimeUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn lifetime_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<LifetimeUpdateModuleData>()
        .expect("LifetimeUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("LifetimeUpdate requires object");
    let behavior = LifetimeUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create LifetimeUpdate");
    let module_name = AsciiString::from("LifetimeUpdate");
    Box::new(LifetimeUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn missile_launcher_building_update_module_data_factory(
    _ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    Box::new(MissileLauncherBuildingUpdateModuleData::default())
}

fn missile_launcher_building_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<MissileLauncherBuildingUpdateModuleData>()
        .expect("MissileLauncherBuildingUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let object = thing
        .as_object()
        .cloned()
        .expect("MissileLauncherBuildingUpdate requires object");
    let behavior = MissileLauncherBuildingUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create MissileLauncherBuildingUpdate");
    let module_name = AsciiString::from("MissileLauncherBuildingUpdate");
    Box::new(MissileLauncherBuildingUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn spy_vision_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpyVisionUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpyVisionUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn spy_vision_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpyVisionUpdateModuleData>()
        .expect("SpyVisionUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let module_name_key = NameKeyGenerator::name_to_key("SpyVisionUpdate");
    let behavior = SpyVisionUpdate::new(module_name_key, module_data_arc.clone(), owner_id);

    let module_name = AsciiString::from("SpyVisionUpdate");
    Box::new(SpyVisionUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn fire_weapon_when_dead_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = FireWeaponWhenDeadBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponWhenDeadBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn fire_weapon_when_dead_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireWeaponWhenDeadBehaviorModuleData>()
        .expect("FireWeaponWhenDeadBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FireWeaponWhenDeadBehavior requires a valid object");
    let behavior = FireWeaponWhenDeadBehavior::new(object, module_data_arc.clone())
        .expect("FireWeaponWhenDeadBehavior failed to initialize");

    let module_name = AsciiString::from("FireWeaponWhenDeadBehavior");
    Box::new(FireWeaponWhenDeadBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn fire_weapon_when_damaged_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = FireWeaponWhenDamagedBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponWhenDamagedBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn fire_weapon_when_damaged_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireWeaponWhenDamagedBehaviorModuleData>()
        .expect("FireWeaponWhenDamagedBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FireWeaponWhenDamagedBehavior requires a valid object");
    let behavior = FireWeaponWhenDamagedBehavior::new(object, module_data_arc.clone())
        .expect("FireWeaponWhenDamagedBehavior failed to initialize");

    let module_name = AsciiString::from("FireWeaponWhenDamagedBehavior");
    Box::new(FireWeaponWhenDamagedBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn fire_weapon_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FireWeaponUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn fire_weapon_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireWeaponUpdateModuleData>()
        .expect("FireWeaponUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FireWeaponUpdate requires a valid object");
    let behavior = FireWeaponUpdate::new(object, module_data_arc.clone())
        .expect("FireWeaponUpdate failed to initialize");

    let module_name = AsciiString::from("FireWeaponUpdate");
    Box::new(FireWeaponUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn fire_ocl_after_weapon_cooldown_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = FireOCLAfterWeaponCooldownUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireOCLAfterWeaponCooldownUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn fire_ocl_after_weapon_cooldown_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireOCLAfterWeaponCooldownUpdateModuleData>()
        .expect("FireOCLAfterWeaponCooldownUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FireOCLAfterWeaponCooldownUpdate requires a valid object");
    let behavior = FireOCLAfterWeaponCooldownUpdate::new(object, module_data_arc.clone())
        .expect("FireOCLAfterWeaponCooldownUpdate failed to initialize");

    let module_name = AsciiString::from("FireOCLAfterWeaponCooldownUpdate");
    Box::new(FireOCLAfterWeaponCooldownUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn weapon_bonus_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WeaponBonusUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WeaponBonusUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn weapon_bonus_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WeaponBonusUpdateModuleData>()
        .expect("WeaponBonusUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("WeaponBonusUpdate requires a valid object");
    let behavior = WeaponBonusUpdate::new(object, module_data_arc.clone())
        .expect("WeaponBonusUpdate failed to initialize");

    let module_name = AsciiString::from("WeaponBonusUpdate");
    Box::new(WeaponBonusUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn emp_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = EMPUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse EMPUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn emp_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<EMPUpdateModuleData>()
        .expect("EMPUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("EMPUpdate requires a valid object");
    let behavior =
        EMPUpdate::new(object, module_data_arc.clone()).expect("EMPUpdate failed to initialize");

    let module_name = AsciiString::from("EMPUpdate");
    Box::new(EMPUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn structure_collapse_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StructureCollapseUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StructureCollapseUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn structure_collapse_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StructureCollapseUpdateModuleData>()
        .expect("StructureCollapseUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("StructureCollapseUpdate requires a valid object");
    let behavior = StructureCollapseUpdate::new(object, module_data_arc.clone())
        .expect("StructureCollapseUpdate failed to initialize");

    let module_name = AsciiString::from("StructureCollapseUpdate");
    Box::new(StructureCollapseUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn float_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FloatUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FloatUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn float_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FloatUpdateModuleData>()
        .expect("FloatUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("FloatUpdate requires a valid object");
    let behavior = FloatUpdate::new(object, module_data_arc.clone())
        .expect("FloatUpdate failed to initialize");

    let module_name = AsciiString::from("FloatUpdate");
    Box::new(FloatUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn enemy_near_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = EnemyNearUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse EnemyNearUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn enemy_near_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<EnemyNearUpdateModuleData>()
        .expect("EnemyNearUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("EnemyNearUpdate requires a valid object");
    let behavior = EnemyNearUpdate::new(object, module_data_arc.clone())
        .expect("EnemyNearUpdate failed to initialize");

    let module_name = AsciiString::from("EnemyNearUpdate");
    Box::new(EnemyNearUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn auto_find_healing_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AutoFindHealingUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AutoFindHealingUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn auto_find_healing_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AutoFindHealingUpdateModuleData>()
        .expect("AutoFindHealingUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("AutoFindHealingUpdate requires a valid object");
    let behavior = AutoFindHealingUpdate::new_typed(object, module_data_arc.clone());

    let module_name = AsciiString::from("AutoFindHealingUpdate");
    Box::new(AutoFindHealingUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn supply_warehouse_crippling_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SupplyWarehouseCripplingBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SupplyWarehouseCripplingBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn supply_warehouse_crippling_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SupplyWarehouseCripplingBehaviorModuleData>()
        .expect("SupplyWarehouseCripplingBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("SupplyWarehouseCripplingBehavior requires a valid object");
    let behavior = SupplyWarehouseCripplingBehavior::new(object, module_data_arc.clone())
        .expect("SupplyWarehouseCripplingBehavior failed to initialize");

    let module_name = AsciiString::from("SupplyWarehouseCripplingBehavior");
    Box::new(SupplyWarehouseCripplingBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn base_regenerate_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BaseRegenerateUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BaseRegenerateUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn base_regenerate_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BaseRegenerateUpdateModuleData>()
        .expect("BaseRegenerateUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("BaseRegenerateUpdate requires a valid object");
    let behavior = BaseRegenerateUpdate::new(object, module_data_arc.clone())
        .expect("BaseRegenerateUpdate failed to initialize");

    let module_name = AsciiString::from("BaseRegenerateUpdate");
    Box::new(BaseRegenerateUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn auto_deposit_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AutoDepositUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AutoDepositUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn auto_deposit_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AutoDepositUpdateModuleData>()
        .expect("AutoDepositUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("AutoDepositUpdate requires a valid object");
    let behavior = AutoDepositUpdate::new(object, module_data_arc.clone())
        .expect("AutoDepositUpdate failed to initialize");

    let module_name = AsciiString::from("AutoDepositUpdate");
    Box::new(AutoDepositUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn power_plant_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PowerPlantUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PowerPlantUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn power_plant_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PowerPlantUpdateModuleData>()
        .expect("PowerPlantUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("PowerPlantUpdate requires a valid object");
    let behavior = PowerPlantUpdate::new(object, module_data_arc.clone())
        .expect("PowerPlantUpdate failed to initialize");

    let module_name = AsciiString::from("PowerPlantUpdate");
    Box::new(PowerPlantUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn tech_building_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TechBuildingBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TechBuildingBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn tech_building_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<TechBuildingBehaviorModuleData>()
        .expect("TechBuildingBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("TechBuildingBehavior requires a valid object");
    let behavior = TechBuildingBehavior::new(object, module_data_arc.clone())
        .expect("TechBuildingBehavior failed to initialize");

    let module_name = AsciiString::from("TechBuildingBehavior");
    Box::new(TechBuildingBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn propaganda_tower_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PropagandaTowerBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PropagandaTowerBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn propaganda_tower_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PropagandaTowerBehaviorModuleData>()
        .expect("PropagandaTowerBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("PropagandaTowerBehavior requires a valid object");
    let behavior = PropagandaTowerBehavior::new(object, module_data_arc.clone())
        .expect("PropagandaTowerBehavior failed to initialize");

    let module_name = AsciiString::from("PropagandaTowerBehavior");
    Box::new(PropagandaTowerBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn assisted_targeting_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AssistedTargetingUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AssistedTargetingUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn assisted_targeting_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AssistedTargetingUpdateModuleData>()
        .expect("AssistedTargetingUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("AssistedTargetingUpdate requires a valid object");
    let behavior = AssistedTargetingUpdate::new(object, module_data_arc.clone())
        .expect("AssistedTargetingUpdate failed to initialize");

    let module_name = AsciiString::from("AssistedTargetingUpdate");
    Box::new(AssistedTargetingUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn dynamic_shroud_clearing_range_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = DynamicShroudClearingRangeUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DynamicShroudClearingRangeUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn dynamic_shroud_clearing_range_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let config = module_data
        .get_dynamic_shroud_clearing_range_update_config()
        .expect("DynamicShroudClearingRangeUpdateModuleData expected");
    let module_data_arc = Arc::new(DynamicShroudClearingRangeUpdateModuleData::from_config(
        config,
        module_data.get_module_tag_name_key(),
    ));
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("DynamicShroudClearingRangeUpdate requires a valid object");
    let behavior = DynamicShroudClearingRangeUpdate::new_with_data(object, module_data_arc.clone())
        .expect("DynamicShroudClearingRangeUpdate failed to initialize");

    let module_name = AsciiString::from("DynamicShroudClearingRangeUpdate");
    Box::new(DynamicShroudClearingRangeUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn cleanup_hazard_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CleanupHazardUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CleanupHazardUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn cleanup_hazard_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CleanupHazardUpdateModuleData>()
        .expect("CleanupHazardUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("CleanupHazardUpdate requires a valid object");
    let behavior = CleanupHazardUpdate::new(object, module_data_arc.clone())
        .expect("CleanupHazardUpdate failed to initialize");

    let module_name = AsciiString::from("CleanupHazardUpdate");
    Box::new(CleanupHazardUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn fire_spread_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FireSpreadUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireSpreadUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn fire_spread_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FireSpreadUpdateModuleData>()
        .expect("FireSpreadUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FireSpreadUpdate requires a valid object");
    let behavior = FireSpreadUpdate::new(owner_id, (*module_data_arc).clone());

    let module_name = AsciiString::from("FireSpreadUpdate");
    Box::new(FireSpreadUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn slaved_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SlavedUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SlavedUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn slaved_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SlavedUpdateModuleData>()
        .expect("SlavedUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let behavior = SlavedUpdate::new(owner_id, module_data_arc.clone())
        .expect("SlavedUpdate failed to initialize");

    let module_name = AsciiString::from("SlavedUpdate");
    Box::new(SlavedUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn mob_member_slaved_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MobMemberSlavedUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MobMemberSlavedUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn mob_member_slaved_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<MobMemberSlavedUpdateModuleData>()
        .expect("MobMemberSlavedUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("MobMemberSlavedUpdate requires a valid object");
    let behavior = MobMemberSlavedUpdate::new(object, module_data_arc.clone())
        .expect("MobMemberSlavedUpdate failed to initialize");

    let module_name = AsciiString::from("MobMemberSlavedUpdate");
    Box::new(MobMemberSlavedUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn command_button_hunt_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CommandButtonHuntUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CommandButtonHuntUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn command_button_hunt_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CommandButtonHuntUpdateModuleData>()
        .expect("CommandButtonHuntUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let behavior = CommandButtonHuntUpdate::new(owner_id, module_data_arc.clone());

    let module_name = AsciiString::from("CommandButtonHuntUpdate");
    Box::new(CommandButtonHuntUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn topple_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ToppleUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ToppleUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn topple_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<ToppleUpdateModuleData>()
        .expect("ToppleUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("ToppleUpdate requires a valid object");
    let behavior = ToppleUpdate::new_from_object_handle(object, module_data_arc.clone());

    let module_name = AsciiString::from("ToppleUpdate");
    Box::new(ToppleUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn structure_topple_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StructureToppleUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StructureToppleUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn structure_topple_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<StructureToppleUpdateModuleData>()
        .expect("StructureToppleUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("StructureToppleUpdate requires a valid object");
    let behavior = StructureToppleUpdate::new(object, module_data_arc.clone())
        .expect("StructureToppleUpdate failed to initialize");

    let module_name = AsciiString::from("StructureToppleUpdate");
    Box::new(StructureToppleUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn rebuild_hole_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = RebuildHoleBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RebuildHoleBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn rebuild_hole_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<RebuildHoleBehaviorModuleData>()
        .expect("RebuildHoleBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = RebuildHoleBehavior::from_module_thing(thing, module_data_arc.clone());

    let module_name = AsciiString::from("RebuildHoleBehavior");
    Box::new(RebuildHoleBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn overcharge_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = OverchargeBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse OverchargeBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn firing_tracker_behavior_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(FiringTrackerBehaviorModuleData::default())
}

fn firing_tracker_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FiringTrackerBehaviorModuleData>()
        .expect("FiringTrackerBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    let behavior = FiringTrackerBehavior::new(owner_id);

    let module_name = AsciiString::from("FiringTracker");
    Box::new(FiringTrackerBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn overcharge_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<OverchargeBehaviorModuleData>()
        .expect("OverchargeBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = OverchargeBehavior::from_module_thing(thing, module_data_arc.clone());

    let module_name = AsciiString::from("OverchargeBehavior");
    Box::new(OverchargeBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn countermeasures_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CountermeasuresBehaviorModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse CountermeasuresBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn countermeasures_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CountermeasuresBehaviorModuleData>()
        .expect("CountermeasuresBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        CountermeasuresBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("CountermeasuresBehavior requires an owning object");

    let module_name = AsciiString::from("CountermeasuresBehavior");
    Box::new(CountermeasuresBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn bunker_buster_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BunkerBusterBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BunkerBusterBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn bunker_buster_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BunkerBusterBehaviorModuleData>()
        .expect("BunkerBusterBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        BunkerBusterBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("BunkerBusterBehavior requires an owning object");

    let module_name = AsciiString::from("BunkerBusterBehavior");
    Box::new(BunkerBusterBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn flight_deck_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn flight_deck_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FlightDeckBehaviorModuleData>()
        .expect("FlightDeckBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        FlightDeckBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("FlightDeckBehavior requires an owning object");

    let module_name = AsciiString::from("FlightDeckBehavior");
    Box::new(FlightDeckBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

#[cfg(feature = "allow_surrender")]
fn pow_truck_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = POWTruckBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse POWTruckBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

#[cfg(feature = "allow_surrender")]
fn pow_truck_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<POWTruckBehaviorModuleData>()
        .expect("POWTruckBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("POWTruckBehavior requires owning object");
    let behavior = POWTruckBehavior::new(object, module_data_arc.clone())
        .expect("POWTruckBehavior::new failed");

    let module_name = AsciiString::from("POWTruckBehavior");
    Box::new(POWTruckBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

#[cfg(feature = "allow_surrender")]
fn prison_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PrisonBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PrisonBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

#[cfg(feature = "allow_surrender")]
fn prison_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PrisonBehaviorModuleData>()
        .expect("PrisonBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(object_id).expect("PrisonBehavior requires owning object");
    let behavior =
        PrisonBehavior::new(object, module_data_arc.clone()).expect("PrisonBehavior::new failed");

    let module_name = AsciiString::from("PrisonBehavior");
    Box::new(PrisonBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

#[cfg(feature = "allow_surrender")]
fn propaganda_center_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PropagandaCenterBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PropagandaCenterBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

#[cfg(feature = "allow_surrender")]
fn propaganda_center_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PropagandaCenterBehaviorModuleData>()
        .expect("PropagandaCenterBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(object_id)
        .expect("PropagandaCenterBehavior requires owning object");
    let behavior = PropagandaCenterBehavior::new(object, module_data_arc.clone())
        .expect("PropagandaCenterBehavior::new failed");

    let module_name = AsciiString::from("PropagandaCenterBehavior");
    Box::new(PropagandaCenterBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn queue_production_exit_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = QueueProductionExitModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse QueueProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn queue_production_exit_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<QueueProductionExitModuleData>()
        .expect("QueueProductionExitModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior =
        QueueProductionExitBehavior::from_module_thing(Arc::clone(&thing), module_data_arc.clone())
            .expect("QueueProductionExitUpdate requires an owning object");

    let module_name = AsciiString::from("QueueProductionExitUpdate");
    Box::new(QueueProductionExitBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn default_production_exit_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = DefaultProductionExitModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse DefaultProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn default_production_exit_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DefaultProductionExitModuleData>()
        .expect("DefaultProductionExitModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = DefaultProductionExitBehavior::from_module_thing(
        Arc::clone(&thing),
        module_data_arc.clone(),
    )
    .expect("DefaultProductionExitUpdate requires an owning object");

    let module_name = AsciiString::from("DefaultProductionExitUpdate");
    Box::new(DefaultProductionExitBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn spawn_point_production_exit_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SpawnPointProductionExitModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse SpawnPointProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn spawn_point_production_exit_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpawnPointProductionExitModuleData>()
        .expect("SpawnPointProductionExitModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = SpawnPointProductionExitBehavior::from_module_thing(
        Arc::clone(&thing),
        module_data_arc.clone(),
    )
    .expect("SpawnPointProductionExitUpdate requires an owning object");

    let module_name = AsciiString::from("SpawnPointProductionExitUpdate");
    Box::new(SpawnPointProductionExitBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn supply_center_production_exit_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SupplyCenterProductionExitModuleData::default();

    if let Some(mut ini) = ini {
        if let Err(err) = data.parse_from_ini(&mut ini) {
            warn!(
                "Failed to parse SupplyCenterProductionExitUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn supply_center_production_exit_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SupplyCenterProductionExitModuleData>()
        .expect("SupplyCenterProductionExitModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = SupplyCenterProductionExitBehavior::from_module_thing(
        Arc::clone(&thing),
        module_data_arc.clone(),
    )
    .expect("SupplyCenterProductionExitUpdate requires an owning object");

    let module_name = AsciiString::from("SupplyCenterProductionExitUpdate");
    Box::new(SupplyCenterProductionExitBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}
