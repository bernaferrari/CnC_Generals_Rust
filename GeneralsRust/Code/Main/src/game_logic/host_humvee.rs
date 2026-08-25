//! Host America Humvee residual polish (transport + TOW air tertiary).
//!
//! Residual slice (playability):
//! - AmericaVehicleHumvee TransportContain Slots=**5**,
//!   PassengersAllowedToFire=Yes, AllowInsideKindOf=INFANTRY.
//! - Upgrade_AmericaTOWMissile already equips ground TOW secondary (host_upgrades).
//! - TOW air tertiary residual: vs aircraft after TOW research, prefer
//!   `HumveeMissileWeaponAir` (dmg **50** / range **320** / air only).
//!
//! Wave 58 residual pack (retail AmericaVehicle.ini / Weapon.ini honesty):
//! - PRIMARY HumveeGun: PrimaryDamage **10**, AttackRange **150**,
//!   DelayBetweenShots **200**ms → **6**f, WeaponSpeed **600**, FireSound HumveeWeapon
//! - SECONDARY HumveeMissileWeapon (TOW ground): PrimaryDamage **30** / radius **5**,
//!   AttackRange **150**, Delay **1000**ms → **30**f, ClipReload **2000**ms → **60**f,
//!   ClipSize **1**, ProjectileObject HumveeMissile, FireSound HumveeWeaponTOW
//! - TERTIARY HumveeMissileWeaponAir: PrimaryDamage **50** / radius **5**,
//!   AttackRange **320**, Delay+ClipReload cycle **90**f, ProjectileObject PatriotMissile
//! - Pre-TOW dummy HumveeMissileWeaponAirDummy: dmg **0.0001**, range **320**
//! - TransportContain: Slots **5**, ExitDelay **250**ms → **8**f, NumberOfExitPaths **3**,
//!   DamagePercentToUnits **100%**, GoAggressiveOnExit **Yes**, PassengersAllowedToFire **Yes**
//! - Body MaxHealth **240**, VisionRange **150**, ShroudClearingRange **320**,
//!   BuildCost **700**, TransportSlotCount **3**
//! - TurretTurnRate **180**, RecenterTime **5000**ms → **150**f
//! - HumveeLocomotor Speed **60**, Upgrade_AmericaTOWMissile weapon-set residual
//!
//! Fail-closed honesty:
//! - HumveeMissile / PatriotMissile MissileAI flight residual (ground TOW non-seek + air TOW seek)
//! - Ground TOW ScatterRadiusVsInfantry **10** residual miss cone closed (deterministic aim offset)
//! - Not full WeaponSet PLAYER_UPGRADE visual turret swap
//! - Not full TransportAIUpdate multi-exit-path / GoAggressiveOnExit matrix
//! - Not Battle/Scout/Hellfire drone ObjectCreationUpgrade (see host_slave_drones)
//! - Not network TOW / transport replication (network deferred)

use super::Weapon;
use glam::Vec3;

/// Logic frames per second (host fixed step).
pub const HUMVEE_LOGIC_FPS: f32 = 30.0;

/// Retail Humvee Missile air tertiary residual.
pub const HUMVEE_MISSILE_WEAPON_AIR: &str = "HumveeMissileWeaponAir";
/// Retail Humvee primary gun residual.
pub const HUMVEE_GUN_WEAPON: &str = "HumveeGun";
/// Retail Humvee ground TOW residual.
pub const HUMVEE_MISSILE_WEAPON: &str = "HumveeMissileWeapon";
/// Retail pre-TOW air dummy residual.
pub const HUMVEE_MISSILE_WEAPON_AIR_DUMMY: &str = "HumveeMissileWeaponAirDummy";
/// Retail TOW upgrade residual name.
pub const HUMVEE_TOW_UPGRADE: &str = "Upgrade_AmericaTOWMissile";
/// Retail fire audio residual names.
pub const HUMVEE_GUN_FIRE_AUDIO: &str = "HumveeWeapon";
pub const HUMVEE_TOW_FIRE_AUDIO: &str = "HumveeWeaponTOW";
/// Retail projectile residual names.
pub const HUMVEE_MISSILE_PROJECTILE: &str = "HumveeMissile";
pub const HUMVEE_AIR_TOW_PROJECTILE: &str = "PatriotMissile";

