//! Host China MiniGunner residual (ground/AA dual gun + continuous fire + chain guns + horde).
//!
//! Residual slice (playability):
//! - `Infa_ChinaInfantryMiniGunner` / MiniGunner variants spawn with PRIMARY
//!   `Infa_MiniGunnerGun` (dmg **10** / range **125** / Delay **500**ms → 15 frames)
//!   and SECONDARY `Infa_MiniGunnerGunAir` (dmg **10** / range **350** / AA only).
//! - Weapon chooser residual: airborne → secondary; ground → primary.
//! - Continuous fire ramp residual (`FiringTracker` ContinuousFireOne=**6** / Two=**12** /
//!   Coast=**1000**ms):
//!   - Base delay **15** frames → MEAN **7** (200% RoF) → FAST **5** (300% RoF).
//! - Chain Guns PLAYER_UPGRADE residual (`Upgrade_ChinaChainGuns`): damage × **1.25**.
//! - Horde residual (same China infantry HordeUpdate as Red Guard: Radius **30**, Count **5**):
//!   RATE_OF_FIRE **150%** stacks with continuous-fire ROF.
//! - Nationalism residual (`Upgrade_Nationalism` while in horde): additional ROF **125%**.
//!
//! Fail-closed honesty:
//! - Not full FiringTracker model-condition CONTINUOUS_FIRE_* animation matrix
//! - Not full bayonet tertiary / CaptureBuilding special residual
//! - Not SCIENCE_RedGuardTraining elite spawn residual
//! - Not network continuous-fire / chain-gun / horde replication (network deferred)

use super::Weapon;
use crate::game_logic::host_gattling_tank::GattlingFireLevel;
use crate::game_logic::host_red_guard::{
    delay_frames_to_reload_secs, INFANTRY_HORDE_ROF_MULT, INFANTRY_NATIONALISM_ROF_MULT,
};

/// Retail primary ground gun.
pub const MINIGUNNER_GUN: &str = "Infa_MiniGunnerGun";
/// Retail secondary anti-air gun.
pub const MINIGUNNER_GUN_AIR: &str = "Infa_MiniGunnerGunAir";
/// Retail Upgrade_ChinaChainGuns (shared with gattling residual).
pub const UPGRADE_CHINA_CHAIN_GUNS: &str = "Upgrade_ChinaChainGuns";

/// Retail Infa_MiniGunnerGun PrimaryDamage.
pub const MINIGUNNER_GROUND_DAMAGE: f32 = 10.0;
/// Retail Infa_MiniGunnerGun AttackRange.
pub const MINIGUNNER_GROUND_RANGE: f32 = 125.0;
/// Retail Infa_MiniGunnerGunAir PrimaryDamage.
pub const MINIGUNNER_AIR_DAMAGE: f32 = 10.0;
/// Retail Infa_MiniGunnerGunAir AttackRange.
pub const MINIGUNNER_AIR_RANGE: f32 = 350.0;

/// Retail DelayBetweenShots 500ms → 15 frames @ 30 FPS.
pub const MINIGUNNER_BASE_DELAY_FRAMES: u32 = 15;
/// ContinuousFireOne residual threshold.
pub const MINIGUNNER_CONTINUOUS_FIRE_ONE: u32 = 6;
/// ContinuousFireTwo residual threshold.
pub const MINIGUNNER_CONTINUOUS_FIRE_TWO: u32 = 12;
/// ContinuousFireCoast 1000ms → 30 frames @ 30 FPS.
pub const MINIGUNNER_COAST_FRAMES: u32 = 30;

/// RATE_OF_FIRE 200% → delay = base / 2.
pub const MINIGUNNER_MEAN_ROF_MULT: f32 = 2.0;
/// RATE_OF_FIRE 300% → delay = base / 3.
pub const MINIGUNNER_FAST_ROF_MULT: f32 = 3.0;
/// Chain Guns WeaponBonus DAMAGE 125%.
pub const MINIGUNNER_CHAIN_GUN_DAMAGE_MULT: f32 = 1.25;

