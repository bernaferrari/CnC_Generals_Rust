//! Host GLA Terrorist residual (TerroristSuicideWeapon + SuicideDynamitePack).
//!
//! Residual slice (playability):
//! - `GLAInfantryTerrorist` / Chem_/Demo_/Slth_/GC_* variants spawn with PRIMARY
//!   `TerroristSuicideWeapon` (self-kill trigger residual; AttackRange residual **5**
//!   matching SuicideDynamitePack AttackRange for host close-range detonation).
//! - Fire residual: self-detonate `SuicideDynamitePack` at self position
//!   (Primary **500** / radius **18** + Secondary **300** / radius **50**),
//!   damage nearby combatants, destroy self (FireWeaponWhenDead + SUICIDED residual).
//!
//! Fail-closed honesty:
//! - Not full ConvertToCarBombCrateCollide matrix (separate host_car_bomb residual)
//! - Not Chem anthrax / Demo_ / Gamma death-weapon variant matrix
//! - Not full SlowDeath SUICIDED fling / OCL poison death matrix
//! - Not network suicide replication (network deferred)

use super::Weapon;

/// Retail primary self-kill trigger weapon.
pub const TERRORIST_SUICIDE_WEAPON: &str = "TerroristSuicideWeapon";
/// Retail FireWeaponWhenDead death weapon.
pub const SUICIDE_DYNAMITE_PACK: &str = "SuicideDynamitePack";

/// SuicideDynamitePack PrimaryDamage residual.
pub const SUICIDE_DYNAMITE_PRIMARY_DAMAGE: f32 = 500.0;
/// SuicideDynamitePack PrimaryDamageRadius residual.
pub const SUICIDE_DYNAMITE_PRIMARY_RADIUS: f32 = 18.0;
/// SuicideDynamitePack SecondaryDamage residual.
pub const SUICIDE_DYNAMITE_SECONDARY_DAMAGE: f32 = 300.0;
/// SuicideDynamitePack SecondaryDamageRadius residual.
pub const SUICIDE_DYNAMITE_SECONDARY_RADIUS: f32 = 50.0;
/// Residual attack range (SuicideDynamitePack AttackRange 5; host close-range).
pub const SUICIDE_DYNAMITE_ATTACK_RANGE: f32 = 5.0;

/// Residual detonation audio (retail FireSound = CarBomberDie).
pub const TERRORIST_DETONATE_AUDIO: &str = "CarBomberDie";

/// Whether template is a residual GLA infantry Terrorist (not combat-bike).
///
/// Fail-closed: name residual. Excludes weapons, combat bike, debris tokens.
pub fn is_terrorist_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names (before exclusion tokens).
    // Note: "TestTerrorist".to_ascii_lowercase() == "testterrorist" (double t).
    if n == "testterrorist"
        || n == "testerrorist"
        || n == "test_terrorist"
        || n == "gla_terrorist"
        || n == "gla_infantryterrorist"
    {
        return true;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("bike")
        || n.contains("combatcycle")
        || n.contains("combat_cycle")
        || n.contains("carbomb")
        || n.contains("car_bomb")
        || n.contains("dynamite")
        || n.contains("suicide")
    {
        return false;
    }
    n.contains("infantryterrorist")
        || n.contains("infantry_terrorist")
        || (n.contains("terrorist") && (n.contains("infantry") || n.contains("gla")))
}

