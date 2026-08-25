//! Host America Comanche combat residual (20mm + anti-tank + rocket pods).
//!
//! Residual slice (playability):
//! - PRIMARY `Comanche20mmCannonWeapon` residual: PrimaryDamage **6** / radius **0**
//!   (intended-only), range **200**, Delay **100**ms → 3 frames. AntiAirborneInfantry
//!   residual (can_target_air honesty for infantry AA residual).
//! - SECONDARY `ComancheAntiTankMissileWeapon` residual at spawn:
//!   Primary **50**/r**5** + Secondary **30**/r**25**, range **200**,
//!   Delay **500**ms → 15 frames, ClipSize **4** honesty (ClipReload 15000ms fail-closed).
//! - `Upgrade_ComancheRocketPods` research equips the real TERTIARY
//!   `ComancheRocketPodWeapon` while preserving the SECONDARY anti-tank missile:
//!   Primary **30**/r**5** + Secondary **10**/r**40**, ClipSize **20**, Delay **200**ms,
//!   ClipReloadTime **30000**ms → **900** frames, ScatterTargetScalar **50**,
//!   **20** ScatterTarget residual clip offsets.
//! - Retail command button `Command_AmericaVehicleComancheFireRocketPods` is
//!   FIRE_WEAPON at position (NEED_TARGET_POS); host residual uses tertiary
//!   slot + AttackingGround / active_weapon_slot lock.
//!
//! Fail-closed honesty:
//! - Not full WeaponSet condition/auto-choose matrix beyond explicit primary,
//!   secondary, and tertiary host slots
//! - ComancheRocketPodRocket projectile + ScatterTarget offset residual closed
//!   (deterministic clip ordinal; not full C++ GameLogicRandom shuffle)
//! - Not full JetAIUpdate turret move-and-fire matrix
//! - Not dual-volley antitank ClipSize cadence matrix
//! - Not network upgrade / clip replication (network deferred)

use super::Weapon;

/// Retail Upgrade_ComancheRocketPods name.
pub const UPGRADE_COMANCHE_ROCKET_PODS: &str = "Upgrade_ComancheRocketPods";

/// Retail ComancheRocketPodWeapon template name (TERTIARY after upgrade).
pub const COMANCHE_ROCKET_POD_WEAPON: &str = "ComancheRocketPodWeapon";
/// Retail ProjectileObject residual.
pub const COMANCHE_ROCKET_POD_PROJECTILE: &str = "ComancheRocketPodRocket";
/// Retail projectile flight residual frames @ 30 FPS (presentation Thing).
pub const COMANCHE_ROCKET_POD_PROJECTILE_LIFETIME_FRAMES: u32 = 12;
/// Retail projectile MaxHealth residual.
pub const COMANCHE_ROCKET_POD_PROJECTILE_MAX_HEALTH: f32 = 100.0;

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
/// Retail Comanche20mmCannonWeapon DamageType residual.
pub const COMANCHE_CANNON_DAMAGE_TYPE: &str = "COMANCHE_VULCAN";
/// Retail Comanche20mmCannonWeapon DeathType residual.
pub const COMANCHE_CANNON_DEATH_TYPE: &str = "NORMAL";