/// HumveeMissile MissileAI InitialVelocity residual (dist/sec).
pub const HUMVEE_GROUND_TOW_MISSILE_INITIAL_VELOCITY: f32 = 150.0;
/// HumveeMissile FuelLifetime 1000ms → 30 frames @ 30 FPS.
pub const HUMVEE_GROUND_TOW_MISSILE_FUEL_MS: u32 = 1_000;
pub const HUMVEE_GROUND_TOW_MISSILE_FUEL_FRAMES: u32 = 30;
/// HumveeMissile DistanceToTravelBeforeTurning residual.
pub const HUMVEE_GROUND_TOW_MISSILE_TURN_DISTANCE: f32 = 3.0;
/// HumveeMissile MaxHealth residual.
pub const HUMVEE_GROUND_TOW_MISSILE_MAX_HEALTH: f32 = 100.0;
/// HumveeMissile IgnitionDelay residual (frames).
pub const HUMVEE_GROUND_TOW_MISSILE_IGNITION_DELAY_FRAMES: u32 = 0;
/// HumveeMissile TryToFollowTarget residual.
pub const HUMVEE_GROUND_TOW_MISSILE_SEEK: bool = false;

/// PatriotMissile (air TOW) InitialVelocity residual (dist/sec).
pub const HUMVEE_AIR_TOW_MISSILE_INITIAL_VELOCITY: f32 = 50.0;
/// PatriotMissile FuelLifetime 10000ms → 300 frames.
pub const HUMVEE_AIR_TOW_MISSILE_FUEL_MS: u32 = 10_000;
pub const HUMVEE_AIR_TOW_MISSILE_FUEL_FRAMES: u32 = 300;
/// PatriotMissile DistanceToTravelBeforeTurning residual.
pub const HUMVEE_AIR_TOW_MISSILE_TURN_DISTANCE: f32 = 5.0;
/// PatriotMissile DistanceToTargetForLock residual.
pub const HUMVEE_AIR_TOW_MISSILE_LOCK_DISTANCE: f32 = 100.0;
/// PatriotMissile MaxHealth residual (shared projectile body).
pub const HUMVEE_AIR_TOW_MISSILE_MAX_HEALTH: f32 = 100.0;
/// PatriotMissile IgnitionDelay residual (frames).
pub const HUMVEE_AIR_TOW_MISSILE_IGNITION_DELAY_FRAMES: u32 = 0;
/// PatriotMissile TryToFollowTarget residual.
pub const HUMVEE_AIR_TOW_MISSILE_SEEK: bool = true;
/// Cruise speed residual (WeaponSpeed listed 600; ignored for projectile — use InitialVelocity cruise).
pub const HUMVEE_TOW_MISSILE_CRUISE_SPEED: f32 = 600.0;

/// Retail TransportContain Slots residual.
pub const HUMVEE_TRANSPORT_SLOTS: usize = 5;
/// Retail TransportSlotCount residual (slots this vehicle takes when carried).
pub const HUMVEE_TRANSPORT_SLOT_COUNT: u32 = 3;
/// Retail PassengersAllowedToFire residual.
pub const HUMVEE_PASSENGERS_ALLOWED_TO_FIRE: bool = true;
/// Retail AllowInsideKindOf = INFANTRY residual (vehicles not allowed).
pub const HUMVEE_ALLOW_INSIDE_INFANTRY_ONLY: bool = true;
/// Retail DamagePercentToUnits residual (percent).
pub const HUMVEE_DAMAGE_PERCENT_TO_UNITS: f32 = 100.0;
/// Retail ExitDelay residual (msec).
pub const HUMVEE_EXIT_DELAY_MS: u32 = 250;
/// ExitDelay 250ms → 8 frames @ 30 FPS.
pub const HUMVEE_EXIT_DELAY_FRAMES: u32 = 8;
/// Retail NumberOfExitPaths residual.
pub const HUMVEE_NUMBER_OF_EXIT_PATHS: u32 = 3;
/// Retail GoAggressiveOnExit residual.
pub const HUMVEE_GO_AGGRESSIVE_ON_EXIT: bool = true;

