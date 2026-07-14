//! Host China Helix PRIMARY minigun residual (HelixMinigunWeapon).
//!
//! Residual slice (playability):
//! - `ChinaVehicleHelix` / general variants spawn with PRIMARY
//!   `HelixMinigunWeapon`: PrimaryDamage **6** / radius **0** (intended-only),
//!   range **115**, Delay **100**ms → 3 frames.
//! - AntiAirborneInfantry residual honesty (`can_target_air = true` for
//!   airborne infantry residual; AntiAirborneVehicle = No).
//! - Minigun remains PRIMARY even when gattling/propaganda/bunker addons install
//!   (retail keeps HelixMinigun always — portable gattling is separate residual).
//!
//! Wave 63 residual pack (retail INI honesty):
//! - Weapon residual: PrimaryDamage **6**, radius **0**, AttackRange **115**,
//!   Delay **100**ms → **3**f, DamageType **COMANCHE_VULCAN**, DeathType **NORMAL**,
//!   ClipSize **0**, ClipReload **0**, FireFX **WeaponFX_Comanche20mmCannonFire**,
//!   AntiAirborneVehicle **No** / AntiAirborneInfantry **Yes**.
//! - Body residual: MaxHealth **300**, Vision **200**, Shroud **600**,
//!   BuildCost **1500**, BuildTime **20**s → **600**f.
//!
//! Fail-closed honesty:
//! - Not full ChinookAIUpdate rotor wash / AutoAcquire idle matrix beyond host AI
//! - Not full COMANCHE_VULCAN damage-type Stinger-site soldier-preserve matrix
//! - Not full Helix gattling addon dual-stream simultaneous fire matrix
//! - Not network Helix minigun replication (network deferred)

use super::Weapon;

/// Logic frames per second (host fixed step).
pub const HELIX_LOGIC_FPS: f32 = 30.0;

/// Retail HelixMinigunWeapon template name.
pub const HELIX_MINIGUN_WEAPON: &str = "HelixMinigunWeapon";

/// Retail HelixMinigunWeapon PrimaryDamage.
pub const HELIX_MINIGUN_DAMAGE: f32 = 6.0;
/// Retail PrimaryDamageRadius residual (0 = intended-only).
pub const HELIX_MINIGUN_PRIMARY_RADIUS: f32 = 0.0;
/// Retail AttackRange.
pub const HELIX_MINIGUN_RANGE: f32 = 115.0;
/// Retail DelayBetweenShots residual (msec).
pub const HELIX_MINIGUN_DELAY_MS: u32 = 100;
/// Retail DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const HELIX_MINIGUN_DELAY_FRAMES: u32 = 3;
/// Retail DamageType residual.
pub const HELIX_MINIGUN_DAMAGE_TYPE: &str = "COMANCHE_VULCAN";
/// Retail DeathType residual.
pub const HELIX_MINIGUN_DEATH_TYPE: &str = "NORMAL";
/// Retail ClipSize residual (0 == infinite).
pub const HELIX_MINIGUN_CLIP_SIZE: u32 = 0;
/// Retail ClipReloadTime residual (msec).
pub const HELIX_MINIGUN_CLIP_RELOAD_MS: u32 = 0;
/// Retail FireFX residual.
pub const HELIX_MINIGUN_FIRE_FX: &str = "WeaponFX_Comanche20mmCannonFire";
/// Retail AntiAirborneVehicle residual.
pub const HELIX_MINIGUN_ANTI_AIRBORNE_VEHICLE: bool = false;
/// Retail AntiAirborneInfantry residual.
pub const HELIX_MINIGUN_ANTI_AIRBORNE_INFANTRY: bool = true;
/// Residual fire audio.
pub const HELIX_MINIGUN_FIRE_AUDIO: &str = "HelixWeaponMachineGun";

// --- Body residual (ChinaVehicleHelix) ---

/// Retail MaxHealth residual.
pub const HELIX_MAX_HEALTH: f32 = 300.0;
/// Retail VisionRange residual.
pub const HELIX_VISION_RANGE: f32 = 200.0;
/// Retail ShroudClearingRange residual.
pub const HELIX_SHROUD_CLEARING_RANGE: f32 = 600.0;
/// Retail BuildCost residual.
pub const HELIX_BUILD_COST: u32 = 1_500;
/// Retail BuildTime residual (seconds).
pub const HELIX_BUILD_TIME_SEC: f32 = 20.0;
/// Retail BuildTime → frames @ 30 FPS.
pub const HELIX_BUILD_TIME_FRAMES: u32 = 600;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn helix_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * HELIX_LOGIC_FPS / 1000.0).round() as u32
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Whether residual fire should apply Helix minigun intended-only residual.
///
/// Slot 0 = primary minigun (addons use separate residual paths).
pub fn should_apply_helix_minigun_residual(is_helix: bool, fired_slot: u8) -> bool {
    is_helix && fired_slot == 0
}

