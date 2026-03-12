use crate::modules::{SpecialPowerModuleInterface, SpecialPowerUpdateInterface};
use game_engine::common::thing::module::Module;
use std::any::Any;

pub(crate) enum SpecialPowerModuleKindMut<'a> {
    SpecialPowerModule(&'a mut crate::object::special_power_module::SpecialPowerModule),
    BaikonurLaunchPower(
        &'a mut crate::object::special_powers::baikonur_launch_power::BaikonurLaunchPower,
    ),
    CashBountyPower(&'a mut crate::object::special_powers::cash_bounty_power::CashBountyPower),
    CashHackSpecialPower(
        &'a mut crate::object::special_powers::cash_hack_special_power::CashHackSpecialPower,
    ),
    CleanupAreaPower(&'a mut crate::object::special_powers::cleanup_area_power::CleanupAreaPower),
    DefectorSpecialPower(
        &'a mut crate::object::special_powers::defector_special_power::DefectorSpecialPower,
    ),
    DemoralizeSpecialPower(
        &'a mut crate::object::special_powers::demoralize_special_power::DemoralizeSpecialPower,
    ),
    FireWeaponPower(&'a mut crate::object::special_powers::fire_weapon_power::FireWeaponPower),
    OclSpecialPower(&'a mut crate::object::special_powers::ocl_special_power::OclSpecialPower),
    SpecialAbility(&'a mut crate::object::special_powers::special_ability::SpecialAbility),
    SpyVisionSpecialPower(
        &'a mut crate::object::special_powers::spy_vision_special_power::SpyVisionSpecialPower,
    ),
}

impl<'a> SpecialPowerModuleKindMut<'a> {
    pub(crate) fn into_interface(self) -> &'a mut dyn SpecialPowerModuleInterface {
        match self {
            Self::SpecialPowerModule(module) => module,
            Self::BaikonurLaunchPower(module) => module,
            Self::CashBountyPower(module) => module,
            Self::CashHackSpecialPower(module) => module,
            Self::CleanupAreaPower(module) => module,
            Self::DefectorSpecialPower(module) => module,
            Self::DemoralizeSpecialPower(module) => module,
            Self::FireWeaponPower(module) => module,
            Self::OclSpecialPower(module) => module,
            Self::SpecialAbility(module) => module,
            Self::SpyVisionSpecialPower(module) => module,
        }
    }

    pub(crate) fn into_base_special_power_module(
        self,
    ) -> Option<&'a mut crate::object::special_power_module::SpecialPowerModule> {
        match self {
            Self::SpecialPowerModule(module) => Some(module),
            _ => None,
        }
    }
}

pub(crate) enum SpecialPowerUpdateKindMut<'a> {
    SpecialPowerUpdate(
        &'a mut crate::object::update::special_power_update::SpecialPowerUpdateModule,
    ),
    SpecialAbilityUpdate(
        &'a mut crate::object::behavior::special_ability_update::SpecialAbilityUpdateModule,
    ),
    SpectreGunshipUpdate(
        &'a mut crate::object::behavior::spectre_gunship_update::SpectreGunshipUpdateModule,
    ),
    SpectreGunshipDeploymentUpdate(
        &'a mut crate::object::behavior::spectre_gunship_deployment_update::SpectreGunshipDeploymentUpdateModule,
    ),
    ParticleUplinkCannonUpdate(
        &'a mut crate::object::behavior::particle_uplink_cannon_update::ParticleUplinkCannonUpdateModule,
    ),
    BattlePlanUpdate(
        &'a mut crate::object::behavior::battle_plan_update::BattlePlanUpdateModule,
    ),
    MissileLauncherBuildingUpdate(
        &'a mut crate::object::behavior::missile_launcher_building_update::MissileLauncherBuildingUpdateModule,
    ),
}

impl<'a> SpecialPowerUpdateKindMut<'a> {
    pub(crate) fn into_interface(self) -> &'a mut dyn SpecialPowerUpdateInterface {
        match self {
            Self::SpecialPowerUpdate(module) => module,
            Self::SpecialAbilityUpdate(module) => module.behavior_mut(),
            Self::SpectreGunshipUpdate(module) => module.behavior_mut(),
            Self::SpectreGunshipDeploymentUpdate(module) => module.behavior_mut(),
            Self::ParticleUplinkCannonUpdate(module) => module.behavior_mut(),
            Self::BattlePlanUpdate(module) => module.behavior_mut(),
            Self::MissileLauncherBuildingUpdate(module) => module.behavior_mut(),
        }
    }
}

/// Returns the concrete special-power module kind for typed dispatch.
pub(crate) fn module_special_power_kind(
    module: &mut dyn Module,
) -> Option<SpecialPowerModuleKindMut<'_>> {
    if module
        .as_any()
        .is::<crate::object::special_power_module::SpecialPowerModule>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_power_module::SpecialPowerModule>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::SpecialPowerModule(module));
    }
    if module
        .as_any()
        .is::<crate::object::special_powers::baikonur_launch_power::BaikonurLaunchPower>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_powers::baikonur_launch_power::BaikonurLaunchPower>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::BaikonurLaunchPower(module));
    }
    if module
        .as_any()
        .is::<crate::object::special_powers::cash_bounty_power::CashBountyPower>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_powers::cash_bounty_power::CashBountyPower>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::CashBountyPower(module));
    }
    if module
        .as_any()
        .is::<crate::object::special_powers::cash_hack_special_power::CashHackSpecialPower>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_powers::cash_hack_special_power::CashHackSpecialPower>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::CashHackSpecialPower(module));
    }
    if module
        .as_any()
        .is::<crate::object::special_powers::cleanup_area_power::CleanupAreaPower>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_powers::cleanup_area_power::CleanupAreaPower>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::CleanupAreaPower(module));
    }
    if module
        .as_any()
        .is::<crate::object::special_powers::defector_special_power::DefectorSpecialPower>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_powers::defector_special_power::DefectorSpecialPower>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::DefectorSpecialPower(module));
    }
    if module
        .as_any()
        .is::<crate::object::special_powers::demoralize_special_power::DemoralizeSpecialPower>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_powers::demoralize_special_power::DemoralizeSpecialPower>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::DemoralizeSpecialPower(module));
    }
    if module
        .as_any()
        .is::<crate::object::special_powers::fire_weapon_power::FireWeaponPower>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_powers::fire_weapon_power::FireWeaponPower>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::FireWeaponPower(module));
    }
    if module
        .as_any()
        .is::<crate::object::special_powers::ocl_special_power::OclSpecialPower>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_powers::ocl_special_power::OclSpecialPower>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::OclSpecialPower(module));
    }
    if module
        .as_any()
        .is::<crate::object::special_powers::special_ability::SpecialAbility>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_powers::special_ability::SpecialAbility>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::SpecialAbility(module));
    }
    if module
        .as_any()
        .is::<crate::object::special_powers::spy_vision_special_power::SpyVisionSpecialPower>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::special_powers::spy_vision_special_power::SpyVisionSpecialPower>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerModuleKindMut::SpyVisionSpecialPower(module));
    }

    None
}

