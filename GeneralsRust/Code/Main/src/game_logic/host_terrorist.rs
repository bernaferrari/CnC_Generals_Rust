//! Host GLA Terrorist residual (TerroristSuicideWeapon + SuicideDynamitePack).
//!
//! Residual slice (playability):
//! - `GLAInfantryTerrorist` / Chem_/Demo_/Slth_/GC_* variants spawn with PRIMARY
//!   `TerroristSuicideWeapon` (self-kill trigger residual; AttackRange residual **5**
//!   matching SuicideDynamitePack AttackRange for host close-range detonation).
//! - Fire residual: self-detonate death weapon at self position:
//!   - Standard: `SuicideDynamitePack` Primary **500**/r**18** + Secondary **300**/r**50**
//!   - Chem Beta: `GC_Chem_SuicideDynamitePackBeta` same rings + MediumPoisonFieldUpgraded
//!   - Chem Gamma: `GC_Chem_SuicideDynamitePackGamma` Primary **600**/r**18** + Secondary
//!     **300**/r**50** + MediumPoisonFieldGamma residual
//!   - Demo: `Demo_SuicideDynamitePack` Primary **700**/r**18** + Secondary **300**/r**50**
//! - Damage nearby combatants, destroy self (FireWeaponWhenDead + SUICIDED residual).
//!
//! Fail-closed honesty:
//! - Not full ConvertToCarBombCrateCollide matrix (separate host_car_bomb residual)
//! - Not full SlowDeath SUICIDED fling / OCL poison particle bone matrix
//! - Not Demo_SuicideDynamitePackPlusFire SUICIDED path for non-terrorists
//!   (host residual closed in host_demo_suicide_bomb TertiarySuicide path;
//!   terrorists stay on Demo_SuicideDynamitePack 700 primary)
//! - Not network suicide replication (network deferred)

use super::Weapon;
use crate::game_logic::host_toxin_tractor::{
    is_chem_general_template, AnthraxResidualTier,
};

/// Retail primary self-kill trigger weapon.
pub const TERRORIST_SUICIDE_WEAPON: &str = "TerroristSuicideWeapon";
/// Retail FireWeaponWhenDead death weapon.
pub const SUICIDE_DYNAMITE_PACK: &str = "SuicideDynamitePack";
/// Chem Beta death weapon residual.
pub const CHEM_SUICIDE_DYNAMITE_BETA: &str = "GC_Chem_SuicideDynamitePackBeta";
/// Chem Gamma death weapon residual.
pub const CHEM_SUICIDE_DYNAMITE_GAMMA: &str = "GC_Chem_SuicideDynamitePackGamma";
/// Demo General death weapon residual.
pub const DEMO_SUICIDE_DYNAMITE_PACK: &str = "Demo_SuicideDynamitePack";

/// SuicideDynamitePack PrimaryDamage residual.
pub const SUICIDE_DYNAMITE_PRIMARY_DAMAGE: f32 = 500.0;
/// Chem Gamma PrimaryDamage residual (GC_Chem_SuicideDynamitePackGamma).
pub const SUICIDE_DYNAMITE_PRIMARY_DAMAGE_GAMMA: f32 = 600.0;
/// Demo_SuicideDynamitePack PrimaryDamage residual.
pub const SUICIDE_DYNAMITE_PRIMARY_DAMAGE_DEMO: f32 = 700.0;
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

/// Host residual terrorist death-weapon profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerroristDeathProfile {
    /// Standard SuicideDynamitePack residual.
    Standard,
    /// Chem Beta (Anthrax Beta) death weapon + upgraded MediumPoisonField.
    ChemBeta,
    /// Chem Gamma death weapon + gamma MediumPoisonField.
    ChemGamma,
    /// Demo General Demo_SuicideDynamitePack residual (HE primary).
    Demo,
}

impl TerroristDeathProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::ChemBeta => "ChemBeta",
            Self::ChemGamma => "ChemGamma",
            Self::Demo => "Demo",
        }
    }

    pub fn primary_damage(self) -> f32 {
        match self {
            Self::Standard | Self::ChemBeta => SUICIDE_DYNAMITE_PRIMARY_DAMAGE,
            Self::ChemGamma => SUICIDE_DYNAMITE_PRIMARY_DAMAGE_GAMMA,
            Self::Demo => SUICIDE_DYNAMITE_PRIMARY_DAMAGE_DEMO,
        }
    }

    pub fn primary_radius(self) -> f32 {
        SUICIDE_DYNAMITE_PRIMARY_RADIUS
    }

    pub fn secondary_damage(self) -> f32 {
        SUICIDE_DYNAMITE_SECONDARY_DAMAGE
    }

    pub fn secondary_radius(self) -> f32 {
        SUICIDE_DYNAMITE_SECONDARY_RADIUS
    }

    /// Whether residual spawns MediumPoisonField on detonation.
    pub fn spawns_poison(self) -> bool {
        matches!(self, Self::ChemBeta | Self::ChemGamma)
    }

    /// Anthrax residual tier for poison field damage.
    pub fn poison_anthrax_tier(self) -> AnthraxResidualTier {
        match self {
            Self::ChemGamma => AnthraxResidualTier::Gamma,
            Self::ChemBeta => AnthraxResidualTier::Beta,
            Self::Standard | Self::Demo => AnthraxResidualTier::None,
        }
    }

    pub fn weapon_name(self) -> &'static str {
        match self {
            Self::Standard => SUICIDE_DYNAMITE_PACK,
            Self::ChemBeta => CHEM_SUICIDE_DYNAMITE_BETA,
            Self::ChemGamma => CHEM_SUICIDE_DYNAMITE_GAMMA,
            Self::Demo => DEMO_SUICIDE_DYNAMITE_PACK,
        }
    }
}

