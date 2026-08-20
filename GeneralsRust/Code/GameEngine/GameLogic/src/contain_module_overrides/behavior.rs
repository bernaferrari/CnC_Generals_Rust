//! Behavior-module data/module factory functions.
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::helpers::*;
use super::*;

active_behavior_factories!(
    bunker_buster_behavior_data_factory,
    bunker_buster_behavior_module_factory,
    BunkerBusterBehaviorModuleData,
    BunkerBusterBehavior,
    "BunkerBusterBehavior"
);
active_behavior_factories!(
    checkpoint_update_data_factory,
    checkpoint_update_module_factory,
    CheckpointUpdateModuleData,
    CheckpointUpdate,
    "CheckpointUpdate"
);
active_behavior_factories!(
    deletion_update_data_factory,
    deletion_update_module_factory,
    DeletionUpdateModuleData,
    DeletionUpdate,
    "DeletionUpdate"
);
active_behavior_factories!(
    dynamic_shroud_clearing_range_update_data_factory,
    dynamic_shroud_clearing_range_update_module_factory,
    DynamicShroudClearingRangeUpdateModuleData,
    DynamicShroudClearingRangeUpdate,
    "DynamicShroudClearingRangeUpdate"
);
active_behavior_factories!(
    enemy_near_update_data_factory,
    enemy_near_update_module_factory,
    EnemyNearUpdateModuleData,
    EnemyNearUpdate,
    "EnemyNearUpdate"
);
active_behavior_factories!(
    fire_ocl_after_weapon_cooldown_update_data_factory,
    fire_ocl_after_weapon_cooldown_update_module_factory,
    FireOCLAfterWeaponCooldownUpdateModuleData,
    FireOCLAfterWeaponCooldownUpdate,
    "FireOCLAfterWeaponCooldownUpdate"
);
active_behavior_factories!(
    fire_weapon_when_damaged_behavior_data_factory,
    fire_weapon_when_damaged_behavior_module_factory,
    FireWeaponWhenDamagedBehaviorModuleData,
    FireWeaponWhenDamagedBehavior,
    "FireWeaponWhenDamagedBehavior"
);
active_behavior_factories!(
    fire_weapon_when_dead_behavior_data_factory,
    fire_weapon_when_dead_behavior_module_factory,
    FireWeaponWhenDeadBehaviorModuleData,
    FireWeaponWhenDeadBehavior,
    "FireWeaponWhenDeadBehavior"
);
active_behavior_factories!(
    fire_weapon_update_data_factory,
    fire_weapon_update_module_factory,
    FireWeaponUpdateModuleData,
    FireWeaponUpdate,
    "FireWeaponUpdate"
);
active_behavior_factories!(
    dynamic_geometry_info_update_data_factory,
    dynamic_geometry_info_update_module_factory,
    DynamicGeometryInfoUpdateModuleData,
    DynamicGeometryInfoUpdate,
    "DynamicGeometryInfoUpdate"
);

active_behavior_factories!(
    firestorm_dynamic_geometry_info_update_data_factory,
    firestorm_dynamic_geometry_info_update_module_factory,
    FirestormDynamicGeometryInfoUpdateModuleData,
    FirestormDynamicGeometryInfoUpdate,
    "FirestormDynamicGeometryInfoUpdate"
);
active_behavior_factories!(
    float_update_data_factory,
    float_update_module_factory,
    FloatUpdateModuleData,
    FloatUpdate,
    "FloatUpdate"
);
active_behavior_factories!(
    generate_minefield_behavior_data_factory,
    generate_minefield_behavior_module_factory,
    GenerateMinefieldBehaviorModuleData,
    GenerateMinefieldBehavior,
    "GenerateMinefieldBehavior"
);
pub(super) fn minefield_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MinefieldBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MinefieldBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn minefield_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<MinefieldBehaviorModuleData>("MinefieldBehavior", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
        return missing_owner_module("MinefieldBehavior", data_for_missing);
    };
    let behavior = MinefieldBehavior::new(object, Arc::clone(&data_arc))
        .expect("MinefieldBehavior failed to initialize");
    let module_name = AsciiString::from("MinefieldBehavior");
    Box::new(MinefieldBehaviorModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

active_behavior_factories!(
    height_die_update_data_factory,
    height_die_update_module_factory,
    HeightDieUpdateModuleData,
    HeightDieUpdate,
    "HeightDieUpdate"
);
active_behavior_factories!(
    hijacker_update_data_factory,
    hijacker_update_module_factory,
    HijackerUpdateModuleData,
    HijackerUpdate,
    "HijackerUpdate"
);
active_behavior_factories!(
    horde_update_data_factory,
    horde_update_module_factory,
    HordeUpdateModuleData,
    HordeUpdate,
    "HordeUpdate"
);
active_behavior_factories!(
    neutron_blast_behavior_data_factory,
    neutron_blast_behavior_module_factory,
    NeutronBlastBehaviorModuleData,
    NeutronBlastBehavior,
    "NeutronBlastBehavior"
);
active_behavior_factories!(
    leaflet_drop_behavior_data_factory,
    leaflet_drop_behavior_module_factory,
    LeafletDropBehaviorModuleData,
    LeafletDropBehavior,
    "LeafletDropBehavior"
);

pub(super) fn missile_launcher_building_update_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = MissileLauncherBuildingUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MissileLauncherBuildingUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn missile_launcher_building_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<MissileLauncherBuildingUpdateModuleData>(
        "MissileLauncherBuildingUpdate",
        &module_data,
    );
    let engine_data: Arc<dyn LegacyModuleData> = data_arc.clone();
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("MissileLauncherBuildingUpdate", &module_data);
    };
    let behavior = MissileLauncherBuildingUpdate::new(object, engine_data)
        .expect("MissileLauncherBuildingUpdate failed to initialize");
    Box::new(MissileLauncherBuildingUpdateModule::new(
        behavior,
        &AsciiString::from("MissileLauncherBuildingUpdate"),
        data_arc,
    ))
}

