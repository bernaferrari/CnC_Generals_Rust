//! Leftover override factories (AI update, upgrade, special power).
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::helpers::*;
use super::*;

pub(super) fn ai_update_interface_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn ai_update_interface_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<AIUpdateModuleData>("AIUpdateInterface", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("AIUpdateInterface");
    Box::new(AIUpdateInterfaceModule::new(module_name_key, data_arc))
}

macro_rules! ai_update_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        pub(super) fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        "Failed to parse {} module data at line {}: {}",
                        $module_name,
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        pub(super) fn $module_factory(
            _thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<$data_ty>($module_name, &module_data);
            let module_name_key = NameKeyGenerator::name_to_key($module_name);
            Box::new(<$module_ty>::new(module_name_key, data_arc))
        }
    };
}

ai_update_factories!(
    transport_ai_update_data_factory,
    transport_ai_update_module_factory,
    TransportAIUpdateModuleData,
    TransportAIUpdateModule,
    "TransportAIUpdate"
);
ai_update_factories!(
    deploy_style_ai_update_data_factory,
    deploy_style_ai_update_module_factory,
    DeployStyleAIUpdateModuleData,
    DeployStyleAIUpdateModule,
    "DeployStyleAIUpdate"
);
ai_update_factories!(
    wander_ai_update_data_factory,
    wander_ai_update_module_factory,
    WanderAIUpdateModuleData,
    WanderAIUpdateModule,
    "WanderAIUpdate"
);
ai_update_factories!(
    jet_ai_update_data_factory,
    jet_ai_update_module_factory,
    JetAIUpdateModuleData,
    JetAIUpdateModule,
    "JetAIUpdate"
);
ai_update_factories!(
    railed_transport_ai_update_data_factory,
    railed_transport_ai_update_module_factory,
    RailedTransportAIUpdateModuleData,
    RailedTransportAIUpdateModule,
    "RailedTransportAIUpdate"
);
ai_update_factories!(
    assault_transport_ai_update_data_factory,
    assault_transport_ai_update_module_factory,
    AssaultTransportAIUpdateModuleData,
    AssaultTransportAIUpdateModule,
    "AssaultTransportAIUpdate"
);
ai_update_factories!(
    deliver_payload_ai_update_data_factory,
    deliver_payload_ai_update_module_factory,
    DeliverPayloadAIUpdateModuleData,
    DeliverPayloadAIUpdateModule,
    "DeliverPayloadAIUpdate"
);
ai_update_factories!(
    hack_internet_ai_update_data_factory,
    hack_internet_ai_update_module_factory,
    HackInternetAIUpdateModuleData,
    HackInternetAIUpdateModule,
    "HackInternetAIUpdate"
);
ai_update_factories!(
    supply_truck_ai_update_data_factory,
    supply_truck_ai_update_module_factory,
    SupplyTruckAIUpdateModuleData,
    SupplyTruckAIUpdateModule,
    "SupplyTruckAIUpdate"
);
ai_update_factories!(
    chinook_ai_update_data_factory,
    chinook_ai_update_module_factory,
    ChinookAIUpdateModuleData,
    ChinookAIUpdateModule,
    "ChinookAIUpdate"
);
ai_update_factories!(
    worker_ai_update_data_factory,
    worker_ai_update_module_factory,
    WorkerAIUpdateModuleData,
    WorkerAIUpdateModule,
    "WorkerAIUpdate"
);
ai_update_factories!(
    dozer_ai_update_data_factory,
    dozer_ai_update_module_factory,
    DozerAIUpdateModuleData,
    DozerAIUpdateModule,
    "DozerAIUpdate"
);

macro_rules! upgrade_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        pub(super) fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        "Failed to parse {} module data at line {}: {}",
                        $module_name,
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        pub(super) fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<$data_ty>($module_name, &module_data);
            let module_name_key = NameKeyGenerator::name_to_key($module_name);
            let owner_id = resolve_owner_id(&thing);
            Box::new(<$module_ty>::new(module_name_key, data_arc, owner_id))
        }
    };
}

