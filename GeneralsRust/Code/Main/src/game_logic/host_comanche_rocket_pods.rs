//! Host America Comanche combat residual (20mm + anti-tank + rocket pods).
//!
//! Residual slice (playability):
//! - PRIMARY `Comanche20mmCannonWeapon` residual: PrimaryDamage **6** / radius **0**
//!   (intended-only), range **200**, Delay **100**ms → 3 frames. AntiAirborneInfantry
//!   residual (can_target_air honesty for infantry AA residual).
//! - SECONDARY `ComancheAntiTankMissileWeapon` residual at spawn:
//!   Primary **50**/r**5** + Secondary **30**/r**25**, range **200**,
//!   Delay **500**ms → 15 frames, ClipSize **4** honesty (ClipReload 15000ms fail-closed).
//! - `Upgrade_ComancheRocketPods` research replaces residual SECONDARY with
//!   `ComancheRocketPodWeapon` (retail WeaponSet TERTIARY + WeaponSetUpgrade):
//!   Primary **30**/r**5** + Secondary **10**/r**40**, ClipSize **20**, Delay **200**ms.
//!   Host residual collapses TERTIARY into secondary slot (fail-closed vs full 3-slot).
//! - Retail command button `Command_AmericaVehicleComancheFireRocketPods` is
//!   FIRE_WEAPON at position (NEED_TARGET_POS); host residual uses secondary
//!   slot + AttackingGround / active_weapon_slot lock.
//!
//! Fail-closed honesty:
//! - Not full WeaponSet PRIMARY/SECONDARY/TERTIARY chooser matrix (host only
//!   carries primary + secondary; rocket pods occupy secondary residual when upgraded)
//! - Not full ScatterTarget clip pattern / 20-rocket volley spacing
//! - Not full JetAIUpdate turret move-and-fire matrix
//! - Not dual-volley antitank ClipSize cadence matrix
//! - Not network upgrade / clip replication (network deferred)

use super::Weapon;

/// Retail Upgrade_ComancheRocketPods name.
pub const UPGRADE_COMANCHE_ROCKET_PODS: &str = "Upgrade_ComancheRocketPods";

/// Retail ComancheRocketPodWeapon template name (TERTIARY after upgrade).
pub const COMANCHE_ROCKET_POD_WEAPON: &str = "ComancheRocketPodWeapon";

/// Retail Comanche20mmCannonWeapon primary residual.
pub const COMANCHE_PRIMARY_WEAPON: &str = "Comanche20mmCannonWeapon";

/// Retail ComancheAntiTankMissileWeapon residual secondary (until rocket pods unlock).
pub const COMANCHE_ANTITANK_WEAPON: &str = "ComancheAntiTankMissileWeapon";

// --- Primary 20mm residual ---
/// Retail Comanche20mmCannonWeapon PrimaryDamage.
pub const COMANCHE_CANNON_DAMAGE: f32 = 6.0;
/// Retail AttackRange.
pub const COMANCHE_CANNON_RANGE: f32 = 200.0;
/// Retail DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const COMANCHE_CANNON_DELAY_FRAMES: u32 = 3;
/// Residual fire audio.
pub const COMANCHE_CANNON_FIRE_AUDIO: &str = "Comanche20mmCannonWeapon";

// --- Anti-tank missile residual ---
/// Retail ComancheAntiTankMissileWeapon PrimaryDamage.
pub const COMANCHE_AT_PRIMARY_DAMAGE: f32 = 50.0;
/// Retail PrimaryDamageRadius.
pub const COMANCHE_AT_PRIMARY_RADIUS: f32 = 5.0;
/// Retail SecondaryDamage.
pub const COMANCHE_AT_SECONDARY_DAMAGE: f32 = 30.0;
/// Retail SecondaryDamageRadius.
pub const COMANCHE_AT_SECONDARY_RADIUS: f32 = 25.0;
/// Retail AttackRange.
pub const COMANCHE_AT_RANGE: f32 = 200.0;
/// Retail DelayBetweenShots 500ms → 15 frames @ 30 FPS.
pub const COMANCHE_AT_DELAY_FRAMES: u32 = 15;
/// Retail ClipSize honesty.
pub const COMANCHE_AT_CLIP_SIZE: u32 = 4;
/// Retail ClipReloadTime 15000ms → 450 frames honesty residual.
pub const COMANCHE_AT_CLIP_RELOAD_FRAMES: u32 = 450;
/// Residual fire audio.
pub const COMANCHE_AT_FIRE_AUDIO: &str = "ComancheAntiTankMissileWeapon";

