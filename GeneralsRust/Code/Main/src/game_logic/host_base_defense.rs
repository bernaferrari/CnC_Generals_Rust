//! Host base-defense structure residual (Patriot / Gattling / Stinger auto-fire).
//!
//! Residual slice (playability):
//! - Base defenses (USA Patriot, China Gattling Cannon, GLA Stinger Site, and
//!   `FSBaseDefense` structures) auto-acquire and damage nearby enemies while
//!   Idle without a manual `AttackObject` / player attack order.
//! - Retail weapon names: `PatriotMissileWeapon` (dmg 30, range 225) and
//!   `GattlingBuildingGun` (dmg 10, range 225) + SECONDARY `GattlingBuildingGunAir`
//!   (dmg 5, range 400, AA only).
//! - China Gattling Cannon continuous-fire ramp residual (`FiringTracker`):
//!   - ContinuousFireOne=**1** / Two=**5** / Coast=**2000**ms (60 frames)
//!   - Base Delay **250**ms (8 frames) → MEAN **4** (200% RoF) → FAST **2** (300% RoF)
//!   - Chain Guns PLAYER_UPGRADE residual (`Upgrade_ChinaChainGuns`): damage × **1.25**
//! - C++ `AIUpdateInterface` AutoAcquireEnemiesWhenIdle residual for stationary
//!   base defenses (not full turret pitch / LOS).
//!
//! Fail-closed honesty:
//! - Not full WeaponSet PRIMARY/SECONDARY/TERTIARY chooser beyond air/ground residual
//! - Not full PointDefenseLaserUpdate missile intercept matrix
//! - Not full AssistedTargetingModule Patriot assist clips
//! - Not full CONTINUOUS_FIRE_* model-condition animation / VoiceRapidFire matrix
//! - Not network base-defense replication (network deferred)

use super::Weapon;
use crate::game_logic::host_gattling_tank::{GattlingFireLevel, GATTLING_CHAIN_GUN_DAMAGE_MULT};
use std::collections::HashSet;

/// Retail Patriot primary weapon template name.
pub const PATRIOT_PRIMARY_WEAPON: &str = "PatriotMissileWeapon";

/// Retail China Gattling Cannon primary weapon template name.
pub const GATTLING_BUILDING_PRIMARY_WEAPON: &str = "GattlingBuildingGun";
/// Retail China Gattling Cannon secondary AA weapon template name.
pub const GATTLING_BUILDING_SECONDARY_WEAPON: &str = "GattlingBuildingGunAir";

/// Retail GattlingBuildingGun PrimaryDamage.
pub const GATTLING_BUILDING_GROUND_DAMAGE: f32 = 10.0;
/// Retail GattlingBuildingGun AttackRange.
pub const GATTLING_BUILDING_GROUND_RANGE: f32 = 225.0;
/// Retail GattlingBuildingGunAir PrimaryDamage.
pub const GATTLING_BUILDING_AIR_DAMAGE: f32 = 5.0;
/// Retail GattlingBuildingGunAir AttackRange.
pub const GATTLING_BUILDING_AIR_RANGE: f32 = 400.0;

/// Retail DelayBetweenShots 250ms → 8 frames @ 30 FPS.
pub const GATTLING_BUILDING_BASE_DELAY_FRAMES: u32 = 8;
/// ContinuousFireOne for building gun (retail = 1).
pub const GATTLING_BUILDING_CONTINUOUS_FIRE_ONE: u32 = 1;
/// ContinuousFireTwo for building gun (retail = 5).
pub const GATTLING_BUILDING_CONTINUOUS_FIRE_TWO: u32 = 5;
/// ContinuousFireCoast 2000ms → 60 frames @ 30 FPS.
pub const GATTLING_BUILDING_COAST_FRAMES: u32 = 60;

/// Residual fire audio for structure gattling.
pub const GATTLING_BUILDING_FIRE_AUDIO: &str = "GattlingCannonWeapon";
/// Retail VoiceRapidFire residual cue when entering FAST.
pub const GATTLING_BUILDING_RAPID_FIRE_AUDIO: &str = "GattlingCannonVoiceRapid";

/// Whether template is a residual base-defense structure that should auto-fire.
///
/// Fail-closed: name + FSBaseDefense kind residual (not full INI module matrix).
/// Excludes Overlord/Helix/tank-mounted gattling payloads (not structures).
pub fn is_base_defense_structure(
    template_name: &str,
    is_structure: bool,
    is_fs_base_defense: bool,
) -> bool {
    if is_fs_base_defense {
        return true;
    }
    if !is_structure {
        return false;
    }
    let n = template_name.to_ascii_lowercase();
    // Vehicle/portable gattling payloads are not structure base defenses.
    if n.contains("overlord") || n.contains("helix") || n.contains("tank") || n.contains("gunship")
    {
        return false;
    }
    n.contains("patriot")
        || n.contains("gattlingcannon")
        || n.contains("gattling_cannon")
        || n.contains("stingersite")
        || n.contains("stinger_site")
        || n.contains("basedefense")
        || n.contains("base_defense")
        || n.contains("firebase")
}