/// Residual fire audio.
pub const MINIGUNNER_FIRE_AUDIO: &str = "RedGuardMinigunnerWeapon";
/// Residual AA fire audio.
pub const MINIGUNNER_AA_FIRE_AUDIO: &str = "GattlingTankWeapon";
/// Retail VoiceRapidFire residual cue when entering FAST.
pub const MINIGUNNER_RAPID_FIRE_AUDIO: &str = "RedMinigunnerVoiceAttack";

/// Whether template is a residual China MiniGunner infantry.
///
/// Fail-closed: name residual. Excludes weapons / science / debris tokens.
pub fn is_minigunner_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("training")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("button")
        || n.contains("command")
        || n.contains("portrait")
        // Weapon tokens: Infa_MiniGunnerGun / Infa_MiniGunnerGunAir (not the infantry unit).
        || n.ends_with("gun")
        || n.ends_with("gunair")
        || n.contains("gungun")
        || n.contains("minigunnergun")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testminigunner" || n == "china_minigunner" || n == "minigunner" {
        return true;
    }
    // Unit template residual: name must include MiniGunner as the unit identity,
    // not just a weapon prefix. Prefer "infantry" / "china" / exact shorthand.
    if n.contains("minigunner") || n.contains("mini_gunner") {
        return true;
    }
    false
}

/// Whether residual target is airborne (AA secondary path).
pub fn target_is_airborne_for_minigunner(is_aircraft: bool, airborne_target: bool) -> bool {
    is_aircraft || airborne_target
}

/// Slot residual: 1 = AA secondary, 0 = ground primary.
pub fn preferred_minigunner_slot(target_is_air: bool) -> u8 {
    if target_is_air {
        1
    } else {
        0
    }
}

/// Continuous-fire ROF multiplier residual (1 / 2 / 3).
pub fn continuous_rof_multiplier(level: GattlingFireLevel) -> f32 {
    match level {
        GattlingFireLevel::Base => 1.0,
        GattlingFireLevel::Mean => MINIGUNNER_MEAN_ROF_MULT,
        GattlingFireLevel::Fast => MINIGUNNER_FAST_ROF_MULT,
    }
}

/// Combined ROF residual: continuous * horde * nationalism.
///
/// Nationalism only applies while in horde (C++ AIUpdate evaluateMoraleBonus).
pub fn minigunner_rof_multiplier(
    level: GattlingFireLevel,
    in_horde: bool,
    has_nationalism: bool,
) -> f32 {
    let mut rof = continuous_rof_multiplier(level);
    if in_horde {
        rof *= INFANTRY_HORDE_ROF_MULT;
        if has_nationalism {
            rof *= INFANTRY_NATIONALISM_ROF_MULT;
        }
    }
    rof
}

/// Delay frames residual: floor(base / ROF), min 1.
pub fn minigunner_delay_frames(
    level: GattlingFireLevel,
    in_horde: bool,
    has_nationalism: bool,
) -> u32 {
    let base = MINIGUNNER_BASE_DELAY_FRAMES as f32;
    let rof = minigunner_rof_multiplier(level, in_horde, has_nationalism);
    (base / rof).floor().max(1.0) as u32
}

