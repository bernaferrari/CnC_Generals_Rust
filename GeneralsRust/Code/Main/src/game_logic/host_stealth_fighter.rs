//! Host America Stealth Fighter residual (science production + combat missiles).
//!
//! Residual slice (playability):
//! - SCIENCE_StealthFighter production unlock residual (gated airfield construct).
//!   Airforce General `AirF_*` variants are free (no science Prerequisite).
//! - `AmericaJetStealthFighter` / USA_ / SupW_/Lazr_ + AirF_ spawn with PRIMARY
//!   `StealthJetMissileWeapon` residual: PrimaryDamage **100** / radius **5**,
//!   range **220**, min **60**, Delay **200**ms → 6 frames. ClipSize **2** honesty
//!   (RETURN_TO_BASE full clip matrix fail-closed).
//! - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
//!   Ground residual only (retail AntiAirborneVehicle/Infantry = No).
//! - Bunker Buster PLAYER_UPGRADE residual remains in host_bunker_buster (structure
//!   garrison kill + bunker mult — applied from combat residual path).
//!
//! Fail-closed honesty:
//! - Not full PrerequisiteSciences rank tree / control-bar science visibility
//! - Not full JetAIUpdate RETURN_TO_BASE / ClipReload 8000ms airfield rearm matrix
//! - Not full StealthJetMissile projectile AI / KillSelfDelay crash path
//! - Not full BunkerBusterBehavior seismic / shockwave matrix (see host_bunker_buster)
//! - Not network stealth-fighter / science replication (network deferred)

use super::Weapon;
use serde::{Deserialize, Serialize};

/// Retail science that gates Stealth Fighter production.
pub const SCIENCE_STEALTH_FIGHTER: &str = "SCIENCE_StealthFighter";

/// Canonical retail USA Stealth Fighter object name.
pub const AMERICA_JET_STEALTH_FIGHTER: &str = "AmericaJetStealthFighter";

/// Host residual alias used by some USA seed tables / HUD labels.
pub const USA_STEALTH_FIGHTER: &str = "USA_StealthFighter";

/// Retail StealthJetMissileWeapon template name.
pub const STEALTH_JET_MISSILE_WEAPON: &str = "StealthJetMissileWeapon";

/// Retail BuildCost residual (AmericaAir.ini AmericaJetStealthFighter).
pub const STEALTH_FIGHTER_BUILD_COST: u32 = 1600;

/// Retail BuildTime residual seconds (AmericaAir.ini = 25.0).
pub const STEALTH_FIGHTER_BUILD_TIME: f32 = 25.0;

/// Retail StealthJetMissileWeapon PrimaryDamage.
pub const STEALTH_FIGHTER_DAMAGE: f32 = 100.0;
/// Retail PrimaryDamageRadius.
pub const STEALTH_FIGHTER_PRIMARY_RADIUS: f32 = 5.0;
/// Retail AttackRange.
pub const STEALTH_FIGHTER_RANGE: f32 = 220.0;
/// Retail MinimumAttackRange.
pub const STEALTH_FIGHTER_MIN_RANGE: f32 = 60.0;
/// Retail DelayBetweenShots 200ms → 6 frames @ 30 FPS.
pub const STEALTH_FIGHTER_DELAY_FRAMES: u32 = 6;
/// Retail ClipSize honesty (full RETURN_TO_BASE rearm fail-closed).
pub const STEALTH_FIGHTER_CLIP_SIZE: u32 = 2;
/// Retail ClipReloadTime 8000ms → 240 frames honesty residual.
pub const STEALTH_FIGHTER_CLIP_RELOAD_FRAMES: u32 = 240;
/// Residual projectile speed.
pub const STEALTH_FIGHTER_PROJECTILE_SPEED: f32 = 1000.0;

/// Residual fire audio.
pub const STEALTH_FIGHTER_FIRE_AUDIO: &str = "StealthJetMissileWeapon";

