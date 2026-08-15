//! Update-module data/module factory functions.
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::helpers::*;
use super::*;

active_behavior_factories!(
    animation_steering_update_data_factory,
    animation_steering_update_module_factory,
    AnimationSteeringUpdateModuleData,
    AnimationSteeringUpdate,
    "AnimationSteeringUpdate"
);
active_behavior_factories!(
    assisted_targeting_update_data_factory,
    assisted_targeting_update_module_factory,
    AssistedTargetingUpdateModuleData,
    AssistedTargetingUpdate,
    "AssistedTargetingUpdate"
);
active_behavior_factories!(
    auto_deposit_update_data_factory,
    auto_deposit_update_module_factory,
    AutoDepositUpdateModuleData,
    AutoDepositUpdate,
    "AutoDepositUpdate"
);
active_behavior_factories!(
    auto_find_healing_update_data_factory,
    auto_find_healing_update_module_factory,
    AutoFindHealingUpdateModuleData,
    AutoFindHealingUpdate,
    "AutoFindHealingUpdate"
);
active_behavior_factories!(
    base_regenerate_update_data_factory,
    base_regenerate_update_module_factory,
    BaseRegenerateUpdateModuleData,
    BaseRegenerateUpdate,
    "BaseRegenerateUpdate"
);

pub(super) fn battle_plan_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BattlePlanUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BattlePlanUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn battle_plan_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<BattlePlanUpdateModuleData>("BattlePlanUpdate", &module_data);
    let engine_data: Arc<dyn LegacyModuleData> = data_arc.clone();
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("BattlePlanUpdate", &module_data);
    };
    let behavior =
        BattlePlanUpdate::new(object, engine_data).expect("BattlePlanUpdate failed to initialize");
    Box::new(BattlePlanUpdateModule::new(
        behavior,
        &AsciiString::from("BattlePlanUpdate"),
        data_arc,
    ))
}

pub(super) fn cleanup_hazard_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn cleanup_hazard_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<CleanupHazardUpdateModuleData>("CleanupHazardUpdate", &module_data);
    let engine_data: Arc<dyn crate::common::ModuleData> = data_arc.clone();
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("CleanupHazardUpdate", &module_data);
    };
    let behavior = CleanupHazardUpdate::new(object, engine_data)
        .expect("CleanupHazardUpdate failed to initialize");
    Box::new(CleanupHazardUpdateModule::new(
        behavior,
        &AsciiString::from("CleanupHazardUpdate"),
        data_arc,
    ))
}

pub(super) fn command_button_hunt_update_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
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

pub(super) fn command_button_hunt_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CommandButtonHuntUpdateModuleData>(
        "CommandButtonHuntUpdate",
        &module_data,
    );
    let owner_id = resolve_owner_id(&thing);
    let behavior = CommandButtonHuntUpdate::new(owner_id, data_arc.clone());
    Box::new(CommandButtonHuntUpdateModule::new(
        behavior,
        &AsciiString::from("CommandButtonHuntUpdate"),
        data_arc,
    ))
}

pub(super) fn spy_vision_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn spy_vision_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SpyVisionUpdateModuleData>("SpyVisionUpdate", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let module_name = AsciiString::from("SpyVisionUpdate");
    let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
    let behavior = SpyVisionUpdate::new(module_name_key, data_arc.clone(), owner_id);
    Box::new(SpyVisionUpdateModule::new(behavior, &module_name, data_arc))
}

pub(super) fn slaved_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn slaved_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SlavedUpdateModuleData>("SlavedUpdate", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let behavior =
        SlavedUpdate::new(owner_id, data_arc.clone()).expect("SlavedUpdate failed to initialize");
    Box::new(SlavedUpdateModule::new(
        behavior,
        &AsciiString::from("SlavedUpdate"),
        data_arc,
    ))
}

pub(super) fn mob_member_slaved_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn mob_member_slaved_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<MobMemberSlavedUpdateModuleData>(
        "MobMemberSlavedUpdate",
        &module_data,
    );
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("MobMemberSlavedUpdate", &module_data);
    };
    let legacy_data: Arc<dyn LegacyModuleData> = data_arc.clone();
    let behavior = MobMemberSlavedUpdate::new(object, legacy_data)
        .expect("MobMemberSlavedUpdate failed to initialize");
    Box::new(MobMemberSlavedUpdateModule::new(
        behavior,
        &AsciiString::from("MobMemberSlavedUpdate"),
        data_arc,
    ))
}

pub(super) fn fire_spread_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn fire_spread_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<FireSpreadUpdateModuleData>("FireSpreadUpdate", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let behavior = FireSpreadUpdate::new(owner_id, (*data_arc).clone());
    Box::new(FireSpreadUpdateModule::new(
        behavior,
        &AsciiString::from("FireSpreadUpdate"),
        data_arc,
    ))
}