/// Whether template is Demo General residual (Demo_ prefix).
pub fn is_demo_general_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.starts_with("demo_") || n.contains("testdemo")
}

/// Resolve residual death-weapon profile from template + anthrax flags.
///
/// Chem templates default to Beta without gamma research (retail Chem baseline).
/// Fail-closed: not full WeaponSet PLAYER_UPGRADE module matrix.
pub fn terrorist_death_profile(
    template_name: &str,
    has_gamma: bool,
    has_beta: bool,
) -> TerroristDeathProfile {
    if is_demo_general_template(template_name) {
        return TerroristDeathProfile::Demo;
    }
    if is_chem_general_template(template_name) || has_gamma || has_beta {
        if has_gamma {
            TerroristDeathProfile::ChemGamma
        } else {
            // Chem baseline = Anthrax Beta residual without research.
            TerroristDeathProfile::ChemBeta
        }
    } else {
        TerroristDeathProfile::Standard
    }
}

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
        || n == "testchemterrorist"
        || n == "testdemoterrorist"
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
/// via `suicide_dynamite_damage_at_profile`.
pub fn terrorist_suicide_weapon() -> Weapon {
    terrorist_suicide_weapon_for_profile(TerroristDeathProfile::Standard)
}

/// Residual suicide weapon damage flag for a death profile.
pub fn terrorist_suicide_weapon_for_profile(profile: TerroristDeathProfile) -> Weapon {
    Weapon {
        damage: profile.primary_damage(),
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

/// SuicideDynamitePack residual damage at distance from terrorist (standard).
///
/// Step residual (full primary inside primary radius, full secondary inside
/// secondary radius) — fail-closed vs continuous falloff / NOT_SIMILAR filter.
pub fn suicide_dynamite_damage_at(distance: f32) -> f32 {
    suicide_dynamite_damage_at_profile(TerroristDeathProfile::Standard, distance)
}

/// Profile-aware dual-ring residual damage.
pub fn suicide_dynamite_damage_at_profile(profile: TerroristDeathProfile, distance: f32) -> f32 {
    if distance <= profile.primary_radius() {
        profile.primary_damage()
    } else if distance <= profile.secondary_radius() {
        profile.secondary_damage()
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
        assert!(is_terrorist_template("TestChemTerrorist"));
        assert!(!is_terrorist_template("TerroristSuicideWeapon"));
        assert!(!is_terrorist_template("SuicideDynamitePack"));
        assert!(!is_terrorist_template("GLAVehicleCombatBikeTerrorist"));
        assert!(!is_terrorist_template("GLAInfantryRebel"));
        assert!(!is_terrorist_template("GLAInfantryTunnelDefender"));
        assert!(!is_terrorist_template("CabooseFullOfTerrorists"));
    }

    #[test]
    fn death_profile_matrix() {
        assert_eq!(
            terrorist_death_profile("GLAInfantryTerrorist", false, false),
            TerroristDeathProfile::Standard
        );
        assert_eq!(
            terrorist_death_profile("Chem_GLAInfantryTerrorist", false, false),
            TerroristDeathProfile::ChemBeta
        );
        assert_eq!(
            terrorist_death_profile("Chem_GLAInfantryTerrorist", true, false),
            TerroristDeathProfile::ChemGamma
        );
        assert_eq!(
            terrorist_death_profile("Demo_GLAInfantryTerrorist", false, false),
            TerroristDeathProfile::Demo
        );
        assert!((TerroristDeathProfile::ChemGamma.primary_damage() - 600.0).abs() < 0.01);
        assert!((TerroristDeathProfile::Demo.primary_damage() - 700.0).abs() < 0.01);
        assert!(TerroristDeathProfile::ChemGamma.spawns_poison());
        assert!(!TerroristDeathProfile::Demo.spawns_poison());
        assert!(!TerroristDeathProfile::Standard.spawns_poison());
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

        // Gamma primary 600 residual.
        assert!(
            (suicide_dynamite_damage_at_profile(TerroristDeathProfile::ChemGamma, 0.0) - 600.0)
                .abs()
                < 0.01
        );
        // Demo primary 700 residual.
        assert!(
            (suicide_dynamite_damage_at_profile(TerroristDeathProfile::Demo, 0.0) - 700.0).abs()
                < 0.01
        );
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
