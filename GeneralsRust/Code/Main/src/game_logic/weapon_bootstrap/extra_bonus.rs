//! C++ `WeaponTemplate::m_extraBonus` (Weapon.cpp:1814-1816).
//!
//! Per-weapon `WeaponBonus = CONDITION FIELD percent` lines are parsed into
//! leftover `WeaponTemplate.extra_bonus`. Live compute must append that set
//! after the global GameData bonuses.

use super::*;
use gamelogic::weapon::{
    WeaponBonus, WeaponBonusConditionFlags, WeaponBonusConditionType, WeaponBonusField,
    WeaponBonusSet,
};

/// C++ `Weapon::computeBonus` extra-set append for the named leftover template.
pub fn append_extra_weapon_bonus(
    name: &str,
    flags: WeaponBonusConditionFlags,
    bonus: &mut WeaponBonus,
) {
    let _ = ensure_host_weapon_store();
    let _ = with_weapon_store(|store| {
        if let Some(extra) = store
            .find_weapon_template(name)
            .and_then(|wt| wt.get_extra_bonus())
        {
            extra.append_bonuses(flags, bonus);
        }
    });
}

/// Seed Ranger ACR extra set when Weapon.ini is not on disk.
///
/// Retail `RangerAdvancedCombatRifle`:
/// `WeaponBonus = DRONE_SPOTTING RATE_OF_FIRE/RANGE/DAMAGE 200%`.
pub fn seed_ranger_drone_spotting_extra(template: &mut WeaponTemplate) {
    if template.extra_bonus.is_some() {
        return;
    }
    let mut bonus = WeaponBonus::new();
    bonus.set_field(WeaponBonusField::RateOfFire, 2.0);
    bonus.set_field(WeaponBonusField::Range, 2.0);
    bonus.set_field(WeaponBonusField::Damage, 2.0);
    let mut set = WeaponBonusSet::new();
    set.set_bonus(WeaponBonusConditionType::DroneSpotting, bonus);
    template.extra_bonus = Some(set);
}