// --- Anti-tank missile residual ---
/// Retail ComancheAntiTankMissileWeapon PrimaryDamage.
pub const COMANCHE_AT_PRIMARY_DAMAGE: f32 = 50.0;
/// Retail PrimaryDamageRadius.
pub const COMANCHE_AT_PRIMARY_RADIUS: f32 = 5.0;
/// Retail ComancheAntiTankMissileWeapon ScatterRadiusVsInfantry residual.
pub const COMANCHE_AT_SCATTER_VS_INFANTRY: f32 = 10.0;
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
/// Retail ClipReloadTime 15000ms in seconds.
pub const COMANCHE_AT_CLIP_RELOAD_SECS: f32 = 15.0;
/// Residual fire audio.
pub const COMANCHE_AT_FIRE_AUDIO: &str = "ComancheAntiTankMissileWeapon";
/// Retail ComancheAntiTankMissileWeapon DamageType residual.
pub const COMANCHE_AT_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail ComancheAntiTankMissileWeapon DeathType residual.
pub const COMANCHE_AT_DEATH_TYPE: &str = "EXPLODED";

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
/// Retail DelayBetweenShots residual (msec).
pub const ROCKET_POD_DELAY_MS: u32 = 200;
/// Retail DelayBetweenShots 200ms → 6 frames @ 30 FPS.
pub const ROCKET_POD_DELAY_FRAMES: u32 = 6;
/// Retail ClipSize residual (20 rockets per volley).
pub const ROCKET_POD_CLIP_SIZE: u32 = 20;
/// Retail ClipReloadTime residual (msec).
pub const ROCKET_POD_CLIP_RELOAD_MS: u32 = 30_000;
/// ClipReloadTime 30000ms → 900 frames @ 30 FPS.
pub const ROCKET_POD_CLIP_RELOAD_FRAMES: u32 = 900;
/// Retail ClipReloadTime 30000ms in seconds.
pub const ROCKET_POD_CLIP_RELOAD_SECS: f32 = 30.0;
/// Retail ScatterTargetScalar residual (replaces ScatterRadius for table scale).
pub const ROCKET_POD_SCATTER_TARGET_SCALAR: f32 = 50.0;
/// Retail ScatterRadius residual (0; table drives offsets).
pub const ROCKET_POD_SCATTER_RADIUS: f32 = 0.0;
/// Residual audio event name.
pub const ROCKET_POD_AUDIO: &str = "ComancheRocketPodWeaponSound";
/// Retail ComancheRocketPodWeapon DamageType residual.
pub const ROCKET_POD_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail ComancheRocketPodWeapon DeathType residual.
pub const ROCKET_POD_DEATH_TYPE: &str = "EXPLODED";

/// Retail ComancheRocketPodWeapon ScatterTarget residual table (ClipSize **20**).
///
/// Offsets are unit-normalized; multiply by `ROCKET_POD_SCATTER_TARGET_SCALAR`
/// for world residual. Host residual: deterministic index into table by shot
/// ordinal (fail-closed vs full C++ clip shuffle).
pub const ROCKET_POD_SCATTER_TARGETS: &[(f32, f32)] = &[
    (0.000, 0.133),
    (0.133, -0.200),
    (-0.067, 0.667),
    (0.300, 0.300),
    (0.767, 0.000),
    (0.500, -0.567),
    (-0.333, -0.800),
    (-0.600, -0.1333),
    (-0.567, 0.433),
    (0.000, 0.133),
    (0.133, -0.200),
    (-0.067, 0.667),
    (0.300, 0.300),
    (0.767, 0.000),
    (0.500, -0.567),
    (-0.333, -0.800),
    (-0.600, -0.1333),
    (-0.567, 0.433),
    (0.000, 0.133),
    (0.133, -0.200),
];

/// ScatterTarget residual world offset for shot index within a clip.
pub fn rocket_pod_scatter_offset(shot_index: u32) -> (f32, f32) {
    if ROCKET_POD_SCATTER_TARGETS.is_empty() {
        return (0.0, 0.0);
    }
    let idx = (shot_index as usize) % ROCKET_POD_SCATTER_TARGETS.len();
    let (nx, ny) = ROCKET_POD_SCATTER_TARGETS[idx];
    (
        nx * ROCKET_POD_SCATTER_TARGET_SCALAR,
        ny * ROCKET_POD_SCATTER_TARGET_SCALAR,
    )
}

/// Aim point residual = intended impact + ScatterTargetScalar table offset (XZ).
pub fn rocket_pod_scatter_impact(
    impact_x: f32,
    impact_y: f32,
    impact_z: f32,
    shot_index: u32,
) -> (f32, f32, f32) {
    let (ox, oz) = rocket_pod_scatter_offset(shot_index);
    (impact_x + ox, impact_y, impact_z + oz)
}