/// Whether template is a residual China Gattling Cannon structure (ramp + AA).
///
/// Fail-closed: name residual. Excludes tank / Overlord / Helix payloads.
pub fn is_gattling_cannon_structure(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Weapons / upgrades / science / debris.
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
    // Portable Overlord/Helix payloads are not the structure residual.
    if n.contains("overlord") || n.contains("helix") {
        return false;
    }
    // Vehicle gattling tanks (ChinaTankGattling / *Vehicle*Gattling*) are host_gattling_tank.
    // General-variant buildings keep a `Tank_` / `Nuke_` prefix and still match *GattlingCannon*.
    if (n.contains("gattlingtank") || n.contains("gatlingtank") || n.contains("tankgattling"))
        && !n.contains("cannon")
    {
        return false;
    }
    if n.contains("vehiclegattling") || n.contains("vehiclegatling") {
        return false;
    }
    n.contains("gattlingcannon")
        || n.contains("gatlingcannon")
        || n.contains("gattling_cannon")
        || n.contains("gatling_cannon")
        || n == "china_gattlingcannon"
        || n == "testgattlingcannon"
        || n == "testgatlingcannon"
}

/// Retail-ish residual weapon name for known host base-defense templates.
pub fn primary_weapon_name_for_defense(template_name: &str) -> Option<&'static str> {
    let n = template_name.to_ascii_lowercase();
    if n.contains("patriot") {
        Some(PATRIOT_PRIMARY_WEAPON)
    } else if n.contains("gattling") || n.contains("gatling") {
        Some(GATTLING_BUILDING_PRIMARY_WEAPON)
    } else {
        None
    }
}

/// Secondary AA residual weapon name for structure gattling.
pub fn secondary_weapon_name_for_defense(template_name: &str) -> Option<&'static str> {
    if is_gattling_cannon_structure(template_name) {
        Some(GATTLING_BUILDING_SECONDARY_WEAPON)
    } else {
        None
    }
}

/// Legal residual target for base-defense auto-fire.
pub fn is_legal_base_defense_target(
    is_alive: bool,
    same_team: bool,
    is_neutral: bool,
    under_construction: bool,
    is_attackable_or_combat_kind: bool,
) -> bool {
    is_alive
        && !same_team
        && !is_neutral
        && !under_construction
        && is_attackable_or_combat_kind
}

/// Slot residual for structure gattling: 1 = AA secondary, 0 = ground primary.
pub fn preferred_gattling_building_slot(target_is_air: bool) -> u8 {
    if target_is_air {
        1
    } else {
        0
    }
}

/// Delay frames residual for continuous-fire level (base / ROF).
///
/// C++ uses floor(delay / ROF). Residual:
/// - Base: 8
/// - Mean: floor(8/2)=4
/// - Fast: floor(8/3)=2
pub fn gattling_building_delay_frames_for_level(level: GattlingFireLevel) -> u32 {
    let base = GATTLING_BUILDING_BASE_DELAY_FRAMES as f32;
    let rof = level.rof_multiplier();
    (base / rof).floor().max(1.0) as u32
}

/// Apply Chain Guns residual damage mult when upgrade present.
pub fn gattling_building_damage_with_chain_guns(base_damage: f32, has_chain_guns: bool) -> f32 {
    if has_chain_guns {
        base_damage * GATTLING_CHAIN_GUN_DAMAGE_MULT
    } else {
        base_damage
    }
}

fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Ground gun residual stats (damage, range, delay_frames) for level + chain guns.
pub fn gattling_building_ground_stats(
    level: GattlingFireLevel,
    has_chain_guns: bool,
) -> (f32, f32, u32) {
    let dmg =
        gattling_building_damage_with_chain_guns(GATTLING_BUILDING_GROUND_DAMAGE, has_chain_guns);
    (
        dmg,
        GATTLING_BUILDING_GROUND_RANGE,
        gattling_building_delay_frames_for_level(level),
    )
}

