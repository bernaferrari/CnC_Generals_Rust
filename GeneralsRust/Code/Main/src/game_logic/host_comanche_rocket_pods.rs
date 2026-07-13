//! Host America Comanche Rocket Pods residual.
//!
//! Residual slice (playability):
//! - `Upgrade_ComancheRocketPods` research equips Comanche residual SECONDARY
//!   `ComancheRocketPodWeapon` (retail WeaponSet TERTIARY + WeaponSetUpgrade).
//! - Firing the rocket-pod slot (object attack or force-attack-ground) applies
//!   area damage residual matching PrimaryDamage/PrimaryDamageRadius and
//!   SecondaryDamage/SecondaryDamageRadius from retail Weapon.ini.
//! - Retail command button `Command_AmericaVehicleComancheFireRocketPods` is
//!   FIRE_WEAPON at position (NEED_TARGET_POS); host residual uses secondary
//!   slot + AttackingGround / active_weapon_slot lock.
//!
//! Fail-closed honesty:
//! - Not full WeaponSet PRIMARY/SECONDARY/TERTIARY chooser matrix (host only
//!   carries primary + secondary; rocket pods occupy secondary residual)
//! - Not full ScatterTarget clip pattern / 20-rocket volley spacing
//! - Not full JetAIUpdate turret move-and-fire matrix
//! - Not network upgrade / clip replication (network deferred)

/// Retail Upgrade_ComancheRocketPods name.
pub const UPGRADE_COMANCHE_ROCKET_PODS: &str = "Upgrade_ComancheRocketPods";

/// Retail ComancheRocketPodWeapon template name (TERTIARY after upgrade).
pub const COMANCHE_ROCKET_POD_WEAPON: &str = "ComancheRocketPodWeapon";

/// Retail Comanche20mmCannonWeapon primary residual.
pub const COMANCHE_PRIMARY_WEAPON: &str = "Comanche20mmCannonWeapon";

/// Retail ComancheAntiTankMissileWeapon residual (not host secondary — pods take slot).
pub const COMANCHE_ANTITANK_WEAPON: &str = "ComancheAntiTankMissileWeapon";

/// Retail ComancheRocketPodWeapon PrimaryDamage.
pub const ROCKET_POD_PRIMARY_DAMAGE: f32 = 30.0;
/// Retail ComancheRocketPodWeapon PrimaryDamageRadius.
pub const ROCKET_POD_PRIMARY_RADIUS: f32 = 5.0;
/// Retail ComancheRocketPodWeapon SecondaryDamage.
pub const ROCKET_POD_SECONDARY_DAMAGE: f32 = 10.0;
/// Retail ComancheRocketPodWeapon SecondaryDamageRadius.
pub const ROCKET_POD_SECONDARY_RADIUS: f32 = 40.0;
/// Retail ComancheRocketPodWeapon AttackRange.
pub const ROCKET_POD_ATTACK_RANGE: f32 = 200.0;
/// Retail DelayBetweenShots 200ms → 6 frames @ 30 FPS.
pub const ROCKET_POD_DELAY_FRAMES: u32 = 6;
/// Residual audio event name.
pub const ROCKET_POD_AUDIO: &str = "ComancheRocketPodWeaponSound";

/// Whether template is a residual Comanche that receives rocket pods.
///
/// Fail-closed: name residual (not full JetAIUpdate / helipad matrix).
pub fn is_comanche_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Projectile / shell / blade debris are not the aircraft.
    if n.contains("rocket") || n.contains("missile") || n.contains("blade") || n.contains("shell")
    {
        return false;
    }
    n.contains("comanche")
}

/// Whether combat should apply rocket-pod area residual instead of single-target HP.
///
/// Host residual: Comanche + upgrade tag + secondary slot (1).
/// Retail AutoChooseSources = NONE — only fires when player locks slot / FIRE_WEAPON.
pub fn should_apply_rocket_pod_area_attack(
    is_comanche: bool,
    has_upgrade: bool,
    fired_slot: u8,
) -> bool {
    is_comanche && has_upgrade && fired_slot == 1
}

/// Whether residual auto weapon-chooser may pick rocket-pod secondary.
/// Always false (retail AutoChooseSources TERTIARY NONE).
pub fn rocket_pods_auto_choose_allowed() -> bool {
    false
}

