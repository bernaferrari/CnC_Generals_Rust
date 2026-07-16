//! Host China Gattling Tank residual (ground/AA dual gun + continuous fire ramp).
//!
//! Residual slice (playability):
//! - Spawns with PRIMARY `GattlingTankGun` (dmg **15** / range **150** / Delay **400**ms)
//!   and SECONDARY `GattlingTankGunAir` (dmg **12** / range **350** / AA only).
//! - Weapon chooser residual: airborne → secondary; ground → primary.
//! - Continuous fire ramp residual (`FiringTracker` ContinuousFireOne/Two):
//!   - Base: Delay 400ms (12 frames @ 30 FPS)
//!   - MEAN (after > ContinuousFireOne=2 consecutive): RATE_OF_FIRE **200%** → 6 frames
//!   - FAST (after > ContinuousFireTwo=6 consecutive): RATE_OF_FIRE **300%** → 4 frames
//!   - Coast **1000**ms (30 frames) without fire resets spin-down residual
//! - Chain Guns PLAYER_UPGRADE residual (`Upgrade_ChinaChainGuns`): damage × **1.25**
//!
//! Fail-closed honesty:
//! - Not full FiringTracker model-condition CONTINUOUS_FIRE_* animation matrix
//! - Not full VoiceRapidFire / muzzle-smoke particle on FAST residual
//! - Not Overlord/Helix/building gattling payload matrix (structure uses host_base_defense)
//! - Not network continuous-fire / chain-gun replication (network deferred)

use super::Weapon;

/// Retail primary ground gun.
pub const GATTLING_TANK_GUN: &str = "GattlingTankGun";
/// Retail secondary anti-air gun.
pub const GATTLING_TANK_GUN_AIR: &str = "GattlingTankGunAir";
/// Retail Upgrade_ChinaChainGuns.
pub const UPGRADE_CHINA_CHAIN_GUNS: &str = "Upgrade_ChinaChainGuns";

/// Retail GattlingTankGun PrimaryDamage.
pub const GATTLING_GROUND_DAMAGE: f32 = 15.0;
/// Retail GattlingTankGun AttackRange.
pub const GATTLING_GROUND_RANGE: f32 = 150.0;
/// Retail GattlingTankGunAir PrimaryDamage.
pub const GATTLING_AIR_DAMAGE: f32 = 12.0;
/// Retail GattlingTankGunAir AttackRange.
pub const GATTLING_AIR_RANGE: f32 = 350.0;

/// Retail DelayBetweenShots 400ms → 12 frames @ 30 FPS.
pub const GATTLING_BASE_DELAY_FRAMES: u32 = 12;
/// ContinuousFireOne for ground gun (shots needed residual threshold).
pub const GATTLING_CONTINUOUS_FIRE_ONE: u32 = 2;
/// ContinuousFireTwo for ground gun.
pub const GATTLING_CONTINUOUS_FIRE_TWO: u32 = 6;
/// ContinuousFireCoast 1000ms → 30 frames @ 30 FPS.
pub const GATTLING_COAST_FRAMES: u32 = 30;

/// RATE_OF_FIRE 200% → delay = base / 2.
pub const GATTLING_MEAN_ROF_MULT: f32 = 2.0;
/// RATE_OF_FIRE 300% → delay = base / 3.
pub const GATTLING_FAST_ROF_MULT: f32 = 3.0;

/// Chain Guns WeaponBonus DAMAGE 125%.
pub const GATTLING_CHAIN_GUN_DAMAGE_MULT: f32 = 1.25;

/// Residual fire audio.
pub const GATTLING_FIRE_AUDIO: &str = "GattlingTankWeapon";
/// Retail VoiceRapidFire residual cue when entering FAST.
pub const GATTLING_RAPID_FIRE_AUDIO: &str = "GattlingTankVoiceRapid";

/// Continuous-fire ramp residual level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GattlingFireLevel {
    /// Base / CONTINUOUS_FIRE_SLOW residual (no ROF bonus).
    #[default]
    Base = 0,
    /// CONTINUOUS_FIRE_MEAN — 200% ROF.
    Mean = 1,
    /// CONTINUOUS_FIRE_FAST — 300% ROF.
    Fast = 2,
}