/// Returns a mutable special-power interface when the concrete module supports it.
pub(crate) fn module_special_power_interface(
    module: &mut dyn Module,
) -> Option<&mut dyn SpecialPowerModuleInterface> {
    module_special_power_kind(module).map(SpecialPowerModuleKindMut::into_interface)
}

pub(crate) fn module_base_special_power_module(
    module: &mut dyn Module,
) -> Option<&mut crate::object::special_power_module::SpecialPowerModule> {
    module_special_power_kind(module)
        .and_then(SpecialPowerModuleKindMut::into_base_special_power_module)
}

pub(crate) fn module_special_power_update_kind(
    module: &mut dyn Module,
) -> Option<SpecialPowerUpdateKindMut<'_>> {
    if module
        .as_any()
        .is::<crate::object::update::special_power_update::SpecialPowerUpdateModule>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::update::special_power_update::SpecialPowerUpdateModule>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerUpdateKindMut::SpecialPowerUpdate(module));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::special_ability_update::SpecialAbilityUpdateModule>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::special_ability_update::SpecialAbilityUpdateModule>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerUpdateKindMut::SpecialAbilityUpdate(module));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::spectre_gunship_update::SpectreGunshipUpdateModule>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::spectre_gunship_update::SpectreGunshipUpdateModule>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerUpdateKindMut::SpectreGunshipUpdate(module));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::spectre_gunship_deployment_update::SpectreGunshipDeploymentUpdateModule>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::spectre_gunship_deployment_update::SpectreGunshipDeploymentUpdateModule>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerUpdateKindMut::SpectreGunshipDeploymentUpdate(module));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::particle_uplink_cannon_update::ParticleUplinkCannonUpdateModule>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::particle_uplink_cannon_update::ParticleUplinkCannonUpdateModule>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerUpdateKindMut::ParticleUplinkCannonUpdate(module));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::battle_plan_update::BattlePlanUpdateModule>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::battle_plan_update::BattlePlanUpdateModule>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerUpdateKindMut::BattlePlanUpdate(module));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::missile_launcher_building_update::MissileLauncherBuildingUpdateModule>()
    {
        let module = (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::missile_launcher_building_update::MissileLauncherBuildingUpdateModule>()
            .expect("type check and downcast must match");
        return Some(SpecialPowerUpdateKindMut::MissileLauncherBuildingUpdate(module));
    }

    None
}

pub(crate) fn module_special_power_update_interface(
    module: &mut dyn Module,
) -> Option<&mut dyn SpecialPowerUpdateInterface> {
    module_special_power_update_kind(module).map(SpecialPowerUpdateKindMut::into_interface)
}