/// True when residual force-fire / FIRE_WEAPON ground path should use rocket pods.
///
/// active_weapon_slot == 1 locks residual tertiary fire (host secondary).
pub fn rocket_pod_ground_fire_active(
    is_comanche: bool,
    has_upgrade: bool,
    has_secondary: bool,
    active_weapon_slot: u8,
) -> bool {
    is_comanche && has_upgrade && has_secondary && active_weapon_slot == 1
}

/// 2D distance residual for splash rings.
pub fn in_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// Residual damage at distance from impact (primary core + secondary ring).
///
/// Retail: PrimaryDamage inside PrimaryDamageRadius; SecondaryDamage inside
/// SecondaryDamageRadius. Host residual: max of the two rings (no double-stack).
pub fn rocket_pod_damage_at_distance(distance: f32) -> f32 {
    if distance <= ROCKET_POD_PRIMARY_RADIUS {
        ROCKET_POD_PRIMARY_DAMAGE
    } else if distance <= ROCKET_POD_SECONDARY_RADIUS {
        ROCKET_POD_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual splash target (enemy/neutral combat kinds; allies residual-hit per
/// RadiusDamageAffects = ALLIES ENEMIES NEUTRALS — host residual hits non-self all teams).
pub fn is_legal_rocket_pod_splash_target(
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

    #[test]
    fn comanche_name_matrix() {
        assert!(is_comanche_template("AmericaVehicleComanche"));
        assert!(is_comanche_template("USA_Comanche"));
        assert!(is_comanche_template("AirF_AmericaVehicleComanche"));
        assert!(is_comanche_template("Lazr_AmericaVehicleComanche"));
        assert!(is_comanche_template("SupW_AmericaVehicleComanche"));
        assert!(is_comanche_template("TestComanche"));
        assert!(!is_comanche_template("USA_Ranger"));
        assert!(!is_comanche_template("ComancheRocketPodRocket"));
        assert!(!is_comanche_template("ComancheAntiTankMissile"));
        assert!(!is_comanche_template("ComancheBlades"));
    }

    #[test]
    fn should_apply_area_gate() {
        assert!(should_apply_rocket_pod_area_attack(true, true, 1));
        assert!(!should_apply_rocket_pod_area_attack(true, true, 0));
        assert!(!should_apply_rocket_pod_area_attack(true, false, 1));
        assert!(!should_apply_rocket_pod_area_attack(false, true, 1));
    }

    #[test]
    fn ground_fire_gate() {
        assert!(rocket_pod_ground_fire_active(true, true, true, 1));
        assert!(!rocket_pod_ground_fire_active(true, true, true, 0));
        assert!(!rocket_pod_ground_fire_active(true, true, false, 1));
        assert!(!rocket_pod_ground_fire_active(true, false, true, 1));
    }

    #[test]
    fn damage_falloff_rings() {
        assert!((rocket_pod_damage_at_distance(0.0) - ROCKET_POD_PRIMARY_DAMAGE).abs() < 0.01);
        assert!((rocket_pod_damage_at_distance(5.0) - ROCKET_POD_PRIMARY_DAMAGE).abs() < 0.01);
        assert!((rocket_pod_damage_at_distance(10.0) - ROCKET_POD_SECONDARY_DAMAGE).abs() < 0.01);
        assert!((rocket_pod_damage_at_distance(40.0) - ROCKET_POD_SECONDARY_DAMAGE).abs() < 0.01);
        assert!((rocket_pod_damage_at_distance(41.0)).abs() < 0.01);
    }

    #[test]
    fn splash_target_matrix() {
        assert!(is_legal_rocket_pod_splash_target(true, false, false, true));
        assert!(!is_legal_rocket_pod_splash_target(false, false, false, true));
        assert!(!is_legal_rocket_pod_splash_target(true, true, false, true));
        assert!(!is_legal_rocket_pod_splash_target(true, false, true, true));
        assert!(!is_legal_rocket_pod_splash_target(true, false, false, false));
    }

    #[test]
    fn radius_2d_check() {
        assert!(in_radius_2d((0.0, 0.0), (30.0, 0.0), 40.0));
        assert!(!in_radius_2d((0.0, 0.0), (50.0, 0.0), 40.0));
    }
}
