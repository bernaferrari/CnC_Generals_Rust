//! Strict additional updates module-data registrations.
use super::*;

pub(super) fn register(factory: &mut impl DataParserRegistrar) {
    register_parsed!(factory, Behavior, "BattleBusSlowDeathBehavior", crate::object::behavior::battle_bus_slow_death_behavior::BattleBusSlowDeathBehaviorModuleData, default);
    register_parsed!(factory, Behavior, "NeutronMissileSlowDeathBehavior", crate::object::behavior::neutron_missile_slow_death_update::NeutronMissileSlowDeathUpdateModuleData, default);
    register_parsed!(
        factory,
        Behavior,
        "NeutronMissileUpdate",
        crate::object::update::neutron_missile_update::NeutronMissileUpdateModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "ToppleUpdate",
        crate::object::behavior::topple_update::ToppleUpdateModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "LifetimeUpdate",
        crate::object::behavior::lifetime_update::LifetimeUpdateModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "OCLUpdate",
        crate::object::update::ocl_update::OCLUpdateModuleData,
        default
    );
    register_empty!(
        factory,
        Behavior,
        "RadiusDecalUpdate",
        crate::object::behavior::radius_decal_update::RadiusDecalUpdateModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "StickyBombUpdate",
        crate::object::behavior::sticky_bomb_update::StickyBombUpdateModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "DemoTrapUpdate",
        crate::object::behavior::demo_trap_update::DemoTrapUpdateModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "PointDefenseLaserUpdate",
        crate::object::behavior::point_defense_laser_update::PointDefenseLaserUpdateModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "ProjectileStreamUpdate",
        crate::object::behavior::projectile_stream_update::ProjectileStreamUpdateModuleData,
        default
    );
    register_parsed!(factory, Behavior, "SmartBombTargetHomingUpdate", crate::object::behavior::smart_bomb_target_homing_update::SmartBombTargetHomingUpdateModuleData, default);
    register_parsed!(
        factory,
        Behavior,
        "TensileFormationUpdate",
        crate::object::behavior::tensile_formation_update::TensileFormationUpdateModuleData,
        default
    );
}

#[cfg(test)]
pub(super) const NAMES: &[&str] = &[
    "BattleBusSlowDeathBehavior",
    "NeutronMissileSlowDeathBehavior",
    "NeutronMissileUpdate",
    "ToppleUpdate",
    "LifetimeUpdate",
    "OCLUpdate",
    "RadiusDecalUpdate",
    "StickyBombUpdate",
    "DemoTrapUpdate",
    "PointDefenseLaserUpdate",
    "ProjectileStreamUpdate",
    "SmartBombTargetHomingUpdate",
    "TensileFormationUpdate",
];