impl GattlingFireLevel {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Mean,
            2 => Self::Fast,
            _ => Self::Base,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::Base => 0,
            Self::Mean => 1,
            Self::Fast => 2,
        }
    }

    pub fn rof_multiplier(self) -> f32 {
        match self {
            Self::Base => 1.0,
            Self::Mean => GATTLING_MEAN_ROF_MULT,
            Self::Fast => GATTLING_FAST_ROF_MULT,
        }
    }
}

/// Whether template is a residual Gattling Tank vehicle.
///
/// Fail-closed: name residual. Excludes structure Gattling Cannon, Overlord/Helix
/// payloads, weapons, and science tokens.
pub fn is_gattling_tank_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Weapon / upgrade / science / debris tokens.
    if n.contains("weapon")
        || n.contains("gun")
        || n.contains("shell")
        || n.contains("projectile")
        || n.contains("missile")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("training")
        || n.contains("dead")
        || n.contains("hulk")
        || n.contains("debris")
    {
        return false;
    }
    // Structure base-defense gattling is host_base_defense residual, not tank.
    if n.contains("cannon")
        && !n.contains("tank")
        && !n.contains("vehicle")
        && (n.contains("gattling") || n.contains("gatling"))
    {
        // e.g. China_GattlingCannon / ChinaGattlingCannon building
        if !n.contains("overlord") && !n.contains("helix") {
            // Building names often "GattlingCannon" without tank/vehicle.
            // OverlordGattlingCannon / HelixGattlingCannon are payload modules — exclude.
            return false;
        }
        return false;
    }
    // Overlord / Helix portable gattling payloads are not the tank residual.
    if n.contains("overlord") || n.contains("helix") {
        return false;
    }
    n.contains("gattlingtank")
        || n.contains("gatlingtank")
        || n.contains("tankgattling")
        || n.contains("tankgatling")
        || n.contains("vehiclegattling")
        || n.contains("vehiclegatling")
        || n == "china_gattlingtank"
        || n == "testgattlingtank"
        || n == "testgatlingtank"
}

/// Whether residual target is airborne (AA secondary path).
pub fn target_is_airborne_for_gattling(is_aircraft: bool, airborne_target: bool) -> bool {
    is_aircraft || airborne_target
}

/// Slot residual for Gattling Tank: 1 = AA secondary, 0 = ground primary.
pub fn preferred_gattling_slot(target_is_air: bool) -> u8 {
    if target_is_air {
        1
    } else {
        0
    }
}

/// Delay frames residual for continuous-fire level (base / ROF).
///
/// C++ uses floor(delay / ROF). Residual:
/// - Base: 12
/// - Mean: floor(12/2)=6
/// - Fast: floor(12/3)=4
pub fn gattling_delay_frames_for_level(level: GattlingFireLevel) -> u32 {
    let base = GATTLING_BASE_DELAY_FRAMES as f32;
    let rof = level.rof_multiplier();
    (base / rof).floor().max(1.0) as u32
}

/// Apply Chain Guns residual damage mult when upgrade present.
pub fn gattling_damage_with_chain_guns(base_damage: f32, has_chain_guns: bool) -> f32 {
    if has_chain_guns {
        base_damage * GATTLING_CHAIN_GUN_DAMAGE_MULT
    } else {
        base_damage
    }
}

/// Whether Chain Guns upgrade is active (tag present).
pub fn has_chain_guns_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("chaingun") || l == "upgrade_chinachainguns"
    })
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Ground gun residual stats (damage, range, delay_frames) for level + chain guns.
pub fn gattling_ground_stats(level: GattlingFireLevel, has_chain_guns: bool) -> (f32, f32, u32) {
    let dmg = gattling_damage_with_chain_guns(GATTLING_GROUND_DAMAGE, has_chain_guns);
    (
        dmg,
        GATTLING_GROUND_RANGE,
        gattling_delay_frames_for_level(level),
    )
}

