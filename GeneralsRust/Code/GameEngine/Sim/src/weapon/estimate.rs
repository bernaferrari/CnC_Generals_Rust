/// Target facts for a weapon damage estimate.
#[derive(Debug, Clone, Copy)]
pub struct DamageEstimateTarget {
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

/// Weapon facts used to estimate damage before accepting a shot.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeaponDamageEstimate {
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
    weapon: &WeaponDamageEstimate,
    victim: Option<&DamageEstimateTarget>,
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