/// Wave 49 residual honesty: clip size + scatter table + reload pack.
pub fn honesty_comanche_rocket_pod_clip_residual_ok() -> bool {
    COMANCHE_ROCKET_POD_PROJECTILE == "ComancheRocketPodRocket"
        && ROCKET_POD_SCATTER_TARGETS.len() as u32 == ROCKET_POD_CLIP_SIZE
        && ROCKET_POD_CLIP_SIZE == 20
        && ROCKET_POD_SCATTER_TARGETS.len() == ROCKET_POD_CLIP_SIZE as usize
        && (ROCKET_POD_SCATTER_TARGET_SCALAR - 50.0).abs() < 0.01
        && (ROCKET_POD_SCATTER_RADIUS - 0.0).abs() < 0.01
        && ROCKET_POD_CLIP_RELOAD_MS == 30_000
        && ROCKET_POD_CLIP_RELOAD_FRAMES == 900
        && ROCKET_POD_DELAY_MS == 200
        && ROCKET_POD_DELAY_FRAMES == 6
        && (ROCKET_POD_PRIMARY_DAMAGE - 30.0).abs() < 0.01
        && (ROCKET_POD_SECONDARY_DAMAGE - 10.0).abs() < 0.01
        && (ROCKET_POD_PRIMARY_RADIUS - 5.0).abs() < 0.01
        && (ROCKET_POD_SECONDARY_RADIUS - 40.0).abs() < 0.01
}
/// Combined residual honesty pack (Wave 71).
pub fn honesty_comanche_rocket_pods_residual_pack_ok() -> bool {
    honesty_comanche_rocket_pod_clip_residual_ok()
}

/// Whether template is a residual Comanche that receives rocket pods / combat residual.
///
/// Fail-closed: name residual (not full JetAIUpdate / helipad matrix).
pub fn is_comanche_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Projectile / shell / blade debris are not the aircraft.
    if n.contains("rocket") || n.contains("missile") || n.contains("blade") || n.contains("shell") {
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
/// Host residual: Comanche + upgrade tag + tertiary slot (2).
/// Retail AutoChooseSources = NONE — only fires when player locks slot / FIRE_WEAPON.
pub fn should_apply_rocket_pod_area_attack(
    is_comanche: bool,
    has_upgrade: bool,
    fired_slot: u8,
) -> bool {
    is_comanche && has_upgrade && fired_slot == 2
}

/// Whether combat should apply anti-tank dual-radius residual on secondary fire.
///
/// Active when the independent secondary slot fires.  Rocket pods no longer
/// displace this retail anti-tank weapon.
pub fn should_apply_comanche_antitank_residual(
    is_comanche: bool,
    fired_slot: u8,
    _rocket_pods_active: bool,
) -> bool {
    is_comanche && fired_slot == 1
}

/// Whether combat should apply primary 20mm intended residual.
pub fn should_apply_comanche_cannon_residual(is_comanche: bool, fired_slot: u8) -> bool {
    is_comanche && fired_slot == 0
}

/// Whether residual auto weapon-chooser may pick rocket-pod tertiary.
/// Always false (retail AutoChooseSources TERTIARY NONE).
pub fn rocket_pods_auto_choose_allowed() -> bool {
    false
}

/// True when residual force-fire / FIRE_WEAPON ground path should use rocket pods.
///
/// active_weapon_slot == 2 locks real tertiary fire.
pub fn rocket_pod_ground_fire_active(
    is_comanche: bool,
    has_upgrade: bool,
    has_tertiary: bool,
    active_weapon_slot: u8,
) -> bool {
    is_comanche && has_upgrade && has_tertiary && active_weapon_slot == 2
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
        clip_size: 0,
        clip_reload_time: 0.0,
        // Retail AntiAirborneInfantry = Yes (infantry AA residual honesty).
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
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
        clip_size: COMANCHE_AT_CLIP_SIZE,
        clip_reload_time: COMANCHE_AT_CLIP_RELOAD_SECS,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 99999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
    }
}