/// Air gun residual stats (damage, range, delay_frames) for level + chain guns.
pub fn gattling_building_air_stats(
    level: GattlingFireLevel,
    has_chain_guns: bool,
) -> (f32, f32, u32) {
    let dmg = gattling_building_damage_with_chain_guns(GATTLING_BUILDING_AIR_DAMAGE, has_chain_guns);
    (
        dmg,
        GATTLING_BUILDING_AIR_RANGE,
        gattling_building_delay_frames_for_level(level),
    )
}

/// Build residual ground Weapon for level + chain guns.
pub fn gattling_building_ground_weapon(level: GattlingFireLevel, has_chain_guns: bool) -> Weapon {
    let (dmg, range, delay) = gattling_building_ground_stats(level, has_chain_guns);
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

/// Build residual air Weapon for level + chain guns.
pub fn gattling_building_air_weapon(level: GattlingFireLevel, has_chain_guns: bool) -> Weapon {
    let (dmg, range, delay) = gattling_building_air_stats(level, has_chain_guns);
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

/// Advance continuous-fire residual state after a structure gattling shot.
///
/// Mirrors C++ `FiringTracker::shotFired` with building thresholds
/// ContinuousFireOne=1 / ContinuousFireTwo=5.
/// Returns `(new_level, consecutive, entered_fast)`.
pub fn gattling_building_on_shot_fired(
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
            if consecutive < GATTLING_BUILDING_CONTINUOUS_FIRE_ONE {
                level = GattlingFireLevel::Base;
            } else if consecutive > GATTLING_BUILDING_CONTINUOUS_FIRE_TWO {
                level = GattlingFireLevel::Fast;
                entered_fast = true;
            }
        }
        GattlingFireLevel::Fast => {
            if consecutive < GATTLING_BUILDING_CONTINUOUS_FIRE_TWO {
                // C++ coolDown: straight to zero from FAST.
                level = GattlingFireLevel::Base;
            }
        }
        GattlingFireLevel::Base => {
            if consecutive > GATTLING_BUILDING_CONTINUOUS_FIRE_ONE {
                level = GattlingFireLevel::Mean;
            }
        }
    }

    (level, consecutive, entered_fast)
}

/// Next coast-until frame after a shot (next possible shot frame + coast residual).
pub fn gattling_building_coast_until_after_shot(
    current_frame: u32,
    level: GattlingFireLevel,
) -> u32 {
    let delay = gattling_building_delay_frames_for_level(level);
    current_frame
        .saturating_add(delay)
        .saturating_add(GATTLING_BUILDING_COAST_FRAMES)
}

