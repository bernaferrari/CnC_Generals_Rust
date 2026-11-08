//! C++ `WeaponTemplate::estimateWeaponTemplateDamage` + `ActiveBody::estimateDamage`.

use super::*;

/// Victim snapshot for the C++ estimate gate (no leftover dual-world).
#[derive(Debug, Clone, Copy)]
pub struct HostEstimateVictim {
    pub kind_structure: bool,
    pub kind_shrubbery: bool,
    pub kind_mine: bool,
    pub kind_booby_trap: bool,
    pub kind_demo_trap: bool,
    pub under_construction: bool,
    /// None means there is no ContainModuleInterface, not an empty container.
    pub contain_count: Option<u32>,
    pub garrisonable: bool,
    pub immune_to_clear_building: bool,
    pub airborne_target: bool,
    /// C++ `ArmorTemplate::adjustDamage` coefficient for this weapon type.
    /// 1.0 = no mitigation (specials-only tests). Tank/Structure SNIPER = 0.
    pub armor_coeff: f32,
}

/// Weapon snapshot for the C++ estimate gate.
#[derive(Debug, Clone, Copy)]
pub struct HostEstimateWeapon {
    pub damage_type_sniper: bool,
    pub damage_type_surrender: bool,
    pub damage_type_disarm: bool,
    pub damage_type_deploy: bool,
    pub damage_type_kill_garrisoned: bool,
    pub death_type_burned: bool,
    pub allow_attack_garrisoned_bldgs: bool,
    pub primary_damage: f32,
}

/// C++ `WeaponTemplate::estimateWeaponTemplateDamage` (Weapon.cpp:546-638)
/// plus `ActiveBody::estimateDamage` specials (ActiveBody.cpp:264-292).
///
/// Returns 0 when C++ `canFireWeaponAtObject` / WeaponSet must refuse the shot.
pub fn estimate_weapon_template_damage(
    weapon: &HostEstimateWeapon,
    victim: Option<&HostEstimateVictim>,
) -> f32 {
    let Some(victim) = victim else {
        return weapon.primary_damage.max(0.0);
    };

    if victim.kind_shrubbery {
        return if weapon.death_type_burned { 1.0 } else { 0.0 };
    }

    if victim.kind_structure && weapon.damage_type_sniper && victim.contain_count == Some(0) {
        return 0.0;
    }

    if weapon.damage_type_surrender || weapon.allow_attack_garrisoned_bldgs {
        if victim.contain_count.is_some_and(|count| count > 0)
            && victim.garrisonable
            && !victim.immune_to_clear_building
        {
            return 1.0;
        }
    }

    if weapon.damage_type_disarm {
        if victim.kind_mine || victim.kind_booby_trap || victim.kind_demo_trap {
            return 1.0;
        }
        return 0.0;
    }

    if weapon.damage_type_deploy && !victim.airborne_target {
        return 1.0;
    }

    if weapon.damage_type_kill_garrisoned {
        if victim.contain_count.is_some_and(|count| count > 0)
            && victim.garrisonable
            && !victim.immune_to_clear_building
        {
            return 1.0;
        }
        return 0.0;
    }

    if weapon.damage_type_sniper && victim.kind_structure && victim.under_construction {
        return 0.0;
    }

    // C++ `ActiveBody::estimateDamage` ends with `m_curArmor.adjustDamage`.
    (weapon.primary_damage.max(0.0) * victim.armor_coeff.max(0.0)).max(0.0)
}

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

/// Build the estimate from the admitted weapon definition.
pub fn host_estimate_weapon_from_name(name: &str) -> HostEstimateWeapon {
    let rule = super::with_host_weapon_store(|store| store.find_weapon_template(name).cloned())
        .ok().flatten();
    let damage_type = rule.as_ref().map(|weapon| weapon.damage_type);
    HostEstimateWeapon {
        damage_type_sniper: damage_type == Some(gamelogic::damage::DamageType::Sniper),
        damage_type_surrender: damage_type == Some(gamelogic::damage::DamageType::Surrender),
        damage_type_disarm: damage_type == Some(gamelogic::damage::DamageType::Disarm),
        damage_type_deploy: damage_type == Some(gamelogic::damage::DamageType::Deploy),
        damage_type_kill_garrisoned: damage_type == Some(gamelogic::damage::DamageType::KillGarrisoned),
        death_type_burned: rule.as_ref().is_some_and(|weapon| weapon.death_type == gamelogic::damage::DeathType::Burned),
        allow_attack_garrisoned_bldgs: rule.as_ref().is_some_and(|weapon| weapon.allow_attack_garrisoned_bldgs),
        primary_damage: rule.as_ref().map_or(0.0, |weapon| weapon.primary_damage),
    }
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
        kind_mine: v.is_kind_of(KindOf::Mine) || v.is_disarmable_mine(),
        kind_booby_trap: v.is_disarmable_mine(),
        kind_demo_trap: v.is_kind_of(KindOf::Mine) || v.is_disarmable_mine(),
        under_construction: v.status.under_construction,
        contain_count: (v.thing().template.contain_module.kind != crate::game_logic::ContainModuleKind::None)
            .then_some(contain_count),
        garrisonable: v.thing().template.contain_module.slots.unwrap_or(0) > 0,
        immune_to_clear_building: false,
        airborne_target: v.status.airborne_target,
        armor_coeff,
    }
}