pub(super) fn rebuild_hole_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn rebuild_hole_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<RebuildHoleBehaviorModuleData>("RebuildHoleBehavior", &module_data);
    let behavior = RebuildHoleBehavior::from_module_thing(thing, data_arc.clone());
    Box::new(RebuildHoleBehaviorModule::new(
        behavior,
        &AsciiString::from("RebuildHoleBehavior"),
        data_arc,
    ))
}

pub(super) fn overcharge_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn overcharge_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<OverchargeBehaviorModuleData>("OverchargeBehavior", &module_data);
    let behavior = OverchargeBehavior::from_module_thing(thing, data_arc.clone());
    Box::new(OverchargeBehaviorModule::new(
        behavior,
        &"OverchargeBehavior".to_string(),
        data_arc,
    ))
}

pub(super) fn auto_heal_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AutoHealBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AutoHealBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn auto_heal_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<AutoHealBehaviorModuleData>("AutoHealBehavior", &module_data);
    let behavior = AutoHealBehavior::from_module_thing(thing, data_arc.clone());
    Box::new(AutoHealBehaviorModule::new(
        behavior,
        &AsciiString::from("AutoHealBehavior"),
        data_arc,
    ))
}

pub(super) fn countermeasures_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CountermeasuresBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CountermeasuresBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn countermeasures_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CountermeasuresBehaviorModuleData>(
        "CountermeasuresBehavior",
        &module_data,
    );
    let behavior = match CountermeasuresBehavior::from_module_thing(thing, data_arc.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
            return missing_owner_module("CountermeasuresBehavior", data_for_missing);
        }
    };
    Box::new(CountermeasuresBehaviorModule::new(
        behavior,
        &AsciiString::from("CountermeasuresBehavior"),
        data_arc,
    ))
}

pub(super) fn dumb_projectile_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DumbProjectileBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DumbProjectileBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn dumb_projectile_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<DumbProjectileBehaviorModuleData>(
        "DumbProjectileBehavior",
        &module_data,
    );
    let behavior = match DumbProjectileBehavior::from_module_thing(thing, data_arc.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
            return missing_owner_module("DumbProjectileBehavior", data_for_missing);
        }
    };
    Box::new(DumbProjectileBehaviorModule::new(
        behavior,
        &AsciiString::from("DumbProjectileBehavior"),
        data_arc,
    ))
}

pub(super) fn bridge_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BridgeBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BridgeBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn bridge_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<BridgeBehaviorModuleData>("BridgeBehavior", &module_data);
    let behavior = match BridgeBehavior::from_module_thing(thing, data_arc.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
            return missing_owner_module("BridgeBehavior", data_for_missing);
        }
    };
    Box::new(BridgeBehaviorModule::new(
        behavior,
        &AsciiString::from("BridgeBehavior"),
        data_arc,
    ))
}

pub(super) fn bridge_scaffold_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn bridge_scaffold_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<BridgeScaffoldBehaviorModuleData>(
        "BridgeScaffoldBehavior",
        &module_data,
    );
    let behavior = match BridgeScaffoldBehavior::from_module_thing(thing, data_arc.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
            return missing_owner_module("BridgeScaffoldBehavior", data_for_missing);
        }
    };
    Box::new(BridgeScaffoldBehaviorModule::new(
        behavior,
        &AsciiString::from("BridgeScaffoldBehavior"),
        data_arc,
    ))
}

pub(super) fn bridge_tower_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn bridge_tower_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<BridgeTowerBehaviorModuleData>("BridgeTowerBehavior", &module_data);
    let behavior = match BridgeTowerBehavior::from_module_thing(thing, data_arc.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
            return missing_owner_module("BridgeTowerBehavior", data_for_missing);
        }
    };
    Box::new(BridgeTowerBehaviorModule::new(
        behavior,
        &AsciiString::from("BridgeTowerBehavior"),
        data_arc,
    ))
}

pub(super) fn structure_collapse_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn structure_collapse_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("StructureCollapseUpdate", &module_data);
    };
    let data_arc = cloned_module_data::<StructureCollapseUpdateModuleData>(
        "StructureCollapseUpdate",
        &module_data,
    );
    let behavior = match StructureCollapseUpdate::new_with_data(owner, data_arc.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
            return missing_owner_module("StructureCollapseUpdate", data_for_missing);
        }
    };
    Box::new(StructureCollapseUpdateModule::new(
        behavior,
        &AsciiString::from("StructureCollapseUpdate"),
        data_arc,
    ))
}