/// Air gun residual stats (damage, range, delay_frames) for level + chain guns.
pub fn gattling_air_stats(level: GattlingFireLevel, has_chain_guns: bool) -> (f32, f32, u32) {
    let dmg = gattling_damage_with_chain_guns(GATTLING_AIR_DAMAGE, has_chain_guns);
    (
        dmg,
        GATTLING_AIR_RANGE,
        gattling_delay_frames_for_level(level),
    )
}

/// Build residual ground Weapon for level + chain guns.
pub fn gattling_ground_weapon(level: GattlingFireLevel, has_chain_guns: bool) -> Weapon {
    let (dmg, range, delay) = gattling_ground_stats(level, has_chain_guns);
    Weapon {
        damage: dmg,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Build residual air Weapon for level + chain guns.
pub fn gattling_air_weapon(level: GattlingFireLevel, has_chain_guns: bool) -> Weapon {
    let (dmg, range, delay) = gattling_air_stats(level, has_chain_guns);
    Weapon {
        damage: dmg,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Advance continuous-fire residual state after a shot.
///
/// Mirrors C++ `FiringTracker::shotFired` spin-up thresholds (exclusive flags).
/// Returns `(new_level, consecutive, entered_fast)`.
pub fn gattling_on_shot_fired(
    previous_level: GattlingFireLevel,
    previous_consecutive: u32,
    previous_victim: Option<u32>,
    new_victim: Option<u32>,
    current_frame: u32,
    coast_until_frame: u32,
) -> (GattlingFireLevel, u32, bool) {
    let same_or_within_coast = match (previous_victim, new_victim) {
        (Some(a), Some(b)) if a == b => true,
        _ if current_frame < coast_until_frame => true,
        _ => false,
    };

    let consecutive = if same_or_within_coast {
        previous_consecutive.saturating_add(1).max(1)
    } else {
        1
    };

    let mut level = previous_level;
    let mut entered_fast = false;

    match previous_level {
        GattlingFireLevel::Mean => {
            if consecutive < GATTLING_CONTINUOUS_FIRE_ONE {
                level = GattlingFireLevel::Base;
            } else if consecutive > GATTLING_CONTINUOUS_FIRE_TWO {
                level = GattlingFireLevel::Fast;
                entered_fast = true;
            }
        }
        GattlingFireLevel::Fast => {
            if consecutive < GATTLING_CONTINUOUS_FIRE_TWO {
                // C++ coolDown: straight to zero from FAST.
                level = GattlingFireLevel::Base;
            }
        }
        GattlingFireLevel::Base => {
            if consecutive > GATTLING_CONTINUOUS_FIRE_ONE {
                level = GattlingFireLevel::Mean;
            }
        }
    }

    (level, consecutive, entered_fast)
}

/// Next coast-until frame after a shot (next possible shot frame + coast residual).
///
/// Fail-closed: uses current_frame + delay_frames + coast (not full PossibleNextShotFrame).
pub fn gattling_coast_until_after_shot(current_frame: u32, level: GattlingFireLevel) -> u32 {
    let delay = gattling_delay_frames_for_level(level);
    current_frame
        .saturating_add(delay)
        .saturating_add(GATTLING_COAST_FRAMES)
}

/// Coast elapsed: spin down to base and clear consecutive residual.
pub fn gattling_coast_spin_down(
    current_frame: u32,
    coast_until_frame: u32,
    level: GattlingFireLevel,
) -> Option<(GattlingFireLevel, u32)> {
    if coast_until_frame == 0 || current_frame <= coast_until_frame {
        return None;
    }
    if matches!(level, GattlingFireLevel::Base) {
        // Already cool; clear consecutive residual.
        return Some((GattlingFireLevel::Base, 0));
    }
    // C++ coolDown from MEAN/FAST → base + consecutive = 0.
    Some((GattlingFireLevel::Base, 0))
}

/// Legal residual gattling hit target.
pub fn is_legal_gattling_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

// --- Wave 69 residual honesty peels (retail weapons / body / continuous fire) ---

/// Logic frames per second residual.
pub const GATTLING_LOGIC_FPS: f32 = 30.0;

/// Convert residual msec → logic frames @ 30 FPS.
pub fn gattling_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * GATTLING_LOGIC_FPS / 1000.0).round() as u32
}

/// Retail ground DelayBetweenShots residual (msec).
pub const GATTLING_BASE_DELAY_MS: u32 = 400;
/// Retail ContinuousFireCoast residual (msec).
pub const GATTLING_COAST_MS: u32 = 1_000;
/// Retail ground DamageType residual.
pub const GATTLING_GROUND_DAMAGE_TYPE: &str = "Gattling";
/// Retail air DamageType residual.
pub const GATTLING_AIR_DAMAGE_TYPE: &str = "SMALL_ARMS";
/// Retail FireFX residual.
pub const GATTLING_FIRE_FX: &str = "WeaponFX_GattlingTankMachineGunFire";
/// Retail ground ClipSize residual (0 = infinite).
pub const GATTLING_CLIP_SIZE: u32 = 0;

/// Retail ChinaTankGattling body residual.
pub const GATTLING_MAX_HEALTH: f32 = 300.0;
pub const GATTLING_BUILD_COST: u32 = 800;
pub const GATTLING_BUILD_TIME_SEC: f32 = 10.0;
pub const GATTLING_BUILD_TIME_FRAMES: u32 = 300;
pub const GATTLING_VISION_RANGE: f32 = 150.0;
pub const GATTLING_SHROUD_CLEARING_RANGE: f32 = 360.0;
pub const GATTLING_TRANSPORT_SLOT_COUNT: u32 = 3;
pub const GATTLING_LOCOMOTOR_SPEED: f32 = 40.0;
pub const GATTLING_LOCOMOTOR_SPEED_DAMAGED: f32 = 40.0;

/// Wave 69 residual honesty: ground/air weapon residual peel.
pub fn honesty_gattling_tank_weapon_residual_ok() -> bool {
    GATTLING_TANK_GUN == "GattlingTankGun"
        && GATTLING_TANK_GUN_AIR == "GattlingTankGunAir"
        && (GATTLING_GROUND_DAMAGE - 15.0).abs() < 0.01
        && (GATTLING_GROUND_RANGE - 150.0).abs() < 0.01
        && (GATTLING_AIR_DAMAGE - 12.0).abs() < 0.01
        && (GATTLING_AIR_RANGE - 350.0).abs() < 0.01
        && GATTLING_BASE_DELAY_MS == 400
        && GATTLING_BASE_DELAY_FRAMES == gattling_ms_to_frames(GATTLING_BASE_DELAY_MS)
        && GATTLING_BASE_DELAY_FRAMES == 12
        && GATTLING_GROUND_DAMAGE_TYPE == "Gattling"
        && GATTLING_AIR_DAMAGE_TYPE == "SMALL_ARMS"
        && GATTLING_FIRE_FX == "WeaponFX_GattlingTankMachineGunFire"
        && GATTLING_FIRE_AUDIO == "GattlingTankWeapon"
        && GATTLING_CLIP_SIZE == 0
        && {
            let g = gattling_ground_weapon(GattlingFireLevel::Base, false);
            let a = gattling_air_weapon(GattlingFireLevel::Base, false);
            (g.damage - 15.0).abs() < 0.01
                && g.can_target_ground
                && !g.can_target_air
                && (a.damage - 12.0).abs() < 0.01
                && a.can_target_air
                && !a.can_target_ground
        }
}

/// Wave 69 residual honesty: continuous-fire ramp residual peel.
pub fn honesty_gattling_tank_continuous_fire_residual_ok() -> bool {
    GATTLING_CONTINUOUS_FIRE_ONE == 2
        && GATTLING_CONTINUOUS_FIRE_TWO == 6
        && GATTLING_COAST_MS == 1_000
        && GATTLING_COAST_FRAMES == gattling_ms_to_frames(GATTLING_COAST_MS)
        && GATTLING_COAST_FRAMES == 30
        && (GATTLING_MEAN_ROF_MULT - 2.0).abs() < 0.01
        && (GATTLING_FAST_ROF_MULT - 3.0).abs() < 0.01
        && gattling_delay_frames_for_level(GattlingFireLevel::Base) == 12
        && gattling_delay_frames_for_level(GattlingFireLevel::Mean) == 6
        && gattling_delay_frames_for_level(GattlingFireLevel::Fast) == 4
        && (GATTLING_CHAIN_GUN_DAMAGE_MULT - 1.25).abs() < 0.01
        && UPGRADE_CHINA_CHAIN_GUNS == "Upgrade_ChinaChainGuns"
        && (gattling_damage_with_chain_guns(15.0, true) - 18.75).abs() < 0.01
        && preferred_gattling_slot(true) == 1
        && preferred_gattling_slot(false) == 0
        && GATTLING_RAPID_FIRE_AUDIO == "GattlingTankVoiceRapid"
}

/// Wave 69 residual honesty: body residual peel.
pub fn honesty_gattling_tank_body_residual_ok() -> bool {
    (GATTLING_MAX_HEALTH - 300.0).abs() < 0.01
        && GATTLING_BUILD_COST == 800
        && (GATTLING_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && GATTLING_BUILD_TIME_FRAMES
            == ((GATTLING_BUILD_TIME_SEC * GATTLING_LOGIC_FPS).round() as u32)
        && GATTLING_BUILD_TIME_FRAMES == 300
        && (GATTLING_VISION_RANGE - 150.0).abs() < 0.01
        && (GATTLING_SHROUD_CLEARING_RANGE - 360.0).abs() < 0.01
        && GATTLING_TRANSPORT_SLOT_COUNT == 3
        && (GATTLING_LOCOMOTOR_SPEED - 40.0).abs() < 0.01
        && (GATTLING_LOCOMOTOR_SPEED_DAMAGED - 40.0).abs() < 0.01
        && is_gattling_tank_template("ChinaTankGattling")
        && !is_gattling_tank_template("ChinaGattlingCannon")
        && !is_gattling_tank_template("GattlingTankGun")
}

/// Combined Wave 69 Gattling Tank residual honesty pack.
pub fn honesty_gattling_tank_residual_pack_ok() -> bool {
    honesty_gattling_tank_weapon_residual_ok()
        && honesty_gattling_tank_continuous_fire_residual_ok()
        && honesty_gattling_tank_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn gattling_tank_name_matrix() {
        assert!(is_gattling_tank_template("ChinaTankGattling"));
        assert!(is_gattling_tank_template("ChinaVehicleGattlingTank"));
        assert!(is_gattling_tank_template("China_GattlingTank"));
        assert!(is_gattling_tank_template("Nuke_ChinaTankGattling"));
        assert!(is_gattling_tank_template("Tank_ChinaTankGattling"));
        assert!(is_gattling_tank_template("TestGattlingTank"));
        // Structure base defense — not tank residual.
        assert!(!is_gattling_tank_template("China_GattlingCannon"));
        assert!(!is_gattling_tank_template("ChinaGattlingCannon"));
        // Overlord / Helix payload — not tank residual.
        assert!(!is_gattling_tank_template(
            "ChinaTankOverlordGattlingCannon"
        ));
        assert!(!is_gattling_tank_template("ChinaHelixGattlingCannon"));
        // Weapons / upgrades.
        assert!(!is_gattling_tank_template("GattlingTankGun"));
        assert!(!is_gattling_tank_template("GattlingTankGunAir"));
        assert!(!is_gattling_tank_template("Upgrade_ChinaChainGuns"));
        assert!(!is_gattling_tank_template("SCIENCE_GattlingTankTraining"));
        assert!(!is_gattling_tank_template("ChinaTankDragon"));
        assert!(!is_gattling_tank_template("USA_Ranger"));
    }

    #[test]
    fn continuous_fire_ramp_thresholds() {
        // Shot 1 → consecutive 1, stay Base.
        let (l1, c1, f1) = gattling_on_shot_fired(GattlingFireLevel::Base, 0, None, Some(10), 0, 0);
        assert_eq!(l1, GattlingFireLevel::Base);
        assert_eq!(c1, 1);
        assert!(!f1);

        // Shot 2 → consecutive 2, still Base (need > 2).
        let (l2, c2, _) = gattling_on_shot_fired(l1, c1, Some(10), Some(10), 12, 100);
        assert_eq!(l2, GattlingFireLevel::Base);
        assert_eq!(c2, 2);

        // Shot 3 → consecutive 3 > 2 → Mean.
        let (l3, c3, f3) = gattling_on_shot_fired(l2, c2, Some(10), Some(10), 24, 100);
        assert_eq!(l3, GattlingFireLevel::Mean);
        assert_eq!(c3, 3);
        assert!(!f3);

        // Continue to shot 7 → Fast.
        let mut level = l3;
        let mut consec = c3;
        for shot in 4..=7 {
            let (nl, nc, entered) =
                gattling_on_shot_fired(level, consec, Some(10), Some(10), shot * 6, 1000);
            level = nl;
            consec = nc;
            if shot == 7 {
                assert_eq!(level, GattlingFireLevel::Fast);
                assert!(entered);
            }
        }
        assert_eq!(consec, 7);
        assert_eq!(level, GattlingFireLevel::Fast);

        // Delays: 12 → 6 → 4.
        assert_eq!(gattling_delay_frames_for_level(GattlingFireLevel::Base), 12);
        assert_eq!(gattling_delay_frames_for_level(GattlingFireLevel::Mean), 6);
        assert_eq!(gattling_delay_frames_for_level(GattlingFireLevel::Fast), 4);
    }

    #[test]
    fn chain_guns_and_air_slot() {
        assert!((gattling_damage_with_chain_guns(15.0, true) - 18.75).abs() < 0.01);
        assert_eq!(preferred_gattling_slot(true), 1);
        assert_eq!(preferred_gattling_slot(false), 0);
        assert!(target_is_airborne_for_gattling(true, false));

        let mut tags = HashSet::new();
        assert!(!has_chain_guns_upgrade(&tags));
        tags.insert(UPGRADE_CHINA_CHAIN_GUNS.to_string());
        assert!(has_chain_guns_upgrade(&tags));

        let g = gattling_ground_weapon(GattlingFireLevel::Base, false);
        assert!((g.damage - 15.0).abs() < 0.01);
        assert!(g.can_target_ground);
        assert!(!g.can_target_air);
        let a = gattling_air_weapon(GattlingFireLevel::Fast, true);
        assert!((a.damage - 15.0).abs() < 0.01); // 12 * 1.25
        assert!(a.can_target_air);
        assert!(!a.can_target_ground);
        assert!((a.reload_time - (4.0 / 30.0)).abs() < 0.01);
    }

    #[test]
    fn coast_spin_down() {
        assert!(gattling_coast_spin_down(10, 20, GattlingFireLevel::Mean).is_none());
        let sd = gattling_coast_spin_down(50, 20, GattlingFireLevel::Fast).unwrap();
        assert_eq!(sd.0, GattlingFireLevel::Base);
        assert_eq!(sd.1, 0);
    }

    #[test]
    fn gattling_tank_residual_pack_honesty_wave69() {
        assert_eq!(gattling_ms_to_frames(400), 12);
        assert_eq!(gattling_ms_to_frames(1000), 30);
        assert!(honesty_gattling_tank_weapon_residual_ok());
        assert!(honesty_gattling_tank_continuous_fire_residual_ok());
        assert!(honesty_gattling_tank_body_residual_ok());
        assert!(honesty_gattling_tank_residual_pack_ok());
        assert_eq!(GATTLING_BUILD_TIME_FRAMES, 300);
        assert_eq!(GATTLING_GROUND_DAMAGE_TYPE, "Gattling");
        assert_eq!(GATTLING_AIR_DAMAGE_TYPE, "SMALL_ARMS");
    }
}
