//! Host America Bunker Buster residual (Stealth Fighter + Upgrade_AmericaBunkerBusters).
//!
//! Residual slice (playability):
//! - `Upgrade_AmericaBunkerBusters` research tags residual Stealth Fighters
//!   (C++ BunkerBusterBehavior::UpgradeRequired on StealthJetMissile).
//! - Combat impact on a structure with residual bunker-buster capability:
//!   - Kills all garrisoned occupants (C++ contain killAllContained /
//!     harmAndForceExitAllContained residual).
//!   - Applies amplified structure damage vs bunkers ("damages bunkers more").
//! - Optional KILL_GARRISONED residual (MicrowaveTankBuildingClearer style):
//!   kill `floor(damage)` occupants without requiring the upgrade.
//!
//! Wave 66 residual pack (retail AmericaAir.ini / Weapon.ini / WeaponObjects.ini):
//! - Missile residual: StealthJetMissileWeapon PrimaryDamage **100** / radius **5** /
//!   range **220** / min **60** / Delay **200**ms → **6**f / ClipSize **2** /
//!   ClipReload **8000**ms → **240**f / DamageType STEALTHJET_MISSILES.
//! - Occupant residual: BunkerBusterAntiTunnel PrimaryDamage **400** / radius **10**.
//! - Shockwave residual: BunkerBusterShockwaveWeaponSmall dmg **10** / radius **50** /
//!   SeismicEffectRadius **200** / Magnitude **5**.
//! - Stealth Fighter body residual: MaxHealth **120**, Vision **180**, Shroud **300**,
//!   BuildCost **1600**, BuildTime **25**s → **750**f, SCIENCE_StealthFighter.
//!
//! Fail-closed honesty:
//! - Not full BunkerBusterBehavior crash-through FX / seismic sim path
//! - Not full projectile StealthJetMissile AI / KillSelfDelay crash path
//! - Not full GarrisonContain isBustable / TunnelContain crash-guard matrix
//! - Not network bunker-buster replication (network deferred)

use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const BUNKER_BUSTER_LOGIC_FPS: f32 = 30.0;

/// Retail upgrade that enables bunker-buster residual on Stealth Fighter missiles.
pub const UPGRADE_AMERICA_BUNKER_BUSTERS: &str = "Upgrade_AmericaBunkerBusters";
/// Retail science gate residual for Stealth Fighter.
pub const SCIENCE_STEALTH_FIGHTER: &str = "SCIENCE_StealthFighter";

/// Retail StealthJetMissileWeapon template name (host seed residual).
pub const STEALTH_JET_MISSILE_WEAPON: &str = "StealthJetMissileWeapon";
/// Retail projectile object residual.
pub const STEALTH_JET_MISSILE_PROJECTILE: &str = "StealthJetMissile";
/// Retail shockwave temp weapon residual.
pub const BUNKER_BUSTER_SHOCKWAVE_WEAPON: &str = "BunkerBusterShockwaveWeaponSmall";
/// Retail occupant damage weapon residual.
pub const BUNKER_BUSTER_OCCUPANT_WEAPON: &str =
    "BunkerBusterAntiTunnelGarrisonWeaponWithABigName";

/// Retail MicrowaveTankBuildingClearer — DamageType = KILL_GARRISONED.
pub const MICROWAVE_BUILDING_CLEARER_WEAPON: &str = "MicrowaveTankBuildingClearer";

/// Residual structure damage multiplier vs bunkers when bunker-buster residual hits.
/// Fail-closed: not full armor / STEALTHJET_MISSILES damage-FX matrix.
pub const BUNKER_BUSTER_STRUCTURE_DAMAGE_MULT: f32 = 1.5;

/// Residual occupant kill damage (matches BunkerBusterAntiTunnel PrimaryDamage 400 residual).
pub const BUNKER_BUSTER_OCCUPANT_DAMAGE: f32 = 400.0;
/// Retail BunkerBusterAntiTunnel PrimaryDamageRadius residual.
pub const BUNKER_BUSTER_OCCUPANT_RADIUS: f32 = 10.0;