pub(super) fn structure_topple_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn structure_topple_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("StructureToppleUpdate", &module_data);
    };
    let data_arc = cloned_module_data::<StructureToppleUpdateModuleData>(
        "StructureToppleUpdate",
        &module_data,
    );
    let behavior = match StructureToppleUpdate::new_with_data(owner, data_arc.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
            return missing_owner_module("StructureToppleUpdate", data_for_missing);
        }
    };
    Box::new(StructureToppleUpdateModule::new(
        behavior,
        &AsciiString::from("StructureToppleUpdate"),
        data_arc,
    ))
}

pub(super) fn grant_stealth_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GrantStealthBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GrantStealthBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn grant_stealth_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("GrantStealthBehavior", &module_data);
    };
    let data_arc =
        cloned_module_data::<GrantStealthBehaviorModuleData>("GrantStealthBehavior", &module_data);
    let behavior = match GrantStealthBehavior::new_with_data(owner, data_arc.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
            return missing_owner_module("GrantStealthBehavior", data_for_missing);
        }
    };
    Box::new(GrantStealthBehaviorModule::new(
        behavior,
        &AsciiString::from("GrantStealthBehavior"),
        data_arc,
    ))
}

pub(super) fn stealth_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CoreStealthUpdateModuleData::default();
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

pub(super) fn stealth_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CoreStealthUpdateModuleData>("StealthUpdate", &module_data);
    let object_id = resolve_owner_id(&thing);
    let module_name_key = NameKeyGenerator::name_to_key("StealthUpdate");
    Box::new(CoreStealthUpdateModule::new(
        module_name_key,
        data_arc,
        object_id,
    ))
}

pub(super) fn transition_damage_fx_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
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

pub(super) fn transition_damage_fx_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<TransitionDamageFXModuleData>("TransitionDamageFX", &module_data);
    let behavior = match TransitionDamageFX::from_module_thing(thing, data_arc.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
            return missing_owner_module("TransitionDamageFX", data_for_missing);
        }
    };
    Box::new(TransitionDamageFXModule::new(
        behavior,
        &AsciiString::from("TransitionDamageFX"),
        data_arc,
    ))
}

pub(super) fn emp_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn emp_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let owner = match TheGameLogic::find_object_by_id(owner_id) {
        Some(object) => object,
        None => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module_auto("EMPUpdate", &module_data);
        }
    };
    let data_arc = cloned_module_data::<EMPUpdateModuleData>("EMPUpdate", &module_data);
    let behavior =
        EMPUpdate::new_with_data(owner, data_arc.clone()).expect("EMPUpdate failed to initialize");
    Box::new(EMPUpdateModule::new(
        behavior,
        &AsciiString::from("EMPUpdate"),
        data_arc,
    ))
}

pub(super) fn bone_fx_update_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn bone_fx_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let data_arc = cloned_module_data::<BoneFXUpdateModuleData>("BoneFXUpdate", &module_data);
    let behavior = BoneFXUpdate::new(owner_id, data_arc.clone());
    Box::new(BoneFXUpdateModule::new(
        behavior,
        &AsciiString::from("BoneFXUpdate"),
        data_arc,
    ))
}

pub(super) fn bone_fx_damage_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(DamageModuleData::default())
}

pub(super) fn bone_fx_damage_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let data_arc = cloned_module_data::<DamageModuleData>("BoneFXDamage", &module_data);
    let behavior = BoneFXDamage::new(owner_id);
    Box::new(BoneFXDamageModule::new(
        behavior,
        &AsciiString::from("BoneFXDamage"),
        data_arc,
    ))
}

pub(super) fn spawn_behavior_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpawnBehaviorModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpawnBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn spawn_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let owner = match TheGameLogic::find_object_by_id(owner_id) {
        Some(object) => object,
        None => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module_auto("SpawnBehavior", &module_data);
        }
    };
    let data_arc = cloned_module_data::<SpawnBehaviorModuleData>("SpawnBehavior", &module_data);
    let behavior = SpawnBehavior::new_with_data(
        owner
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(crate::common::INVALID_ID),
        data_arc.clone(),
    )
    .expect("SpawnBehavior failed to initialize");
    Box::new(SpawnBehaviorModule::new(
        behavior,
        &AsciiString::from("SpawnBehavior"),
        data_arc,
    ))
}

pub(super) fn particle_uplink_cannon_update_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = ParticleUplinkCannonUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ParticleUplinkCannonUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn particle_uplink_cannon_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("ParticleUplinkCannonUpdate", &module_data);
    };
    let data_arc = cloned_module_data::<ParticleUplinkCannonUpdateModuleData>(
        "ParticleUplinkCannonUpdate",
        &module_data,
    );
    let behavior = ParticleUplinkCannonUpdate::new_with_data(owner, data_arc.clone())
        .expect("ParticleUplinkCannonUpdate failed to initialize");
    Box::new(ParticleUplinkCannonUpdateModule::new(
        behavior,
        &AsciiString::from("ParticleUplinkCannonUpdate"),
        data_arc,
    ))
}