/// Residual TerroristSuicideWeapon bound at spawn (self-kill flag residual).
///
/// Host residual uses SuicideDynamitePack primary damage as the attack-damage
/// flag so combat fire path can detect suicide residual; real AOE is applied
/// via `suicide_dynamite_damage_at`.
pub fn terrorist_suicide_weapon() -> Weapon {
    Weapon {
        damage: SUICIDE_DYNAMITE_PRIMARY_DAMAGE,
        range: SUICIDE_DYNAMITE_ATTACK_RANGE,
        min_range: 0.0,
        reload_time: 0.05,
        last_fire_time: 0.0,
        ammo: Some(1),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// SuicideDynamitePack residual damage at distance from terrorist.
///
/// Step residual (full primary inside primary radius, full secondary inside
/// secondary radius) — fail-closed vs continuous falloff / NOT_SIMILAR filter.
pub fn suicide_dynamite_damage_at(distance: f32) -> f32 {
    if distance <= SUICIDE_DYNAMITE_PRIMARY_RADIUS {
        SUICIDE_DYNAMITE_PRIMARY_DAMAGE
    } else if distance <= SUICIDE_DYNAMITE_SECONDARY_RADIUS {
        SUICIDE_DYNAMITE_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual AOE target for suicide detonation.
pub fn is_legal_terrorist_aoe_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply Terrorist suicide residual path.
pub fn should_apply_terrorist_residual(is_terrorist: bool) -> bool {
    is_terrorist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrorist_name_matrix() {
        assert!(is_terrorist_template("GLAInfantryTerrorist"));
        assert!(is_terrorist_template("GLA_Terrorist"));
        assert!(is_terrorist_template("Demo_GLAInfantryTerrorist"));
        assert!(is_terrorist_template("Chem_GLAInfantryTerrorist"));
        assert!(is_terrorist_template("Slth_GLAInfantryTerrorist"));
        assert!(is_terrorist_template("TestTerrorist"));
        assert!(!is_terrorist_template("TerroristSuicideWeapon"));
        assert!(!is_terrorist_template("SuicideDynamitePack"));
        assert!(!is_terrorist_template("GLAVehicleCombatBikeTerrorist"));
        assert!(!is_terrorist_template("GLAInfantryRebel"));
        assert!(!is_terrorist_template("GLAInfantryTunnelDefender"));
        assert!(!is_terrorist_template("CabooseFullOfTerrorists"));
    }

    #[test]
    fn suicide_weapon_is_close_range_one_shot() {
        let w = terrorist_suicide_weapon();
        assert!((w.range - SUICIDE_DYNAMITE_ATTACK_RANGE).abs() < f32::EPSILON);
        assert_eq!(w.ammo, Some(1));
        assert!(w.can_target_ground);
        assert!(!w.can_target_air);
        assert!((w.damage - SUICIDE_DYNAMITE_PRIMARY_DAMAGE).abs() < 0.01);
    }

    #[test]
    fn aoe_damage_rings() {
        assert!(
            (suicide_dynamite_damage_at(0.0) - SUICIDE_DYNAMITE_PRIMARY_DAMAGE).abs() < 0.01
        );
        assert!(
            (suicide_dynamite_damage_at(SUICIDE_DYNAMITE_PRIMARY_RADIUS)
                - SUICIDE_DYNAMITE_PRIMARY_DAMAGE)
                .abs()
                < 0.01
        );
        assert!(
            (suicide_dynamite_damage_at(SUICIDE_DYNAMITE_PRIMARY_RADIUS + 0.1)
                - SUICIDE_DYNAMITE_SECONDARY_DAMAGE)
                .abs()
                < 0.01
        );
        assert!(
            (suicide_dynamite_damage_at(SUICIDE_DYNAMITE_SECONDARY_RADIUS)
                - SUICIDE_DYNAMITE_SECONDARY_DAMAGE)
                .abs()
                < 0.01
        );
        assert!(suicide_dynamite_damage_at(SUICIDE_DYNAMITE_SECONDARY_RADIUS + 1.0) <= 0.0);
    }

    #[test]
    fn residual_gate() {
        assert!(should_apply_terrorist_residual(true));
        assert!(!should_apply_terrorist_residual(false));
        assert!(is_legal_terrorist_aoe_target(true, false, false, true));
        assert!(!is_legal_terrorist_aoe_target(false, false, false, true));
        assert!(!is_legal_terrorist_aoe_target(true, true, false, true));
    }
}