/// Whether Chain Guns upgrade is active on a structure gattling residual host.
pub fn gattling_building_has_chain_guns(applied_upgrades: &HashSet<String>) -> bool {
    crate::game_logic::host_gattling_tank::has_chain_guns_upgrade(applied_upgrades)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn base_defense_name_matrix() {
        assert!(is_base_defense_structure("USA_Patriot", true, false));
        assert!(is_base_defense_structure("AmericaPatriotBattery", true, false));
        assert!(is_base_defense_structure("Lazr_PatriotMissileSystem", true, false));
        assert!(is_base_defense_structure("China_GattlingCannon", true, false));
        assert!(is_base_defense_structure("ChinaGattlingCannon", true, false));
        assert!(is_base_defense_structure("GLA_StingerSite", true, false));
        assert!(is_base_defense_structure("AnyTower", true, true));
        assert!(!is_base_defense_structure("USA_Barracks", true, false));
        assert!(!is_base_defense_structure("USA_Ranger", false, false));
        assert!(!is_base_defense_structure(
            "ChinaTankOverlordGattlingCannon",
            false,
            false
        ));
        assert!(!is_base_defense_structure(
            "ChinaHelixGattlingCannon",
            false,
            false
        ));
        assert!(!is_base_defense_structure("USA_SupplyCenter", true, false));
    }

    #[test]
    fn gattling_cannon_structure_name_matrix() {
        assert!(is_gattling_cannon_structure("China_GattlingCannon"));
        assert!(is_gattling_cannon_structure("ChinaGattlingCannon"));
        assert!(is_gattling_cannon_structure("Nuke_ChinaGattlingCannon"));
        assert!(is_gattling_cannon_structure("Tank_ChinaGattlingCannon"));
        assert!(is_gattling_cannon_structure("Infa_ChinaGattlingCannon"));
        assert!(is_gattling_cannon_structure("TestGattlingCannon"));
        // Tank residual — not structure.
        assert!(!is_gattling_cannon_structure("ChinaTankGattling"));
        assert!(!is_gattling_cannon_structure("ChinaVehicleGattlingTank"));
        // Overlord / Helix payload — not structure residual.
        assert!(!is_gattling_cannon_structure("ChinaTankOverlordGattlingCannon"));
        assert!(!is_gattling_cannon_structure("ChinaHelixGattlingCannon"));
        // Weapons / upgrades.
        assert!(!is_gattling_cannon_structure("GattlingBuildingGun"));
        assert!(!is_gattling_cannon_structure("GattlingBuildingGunAir"));
        assert!(!is_gattling_cannon_structure("Upgrade_ChinaChainGuns"));
        assert!(!is_gattling_cannon_structure("USA_Patriot"));
    }

    #[test]
    fn defense_weapon_name_lookup() {
        assert_eq!(
            primary_weapon_name_for_defense("USA_Patriot"),
            Some(PATRIOT_PRIMARY_WEAPON)
        );
        assert_eq!(
            primary_weapon_name_for_defense("China_GattlingCannon"),
            Some(GATTLING_BUILDING_PRIMARY_WEAPON)
        );
        assert_eq!(
            secondary_weapon_name_for_defense("China_GattlingCannon"),
            Some(GATTLING_BUILDING_SECONDARY_WEAPON)
        );
        assert_eq!(secondary_weapon_name_for_defense("USA_Patriot"), None);
        assert_eq!(primary_weapon_name_for_defense("GLA_StingerSite"), None);
        assert_eq!(primary_weapon_name_for_defense("USA_Ranger"), None);
    }

    #[test]
    fn legal_target_matrix() {
        assert!(is_legal_base_defense_target(true, false, false, false, true));
        assert!(!is_legal_base_defense_target(false, false, false, false, true));
        assert!(!is_legal_base_defense_target(true, true, false, false, true));
        assert!(!is_legal_base_defense_target(true, false, true, false, true));
        assert!(!is_legal_base_defense_target(true, false, false, true, true));
        assert!(!is_legal_base_defense_target(true, false, false, false, false));
    }

    #[test]
    fn continuous_fire_ramp_thresholds_building() {
        // Shot 1 → consecutive 1, stay Base (need > 1).
        let (l1, c1, f1) =
            gattling_building_on_shot_fired(GattlingFireLevel::Base, 0, None, Some(10), 0, 0);
        assert_eq!(l1, GattlingFireLevel::Base);
        assert_eq!(c1, 1);
        assert!(!f1);

        // Shot 2 → consecutive 2 > 1 → Mean.
        let (l2, c2, f2) =
            gattling_building_on_shot_fired(l1, c1, Some(10), Some(10), 8, 100);
        assert_eq!(l2, GattlingFireLevel::Mean);
        assert_eq!(c2, 2);
        assert!(!f2);

        // Continue to shot 6 → Fast (consecutive 6 > 5).
        let mut level = l2;
        let mut consec = c2;
        for shot in 3..=6 {
            let (nl, nc, entered) = gattling_building_on_shot_fired(
                level,
                consec,
                Some(10),
                Some(10),
                shot * 4,
                1000,
            );
            level = nl;
            consec = nc;
            if shot == 6 {
                assert_eq!(level, GattlingFireLevel::Fast);
                assert!(entered || level == GattlingFireLevel::Fast);
            }
        }
        assert_eq!(level, GattlingFireLevel::Fast);
        assert_eq!(consec, 6);
    }

    #[test]
    fn delay_and_chain_guns_math() {
        assert_eq!(
            gattling_building_delay_frames_for_level(GattlingFireLevel::Base),
            8
        );
        assert_eq!(
            gattling_building_delay_frames_for_level(GattlingFireLevel::Mean),
            4
        );
        assert_eq!(
            gattling_building_delay_frames_for_level(GattlingFireLevel::Fast),
            2
        );

        let ground = gattling_building_ground_weapon(GattlingFireLevel::Base, false);
        assert!((ground.damage - 10.0).abs() < 0.01);
        assert!((ground.range - 225.0).abs() < 0.01);
        assert!(!ground.can_target_air);
        assert!(ground.can_target_ground);

        let air = gattling_building_air_weapon(GattlingFireLevel::Base, false);
        assert!((air.damage - 5.0).abs() < 0.01);
        assert!((air.range - 400.0).abs() < 0.01);
        assert!(air.can_target_air);
        assert!(!air.can_target_ground);

        let chained = gattling_building_ground_weapon(GattlingFireLevel::Base, true);
        assert!((chained.damage - 12.5).abs() < 0.01);

        let mut tags = HashSet::new();
        assert!(!gattling_building_has_chain_guns(&tags));
        tags.insert("Upgrade_ChinaChainGuns".to_string());
        assert!(gattling_building_has_chain_guns(&tags));

        assert_eq!(preferred_gattling_building_slot(false), 0);
        assert_eq!(preferred_gattling_building_slot(true), 1);
    }
}