/// Retail HumveeGun PrimaryDamage residual.
pub const HUMVEE_GUN_DAMAGE: f32 = 10.0;
/// Retail HumveeGun AttackRange residual.
pub const HUMVEE_GUN_RANGE: f32 = 150.0;
/// Retail HumveeGun DelayBetweenShots residual (msec).
pub const HUMVEE_GUN_DELAY_MS: u32 = 200;
/// Delay 200ms → 6 frames @ 30 FPS.
pub const HUMVEE_GUN_DELAY_FRAMES: u32 = 6;
/// Retail HumveeGun WeaponSpeed residual.
pub const HUMVEE_GUN_WEAPON_SPEED: f32 = 600.0;

/// Retail HumveeMissileWeapon (ground TOW) PrimaryDamage residual.
pub const HUMVEE_GROUND_TOW_DAMAGE: f32 = 30.0;
/// Retail HumveeMissileWeapon PrimaryDamageRadius residual.
pub const HUMVEE_GROUND_TOW_RADIUS: f32 = 5.0;
/// Retail HumveeMissileWeapon AttackRange residual.
pub const HUMVEE_GROUND_TOW_RANGE: f32 = 150.0;
/// Retail DelayBetweenShots residual (msec).
pub const HUMVEE_GROUND_TOW_DELAY_MS: u32 = 1_000;
/// Delay 1000ms → 30 frames @ 30 FPS.
pub const HUMVEE_GROUND_TOW_DELAY_FRAMES: u32 = 30;
/// Retail ClipReloadTime residual (msec).
pub const HUMVEE_GROUND_TOW_CLIP_RELOAD_MS: u32 = 2_000;
/// ClipReload 2000ms → 60 frames @ 30 FPS.
pub const HUMVEE_GROUND_TOW_CLIP_RELOAD_FRAMES: u32 = 60;
/// Residual full ground TOW cycle = Delay + ClipReload → 90 frames.
pub const HUMVEE_GROUND_TOW_CYCLE_FRAMES: u32 = 90;
/// Retail ClipSize residual.
pub const HUMVEE_GROUND_TOW_CLIP_SIZE: u32 = 1;
/// Retail ScatterRadiusVsInfantry residual.
pub const HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY: f32 = 10.0;
/// Retail ground TOW WeaponSpeed residual.
pub const HUMVEE_GROUND_TOW_WEAPON_SPEED: f32 = 600.0;
/// Retail HumveeMissileWeapon DamageType residual.
pub const HUMVEE_TOW_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail HumveeMissileWeapon DeathType residual.
pub const HUMVEE_TOW_DEATH_TYPE: &str = "EXPLODED";

/// Retail HumveeMissileWeaponAir PrimaryDamage residual.
pub const HUMVEE_AIR_TOW_DAMAGE: f32 = 50.0;
/// Retail HumveeMissileWeaponAir PrimaryDamageRadius residual.
pub const HUMVEE_AIR_TOW_RADIUS: f32 = 5.0;
/// Retail HumveeMissileWeaponAir AttackRange residual.
pub const HUMVEE_AIR_TOW_RANGE: f32 = 320.0;
/// Retail DelayBetweenShots residual (msec).
pub const HUMVEE_AIR_TOW_DELAY_MS: u32 = 1_000;
/// Delay 1000ms → 30 frames.
pub const HUMVEE_AIR_TOW_DELAY_FRAMES: u32 = 30;
/// Retail ClipReloadTime residual (msec).
pub const HUMVEE_AIR_TOW_CLIP_RELOAD_MS: u32 = 2_000;
/// ClipReload 2000ms → 60 frames.
pub const HUMVEE_AIR_TOW_CLIP_RELOAD_FRAMES: u32 = 60;
/// Retail DelayBetweenShots 1000ms + ClipReload 2000ms residual cycle ≈ 90 frames.
/// Fail-closed: use 30 frames (Delay) + 60 frames (reload) collapsed to 90.
pub const HUMVEE_AIR_TOW_CYCLE_FRAMES: u32 = 90;
/// Retail air TOW WeaponSpeed residual.
pub const HUMVEE_AIR_TOW_WEAPON_SPEED: f32 = 600.0;
/// Retail air dummy PrimaryDamage residual.
pub const HUMVEE_AIR_DUMMY_DAMAGE: f32 = 0.0001;
/// Retail air dummy AttackRange residual.
pub const HUMVEE_AIR_DUMMY_RANGE: f32 = 320.0;