/// Build residual Helix PRIMARY minigun Weapon.
pub fn helix_minigun_weapon() -> Weapon {
    Weapon {
        damage: HELIX_MINIGUN_DAMAGE,
        range: HELIX_MINIGUN_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(HELIX_MINIGUN_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        // AntiAirborneInfantry = Yes, AntiAirborneVehicle = No residual honesty.
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Legal residual minigun target.
pub fn is_legal_helix_minigun_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

// --- Wave 63 residual honesty packs ---

/// Wave 63 residual honesty: Helix minigun weapon residual peel.
pub fn honesty_helix_minigun_weapon_residual_ok() -> bool {
    HELIX_MINIGUN_WEAPON == "HelixMinigunWeapon"
        && (HELIX_MINIGUN_DAMAGE - 6.0).abs() < 0.01
        && (HELIX_MINIGUN_PRIMARY_RADIUS - 0.0).abs() < 0.01
        && (HELIX_MINIGUN_RANGE - 115.0).abs() < 0.01
        && HELIX_MINIGUN_DELAY_MS == 100
        && HELIX_MINIGUN_DELAY_FRAMES == helix_ms_to_frames(HELIX_MINIGUN_DELAY_MS)
        && HELIX_MINIGUN_DELAY_FRAMES == 3
        && HELIX_MINIGUN_DAMAGE_TYPE == "COMANCHE_VULCAN"
        && HELIX_MINIGUN_DEATH_TYPE == "NORMAL"
        && HELIX_MINIGUN_CLIP_SIZE == 0
        && HELIX_MINIGUN_CLIP_RELOAD_MS == 0
        && HELIX_MINIGUN_FIRE_FX == "WeaponFX_Comanche20mmCannonFire"
        && HELIX_MINIGUN_FIRE_AUDIO == "HelixWeaponMachineGun"
        && !HELIX_MINIGUN_ANTI_AIRBORNE_VEHICLE
        && HELIX_MINIGUN_ANTI_AIRBORNE_INFANTRY
        && {
            let w = helix_minigun_weapon();
            (w.damage - 6.0).abs() < 0.01
                && (w.range - 115.0).abs() < 0.01
                && w.can_target_air
                && w.can_target_ground
                && (w.reload_time - (3.0 / 30.0)).abs() < 0.001
        }
}

/// Wave 63 residual honesty: Helix chassis body residual peel.
pub fn honesty_helix_minigun_body_residual_ok() -> bool {
    (HELIX_MAX_HEALTH - 300.0).abs() < 0.01
        && (HELIX_VISION_RANGE - 200.0).abs() < 0.01
        && (HELIX_SHROUD_CLEARING_RANGE - 600.0).abs() < 0.01
        && HELIX_BUILD_COST == 1_500
        && (HELIX_BUILD_TIME_SEC - 20.0).abs() < 0.01
        && HELIX_BUILD_TIME_FRAMES == ((HELIX_BUILD_TIME_SEC * HELIX_LOGIC_FPS).round() as u32)
        && HELIX_BUILD_TIME_FRAMES == 600
        && should_apply_helix_minigun_residual(true, 0)
        && !should_apply_helix_minigun_residual(true, 1)
}

/// Combined Wave 63 Helix minigun residual honesty pack.
pub fn honesty_helix_minigun_residual_pack_ok() -> bool {
    honesty_helix_minigun_weapon_residual_ok() && honesty_helix_minigun_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_overlord_addons::is_helix_template;

    #[test]
    fn helix_template_matrix_for_minigun() {
        assert!(is_helix_template("ChinaVehicleHelix"));
        assert!(is_helix_template("China_Helix"));
        assert!(is_helix_template("Nuke_ChinaVehicleHelix"));
        assert!(is_helix_template("TestHelix"));
        assert!(!is_helix_template("HelixMinigunWeapon"));
        assert!(!is_helix_template("Upgrade_HelixNapalmBomb"));
        assert!(!is_helix_template("ChinaTankOverlord"));
    }

    #[test]
    fn minigun_weapon_stats() {
        let w = helix_minigun_weapon();
        assert!((w.damage - HELIX_MINIGUN_DAMAGE).abs() < 0.01);
        assert!((w.range - HELIX_MINIGUN_RANGE).abs() < 0.01);
        assert!((w.reload_time - (3.0 / 30.0)).abs() < 0.001);
        assert!(w.can_target_air);
        assert!(w.can_target_ground);
    }

    #[test]
    fn residual_slot_gate() {
        assert!(should_apply_helix_minigun_residual(true, 0));
        assert!(!should_apply_helix_minigun_residual(true, 1));
        assert!(!should_apply_helix_minigun_residual(false, 0));
    }

    #[test]
    fn legal_target_gate() {
        assert!(is_legal_helix_minigun_target(true, false, false, true));
        assert!(!is_legal_helix_minigun_target(false, false, false, true));
        assert!(!is_legal_helix_minigun_target(true, true, false, true));
        assert!(!is_legal_helix_minigun_target(true, false, true, true));
        assert!(!is_legal_helix_minigun_target(true, false, false, false));
    }

    #[test]
    fn helix_minigun_residual_pack_honesty_wave63() {
        assert!(honesty_helix_minigun_weapon_residual_ok());
        assert!(honesty_helix_minigun_body_residual_ok());
        assert!(honesty_helix_minigun_residual_pack_ok());
        assert_eq!(helix_ms_to_frames(100), 3);
        assert_eq!(helix_ms_to_frames(0), 0);
        assert_eq!(HELIX_BUILD_TIME_FRAMES, 600);
        assert_eq!(HELIX_MINIGUN_DAMAGE_TYPE, "COMANCHE_VULCAN");
        assert!(HELIX_MINIGUN_ANTI_AIRBORNE_INFANTRY);
        assert!(!HELIX_MINIGUN_ANTI_AIRBORNE_VEHICLE);
    }
}