// --- Rocket pod residual ---
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

/// Whether template is a residual Comanche that receives rocket pods / combat residual.
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

/// Whether residual fire should apply Comanche combat residual (any slot).
pub fn should_apply_comanche_residual(is_comanche: bool) -> bool {
    is_comanche
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

/// Whether combat should apply anti-tank dual-radius residual on secondary fire.
///
/// Active when secondary slot fires and rocket pods are **not** equipped (or not
/// the weapon occupying the secondary residual slot).
pub fn should_apply_comanche_antitank_residual(
    is_comanche: bool,
    fired_slot: u8,
    rocket_pods_active: bool,
) -> bool {
    is_comanche && fired_slot == 1 && !rocket_pods_active
}

/// Whether combat should apply primary 20mm intended residual.
pub fn should_apply_comanche_cannon_residual(is_comanche: bool, fired_slot: u8) -> bool {
    is_comanche && fired_slot == 0
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

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Comanche primary 20mm Weapon.
pub fn comanche_cannon_weapon() -> Weapon {
    Weapon {
        damage: COMANCHE_CANNON_DAMAGE,
        range: COMANCHE_CANNON_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(COMANCHE_CANNON_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        // Retail AntiAirborneInfantry = Yes (infantry AA residual honesty).
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Build residual Comanche anti-tank secondary Weapon.
pub fn comanche_antitank_weapon() -> Weapon {
    Weapon {
        damage: COMANCHE_AT_PRIMARY_DAMAGE,
        range: COMANCHE_AT_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(COMANCHE_AT_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(COMANCHE_AT_CLIP_SIZE),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 99999.0,
        pre_attack_delay: 0.0,
    }
}

/// Build residual Comanche rocket-pod secondary Weapon (after upgrade).
pub fn comanche_rocket_pod_weapon() -> Weapon {
    Weapon {
        damage: ROCKET_POD_PRIMARY_DAMAGE,
        range: ROCKET_POD_ATTACK_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(ROCKET_POD_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(20),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 99999.0,
        pre_attack_delay: 0.0,
    }
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

/// Dual-radius residual damage for anti-tank missiles.
pub fn comanche_antitank_damage_at(distance_from_impact: f32) -> f32 {
    if distance_from_impact <= COMANCHE_AT_PRIMARY_RADIUS {
        COMANCHE_AT_PRIMARY_DAMAGE
    } else if distance_from_impact <= COMANCHE_AT_SECONDARY_RADIUS {
        COMANCHE_AT_SECONDARY_DAMAGE
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

/// Alias for anti-tank / cannon residual legality (same residual gates).
pub fn is_legal_comanche_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_legal_rocket_pod_splash_target(is_alive, is_self, under_construction, is_combat_kind)
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
    fn cannon_and_antitank_slot_gates() {
        assert!(should_apply_comanche_cannon_residual(true, 0));
        assert!(!should_apply_comanche_cannon_residual(true, 1));
        assert!(should_apply_comanche_antitank_residual(true, 1, false));
        assert!(!should_apply_comanche_antitank_residual(true, 1, true));
        assert!(!should_apply_comanche_antitank_residual(true, 0, false));
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

        assert!((comanche_antitank_damage_at(0.0) - 50.0).abs() < 0.01);
        assert!((comanche_antitank_damage_at(5.0) - 50.0).abs() < 0.01);
        assert!((comanche_antitank_damage_at(15.0) - 30.0).abs() < 0.01);
        assert!((comanche_antitank_damage_at(26.0)).abs() < 0.01);
    }

    #[test]
    fn weapon_builders() {
        let c = comanche_cannon_weapon();
        assert!((c.damage - 6.0).abs() < 0.01);
        assert!((c.range - 200.0).abs() < 0.01);
        assert!((c.reload_time - 3.0 / 30.0).abs() < 0.01);
        assert!(c.can_target_air);

        let at = comanche_antitank_weapon();
        assert!((at.damage - 50.0).abs() < 0.01);
        assert_eq!(at.ammo, Some(4));
        assert!(!at.can_target_air);

        let pods = comanche_rocket_pod_weapon();
        assert!((pods.damage - 30.0).abs() < 0.01);
        assert_eq!(pods.ammo, Some(20));
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