macro_rules! empty_upgrade_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        pub(super) fn $data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            Box::new(<$data_ty>::default())
        }

        pub(super) fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<$data_ty>($module_name, &module_data);
            let module_name_key = NameKeyGenerator::name_to_key($module_name);
            let owner_id = resolve_owner_id(&thing);
            Box::new(<$module_ty>::new(module_name_key, data_arc, owner_id))
        }
    };
}

upgrade_factories!(
    armor_upgrade_data_factory,
    armor_upgrade_module_factory,
    ArmorUpgradeModuleData,
    ArmorUpgrade,
    "ArmorUpgrade"
);
upgrade_factories!(
    status_bits_upgrade_data_factory,
    status_bits_upgrade_module_factory,
    StatusBitsUpgradeModuleData,
    StatusBitsUpgrade,
    "StatusBitsUpgrade"
);
upgrade_factories!(
    active_shroud_upgrade_data_factory,
    active_shroud_upgrade_module_factory,
    ActiveShroudUpgradeModuleData,
    ActiveShroudUpgrade,
    "ActiveShroudUpgrade"
);

upgrade_factories!(
    command_set_upgrade_data_factory,
    command_set_upgrade_module_factory,
    CommandSetUpgradeModuleData,
    CommandSetUpgrade,
    "CommandSetUpgrade"
);
upgrade_factories!(
    cost_modifier_upgrade_data_factory,
    cost_modifier_upgrade_module_factory,
    CostModifierUpgradeModuleData,
    CostModifierUpgrade,
    "CostModifierUpgrade"
);
upgrade_factories!(
    experience_scalar_upgrade_data_factory,
    experience_scalar_upgrade_module_factory,
    ExperienceScalarUpgradeModuleData,
    ExperienceScalarUpgrade,
    "ExperienceScalarUpgrade"
);
upgrade_factories!(
    grant_science_upgrade_data_factory,
    grant_science_upgrade_module_factory,
    GrantScienceUpgradeModuleData,
    GrantScienceUpgrade,
    "GrantScienceUpgrade"
);
upgrade_factories!(
    locomotor_set_upgrade_data_factory,
    locomotor_set_upgrade_module_factory,
    LocomotorSetUpgradeModuleData,
    LocomotorSetUpgrade,
    "LocomotorSetUpgrade"
);
upgrade_factories!(
    max_health_upgrade_data_factory,
    max_health_upgrade_module_factory,
    MaxHealthUpgradeModuleData,
    MaxHealthUpgrade,
    "MaxHealthUpgrade"
);
upgrade_factories!(
    model_condition_upgrade_data_factory,
    model_condition_upgrade_module_factory,
    ModelConditionUpgradeModuleData,
    ModelConditionUpgrade,
    "ModelConditionUpgrade"
);
upgrade_factories!(
    object_creation_upgrade_data_factory,
    object_creation_upgrade_module_factory,
    ObjectCreationUpgradeModuleData,
    ObjectCreationUpgrade,
    "ObjectCreationUpgrade"
);
upgrade_factories!(
    passengers_fire_upgrade_data_factory,
    passengers_fire_upgrade_module_factory,
    PassengersFireUpgradeModuleData,
    PassengersFireUpgrade,
    "PassengersFireUpgrade"
);
upgrade_factories!(
    power_plant_upgrade_data_factory,
    power_plant_upgrade_module_factory,
    PowerPlantUpgradeModuleData,
    PowerPlantUpgrade,
    "PowerPlantUpgrade"
);
upgrade_factories!(
    radar_upgrade_data_factory,
    radar_upgrade_module_factory,
    RadarUpgradeModuleData,
    RadarUpgrade,
    "RadarUpgrade"
);
upgrade_factories!(
    replace_object_upgrade_data_factory,
    replace_object_upgrade_module_factory,
    ReplaceObjectUpgradeModuleData,
    ReplaceObjectUpgrade,
    "ReplaceObjectUpgrade"
);
upgrade_factories!(
    stealth_upgrade_data_factory,
    stealth_upgrade_module_factory,
    StealthUpgradeModuleData,
    StealthUpgrade,
    "StealthUpgrade"
);
upgrade_factories!(
    subobjects_upgrade_data_factory,
    subobjects_upgrade_module_factory,
    SubObjectsUpgradeModuleData,
    SubObjectsUpgrade,
    "SubObjectsUpgrade"
);
upgrade_factories!(
    unpause_special_power_upgrade_data_factory,
    unpause_special_power_upgrade_module_factory,
    UnpauseSpecialPowerUpgradeModuleData,
    UnpauseSpecialPowerUpgrade,
    "UnpauseSpecialPowerUpgrade"
);
upgrade_factories!(
    weapon_bonus_upgrade_data_factory,
    weapon_bonus_upgrade_module_factory,
    WeaponBonusUpgradeModuleData,
    WeaponBonusUpgrade,
    "WeaponBonusUpgrade"
);
upgrade_factories!(
    weapon_set_upgrade_data_factory,
    weapon_set_upgrade_module_factory,
    WeaponSetUpgradeModuleData,
    WeaponSetUpgrade,
    "WeaponSetUpgrade"
);