/// Residual shockwave-style structure bonus when bunker is occupied (host playability).
/// Fail-closed: not full BunkerBusterShockwaveWeaponSmall PrimaryDamage 10 radius 50.
pub const BUNKER_BUSTER_OCCUPIED_BONUS_DAMAGE: f32 = 10.0;
/// Retail BunkerBusterShockwaveWeaponSmall PrimaryDamageRadius residual.
pub const BUNKER_BUSTER_SHOCKWAVE_RADIUS: f32 = 50.0;
/// Retail BunkerBusterBehavior SeismicEffectRadius residual.
pub const BUNKER_BUSTER_SEISMIC_RADIUS: f32 = 200.0;
/// Retail BunkerBusterBehavior SeismicEffectMagnitude residual.
pub const BUNKER_BUSTER_SEISMIC_MAGNITUDE: f32 = 5.0;
/// Retail CrashThroughBunkerFXFrequency residual (msec).
pub const BUNKER_BUSTER_CRASH_FX_FREQUENCY_MS: u32 = 571;

/// Retail StealthJetMissileWeapon PrimaryDamage residual.
pub const STEALTH_JET_MISSILE_DAMAGE: f32 = 100.0;
/// Retail StealthJetMissileWeapon PrimaryDamageRadius residual.
pub const STEALTH_JET_MISSILE_RADIUS: f32 = 5.0;
/// Retail StealthJetMissileWeapon AttackRange residual.
pub const STEALTH_JET_MISSILE_RANGE: f32 = 220.0;
/// Retail StealthJetMissileWeapon MinimumAttackRange residual.
pub const STEALTH_JET_MISSILE_MIN_RANGE: f32 = 60.0;
/// Retail DelayBetweenShots residual (msec).
pub const STEALTH_JET_MISSILE_DELAY_MS: u32 = 200;
/// Delay 200ms → 6 frames @ 30 FPS.
pub const STEALTH_JET_MISSILE_DELAY_FRAMES: u32 = 6;
/// Retail ClipSize residual.
pub const STEALTH_JET_MISSILE_CLIP_SIZE: u32 = 2;
/// Retail ClipReloadTime residual (msec).
pub const STEALTH_JET_MISSILE_CLIP_RELOAD_MS: u32 = 8000;
/// ClipReload 8000ms → 240 frames @ 30 FPS.
pub const STEALTH_JET_MISSILE_CLIP_RELOAD_FRAMES: u32 = 240;
/// Retail DamageType residual.
pub const STEALTH_JET_MISSILE_DAMAGE_TYPE: &str = "STEALTHJET_MISSILES";
/// Retail DeathType residual.
pub const STEALTH_JET_MISSILE_DEATH_TYPE: &str = "EXPLODED";

// --- Stealth Fighter body residual ---

/// Retail ActiveBody MaxHealth residual.
pub const STEALTH_FIGHTER_MAX_HEALTH: f32 = 120.0;
/// Retail VisionRange residual.
pub const STEALTH_FIGHTER_VISION_RANGE: f32 = 180.0;
/// Retail ShroudClearingRange residual.
pub const STEALTH_FIGHTER_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail BuildCost residual.
pub const STEALTH_FIGHTER_BUILD_COST: u32 = 1600;
/// Retail BuildTime residual (seconds).
pub const STEALTH_FIGHTER_BUILD_TIME_SEC: f32 = 25.0;
/// BuildTime 25s → 750 frames @ 30 FPS.
pub const STEALTH_FIGHTER_BUILD_TIME_FRAMES: u32 = 750;

/// Activate / impact audio residual.
pub const BUNKER_BUSTER_AUDIO: &str = "StealthJetMissileWeapon";
/// Retail DetonationFX residual.
pub const BUNKER_BUSTER_DETONATION_FX: &str = "FX_BunkerBusterExplosion";

/// Convert residual milliseconds to logic frames @ 30 FPS (round half-up).
pub fn bunker_buster_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * BUNKER_BUSTER_LOGIC_FPS / 1000.0).round() as u32
}

/// Whether template is a residual Stealth Fighter / bunker-buster carrier.
///
/// Fail-closed: name residual (not full WeaponSet / JetAIUpdate matrix).
/// Excludes projectile shells (`StealthJetMissile` alone as projectile object).
pub fn is_bunker_buster_carrier(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Projectile / missile objects are not the plane.
    if n == "stealthjetmissile"
        || n.ends_with("jetmissile")
        || n.contains("projectile")
        || n.contains("shell")
    {
        // Keep plane names that include "missile" only if also fighter.
        if !(n.contains("fighter") || n.contains("jetstealth")) {
            return false;
        }
    }
    if n == "teststealthfighter" || n == "testbunkerbuster" {
        return true;
    }
    // Retail BunkerBusterBehavior lives on StealthJetMissile (Stealth Fighter).
    // Fail-closed: Aurora bombers are not bunker-buster carriers.
    n.contains("stealthfighter") || n.contains("jetstealth") || n.contains("stealthjet")
}

