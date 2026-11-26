//! Collect receiving world facts for weapon damage estimation.

use super::*;
pub use generals_sim::weapon::{
    DamageEstimateTarget as HostEstimateVictim,
    WeaponDamageEstimate as HostEstimateWeapon,
    estimate_weapon_template_damage,
};

/// C++ `WeaponTemplate::getDamageType` for an admitted weapon definition.
pub fn host_weapon_is_sniper_damage(name: &str) -> bool {
    super::with_host_weapon_store(|store| {
        store.find_weapon_template(name)
            .is_some_and(|weapon| weapon.damage_type == gamelogic::damage::DamageType::Sniper)
    })
    .unwrap_or(false)
}

/// C++ `WeaponTemplate::m_allowAttackGarrisonedBldgs` (estimate-only).
pub fn host_weapon_allows_attack_garrisoned_bldgs(name: &str) -> bool {
    host_allow_attack_garrisoned_for_weapon_name(name)
}

pub(in crate::game_logic) fn damage_estimate_from_rule(
    rule: &gamelogic::weapon::WeaponTemplate,
    primary_damage: f32,
) -> HostEstimateWeapon {
    use gamelogic::damage::{DamageType, DeathType};

    HostEstimateWeapon {
        damage_type_sniper: rule.damage_type == DamageType::Sniper,
        damage_type_surrender: rule.damage_type == DamageType::Surrender,
        damage_type_disarm: rule.damage_type == DamageType::Disarm,
        damage_type_deploy: rule.damage_type == DamageType::Deploy,
        damage_type_kill_garrisoned: rule.damage_type == DamageType::KillGarrisoned,
        death_type_burned: rule.death_type == DeathType::Burned,
        allow_attack_garrisoned_bldgs: rule.allow_attack_garrisoned_bldgs,
        primary_damage,
    }
}

/// Read all estimate inputs for a mapless slot in one compatibility catalog lookup.
pub(in crate::game_logic) fn host_estimate_named_weapon(
    name: &str,
) -> Option<(gamelogic::damage::DamageType, HostEstimateWeapon)> {
    super::with_host_weapon_store(|store| {
        let rule = store.find_weapon_template(name)?;
        Some((
            rule.damage_type,
            damage_estimate_from_rule(rule, rule.primary_damage),
        ))
    })
    .ok()
    .flatten()
}

/// Build the estimate from the admitted weapon definition.
pub fn host_estimate_weapon_from_name(name: &str) -> HostEstimateWeapon {
    host_estimate_named_weapon(name).map_or_else(HostEstimateWeapon::default, |(_, estimate)| estimate)
}

/// Build a live victim snapshot, including residual armor coefficient.
pub fn host_estimate_victim_from_object(
    v: &crate::game_logic::Object,
    contain_count: u32,
    armor_coeff: f32,
) -> HostEstimateVictim {
    use crate::game_logic::KindOf;
    HostEstimateVictim {
        kind_structure: v.is_kind_of(KindOf::Structure),
        kind_shrubbery: v.template_name.to_ascii_lowercase().contains("shrub"),
        // C++ Weapon.cpp:601-608 reads exactly the authored KINDOF_MINE /
        // KINDOF_BOOBY_TRAP / KINDOF_DEMOTRAP bits, not template-name
        // substrings.
        kind_mine: v.is_kind_of(KindOf::Mine),
        kind_booby_trap: v.is_kind_of(KindOf::BoobyTrap),
        kind_demo_trap: v.is_kind_of(KindOf::DemoTrap),
        under_construction: v.status.under_construction,
        contain_count: (v.thing().template.contain_module.kind != crate::game_logic::ContainModuleKind::None)
            .then_some(contain_count),
        garrisonable: v.thing().template.contain_module.slots.unwrap_or(0) > 0,
        immune_to_clear_building: false,
        airborne_target: v.status.airborne_target,
        armor_coeff,
    }
}