macro_rules! special_power_factories {
    (
        $data_factory:ident,
        $module_factory:ident,
        $data_ty:ty,
        $module_ty:ty,
        $module_name:literal
    ) => {
        pub(super) fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        concat!(
                            "Failed to parse ",
                            $module_name,
                            " module data at line {}: {}"
                        ),
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        pub(super) fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let typed_data = cloned_module_data_or_default::<$data_ty>($module_name, &module_data);
            Box::new(<$module_ty>::new(
                NameKeyGenerator::name_to_key($module_name),
                resolve_owner_id(&thing),
                typed_data,
            ))
        }
    };
}

pub(super) fn baikonur_launch_power_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
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

pub(super) fn baikonur_launch_power_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<BaikonurLaunchPowerModuleData>("BaikonurLaunchPower", &module_data);
    Box::new(BaikonurLaunchPower::new(
        NameKeyGenerator::name_to_key("BaikonurLaunchPower"),
        resolve_owner_id(&thing),
        data_arc,
    ))
}

special_power_factories!(
    cash_bounty_power_module_data_factory,
    cash_bounty_power_module_factory,
    CashBountyPowerModuleData,
    CashBountyPower,
    "CashBountyPower"
);
special_power_factories!(
    cash_hack_special_power_module_data_factory,
    cash_hack_special_power_module_factory,
    CashHackSpecialPowerModuleData,
    CashHackSpecialPower,
    "CashHackSpecialPower"
);
special_power_factories!(
    cleanup_area_power_module_data_factory,
    cleanup_area_power_module_factory,
    CleanupAreaPowerModuleData,
    CleanupAreaPower,
    "CleanupAreaPower"
);
special_power_factories!(
    fire_weapon_power_module_data_factory,
    fire_weapon_power_module_factory,
    FireWeaponPowerModuleData,
    FireWeaponPower,
    "FireWeaponPower"
);
special_power_factories!(
    ocl_special_power_module_data_factory,
    ocl_special_power_module_factory,
    OclSpecialPowerModuleData,
    OclSpecialPower,
    "OCLSpecialPower"
);
special_power_factories!(
    special_ability_module_data_factory,
    special_ability_module_factory,
    SpecialAbilityModuleData,
    SpecialAbility,
    "SpecialAbility"
);
special_power_factories!(
    spy_vision_special_power_module_data_factory,
    spy_vision_special_power_module_factory,
    SpyVisionSpecialPowerModuleData,
    SpyVisionSpecialPower,
    "SpyVisionSpecialPower"
);
special_power_factories!(
    defector_special_power_module_data_factory,
    defector_special_power_module_factory,
    DefectorSpecialPowerModuleData,
    DefectorSpecialPower,
    "DefectorSpecialPower"
);
