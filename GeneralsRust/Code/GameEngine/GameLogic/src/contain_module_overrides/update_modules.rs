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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<BattlePlanUpdateModuleData>());
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("BattlePlanUpdate", &module_data);
    };
    let behavior =
        BattlePlanUpdate::new_with_module_data(object, module_data.clone()).expect("BattlePlanUpdate failed to initialize");
    Box::new(BattlePlanUpdateModule::new(
        behavior,
        &AsciiString::from("BattlePlanUpdate"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CleanupHazardUpdateModuleData>());
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("CleanupHazardUpdate", &module_data);
    };
    let behavior = CleanupHazardUpdate::new(object, module_data.clone())
        .expect("CleanupHazardUpdate failed to initialize");
    Box::new(CleanupHazardUpdateModule::new(
        behavior,
        &AsciiString::from("CleanupHazardUpdate"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CommandButtonHuntUpdateModuleData>());
    let owner_id = resolve_owner_id(&thing);
    let behavior = CommandButtonHuntUpdate::new(owner_id, module_data.clone());
    Box::new(CommandButtonHuntUpdateModule::new(
        behavior,
        &AsciiString::from("CommandButtonHuntUpdate"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<SpyVisionUpdateModuleData>());
    let owner_id = resolve_owner_id(&thing);
    let module_name = AsciiString::from("SpyVisionUpdate");
    let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
    let behavior = SpyVisionUpdate::new(module_name_key, module_data.clone(), owner_id);
    Box::new(SpyVisionUpdateModule::new(behavior, &module_name, module_data))
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<SlavedUpdateModuleData>());
    let owner_id = resolve_owner_id(&thing);
    let behavior =
        SlavedUpdate::new(owner_id, module_data.clone()).expect("SlavedUpdate failed to initialize");
    Box::new(SlavedUpdateModule::new(
        behavior,
        &AsciiString::from("SlavedUpdate"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<MobMemberSlavedUpdateModuleData>());
    let owner_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("MobMemberSlavedUpdate", &module_data);
    };
    let behavior = MobMemberSlavedUpdate::new(object, module_data.clone())
        .expect("MobMemberSlavedUpdate failed to initialize");
    Box::new(MobMemberSlavedUpdateModule::new(
        behavior,
        &AsciiString::from("MobMemberSlavedUpdate"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<FireSpreadUpdateModuleData>());
    let behavior = FireSpreadUpdate::with_module_data(resolve_owner_id(&thing), module_data.clone());
    Box::new(FireSpreadUpdateModule::new(
        behavior,
        &AsciiString::from("FireSpreadUpdate"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<RebuildHoleBehaviorModuleData>());
    let behavior = RebuildHoleBehavior::from_module_thing(thing, module_data.clone());
    Box::new(RebuildHoleBehaviorModule::new(
        behavior,
        &AsciiString::from("RebuildHoleBehavior"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<OverchargeBehaviorModuleData>());
    let behavior = OverchargeBehavior::from_module_thing(thing, module_data.clone());
    Box::new(OverchargeBehaviorModule::new(
        behavior,
        &"OverchargeBehavior".to_string(),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<AutoHealBehaviorModuleData>());
    let behavior = AutoHealBehavior::from_module_thing(thing, module_data.clone());
    Box::new(AutoHealBehaviorModule::new(
        behavior,
        &AsciiString::from("AutoHealBehavior"),
        module_data,
    ))
}

pub(crate) fn register_inert_auto_heal(
    factory: &mut game_engine::common::thing::module_factory::ModuleFactory,
) -> Result<(), String> {
    factory.register_override(
        "AutoHealBehavior",
        ModuleType::Behavior,
        auto_heal_behavior_module_factory_inert,
        auto_heal_behavior_data_factory,
    )
}
fn auto_heal_behavior_module_factory_inert(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<AutoHealBehaviorModuleData>());
    let id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or(crate::common::INVALID_ID);
    Box::new(AutoHealBehaviorModule::new(
        AutoHealBehavior::construct_inert_with_object_id(id, module_data.clone()),
        &AsciiString::from("AutoHealBehavior"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CountermeasuresBehaviorModuleData>());
    let behavior = match CountermeasuresBehavior::from_module_thing(thing, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module("CountermeasuresBehavior", module_data);
        }
    };
    Box::new(CountermeasuresBehaviorModule::new(
        behavior,
        &AsciiString::from("CountermeasuresBehavior"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<DumbProjectileBehaviorModuleData>());
    let behavior = match DumbProjectileBehavior::from_module_thing(thing, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module("DumbProjectileBehavior", module_data);
        }
    };
    Box::new(DumbProjectileBehaviorModule::new(
        behavior,
        &AsciiString::from("DumbProjectileBehavior"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<BridgeBehaviorModuleData>());
    let behavior = match BridgeBehavior::from_module_thing(thing, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module("BridgeBehavior", module_data);
        }
    };
    Box::new(BridgeBehaviorModule::new(
        behavior,
        &AsciiString::from("BridgeBehavior"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<BridgeScaffoldBehaviorModuleData>());
    let behavior = match BridgeScaffoldBehavior::from_module_thing(thing, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module("BridgeScaffoldBehavior", module_data);
        }
    };
    Box::new(BridgeScaffoldBehaviorModule::new(
        behavior,
        &AsciiString::from("BridgeScaffoldBehavior"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<BridgeTowerBehaviorModuleData>());
    let behavior = match BridgeTowerBehavior::from_module_thing(thing, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module("BridgeTowerBehavior", module_data);
        }
    };
    Box::new(BridgeTowerBehaviorModule::new(
        behavior,
        &AsciiString::from("BridgeTowerBehavior"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("StructureCollapseUpdate", &module_data);
    };
    assert!(module_data.is::<StructureCollapseUpdateModuleData>());
    let behavior = match StructureCollapseUpdate::new_with_data(owner, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module("StructureCollapseUpdate", module_data);
        }
    };
    Box::new(StructureCollapseUpdateModule::new(
        behavior,
        &AsciiString::from("StructureCollapseUpdate"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("StructureToppleUpdate", &module_data);
    };
    assert!(module_data.is::<StructureToppleUpdateModuleData>());
    let behavior = match StructureToppleUpdate::new_with_data(owner, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module("StructureToppleUpdate", module_data);
        }
    };
    Box::new(StructureToppleUpdateModule::new(
        behavior,
        &AsciiString::from("StructureToppleUpdate"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("GrantStealthBehavior", &module_data);
    };
    assert!(module_data.is::<GrantStealthBehaviorModuleData>());
    let behavior = match GrantStealthBehavior::new_with_data(owner, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module("GrantStealthBehavior", module_data);
        }
    };
    Box::new(GrantStealthBehaviorModule::new(
        behavior,
        &AsciiString::from("GrantStealthBehavior"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CoreStealthUpdateModuleData>());
    let object_id = resolve_owner_id(&thing);
    let module_name_key = NameKeyGenerator::name_to_key("StealthUpdate");
    Box::new(CoreStealthUpdateModule::new_staged(
        module_name_key,
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<TransitionDamageFXModuleData>());
    let behavior = match TransitionDamageFX::from_module_thing(thing, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(_) => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module("TransitionDamageFX", module_data);
        }
    };
    Box::new(TransitionDamageFXModule::new(
        behavior,
        &AsciiString::from("TransitionDamageFX"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let owner = match TheGameLogic::find_object_by_id(owner_id) {
        Some(object) => object,
        None => {
            // Wave 449: missing owner → no-op module.
            return missing_owner_module_auto("EMPUpdate", &module_data);
        }
    };
    assert!(module_data.is::<EMPUpdateModuleData>());
    let behavior =
        EMPUpdate::new_with_data(owner, module_data.clone()).expect("EMPUpdate failed to initialize");
    Box::new(EMPUpdateModule::new(
        behavior,
        &AsciiString::from("EMPUpdate"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    assert!(module_data.is::<BoneFXUpdateModuleData>());
    let behavior = BoneFXUpdate::new(owner_id, module_data.clone());
    Box::new(BoneFXUpdateModule::new(
        behavior,
        &AsciiString::from("BoneFXUpdate"),
        module_data,
    ))
}

pub(super) fn bone_fx_damage_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(DamageModuleData::default())
}

pub(super) fn bone_fx_damage_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    assert!(module_data.is::<DamageModuleData>());
    let behavior = BoneFXDamage::new(owner_id);
    Box::new(BoneFXDamageModule::new(
        behavior,
        &AsciiString::from("BoneFXDamage"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    // C++ SpawnBehavior(thing, moduleData): the supplied thing is the owner;
    // owned construction must not require a registry publication.
    let owner_id = resolve_owner_id(&thing);
    if owner_id == crate::common::INVALID_ID {
        return missing_owner_module_auto("SpawnBehavior", &module_data);
    }
    assert!(module_data.is::<SpawnBehaviorModuleData>());
    let behavior = SpawnBehavior::new_with_data(owner_id, module_data.clone())
    .expect("SpawnBehavior failed to initialize");
    Box::new(SpawnBehaviorModule::new(
        behavior,
        &AsciiString::from("SpawnBehavior"),
        module_data,
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
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let owner_id = resolve_owner_id(&thing);
    let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("ParticleUplinkCannonUpdate", &module_data);
    };
    assert!(module_data.is::<ParticleUplinkCannonUpdateModuleData>());
    let behavior = ParticleUplinkCannonUpdate::new_with_data(owner, module_data.clone())
        .expect("ParticleUplinkCannonUpdate failed to initialize");
    Box::new(ParticleUplinkCannonUpdateModule::new(
        behavior,
        &AsciiString::from("ParticleUplinkCannonUpdate"),
        module_data,
    ))
}
