//! Host America Humvee residual polish (transport + TOW air tertiary).
//!
//! Residual slice (playability):
//! - AmericaVehicleHumvee TransportContain Slots=**5**,
//!   PassengersAllowedToFire=Yes, AllowInsideKindOf=INFANTRY.
//! - Upgrade_AmericaTOWMissile already equips ground TOW secondary (host_upgrades).
//! - TOW air tertiary residual: vs aircraft after TOW research, prefer
//!   `HumveeMissileWeaponAir` (dmg **50** / range **320** / air only).
//!
//! Fail-closed honesty:
//! - Not full WeaponSet PLAYER_UPGRADE visual turret swap
//! - Not full TransportAIUpdate multi-exit-path / GoAggressiveOnExit matrix
//! - Not Battle/Scout/Hellfire drone ObjectCreationUpgrade (see host_slave_drones)
//! - Not network TOW / transport replication (network deferred)

use super::Weapon;

/// Retail Humvee Missile air tertiary residual.
pub const HUMVEE_MISSILE_WEAPON_AIR: &str = "HumveeMissileWeaponAir";

/// Retail TransportContain Slots residual.
pub const HUMVEE_TRANSPORT_SLOTS: usize = 5;

/// Retail HumveeMissileWeaponAir PrimaryDamage residual.
pub const HUMVEE_AIR_TOW_DAMAGE: f32 = 50.0;
/// Retail HumveeMissileWeaponAir AttackRange residual.
pub const HUMVEE_AIR_TOW_RANGE: f32 = 320.0;
/// Retail DelayBetweenShots 1000ms + ClipReload 2000ms residual cycle ≈ 90 frames.
/// Fail-closed: use 30 frames (Delay) + 60 frames (reload) collapsed to 90.
pub const HUMVEE_AIR_TOW_DELAY_FRAMES: u32 = 90;

/// Whether template is a residual America Humvee.
pub fn is_humvee_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n == "testhumvee" || n == "usa_humvee" || n == "goldenhumvee" {
        return true;
    }
    if n.contains("weapon")
        || n.contains("missile")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.starts_with("upgrade")
    {
        return false;
    }
    n.contains("humvee") || n.contains("hummer")
}

/// Prefer air TOW residual when Humvee has TOW upgrade and target is aircraft.
pub fn humvee_prefer_air_tow(
    is_humvee: bool,
    has_tow_upgrade: bool,
    target_is_air: bool,
) -> bool {
    is_humvee && has_tow_upgrade && target_is_air
}

/// Build residual Humvee air TOW tertiary weapon (bound as secondary when air).
pub fn humvee_air_tow_weapon() -> Weapon {
    Weapon {
        damage: HUMVEE_AIR_TOW_DAMAGE,
        range: HUMVEE_AIR_TOW_RANGE,
        min_range: 0.0,
        reload_time: HUMVEE_AIR_TOW_DELAY_FRAMES as f32 / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: 600.0,
        pre_attack_delay: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humvee_name_matrix() {
        assert!(is_humvee_template("AmericaVehicleHumvee"));
        assert!(is_humvee_template("USA_Humvee"));
        assert!(is_humvee_template("TestHumvee"));
        assert!(!is_humvee_template("HumveeMissileWeapon"));
        assert!(!is_humvee_template("AmericaTankCrusader"));
    }

    #[test]
    fn air_tow_prefer_gate() {
        assert!(humvee_prefer_air_tow(true, true, true));
        assert!(!humvee_prefer_air_tow(true, false, true));
        assert!(!humvee_prefer_air_tow(true, true, false));
        assert!(!humvee_prefer_air_tow(false, true, true));
    }

    #[test]
    fn air_tow_stats() {
        let w = humvee_air_tow_weapon();
        assert!((w.damage - 50.0).abs() < 0.01);
        assert!((w.range - 320.0).abs() < 0.01);
        assert!(w.can_target_air && !w.can_target_ground);
    }

    #[test]
    fn transport_slots() {
        assert_eq!(HUMVEE_TRANSPORT_SLOTS, 5);
    }
}