/// Whether residual target structure name is a bunker / garrison building.
pub fn is_bunker_structure_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("bunker")
        || n.contains("garrison")
        || n == "testbunker"
        || n.contains("tunnelemp")
        || n.contains("tunnelnetwork")
}

/// Whether residual attacker is a KILL_GARRISONED clearer (Microwave Tank residual).
pub fn is_kill_garrisoned_clearer(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("microwave") || n == "testmicrowave" || n.contains("buildingclearer")
}

/// Slot-aware residual: bunker-buster full bust (kill occupants + bunker mult damage).
///
/// C++ BunkerBusterBehavior requires Upgrade_AmericaBunkerBusters on controlling player.
pub fn should_apply_bunker_buster(
    has_upgrade: bool,
    is_carrier: bool,
    target_is_structure: bool,
) -> bool {
    has_upgrade && is_carrier && target_is_structure
}

/// KILL_GARRISONED residual: microwave-style occupant kill without upgrade.
pub fn should_apply_kill_garrisoned(is_clearer: bool, target_is_structure: bool) -> bool {
    is_clearer && target_is_structure
}

/// Structure HP damage residual for bunker buster hit.
///
/// - Base weapon damage always applies.
/// - Bunkers take residual mult (damages bunkers more).
/// - Occupied bunkers get residual occupied bonus (shockwave residual).
pub fn bunker_buster_structure_damage(
    base_damage: f32,
    is_bunker: bool,
    had_occupants: bool,
) -> f32 {
    let mut dmg = base_damage.max(0.0);
    if is_bunker {
        dmg *= BUNKER_BUSTER_STRUCTURE_DAMAGE_MULT;
    }
    if had_occupants {
        dmg += BUNKER_BUSTER_OCCUPIED_BONUS_DAMAGE;
    }
    dmg
}

/// How many occupants KILL_GARRISONED residual kills (C++ amount.floor()).
pub fn kill_garrisoned_count(damage_amount: f32, contained_count: usize) -> usize {
    if contained_count == 0 {
        return 0;
    }
    let kills = damage_amount.floor().max(0.0) as usize;
    kills.min(contained_count)
}

/// Host residual honesty counters for bunker buster / kill-garrisoned.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBunkerBusterRegistry {
    /// Successful bunker-buster residual impacts (upgrade path).
    pub blasts: u32,
    /// Occupants killed by bunker-buster or KILL_GARRISONED residual.
    pub occupants_killed: u32,
    /// Times residual applied amplified bunker structure damage.
    pub bunker_damage_hits: u32,
    /// Total residual structure HP damage applied by bunker-buster path.
    pub structure_damage_dealt: f32,
    /// KILL_GARRISONED clearer residual applications (microwave style).
    pub kill_garrisoned_hits: u32,
}

impl HostBunkerBusterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn record_bunker_buster_blast(
        &mut self,
        occupants_killed: u32,
        structure_damage: f32,
        bunker_amplified: bool,
    ) {
        self.blasts = self.blasts.saturating_add(1);
        self.occupants_killed = self.occupants_killed.saturating_add(occupants_killed);
        self.structure_damage_dealt += structure_damage.max(0.0);
        if bunker_amplified {
            self.bunker_damage_hits = self.bunker_damage_hits.saturating_add(1);
        }
    }

    pub fn record_kill_garrisoned(&mut self, occupants_killed: u32) {
        self.kill_garrisoned_hits = self.kill_garrisoned_hits.saturating_add(1);
        self.occupants_killed = self.occupants_killed.saturating_add(occupants_killed);
    }

    /// Residual honesty: at least one bunker-buster blast applied.
    pub fn honesty_blast_ok(&self) -> bool {
        self.blasts > 0
    }

    /// Residual honesty: at least one garrison occupant killed.
    pub fn honesty_garrison_kill_ok(&self) -> bool {
        self.occupants_killed > 0
    }

    /// Residual honesty: amplified bunker structure damage applied.
    pub fn honesty_bunker_damage_ok(&self) -> bool {
        self.bunker_damage_hits > 0 && self.structure_damage_dealt > 0.0
    }

    /// Combined host path: blast + (garrison kill or bunker damage).
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_blast_ok()
            && (self.honesty_garrison_kill_ok() || self.honesty_bunker_damage_ok())
    }

    /// KILL_GARRISONED residual honesty.
    pub fn honesty_kill_garrisoned_ok(&self) -> bool {
        self.kill_garrisoned_hits > 0 && self.occupants_killed > 0
    }
}