/// Apply Chain Guns residual damage mult when upgrade present.
pub fn minigunner_damage_with_chain_guns(base_damage: f32, has_chain_guns: bool) -> f32 {
    if has_chain_guns {
        base_damage * MINIGUNNER_CHAIN_GUN_DAMAGE_MULT
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

/// Ground gun residual stats (damage, range, delay_frames).
pub fn minigunner_ground_stats(
    level: GattlingFireLevel,
    has_chain_guns: bool,
    in_horde: bool,
    has_nationalism: bool,
) -> (f32, f32, u32) {
    let dmg = minigunner_damage_with_chain_guns(MINIGUNNER_GROUND_DAMAGE, has_chain_guns);
    (
        dmg,
        MINIGUNNER_GROUND_RANGE,
        minigunner_delay_frames(level, in_horde, has_nationalism),
    )
}

/// Air gun residual stats (damage, range, delay_frames).
pub fn minigunner_air_stats(
    level: GattlingFireLevel,
    has_chain_guns: bool,
    in_horde: bool,
    has_nationalism: bool,
) -> (f32, f32, u32) {
    let dmg = minigunner_damage_with_chain_guns(MINIGUNNER_AIR_DAMAGE, has_chain_guns);
    (
        dmg,
        MINIGUNNER_AIR_RANGE,
        minigunner_delay_frames(level, in_horde, has_nationalism),
    )
}

/// Build residual ground Weapon.
pub fn minigunner_ground_weapon(
    level: GattlingFireLevel,
    has_chain_guns: bool,
    in_horde: bool,
    has_nationalism: bool,
) -> Weapon {
    let (dmg, range, delay) =
        minigunner_ground_stats(level, has_chain_guns, in_horde, has_nationalism);
    Weapon {
        damage: dmg,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Build residual air Weapon.
pub fn minigunner_air_weapon(
    level: GattlingFireLevel,
    has_chain_guns: bool,
    in_horde: bool,
    has_nationalism: bool,
) -> Weapon {
    let (dmg, range, delay) =
        minigunner_air_stats(level, has_chain_guns, in_horde, has_nationalism);
    Weapon {
        damage: dmg,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Advance continuous-fire residual state after a shot (MiniGunner thresholds).
///
/// Mirrors C++ `FiringTracker::shotFired` spin-up with ContinuousFireOne=6 / Two=12.
/// Returns `(new_level, consecutive, entered_fast)`.
pub fn minigunner_on_shot_fired(
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
            if consecutive < MINIGUNNER_CONTINUOUS_FIRE_ONE {
                level = GattlingFireLevel::Base;
            } else if consecutive > MINIGUNNER_CONTINUOUS_FIRE_TWO {
                level = GattlingFireLevel::Fast;
                entered_fast = true;
            }
        }
        GattlingFireLevel::Fast => {
            if consecutive < MINIGUNNER_CONTINUOUS_FIRE_TWO {
                level = GattlingFireLevel::Base;
            }
        }
        GattlingFireLevel::Base => {
            if consecutive > MINIGUNNER_CONTINUOUS_FIRE_ONE {
                level = GattlingFireLevel::Mean;
            }
        }
    }

    (level, consecutive, entered_fast)
}

/// Next coast-until frame after a shot.
pub fn minigunner_coast_until_after_shot(
    current_frame: u32,
    level: GattlingFireLevel,
    in_horde: bool,
    has_nationalism: bool,
) -> u32 {
    let delay = minigunner_delay_frames(level, in_horde, has_nationalism);
    current_frame
        .saturating_add(delay)
        .saturating_add(MINIGUNNER_COAST_FRAMES)
}

/// Legal residual minigunner hit target.
pub fn is_legal_minigunner_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply MiniGunner residual path.
pub fn should_apply_minigunner_residual(is_minigunner: bool) -> bool {
    is_minigunner
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn minigunner_name_matrix() {
        assert!(is_minigunner_template("Infa_ChinaInfantryMiniGunner"));
        assert!(is_minigunner_template("ChinaInfantryMiniGunner"));
        assert!(is_minigunner_template("China_MiniGunner"));
        assert!(is_minigunner_template("TestMiniGunner"));
        assert!(is_minigunner_template("Nuke_ChinaInfantryMiniGunner"));
        assert!(!is_minigunner_template("Infa_MiniGunnerGun"));
        assert!(!is_minigunner_template("Infa_MiniGunnerGunAir"));
        assert!(!is_minigunner_template("Upgrade_ChinaChainGuns"));
        assert!(!is_minigunner_template("ChinaInfantryRedguard"));
        assert!(!is_minigunner_template("ChinaTankGattling"));
        assert!(!is_minigunner_template("Command_ConstructChinaInfantryMiniGunner"));
    }

    #[test]
    fn base_gun_stats() {
        let (d, r, f) = minigunner_ground_stats(GattlingFireLevel::Base, false, false, false);
        assert!((d - 10.0).abs() < 0.01);
        assert!((r - 125.0).abs() < 0.01);
        assert_eq!(f, 15);
        let w = minigunner_ground_weapon(GattlingFireLevel::Base, false, false, false);
        assert!((w.reload_time - (15.0 / 30.0)).abs() < 0.01);
        assert!(!w.can_target_air && w.can_target_ground);
        let a = minigunner_air_weapon(GattlingFireLevel::Base, false, false, false);
        assert!((a.range - 350.0).abs() < 0.01);
        assert!(a.can_target_air && !a.can_target_ground);
    }

    #[test]
    fn continuous_fire_ramp_delays() {
        // Base 15, MEAN floor(15/2)=7, FAST floor(15/3)=5
        assert_eq!(
            minigunner_delay_frames(GattlingFireLevel::Base, false, false),
            15
        );
        assert_eq!(
            minigunner_delay_frames(GattlingFireLevel::Mean, false, false),
            7
        );
        assert_eq!(
            minigunner_delay_frames(GattlingFireLevel::Fast, false, false),
            5
        );
    }

    #[test]
    fn horde_and_nationalism_stack_with_ramp() {
        // Base + horde: floor(15/1.5)=10
        assert_eq!(
            minigunner_delay_frames(GattlingFireLevel::Base, true, false),
            10
        );
        // Base + horde + nationalism: floor(15/1.875)=8
        assert_eq!(
            minigunner_delay_frames(GattlingFireLevel::Base, true, true),
            8
        );
        // MEAN + horde: floor(15/(2*1.5))=floor(5)=5
        assert_eq!(
            minigunner_delay_frames(GattlingFireLevel::Mean, true, false),
            5
        );
        // Nationalism without horde does nothing residual.
        assert_eq!(
            minigunner_delay_frames(GattlingFireLevel::Base, false, true),
            15
        );
    }

    #[test]
    fn chain_guns_damage() {
        assert!((minigunner_damage_with_chain_guns(10.0, false) - 10.0).abs() < 0.01);
        assert!((minigunner_damage_with_chain_guns(10.0, true) - 12.5).abs() < 0.01);
        let mut set = HashSet::new();
        set.insert(UPGRADE_CHINA_CHAIN_GUNS.to_string());
        assert!(has_chain_guns_upgrade(&set));
    }

    #[test]
    fn continuous_fire_shot_thresholds() {
        // Need consecutive > 6 for MEAN.
        let (lvl, c, _) = minigunner_on_shot_fired(
            GattlingFireLevel::Base,
            6,
            Some(1),
            Some(1),
            10,
            100,
        );
        assert_eq!(c, 7);
        assert_eq!(lvl, GattlingFireLevel::Mean);

        // Need consecutive > 12 for FAST.
        let (lvl2, c2, entered) = minigunner_on_shot_fired(
            GattlingFireLevel::Mean,
            12,
            Some(1),
            Some(1),
            20,
            100,
        );
        assert_eq!(c2, 13);
        assert_eq!(lvl2, GattlingFireLevel::Fast);
        assert!(entered);
    }

    #[test]
    fn slot_and_legal() {
        assert_eq!(preferred_minigunner_slot(false), 0);
        assert_eq!(preferred_minigunner_slot(true), 1);
        assert!(should_apply_minigunner_residual(true));
        assert!(!should_apply_minigunner_residual(false));
        assert!(is_legal_minigunner_target(true, false, false, true));
        assert!(!is_legal_minigunner_target(false, false, false, true));
        assert!(!is_legal_minigunner_target(true, true, false, true));
    }
}
