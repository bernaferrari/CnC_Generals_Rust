//! Host GameLogic tests — `science_and_upgrades`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

fn wave21_guard_weapon() -> Weapon {
    Weapon {
        range: 400.0,
        can_target_ground: true,
        can_target_air: true,
        ..Default::default()
    }
}

/// A WeaponSet slot always names an authored WeaponTemplate: target legality
/// reads its AntiMask and estimateWeaponDamage its DamageType
/// (WeaponSet.cpp getAbleToAttackSpecificObject, Weapon.cpp
/// estimateWeaponDamage). Parse the test's stated stats as Weapon.ini.
fn arm_guard(object: &mut crate::game_logic::Object) {
    crate::fixture_content::arm_with_stats(object, wave21_guard_weapon());
}

fn mark_guard_scan_due(logic: &mut GameLogic, id: crate::game_logic::ObjectId) {
    let scan_deadline = logic.frame;
    logic
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .set_guard_scan_deadline(Some(scan_deadline));
}

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod guards_and_retaliation;
mod player_events_and_crates;
mod upgrades_power_capture;

mod guard_trigger_ownership;