/// Retail MaxHealth residual.
pub const HUMVEE_MAX_HEALTH: f32 = 240.0;
/// Retail VisionRange residual.
pub const HUMVEE_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const HUMVEE_SHROUD_CLEARING_RANGE: f32 = 320.0;
/// Retail BuildCost residual.
pub const HUMVEE_BUILD_COST: u32 = 700;
/// Retail BuildTime residual (seconds).
pub const HUMVEE_BUILD_TIME_SEC: f32 = 10.0;
/// Retail TurretTurnRate residual (degrees/sec).
pub const HUMVEE_TURRET_TURN_RATE: f32 = 180.0;
/// Retail RecenterTime residual (msec).
pub const HUMVEE_TURRET_RECENTER_MS: u32 = 5_000;
/// RecenterTime 5000ms → 150 frames @ 30 FPS.
pub const HUMVEE_TURRET_RECENTER_FRAMES: u32 = 150;
/// Retail HumveeLocomotor Speed residual.
pub const HUMVEE_LOCOMOTOR_SPEED: f32 = 60.0;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn humvee_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / HUMVEE_LOGIC_FPS)).round() as u32
}

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
/// Whether combat should route through Humvee TOW projectile residual (secondary slot + upgrade).
pub fn should_apply_humvee_tow_residual(
    is_humvee: bool,
    has_tow_upgrade: bool,
    secondary_slot: bool,
) -> bool {
    is_humvee && has_tow_upgrade && secondary_slot
}

/// Per-frame ground TOW missile step speed (dist/frame) after ignition.
pub fn humvee_ground_tow_missile_step_speed(ignited: bool) -> f32 {
    if ignited {
        HUMVEE_TOW_MISSILE_CRUISE_SPEED / HUMVEE_LOGIC_FPS
    } else {
        HUMVEE_GROUND_TOW_MISSILE_INITIAL_VELOCITY / HUMVEE_LOGIC_FPS
    }
}

/// Per-frame air TOW missile step speed (dist/frame) after ignition.
pub fn humvee_air_tow_missile_step_speed(ignited: bool) -> f32 {
    if ignited {
        HUMVEE_TOW_MISSILE_CRUISE_SPEED / HUMVEE_LOGIC_FPS
    } else {
        HUMVEE_AIR_TOW_MISSILE_INITIAL_VELOCITY / HUMVEE_LOGIC_FPS
    }
}

/// Estimated flight frames for ground TOW straight-line residual.
pub fn humvee_ground_tow_flight_frames(distance: f32) -> u32 {
    let step = humvee_ground_tow_missile_step_speed(true).max(0.001);
    (distance / step).ceil() as u32
}

/// Estimated flight frames for air TOW residual.
pub fn humvee_air_tow_flight_frames(distance: f32) -> u32 {
    let step = humvee_air_tow_missile_step_speed(true).max(0.001);
    (distance / step).ceil() as u32
}

pub fn humvee_prefer_air_tow(is_humvee: bool, has_tow_upgrade: bool, target_is_air: bool) -> bool {
    is_humvee && has_tow_upgrade && target_is_air
}

