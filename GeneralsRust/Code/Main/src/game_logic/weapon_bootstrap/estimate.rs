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
    pub contain_count: u32,
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

    if victim.kind_structure && weapon.damage_type_sniper && victim.contain_count == 0 {
        return 0.0;
    }

    if weapon.damage_type_surrender || weapon.allow_attack_garrisoned_bldgs {
        if victim.contain_count > 0 && victim.garrisonable && !victim.immune_to_clear_building {
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
        if victim.contain_count > 0 && victim.garrisonable && !victim.immune_to_clear_building {
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

/// C++ Weapon.ini DamageType=SNIPER residual (Pathfinder / Jarmen primary).
pub fn host_weapon_is_sniper_damage(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("pathfindersniper")
        || n.contains("sniperrifle")
        || (n.contains("sniper") && !n.contains("snipe"))
}

/// C++ `WeaponTemplate::m_allowAttackGarrisonedBldgs`.
pub fn host_weapon_allows_attack_garrisoned_bldgs(name: &str) -> bool {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.allow_attack_garrisoned_bldgs)
    })
    .ok()
    .flatten();
    if let Some(flag) = from_store {
        return flag;
    }
    let n = name.to_ascii_lowercase();
    n.contains("microwave") || n.contains("flashbang") || n.contains("cleargarrison")
}

/// Build the weapon snapshot from a live slot name and primary damage.
pub fn host_estimate_weapon_from_name(name: &str, primary_damage: f32) -> HostEstimateWeapon {
    let n = name.to_ascii_lowercase();
    HostEstimateWeapon {
        damage_type_sniper: host_weapon_is_sniper_damage(name),
        damage_type_surrender: host_weapon_is_surrender_damage(name),
        damage_type_disarm: host_weapon_is_disarm_damage(name),
        damage_type_deploy: host_weapon_is_deploy_damage(name),
        damage_type_kill_garrisoned: host_weapon_is_kill_garrisoned_damage(name),
        death_type_burned: n.contains("flame")
            || n.contains("napalm")
            || n.contains("inferno")
            || n.contains("firebomb"),
        allow_attack_garrisoned_bldgs: host_weapon_allows_attack_garrisoned_bldgs(name),
        primary_damage,
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
        contain_count,
        garrisonable: v.thing.template.contain_module.slots.unwrap_or(0) > 0,
        immune_to_clear_building: false,
        airborne_target: v.status.airborne_target,
        armor_coeff,
    }
}

/// C++ `DAMAGE_KILLPILOT` slot used by the Jarmen cursor skip.
/// Primary `GLAJarmenKellRifle` is SNIPER and must not match.
pub fn host_weapon_is_kill_pilot_cursor_slot(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("vehiclepilot")
        || n.contains("pilotsniper")
        || n.contains("killpilot")
        || n.contains("kill_pilot")
}