// --- Wave 66 residual honesty packs ---

/// Wave 66 residual honesty: StealthJetMissile weapon residual peel.
pub fn honesty_bunker_buster_missile_residual_ok() -> bool {
    STEALTH_JET_MISSILE_WEAPON == "StealthJetMissileWeapon"
        && STEALTH_JET_MISSILE_PROJECTILE == "StealthJetMissile"
        && (STEALTH_JET_MISSILE_DAMAGE - 100.0).abs() < 0.01
        && (STEALTH_JET_MISSILE_RADIUS - 5.0).abs() < 0.01
        && (STEALTH_JET_MISSILE_RANGE - 220.0).abs() < 0.01
        && (STEALTH_JET_MISSILE_MIN_RANGE - 60.0).abs() < 0.01
        && STEALTH_JET_MISSILE_DELAY_MS == 200
        && STEALTH_JET_MISSILE_DELAY_FRAMES
            == bunker_buster_ms_to_frames(STEALTH_JET_MISSILE_DELAY_MS)
        && STEALTH_JET_MISSILE_DELAY_FRAMES == 6
        && STEALTH_JET_MISSILE_CLIP_SIZE == 2
        && STEALTH_JET_MISSILE_CLIP_RELOAD_MS == 8000
        && STEALTH_JET_MISSILE_CLIP_RELOAD_FRAMES
            == bunker_buster_ms_to_frames(STEALTH_JET_MISSILE_CLIP_RELOAD_MS)
        && STEALTH_JET_MISSILE_CLIP_RELOAD_FRAMES == 240
        && STEALTH_JET_MISSILE_DAMAGE_TYPE == "STEALTHJET_MISSILES"
        && STEALTH_JET_MISSILE_DEATH_TYPE == "EXPLODED"
        && BUNKER_BUSTER_AUDIO == "StealthJetMissileWeapon"
}

/// Wave 66 residual honesty: bunker-buster behavior residual peel.
pub fn honesty_bunker_buster_behavior_residual_ok() -> bool {
    UPGRADE_AMERICA_BUNKER_BUSTERS == "Upgrade_AmericaBunkerBusters"
        && BUNKER_BUSTER_SHOCKWAVE_WEAPON == "BunkerBusterShockwaveWeaponSmall"
        && BUNKER_BUSTER_OCCUPANT_WEAPON
            == "BunkerBusterAntiTunnelGarrisonWeaponWithABigName"
        && (BUNKER_BUSTER_STRUCTURE_DAMAGE_MULT - 1.5).abs() < 0.001
        && (BUNKER_BUSTER_OCCUPANT_DAMAGE - 400.0).abs() < 0.01
        && (BUNKER_BUSTER_OCCUPANT_RADIUS - 10.0).abs() < 0.01
        && (BUNKER_BUSTER_OCCUPIED_BONUS_DAMAGE - 10.0).abs() < 0.01
        && (BUNKER_BUSTER_SHOCKWAVE_RADIUS - 50.0).abs() < 0.01
        && (BUNKER_BUSTER_SEISMIC_RADIUS - 200.0).abs() < 0.01
        && (BUNKER_BUSTER_SEISMIC_MAGNITUDE - 5.0).abs() < 0.01
        && BUNKER_BUSTER_CRASH_FX_FREQUENCY_MS == 571
        && BUNKER_BUSTER_DETONATION_FX == "FX_BunkerBusterExplosion"
        && {
            let d = bunker_buster_structure_damage(100.0, true, true);
            (d - 160.0).abs() < 0.01
        }
}