/// Normalize science / template identity (alphanumeric lower).
pub fn normalize_identity(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether a science / purchase name is SCIENCE_StealthFighter residual.
pub fn is_stealth_fighter_science(name: &str) -> bool {
    let n = normalize_identity(name);
    n == "sciencestealthfighter" || n == "stealthfighter"
}

/// Whether template is a residual living Stealth Fighter jet.
///
/// Fail-closed: name residual. Excludes missiles / weapons / hulks.
pub fn is_stealth_fighter_template(template_name: &str) -> bool {
    let n = normalize_identity(template_name);
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "usastealthfighter"
        || n == "teststealthfighter"
        || n == "americajetstealthfighter"
    {
        return true;
    }
    // Exclude non-living residual objects / projectiles / science tokens.
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.contains("exhaust")
        || n.contains("locomotor")
        || n.starts_with("upgrade")
        || n.contains("crate")
        || n.contains("science")
        || n == "stealthjetmissile"
        || n.ends_with("jetmissile")
    {
        return false;
    }
    n.contains("stealthfighter") || n.contains("jetstealth")
}

/// Whether residual fire should apply Stealth Fighter residual path.
pub fn should_apply_stealth_fighter_residual(is_stealth_fighter: bool) -> bool {
    is_stealth_fighter
}

/// Whether a unit template requires SCIENCE_StealthFighter for production.
///
/// Explicitly **not** gated: AirF_AmericaJetStealthFighter (Airforce General free).
pub fn requires_stealth_fighter_science(template_name: &str) -> bool {
    let n = normalize_identity(template_name);
    if n.is_empty() {
        return false;
    }
    // Airforce General residual: no science Prerequisite in retail.
    if n.starts_with("airf") {
        return false;
    }
    if n.contains("stealthfighter") || n.contains("jetstealth") {
        return true;
    }
    false
}

/// Production gate: science-gated templates require unlock; others always ok.
pub fn player_may_produce_stealth_aircraft(has_science: bool, template_name: &str) -> bool {
    if !requires_stealth_fighter_science(template_name) {
        return true;
    }
    has_science
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Stealth Fighter primary Weapon.
pub fn stealth_fighter_weapon() -> Weapon {
    Weapon {
        damage: STEALTH_FIGHTER_DAMAGE,
        range: STEALTH_FIGHTER_RANGE,
        min_range: STEALTH_FIGHTER_MIN_RANGE,
        reload_time: delay_frames_to_reload_secs(STEALTH_FIGHTER_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(STEALTH_FIGHTER_CLIP_SIZE),
        // Retail AntiAirborneVehicle/Infantry = No — ground residual only.
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: STEALTH_FIGHTER_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// Residual damage at distance from impact (intended / primary ring).
pub fn stealth_fighter_damage_at(distance_from_impact: f32) -> f32 {
    if distance_from_impact <= STEALTH_FIGHTER_PRIMARY_RADIUS {
        STEALTH_FIGHTER_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual splash / fire target.
pub fn is_legal_stealth_fighter_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Host residual honesty registry for Stealth Fighter science → production.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostStealthFighterRegistry {
    /// Times SCIENCE_StealthFighter was unlocked on a player (session residual).
    pub science_unlock_count: u32,
    /// Times a science-gated stealth aircraft was accepted into a production queue.
    pub production_enqueue_count: u32,
    /// Times a science-gated stealth aircraft finished production and spawned.
    pub production_spawn_count: u32,
    /// Times production was rejected solely due to missing science.
    pub production_denied_count: u32,
}

impl HostStealthFighterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_science_unlock(&mut self) {
        self.science_unlock_count = self.science_unlock_count.saturating_add(1);
    }

    pub fn record_production_enqueue(&mut self) {
        self.production_enqueue_count = self.production_enqueue_count.saturating_add(1);
    }

    pub fn record_production_spawn(&mut self) {
        self.production_spawn_count = self.production_spawn_count.saturating_add(1);
    }

    pub fn record_production_denied(&mut self) {
        self.production_denied_count = self.production_denied_count.saturating_add(1);
    }

    pub fn honesty_unlock_ok(&self) -> bool {
        self.science_unlock_count > 0
    }

    pub fn honesty_produce_ok(&self) -> bool {
        self.production_enqueue_count > 0
    }

    pub fn honesty_deny_ok(&self) -> bool {
        self.production_denied_count > 0
    }

    pub fn honesty_spawn_ok(&self) -> bool {
        self.production_spawn_count > 0
    }

    pub fn honesty_ok(&self) -> bool {
        self.honesty_unlock_ok() && self.honesty_produce_ok()
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_ok() && self.honesty_spawn_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn science_name_recognition() {
        assert!(is_stealth_fighter_science(SCIENCE_STEALTH_FIGHTER));
        assert!(is_stealth_fighter_science("SCIENCE_StealthFighter"));
        assert!(is_stealth_fighter_science("stealthfighter"));
        assert!(!is_stealth_fighter_science("SCIENCE_StealthFighter_x"));
        assert!(!is_stealth_fighter_science("SCIENCE_CashBounty1"));
        assert!(!is_stealth_fighter_science("SCIENCE_Paladin"));
    }

    #[test]
    fn template_science_gate_matrix() {
        assert!(requires_stealth_fighter_science(AMERICA_JET_STEALTH_FIGHTER));
        assert!(requires_stealth_fighter_science("SupW_AmericaJetStealthFighter"));
        assert!(requires_stealth_fighter_science("Lazr_AmericaJetStealthFighter"));
        assert!(requires_stealth_fighter_science("CINE_AmericaJetStealthFighter"));
        assert!(requires_stealth_fighter_science(USA_STEALTH_FIGHTER));
        assert!(!requires_stealth_fighter_science("AirF_AmericaJetStealthFighter"));
        assert!(!requires_stealth_fighter_science("USA_Raptor"));
        assert!(!requires_stealth_fighter_science("TestAircraft"));
    }

    #[test]
    fn living_template_matrix() {
        assert!(is_stealth_fighter_template("AmericaJetStealthFighter"));
        assert!(is_stealth_fighter_template("USA_StealthFighter"));
        assert!(is_stealth_fighter_template("TestStealthFighter"));
        assert!(is_stealth_fighter_template("AirF_AmericaJetStealthFighter"));
        assert!(is_stealth_fighter_template("SupW_AmericaJetStealthFighter"));
        assert!(is_stealth_fighter_template("Lazr_AmericaJetStealthFighter"));
        assert!(!is_stealth_fighter_template("StealthJetMissile"));
        assert!(!is_stealth_fighter_template("StealthJetMissileWeapon"));
        assert!(!is_stealth_fighter_template("AmericaJetRaptor"));
        assert!(!is_stealth_fighter_template("Upgrade_AmericaBunkerBusters"));
    }

    #[test]
    fn production_gate_requires_science() {
        assert!(!player_may_produce_stealth_aircraft(false, AMERICA_JET_STEALTH_FIGHTER));
        assert!(player_may_produce_stealth_aircraft(true, AMERICA_JET_STEALTH_FIGHTER));
        assert!(player_may_produce_stealth_aircraft(
            false,
            "AirF_AmericaJetStealthFighter"
        ));
        assert!(player_may_produce_stealth_aircraft(false, "USA_Raptor"));
    }

    #[test]
    fn weapon_and_splash() {
        let w = stealth_fighter_weapon();
        assert!((w.damage - 100.0).abs() < 0.01);
        assert!((w.range - 220.0).abs() < 0.01);
        assert!((w.min_range - 60.0).abs() < 0.01);
        assert!((w.reload_time - 6.0 / 30.0).abs() < 0.01);
        assert_eq!(w.ammo, Some(2));
        assert!(!w.can_target_air);
        assert!(w.can_target_ground);

        assert!((stealth_fighter_damage_at(0.0) - 100.0).abs() < 0.01);
        assert!((stealth_fighter_damage_at(5.0) - 100.0).abs() < 0.01);
        assert!((stealth_fighter_damage_at(6.0)).abs() < 0.01);
    }

    #[test]
    fn honesty_tracks_unlock_produce_spawn() {
        let mut reg = HostStealthFighterRegistry::new();
        assert!(!reg.honesty_ok());
        reg.record_science_unlock();
        assert!(reg.honesty_unlock_ok());
        assert!(!reg.honesty_ok());
        reg.record_production_enqueue();
        assert!(reg.honesty_ok());
        assert!(!reg.honesty_host_path_ok());
        reg.record_production_spawn();
        assert!(reg.honesty_host_path_ok());
        reg.record_production_denied();
        assert!(reg.honesty_deny_ok());
    }
}
