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
//! Fail-closed honesty:
//! - Not full ChinookAIUpdate rotor wash / AutoAcquire idle matrix beyond host AI
//! - Not full COMANCHE_VULCAN damage-type Stinger-site soldier-preserve matrix
//! - Not full Helix gattling addon dual-stream simultaneous fire matrix
//! - Not network Helix minigun replication (network deferred)

use super::Weapon;

/// Retail HelixMinigunWeapon template name.
pub const HELIX_MINIGUN_WEAPON: &str = "HelixMinigunWeapon";

/// Retail HelixMinigunWeapon PrimaryDamage.
pub const HELIX_MINIGUN_DAMAGE: f32 = 6.0;
/// Retail AttackRange.
pub const HELIX_MINIGUN_RANGE: f32 = 115.0;
/// Retail DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const HELIX_MINIGUN_DELAY_FRAMES: u32 = 3;
/// Residual fire audio.
pub const HELIX_MINIGUN_FIRE_AUDIO: &str = "HelixWeaponMachineGun";

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
}