/// Wave 66 residual honesty: Stealth Fighter body residual peel.
pub fn honesty_bunker_buster_body_residual_ok() -> bool {
    (STEALTH_FIGHTER_MAX_HEALTH - 120.0).abs() < 0.01
        && (STEALTH_FIGHTER_VISION_RANGE - 180.0).abs() < 0.01
        && (STEALTH_FIGHTER_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && STEALTH_FIGHTER_BUILD_COST == 1600
        && (STEALTH_FIGHTER_BUILD_TIME_SEC - 25.0).abs() < 0.01
        && STEALTH_FIGHTER_BUILD_TIME_FRAMES
            == ((STEALTH_FIGHTER_BUILD_TIME_SEC * BUNKER_BUSTER_LOGIC_FPS).round() as u32)
        && STEALTH_FIGHTER_BUILD_TIME_FRAMES == 750
        && SCIENCE_STEALTH_FIGHTER == "SCIENCE_StealthFighter"
        && is_bunker_buster_carrier("AmericaJetStealthFighter")
        && !is_bunker_buster_carrier("StealthJetMissile")
}

/// Combined Wave 66 Bunker Buster residual honesty pack.
pub fn honesty_bunker_buster_residual_pack_ok() -> bool {
    honesty_bunker_buster_missile_residual_ok()
        && honesty_bunker_buster_behavior_residual_ok()
        && honesty_bunker_buster_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carrier_name_matrix() {
        assert!(is_bunker_buster_carrier("AmericaJetStealthFighter"));
        assert!(is_bunker_buster_carrier("USA_StealthFighter"));
        assert!(is_bunker_buster_carrier("SupW_AmericaJetStealthFighter"));
        assert!(is_bunker_buster_carrier("TestStealthFighter"));
        assert!(!is_bunker_buster_carrier("AmericaJetAurora"));
        assert!(!is_bunker_buster_carrier("StealthJetMissile"));
        assert!(!is_bunker_buster_carrier("USA_Ranger"));
        assert!(!is_bunker_buster_carrier("ChinaVehicleNukeCannon"));
    }

    #[test]
    fn bunker_structure_name_matrix() {
        assert!(is_bunker_structure_name("TestBunker"));
        assert!(is_bunker_structure_name("AmericaBunker"));
        assert!(is_bunker_structure_name("CivilianBunker"));
        assert!(!is_bunker_structure_name("TestBarracks"));
        assert!(!is_bunker_structure_name("USA_CommandCenter"));
    }

    #[test]
    fn should_apply_gates() {
        assert!(should_apply_bunker_buster(true, true, true));
        assert!(!should_apply_bunker_buster(false, true, true));
        assert!(!should_apply_bunker_buster(true, false, true));
        assert!(!should_apply_bunker_buster(true, true, false));
        assert!(should_apply_kill_garrisoned(true, true));
        assert!(!should_apply_kill_garrisoned(true, false));
    }

    #[test]
    fn structure_damage_amplifies_bunkers() {
        let base = 100.0;
        let normal = bunker_buster_structure_damage(base, false, false);
        assert!((normal - 100.0).abs() < 0.01);
        let bunker = bunker_buster_structure_damage(base, true, false);
        assert!(
            (bunker - 150.0).abs() < 0.01,
            "bunker mult residual 1.5x, got {bunker}"
        );
        let occupied = bunker_buster_structure_damage(base, true, true);
        assert!(
            (occupied - 160.0).abs() < 0.01,
            "occupied bunker +10 residual, got {occupied}"
        );
    }

    #[test]
    fn kill_garrisoned_count_matrix() {
        assert_eq!(kill_garrisoned_count(1.0, 5), 1);
        assert_eq!(kill_garrisoned_count(3.5, 5), 3);
        assert_eq!(kill_garrisoned_count(10.0, 2), 2);
        assert_eq!(kill_garrisoned_count(0.0, 5), 0);
        assert_eq!(kill_garrisoned_count(5.0, 0), 0);
    }

    #[test]
    fn honesty_tracks_blast_and_kills() {
        let mut reg = HostBunkerBusterRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        reg.record_bunker_buster_blast(2, 160.0, true);
        assert!(reg.honesty_blast_ok());
        assert!(reg.honesty_garrison_kill_ok());
        assert!(reg.honesty_bunker_damage_ok());
        assert!(reg.honesty_host_path_ok());
        reg.record_kill_garrisoned(1);
        assert!(reg.honesty_kill_garrisoned_ok());
        assert_eq!(reg.occupants_killed, 3);
    }

    #[test]
    fn bunker_buster_residual_pack_honesty_wave66() {
        assert_eq!(bunker_buster_ms_to_frames(200), 6);
        assert_eq!(bunker_buster_ms_to_frames(8000), 240);
        assert!(honesty_bunker_buster_missile_residual_ok());
        assert!(honesty_bunker_buster_behavior_residual_ok());
        assert!(honesty_bunker_buster_body_residual_ok());
        assert!(honesty_bunker_buster_residual_pack_ok());
        assert_eq!(STEALTH_JET_MISSILE_DAMAGE_TYPE, "STEALTHJET_MISSILES");
        assert_eq!(STEALTH_FIGHTER_BUILD_TIME_FRAMES, 750);
        assert_eq!(STEALTH_JET_MISSILE_CLIP_SIZE, 2);
    }
}