active_behavior_factories!(
    parking_place_behavior_data_factory,
    parking_place_behavior_module_factory,
    ParkingPlaceBehaviorModuleData,
    ParkingPlaceBehavior,
    "ParkingPlaceBehavior"
);
active_behavior_factories!(
    pilot_find_vehicle_update_data_factory,
    pilot_find_vehicle_update_module_factory,
    PilotFindVehicleUpdateModuleData,
    PilotFindVehicleUpdate,
    "PilotFindVehicleUpdate"
);
active_behavior_factories!(
    power_plant_update_data_factory,
    power_plant_update_module_factory,
    PowerPlantUpdateModuleData,
    PowerPlantUpdate,
    "PowerPlantUpdate"
);
active_behavior_factories!(
    propaganda_tower_behavior_data_factory,
    propaganda_tower_behavior_module_factory,
    PropagandaTowerBehaviorModuleData,
    PropagandaTowerBehavior,
    "PropagandaTowerBehavior"
);
active_behavior_factories!(
    radar_update_data_factory,
    radar_update_module_factory,
    RadarUpdateModuleData,
    RadarUpdate,
    "RadarUpdate"
);
active_behavior_factories!(
    spectre_gunship_deployment_update_data_factory,
    spectre_gunship_deployment_update_module_factory,
    SpectreGunshipDeploymentUpdateModuleData,
    SpectreGunshipDeploymentUpdate,
    "SpectreGunshipDeploymentUpdate"
);
active_behavior_factories!(
    spectre_gunship_update_data_factory,
    spectre_gunship_update_module_factory,
    SpectreGunshipUpdateModuleData,
    SpectreGunshipUpdate,
    "SpectreGunshipUpdate"
);
active_behavior_factories!(
    stealth_detector_update_data_factory,
    stealth_detector_update_module_factory,
    StealthDetectorUpdateModuleData,
    StealthDetectorUpdate,
    "StealthDetectorUpdate"
);
active_behavior_factories!(
    tech_building_behavior_data_factory,
    tech_building_behavior_module_factory,
    TechBuildingBehaviorModuleData,
    TechBuildingBehavior,
    "TechBuildingBehavior"
);
active_behavior_factories!(
    wave_guide_update_data_factory,
    wave_guide_update_module_factory,
    WaveGuideUpdateModuleData,
    WaveGuideUpdate,
    "WaveGuideUpdate"
);
active_behavior_factories!(
    weapon_bonus_update_data_factory,
    weapon_bonus_update_module_factory,
    WeaponBonusUpdateModuleData,
    WeaponBonusUpdate,
    "WeaponBonusUpdate"
);
active_behavior_factories!(
    physics_behavior_data_factory,
    physics_behavior_module_factory,
    PhysicsBehaviorModuleData,
    PhysicsBehaviorUpdate,
    "PhysicsBehavior"
);
active_behavior_factories!(
    flammable_update_data_factory,
    flammable_update_module_factory,
    FlammableUpdateModuleData,
    FlammableUpdate,
    "FlammableUpdate"
);

pub(super) fn special_ability_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn special_ability_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<SpecialAbilityUpdateModuleData>("SpecialAbilityUpdate", &module_data);
    let engine_data: Arc<dyn LegacyModuleData> = data_arc.clone();
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SpecialAbilityUpdate", &module_data);
    };
    let behavior = SpecialAbilityUpdate::new(Arc::downgrade(&object), engine_data);
    let module_name = AsciiString::from("SpecialAbilityUpdate");
    Box::new(SpecialAbilityUpdateModule::new(
        behavior,
        &module_name,
        data_arc,
    ))
}

impl Snapshotable for MissileAIUpdateBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.update.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.update.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.update.load_post_process()
    }
}

pub(super) fn missile_ai_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MissileAIUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MissileAIUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn missile_ai_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    active_behavior_module::<MissileAIUpdateBehavior, MissileAIUpdateModuleData>(
        thing,
        module_data,
        "MissileAIUpdate",
        MissileAIUpdateBehavior::new,
    )
}

pub(super) fn railroad_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn railroad_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<RailroadBehaviorModuleData>("RailroadBehavior", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("RailroadBehavior", &module_data);
    };
    let module_name_key = NameKeyGenerator::name_to_key("RailroadBehavior");
    Box::new(
        RailroadBehaviorModule::new(module_name_key, data_arc, object)
            .expect("RailroadBehavior init failed"),
    )
}