/// Whether residual name matches Upgrade_AmericaTOWMissile residual.
pub fn is_humvee_tow_upgrade_name(name: &str) -> bool {
    let n = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    n == "upgradeamericatowmissile" || n == "upgrade_americatowmissile" || n.ends_with("towmissile")
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Humvee PRIMARY gun weapon.
pub fn humvee_gun_weapon() -> Weapon {
    Weapon {
        damage: HUMVEE_GUN_DAMAGE,
        range: HUMVEE_GUN_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(HUMVEE_GUN_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: HUMVEE_GUN_WEAPON_SPEED,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
    }
}

/// Build residual Humvee ground TOW secondary weapon (post Upgrade_AmericaTOWMissile).
pub fn humvee_ground_tow_weapon() -> Weapon {
    Weapon {
        damage: HUMVEE_GROUND_TOW_DAMAGE,
        range: HUMVEE_GROUND_TOW_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(HUMVEE_GROUND_TOW_CYCLE_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(HUMVEE_GROUND_TOW_CLIP_SIZE),
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: HUMVEE_GROUND_TOW_WEAPON_SPEED,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
    }
}

/// Build residual Humvee air TOW tertiary weapon (bound as secondary when air).
pub fn humvee_air_tow_weapon() -> Weapon {
    Weapon {
        damage: HUMVEE_AIR_TOW_DAMAGE,
        range: HUMVEE_AIR_TOW_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(HUMVEE_AIR_TOW_CYCLE_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(1),
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: HUMVEE_AIR_TOW_WEAPON_SPEED,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
    }
}

/// Residual ground TOW splash damage at distance from impact.
pub fn humvee_ground_tow_damage_at(distance_from_impact: f32) -> f32 {
    if distance_from_impact <= HUMVEE_GROUND_TOW_RADIUS {
        HUMVEE_GROUND_TOW_DAMAGE
    } else {
        0.0
    }
}

/// Residual air TOW splash damage at distance from impact.
pub fn humvee_air_tow_damage_at(distance_from_impact: f32) -> f32 {
    if distance_from_impact <= HUMVEE_AIR_TOW_RADIUS {
        HUMVEE_AIR_TOW_DAMAGE
    } else {
        0.0
    }
}

// --- Wave 58 residual honesty packs ---

/// Wave 58 residual honesty: primary gun residual.
pub fn honesty_humvee_gun_residual_ok() -> bool {
    HUMVEE_GUN_WEAPON == "HumveeGun"
        && (HUMVEE_GUN_DAMAGE - 10.0).abs() < 0.01
        && (HUMVEE_GUN_RANGE - 150.0).abs() < 0.01
        && HUMVEE_GUN_DELAY_MS == 200
        && HUMVEE_GUN_DELAY_FRAMES == humvee_ms_to_frames(HUMVEE_GUN_DELAY_MS)
        && (HUMVEE_GUN_WEAPON_SPEED - 600.0).abs() < 0.01
        && HUMVEE_GUN_FIRE_AUDIO == "HumveeWeapon"
}

/// Wave 58 residual honesty: ground TOW residual.
pub fn honesty_humvee_ground_tow_residual_ok() -> bool {
    HUMVEE_MISSILE_WEAPON == "HumveeMissileWeapon"
        && (HUMVEE_GROUND_TOW_DAMAGE - 30.0).abs() < 0.01
        && (HUMVEE_GROUND_TOW_RADIUS - 5.0).abs() < 0.01
        && (HUMVEE_GROUND_TOW_RANGE - 150.0).abs() < 0.01
        && HUMVEE_GROUND_TOW_DELAY_MS == 1_000
        && HUMVEE_GROUND_TOW_DELAY_FRAMES == humvee_ms_to_frames(HUMVEE_GROUND_TOW_DELAY_MS)
        && HUMVEE_GROUND_TOW_CLIP_RELOAD_MS == 2_000
        && HUMVEE_GROUND_TOW_CLIP_RELOAD_FRAMES
            == humvee_ms_to_frames(HUMVEE_GROUND_TOW_CLIP_RELOAD_MS)
        && HUMVEE_GROUND_TOW_CYCLE_FRAMES
            == HUMVEE_GROUND_TOW_DELAY_FRAMES + HUMVEE_GROUND_TOW_CLIP_RELOAD_FRAMES
        && HUMVEE_GROUND_TOW_CLIP_SIZE == 1
        && (HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && HUMVEE_MISSILE_PROJECTILE == "HumveeMissile"
        && HUMVEE_TOW_FIRE_AUDIO == "HumveeWeaponTOW"
        && (humvee_ground_tow_damage_at(0.0) - 30.0).abs() < 0.01
        && humvee_ground_tow_damage_at(6.0).abs() < 0.01
}

/// Wave 58 residual honesty: air TOW tertiary residual.
pub fn honesty_humvee_air_tow_residual_ok() -> bool {
    HUMVEE_MISSILE_WEAPON_AIR == "HumveeMissileWeaponAir"
        && (HUMVEE_AIR_TOW_DAMAGE - 50.0).abs() < 0.01
        && (HUMVEE_AIR_TOW_RADIUS - 5.0).abs() < 0.01
        && (HUMVEE_AIR_TOW_RANGE - 320.0).abs() < 0.01
        && HUMVEE_AIR_TOW_DELAY_MS == 1_000
        && HUMVEE_AIR_TOW_DELAY_FRAMES == humvee_ms_to_frames(HUMVEE_AIR_TOW_DELAY_MS)
        && HUMVEE_AIR_TOW_CLIP_RELOAD_MS == 2_000
        && HUMVEE_AIR_TOW_CLIP_RELOAD_FRAMES == humvee_ms_to_frames(HUMVEE_AIR_TOW_CLIP_RELOAD_MS)
        && HUMVEE_AIR_TOW_CYCLE_FRAMES
            == HUMVEE_AIR_TOW_DELAY_FRAMES + HUMVEE_AIR_TOW_CLIP_RELOAD_FRAMES
        && HUMVEE_AIR_TOW_PROJECTILE == "PatriotMissile"
        && (HUMVEE_AIR_DUMMY_DAMAGE - 0.0001).abs() < 0.00001
        && (HUMVEE_AIR_DUMMY_RANGE - 320.0).abs() < 0.01
        && HUMVEE_MISSILE_WEAPON_AIR_DUMMY == "HumveeMissileWeaponAirDummy"
        && humvee_prefer_air_tow(true, true, true)
        && !humvee_prefer_air_tow(true, false, true)
        && (humvee_air_tow_damage_at(5.0) - 50.0).abs() < 0.01
}

/// Wave 58 residual honesty: transport residual.
pub fn honesty_humvee_transport_residual_ok() -> bool {
    HUMVEE_TRANSPORT_SLOTS == 5
        && HUMVEE_TRANSPORT_SLOT_COUNT == 3
        && HUMVEE_PASSENGERS_ALLOWED_TO_FIRE
        && HUMVEE_ALLOW_INSIDE_INFANTRY_ONLY
        && (HUMVEE_DAMAGE_PERCENT_TO_UNITS - 100.0).abs() < 0.01
        && HUMVEE_EXIT_DELAY_MS == 250
        && HUMVEE_EXIT_DELAY_FRAMES == humvee_ms_to_frames(HUMVEE_EXIT_DELAY_MS)
        && HUMVEE_NUMBER_OF_EXIT_PATHS == 3
        && HUMVEE_GO_AGGRESSIVE_ON_EXIT
}

/// Wave 58 residual honesty: body / vision / TOW upgrade residual.
pub fn honesty_humvee_body_tow_upgrade_residual_ok() -> bool {
    (HUMVEE_MAX_HEALTH - 240.0).abs() < 0.01
        && (HUMVEE_VISION_RANGE - 150.0).abs() < 0.01
        && (HUMVEE_SHROUD_CLEARING_RANGE - 320.0).abs() < 0.01
        && HUMVEE_BUILD_COST == 700
        && (HUMVEE_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && (HUMVEE_TURRET_TURN_RATE - 180.0).abs() < 0.01
        && HUMVEE_TURRET_RECENTER_MS == 5_000
        && HUMVEE_TURRET_RECENTER_FRAMES == humvee_ms_to_frames(HUMVEE_TURRET_RECENTER_MS)
        && (HUMVEE_LOCOMOTOR_SPEED - 60.0).abs() < 0.01
        && HUMVEE_TOW_UPGRADE == "Upgrade_AmericaTOWMissile"
        && is_humvee_tow_upgrade_name("Upgrade_AmericaTOWMissile")
        && !is_humvee_tow_upgrade_name("Upgrade_AmericaBattleDrone")
}

/// Combined Wave 58 Humvee residual honesty pack.
/// Wave residual honesty: HumveeMissile / PatriotMissile MissileAI peels.
pub fn honesty_humvee_tow_missile_projectile_ok() -> bool {
    HUMVEE_MISSILE_PROJECTILE == "HumveeMissile"
        && HUMVEE_AIR_TOW_PROJECTILE == "PatriotMissile"
        && (HUMVEE_GROUND_TOW_MISSILE_INITIAL_VELOCITY - 150.0).abs() < 0.01
        && HUMVEE_GROUND_TOW_MISSILE_FUEL_MS == 1_000
        && HUMVEE_GROUND_TOW_MISSILE_FUEL_FRAMES == 30
        && (HUMVEE_GROUND_TOW_MISSILE_TURN_DISTANCE - 3.0).abs() < 0.01
        && !HUMVEE_GROUND_TOW_MISSILE_SEEK
        && (HUMVEE_AIR_TOW_MISSILE_INITIAL_VELOCITY - 50.0).abs() < 0.01
        && HUMVEE_AIR_TOW_MISSILE_FUEL_MS == 10_000
        && HUMVEE_AIR_TOW_MISSILE_FUEL_FRAMES == 300
        && (HUMVEE_AIR_TOW_MISSILE_TURN_DISTANCE - 5.0).abs() < 0.01
        && (HUMVEE_AIR_TOW_MISSILE_LOCK_DISTANCE - 100.0).abs() < 0.01
        && HUMVEE_AIR_TOW_MISSILE_SEEK
        && (HUMVEE_TOW_MISSILE_CRUISE_SPEED - 600.0).abs() < 0.01
}

/// Apply HumveeMissileWeapon (ground TOW) ScatterRadiusVsInfantry residual to aim.
///
/// Air TOW (`HumveeMissileWeaponAir` / PatriotMissile) does not apply this residual
/// (anti-air path; infantry scatter not retail-relevant).
pub fn humvee_tow_scatter_aim(
    aim: Vec3,
    target_is_infantry: bool,
    air: bool,
    seed: u32,
) -> (Vec3, bool) {
    use crate::game_logic::weapon_bootstrap::{host_effective_scatter_radius, scatter_aim_offset};
    if air {
        return (aim, false);
    }
    let mut scatter = host_effective_scatter_radius(HUMVEE_MISSILE_WEAPON, target_is_infantry);
    if target_is_infantry && scatter <= 0.0 {
        scatter = HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY;
    }
    if scatter <= 0.0 {
        return (aim, false);
    }
    let off = scatter_aim_offset(seed, scatter);
    (Vec3::new(aim.x + off.x, aim.y, aim.z + off.z), true)
}

/// Whether Humvee ground TOW residual misses intended infantry via ScatterRadiusVsInfantry.
///
/// Air TOW has no scatter peel. Miss = offset exceeds hit radius **and** impact outside
/// ground TOW splash (**5**).
pub fn humvee_tow_scatter_misses_infantry(
    target_is_infantry: bool,
    air: bool,
    seed: u32,
    target_hit_radius: f32,
) -> bool {
    use crate::game_logic::weapon_bootstrap::scatter_misses_intended_target;
    if air || !target_is_infantry {
        return false;
    }
    scatter_misses_intended_target(
        HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY,
        seed,
        target_hit_radius,
    )
}

/// Wave residual honesty: Humvee ground TOW ScatterRadiusVsInfantry peels (**10**).
pub fn honesty_humvee_tow_scatter_vs_infantry_ok() -> bool {
    use crate::game_logic::weapon_bootstrap::host_effective_scatter_radius;
    let vs = host_effective_scatter_radius(HUMVEE_MISSILE_WEAPON, true);
    let ground = host_effective_scatter_radius(HUMVEE_MISSILE_WEAPON, false);
    (HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && ((vs - 10.0).abs() < 0.01 || vs <= 0.0)
        && ground.abs() < 0.01
        && {
            let (sc, applied) = humvee_tow_scatter_aim(Vec3::new(0.0, 0.0, 0.0), true, false, 3);
            applied && sc.length() <= HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY + 0.01
        }
        && {
            let (_, applied) = humvee_tow_scatter_aim(Vec3::new(0.0, 0.0, 0.0), true, true, 3);
            !applied
        }
}

pub fn honesty_humvee_residual_pack_ok() -> bool {
    honesty_humvee_gun_residual_ok()
        && honesty_humvee_ground_tow_residual_ok()
        && honesty_humvee_air_tow_residual_ok()
        && honesty_humvee_transport_residual_ok()
        && honesty_humvee_body_tow_upgrade_residual_ok()
        && honesty_humvee_tow_missile_projectile_ok()
        && honesty_humvee_tow_scatter_vs_infantry_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humvee_tow_scatter_vs_infantry_peels() {
        assert!(honesty_humvee_tow_scatter_vs_infantry_ok());
        let aim = Vec3::new(140.0, 0.0, 18.0);
        let (no_sc, applied) = humvee_tow_scatter_aim(aim, false, false, 31);
        assert!(!applied);
        assert_eq!(no_sc, aim);
        let (air_sc, applied) = humvee_tow_scatter_aim(aim, true, true, 31);
        assert!(!applied);
        assert_eq!(air_sc, aim);
        let (sc, applied) = humvee_tow_scatter_aim(aim, true, false, 31);
        assert!(applied);
        let d = ((sc.x - aim.x).powi(2) + (sc.z - aim.z).powi(2)).sqrt();
        assert!(d > 0.01 && d <= HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY + 0.01);

        assert!(humvee_tow_scatter_misses_infantry(true, false, 43, 0.5));
        assert!(!humvee_tow_scatter_misses_infantry(true, false, 43, 100.0));
        assert!(!humvee_tow_scatter_misses_infantry(true, true, 43, 0.5));
        assert!(!humvee_tow_scatter_misses_infantry(false, false, 43, 0.5));
    }

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
        assert!((w.reload_time - 3.0).abs() < 0.05);
    }

    #[test]
    fn ground_tow_and_gun_stats() {
        let gun = humvee_gun_weapon();
        assert!((gun.damage - 10.0).abs() < 0.01);
        assert!((gun.range - 150.0).abs() < 0.01);
        assert!((gun.reload_time - (6.0 / 30.0)).abs() < 0.001);

        let tow = humvee_ground_tow_weapon();
        assert!((tow.damage - 30.0).abs() < 0.01);
        assert!((tow.range - 150.0).abs() < 0.01);
        assert!(!tow.can_target_air && tow.can_target_ground);
        assert!((tow.reload_time - 3.0).abs() < 0.05);
    }

    #[test]
    fn transport_slots() {
        assert_eq!(HUMVEE_TRANSPORT_SLOTS, 5);
        assert!(HUMVEE_PASSENGERS_ALLOWED_TO_FIRE);
        assert_eq!(HUMVEE_EXIT_DELAY_FRAMES, 8);
    }

    #[test]
    fn tow_missile_projectile_peels() {
        assert!(honesty_humvee_tow_missile_projectile_ok());
        assert!(should_apply_humvee_tow_residual(true, true, true));
        assert!(!should_apply_humvee_tow_residual(true, false, true));
        assert!(!should_apply_humvee_tow_residual(true, true, false));
        assert!(humvee_ground_tow_flight_frames(150.0) >= 1);
        assert!(humvee_air_tow_flight_frames(320.0) >= 1);
    }

    #[test]
    fn humvee_residual_pack_honesty_wave58() {
        assert!(honesty_humvee_tow_scatter_vs_infantry_ok());
        assert!(honesty_humvee_residual_pack_ok());
        assert_eq!(humvee_ms_to_frames(200), 6);
        assert_eq!(humvee_ms_to_frames(1_000), 30);
        assert_eq!(humvee_ms_to_frames(2_000), 60);
        assert_eq!(humvee_ms_to_frames(250), 8);
        assert_eq!(humvee_ms_to_frames(5_000), 150);
        assert!(is_humvee_tow_upgrade_name("Upgrade_AmericaTOWMissile"));
    }
}