/// Build residual Comanche rocket-pod tertiary Weapon (after upgrade).
pub fn comanche_rocket_pod_weapon() -> Weapon {
    Weapon {
        damage: ROCKET_POD_PRIMARY_DAMAGE,
        range: ROCKET_POD_ATTACK_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(ROCKET_POD_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(ROCKET_POD_CLIP_SIZE),
        clip_size: ROCKET_POD_CLIP_SIZE,
        clip_reload_time: ROCKET_POD_CLIP_RELOAD_SECS,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 99999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
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

/// C++ ScatterRadiusVsInfantry residual on ComancheAntiTankMissileWeapon.
pub fn comanche_antitank_scatter_aim(
    aim: glam::Vec3,
    target_is_infantry: bool,
    seed: u32,
) -> (glam::Vec3, bool) {
    use crate::game_logic::weapon_bootstrap::{host_effective_scatter_radius, scatter_aim_offset};
    let mut scatter = host_effective_scatter_radius(COMANCHE_ANTITANK_WEAPON, target_is_infantry);
    if target_is_infantry && scatter <= 0.0 {
        scatter = COMANCHE_AT_SCATTER_VS_INFANTRY;
    }
    if scatter <= 0.0 {
        return (aim, false);
    }
    let off = scatter_aim_offset(seed, scatter);
    (glam::Vec3::new(aim.x + off.x, aim.y, aim.z + off.z), true)
}

/// Whether Comanche AT residual misses intended infantry via ScatterRadiusVsInfantry.
pub fn comanche_antitank_scatter_misses_infantry(
    target_is_infantry: bool,
    seed: u32,
    target_hit_radius: f32,
) -> bool {
    use crate::game_logic::weapon_bootstrap::scatter_misses_intended_target;
    if !target_is_infantry {
        return false;
    }
    scatter_misses_intended_target(COMANCHE_AT_SCATTER_VS_INFANTRY, seed, target_hit_radius)
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
        assert!(should_apply_rocket_pod_area_attack(true, true, 2));
        assert!(!should_apply_rocket_pod_area_attack(true, true, 1));
        assert!(!should_apply_rocket_pod_area_attack(true, true, 0));
        assert!(!should_apply_rocket_pod_area_attack(true, false, 2));
        assert!(!should_apply_rocket_pod_area_attack(false, true, 2));
    }

    #[test]
    fn cannon_and_antitank_slot_gates() {
        assert!(should_apply_comanche_cannon_residual(true, 0));
        assert!(!should_apply_comanche_cannon_residual(true, 1));
        assert!(should_apply_comanche_antitank_residual(true, 1, false));
        assert!(should_apply_comanche_antitank_residual(true, 1, true));
        assert!(!should_apply_comanche_antitank_residual(true, 0, false));
    }

    #[test]
    fn ground_fire_gate() {
        assert!(rocket_pod_ground_fire_active(true, true, true, 2));
        assert!(!rocket_pod_ground_fire_active(true, true, true, 1));
        assert!(!rocket_pod_ground_fire_active(true, true, false, 2));
        assert!(!rocket_pod_ground_fire_active(true, false, true, 2));
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
        assert_eq!(at.clip_size, COMANCHE_AT_CLIP_SIZE);
        assert!((at.clip_reload_time - COMANCHE_AT_CLIP_RELOAD_SECS).abs() < f32::EPSILON);
        assert!(!at.can_target_air);

        let pods = comanche_rocket_pod_weapon();
        assert!((pods.damage - 30.0).abs() < 0.01);
        assert_eq!(pods.ammo, Some(20));
        assert_eq!(pods.clip_size, ROCKET_POD_CLIP_SIZE);
        assert!((pods.clip_reload_time - ROCKET_POD_CLIP_RELOAD_SECS).abs() < f32::EPSILON);
    }

    #[test]
    fn splash_target_matrix() {
        assert!(is_legal_rocket_pod_splash_target(true, false, false, true));
        assert!(!is_legal_rocket_pod_splash_target(
            false, false, false, true
        ));
        assert!(!is_legal_rocket_pod_splash_target(true, true, false, true));
        assert!(!is_legal_rocket_pod_splash_target(true, false, true, true));
        assert!(!is_legal_rocket_pod_splash_target(
            true, false, false, false
        ));
    }

    #[test]
    fn radius_2d_check() {
        assert!(in_radius_2d((0.0, 0.0), (30.0, 0.0), 40.0));
        assert!(!in_radius_2d((0.0, 0.0), (50.0, 0.0), 40.0));
    }

    /// Wave 49: ClipSize **20** + ScatterTargetScalar **50** residual honesty.
    #[test]
    fn comanche_rocket_pod_scatter_clip_residual_honesty() {
        assert!(honesty_comanche_rocket_pod_clip_residual_ok());
        assert_eq!(ROCKET_POD_CLIP_SIZE, 20);
        assert_eq!(ROCKET_POD_SCATTER_TARGETS.len(), 20);
        assert_eq!(ROCKET_POD_CLIP_RELOAD_FRAMES, 900);
        let pods = comanche_rocket_pod_weapon();
        assert_eq!(pods.ammo, Some(ROCKET_POD_CLIP_SIZE));
        // Shot 0: (0, 0.133) * 50 = (0, 6.65)
        let (ox, oy) = rocket_pod_scatter_offset(0);
        assert!((ox - 0.0).abs() < 0.01);
        assert!((oy - 0.133 * 50.0).abs() < 0.01);
        // Shot 4: (0.767, 0) * 50
        let (ox4, oy4) = rocket_pod_scatter_offset(4);
        assert!((ox4 - 0.767 * 50.0).abs() < 0.01);
        assert!((oy4 - 0.0).abs() < 0.01);
        // Wraps at ClipSize.
        let a = rocket_pod_scatter_offset(0);
        let b = rocket_pod_scatter_offset(20);
        assert!((a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01);
    }
    /// Wave 71 residual pack honesty gate.
    #[test]
    fn comanche_rocket_pods_residual_pack_honesty_wave71() {
        assert!(honesty_comanche_rocket_pods_residual_pack_ok());
        assert_eq!(ROCKET_POD_CLIP_SIZE, 20);
        assert_eq!(ROCKET_POD_CLIP_RELOAD_FRAMES, 900);
        assert_eq!(ROCKET_POD_SCATTER_TARGETS.len(), 20);
    }

    #[test]
    fn comanche_antitank_scatter_vs_infantry_peels() {
        assert!((COMANCHE_AT_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);
        let aim = glam::Vec3::new(10.0, 0.0, 20.0);
        let (sc, applied) = comanche_antitank_scatter_aim(aim, true, 31);
        assert!(applied);
        let d = ((sc.x - aim.x).powi(2) + (sc.z - aim.z).powi(2)).sqrt();
        assert!(d > 0.01 && d <= COMANCHE_AT_SCATTER_VS_INFANTRY + 0.01);
        let (sc2, _) = comanche_antitank_scatter_aim(aim, true, 31);
        assert_eq!(sc, sc2);
        let (nosc, applied2) = comanche_antitank_scatter_aim(aim, false, 31);
        assert!(!applied2);
        assert_eq!(nosc, aim);
        assert!(comanche_antitank_scatter_misses_infantry(true, 67, 0.5));
        assert!(!comanche_antitank_scatter_misses_infantry(true, 67, 100.0));
        assert!(!comanche_antitank_scatter_misses_infantry(false, 67, 0.5));
    }
}
