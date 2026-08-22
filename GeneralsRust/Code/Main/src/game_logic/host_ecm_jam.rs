//! Host China ECM Tank / jammer residual (weapon jam aura).
//!
//! C++ live path:
//! - `ECMTankVehicleDisabler` is SUBDUAL_VEHICLE (ActiveBody.cpp:471-487) — no HP.
//!   Vehicles jam only after `isSubdued()` (`maxHealth <= subdual`, :1292-1294).
//!   Infantry / aircraft are not vehicle-disabler targets.
//! - `ECMTankMissileJammer` SUBDUAL_MISSILE never subtracts HP; `onSubdualChange`
//!   (:1280-1286) calls `MissileAIUpdate::projectileNowJammed` (scatter, lose lock).
//!   `DumbProjectileBehavior::projectileNowJammed` is empty (tank shells keep flying).
//! - SubdualDamageHelper heals 50 / 500ms so disable lingers after the beam stops.
//!
//! Wave 54 residual pack (retail INI honesty):
//! - ECMTankMissileJammer PrimaryDamageRadius **150**, PrimaryDamage **100**,
//!   AttackRange **15**, MinimumAttackRange **10**, DelayBetweenShots **650**ms → **20**f,
//!   RadiusDamageAffects ENEMIES NEUTRALS, DamageType SUBDUAL_MISSILE
//! - ECMTankVehicleDisabler AttackRange **200**, PrimaryDamage **24**,
//!   DelayBetweenShots **100**ms → **3**f, DamageType SUBDUAL_VEHICLE,
//!   LaserName ECMDisableStream, FireSound FrequencyJammerWeaponLoop
//! - FireWeaponUpdate ExclusiveWeaponDelay **1000**ms → **30**f
//! - ActiveBody SubdualDamageCap **600**, HealRate **500**ms → **15**f, HealAmount **50**
//! - VisionRange **150**; vehicle name residual list (China/Tank_/Nuke_/Infa_)
//!
//! Fail-closed honesty:
//! - ExclusiveWeaponDelay vs last_fire_frame is live (not full multi-weapon matrix)
//! - Not full ally relationship / underpower / DISABLED_SUBDUED FX tint matrix
//! - Not network jam replication (network deferred)

/// Logic frames per second (host fixed step).
pub const ECM_LOGIC_FPS: f32 = 30.0;

/// Retail ECMTankMissileJammer PrimaryDamageRadius residual (= 150).
/// Vehicle-disabler beam uses [`ECM_VEHICLE_DISABLER_ATTACK_RANGE`] (200).
pub const HOST_ECM_JAM_RADIUS: f32 = 150.0;

/// Retail ECMTankMissileJammer PrimaryDamage residual.
pub const ECM_MISSILE_JAMMER_PRIMARY_DAMAGE: f32 = 100.0;
/// Retail ECMTankMissileJammer AttackRange residual.
pub const ECM_MISSILE_JAMMER_ATTACK_RANGE: f32 = 15.0;
/// Retail ECMTankMissileJammer MinimumAttackRange residual.
pub const ECM_MISSILE_JAMMER_MIN_ATTACK_RANGE: f32 = 10.0;
/// Retail ECMTankMissileJammer DelayBetweenShots residual (msec).
pub const ECM_MISSILE_JAMMER_DELAY_MS: u32 = 650;
/// DelayBetweenShots 650ms → 20 frames @ 30 FPS (round).
pub const ECM_MISSILE_JAMMER_DELAY_FRAMES: u32 = 20;
/// Retail weapon template name.
pub const ECM_MISSILE_JAMMER_WEAPON: &str = "ECMTankMissileJammer";
/// Retail FireFX residual.
pub const ECM_MISSILE_JAMMER_FIRE_FX: &str = "FX_ECMTankMissileJammerPulse";
/// C++ MissileAIUpdate.cpp:56 `DistanceScatterWhenJammed` default.
pub const ECM_MISSILE_JAM_SCATTER_RADIUS: f32 = 75.0;
/// Max missiles one jammer pulse can jam residual.
pub const ECM_MISSILE_JAM_MAX_PER_PULSE: u32 = 4;

/// Retail DamageType residual marker.
pub const ECM_MISSILE_JAMMER_DAMAGE_TYPE: &str = "SUBDUAL_MISSILE";

/// Retail ECMTankVehicleDisabler AttackRange residual.
pub const ECM_VEHICLE_DISABLER_ATTACK_RANGE: f32 = 200.0;
/// Retail ECMTankVehicleDisabler PrimaryDamage residual.
pub const ECM_VEHICLE_DISABLER_PRIMARY_DAMAGE: f32 = 24.0;
/// Retail ECMTankVehicleDisabler DelayBetweenShots residual (msec).
pub const ECM_VEHICLE_DISABLER_DELAY_MS: u32 = 100;
/// DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const ECM_VEHICLE_DISABLER_DELAY_FRAMES: u32 = 3;
/// Retail weapon template name.
pub const ECM_VEHICLE_DISABLER_WEAPON: &str = "ECMTankVehicleDisabler";
/// Retail DamageType residual marker.
pub const ECM_VEHICLE_DISABLER_DAMAGE_TYPE: &str = "SUBDUAL_VEHICLE";
/// Retail laser stream residual.
pub const ECM_DISABLE_STREAM_LASER: &str = "ECMDisableStream";
/// Retail laser bone residual.
pub const ECM_DISABLE_STREAM_BONE: &str = "WEAPONA01";
/// Retail FireSound residual.
pub const ECM_VEHICLE_DISABLER_FIRE_SOUND: &str = "FrequencyJammerWeaponLoop";
/// Retail FireSoundLoopTime residual (msec).
pub const ECM_VEHICLE_DISABLER_FIRE_SOUND_LOOP_MS: u32 = 120;

/// Retail FireWeaponUpdate ExclusiveWeaponDelay residual (msec).
pub const ECM_EXCLUSIVE_WEAPON_DELAY_MS: u32 = 1_000;
/// ExclusiveWeaponDelay 1000ms → 30 frames @ 30 FPS.
pub const ECM_EXCLUSIVE_WEAPON_DELAY_FRAMES: u32 = 30;

/// C++ `FireWeaponUpdate::isOkayToFire` ExclusiveWeaponDelay gate.
///
/// `current < lastShot + ExclusiveWeaponDelay` suppresses the jammer pulse.
/// `last_shot_fired_frame == 0` means never fired (live `last_fire_frame` default)
/// so the jammer may pulse until a real weapon (vehicle disabler) stamps a shot.
pub fn ecm_exclusive_weapon_delay_blocks(current_frame: u32, last_shot_fired_frame: u32) -> bool {
    last_shot_fired_frame > 0
        && ECM_EXCLUSIVE_WEAPON_DELAY_FRAMES > 0
        && current_frame < last_shot_fired_frame.saturating_add(ECM_EXCLUSIVE_WEAPON_DELAY_FRAMES)
}

/// Retail ChinaTankECM ActiveBody SubdualDamageCap residual.
pub const ECM_SUBDUAL_DAMAGE_CAP: f32 = 600.0;
/// Retail SubdualDamageHealRate residual (msec).
pub const ECM_SUBDUAL_HEAL_RATE_MS: u32 = 500;
/// SubdualDamageHealRate 500ms → 15 frames @ 30 FPS.
pub const ECM_SUBDUAL_HEAL_RATE_FRAMES: u32 = 15;
/// Retail SubdualDamageHealAmount residual.
pub const ECM_SUBDUAL_HEAL_AMOUNT: f32 = 50.0;

/// Retail ChinaTankECM VisionRange residual.
pub const ECM_TANK_VISION_RANGE: f32 = 150.0;
/// Retail ChinaTankECM MaxHealth residual.
pub const ECM_TANK_MAX_HEALTH: f32 = 300.0;
/// Retail ChinaTankECM BuildCost residual.
pub const ECM_TANK_BUILD_COST: u32 = 800;

/// Retail primary vehicle list residual markers (China + general variants).
pub const ECM_TANK_VEHICLE_MARKERS: &[&str] = &[
    "chinatankecm",
    "tank_chinatankecm",
    "nuke_chinatankecm",
    "infa_chinatankecm",
];

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn ecm_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / ECM_LOGIC_FPS)).round() as u32
}

/// Whether template is a residual ECM tank / frequency jammer source.
///
/// Fail-closed: name-based residual (not full INI FireWeaponUpdate / WeaponSet matrix).

/// Deterministic scatter offset residual for a jammed missile.
pub fn ecm_missile_scatter_offset(seed: u32) -> (f32, f32) {
    // Unit-circle sample from seed (no RNG stream required).
    let a = ((seed.wrapping_mul(1103515245).wrapping_add(12345)) % 360) as f32;
    let rad = a * std::f32::consts::PI / 180.0;
    let r = ECM_MISSILE_JAM_SCATTER_RADIUS * (0.35 + 0.65 * (((seed >> 8) % 100) as f32 / 100.0));
    (rad.cos() * r, rad.sin() * r)
}

/// Whether template/name is residual jam-interceptable **guided** missile.
///
/// C++ `MissileAIUpdate::projectileNowJammed` only. Dumb shells
/// (`DumbProjectileBehavior::projectileNowJammed` empty) are excluded.
pub fn is_ecm_jam_missile_name(template_name: &str) -> bool {
    if is_dumb_projectile_shell_name(template_name) {
        return false;
    }
    let n = template_name.to_ascii_lowercase();
    if n.contains("defender") || n.contains("battery") || n.contains("site") {
        return false;
    }
    n.contains("missile")
        || n.contains("rocket")
        || n.contains("tomahawk")
        || n.contains("scud")
        || n.contains("napalm")
        || n.contains("stinger")
        || n.contains("rpg")
}

/// C++ `DumbProjectileBehavior::projectileNowJammed` is empty — tank/cannon shells
/// keep their aim when an ECM pulse hits.
pub fn is_dumb_projectile_shell_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("shell") || n.contains("howitzer") || n.contains("cannonshot")
}

/// Whether object flags indicate an in-flight **guided** missile residual.
pub fn is_ecm_jam_projectile_flags(
    is_projectile_kind: bool,
    template_name: &str,
    already_jammed: bool,
) -> bool {
    if already_jammed || is_dumb_projectile_shell_name(template_name) {
        return false;
    }
    is_projectile_kind || is_ecm_jam_missile_name(template_name)
}

pub fn is_ecm_jammer(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    // ChinaTankECM, Tank_ChinaTankECM, Nuke_ChinaTankECM, Infa_ChinaTankECM, …
    if n.contains("tankecm") || n.contains("ecmtank") {
        return true;
    }
    // FrequencyJammer voice-named residual / cinematic variants.
    if n.contains("frequencyjammer") || n.contains("missilejammer") {
        return true;
    }
    // Explicit residual test / shorthand names.
    if n == "testecmtank" || (n.ends_with("ecm") && n.contains("tank")) {
        return true;
    }
    false
}

/// Whether residual template is on the primary China ECM vehicle list.
pub fn is_ecm_tank_vehicle_list(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    ECM_TANK_VEHICLE_MARKERS.iter().any(|m| n == *m)
        || (n.contains("chinatankecm") && !n.contains("debris") && !n.contains("hulk"))
}

/// Whether residual target can have weapons jammed by an ECM vehicle disabler.
///
/// C++ `ECMTankVehicleDisabler` hits ground vehicles only (not infantry/aircraft
/// / structures). Live path uses [`is_legal_ecm_vehicle_disabler_target`].
pub fn is_legal_ecm_jam_target(
    is_structure: bool,
    is_alive: bool,
    enemy_or_neutral: bool,
    is_self: bool,
    under_construction: bool,
    has_weapon: bool,
) -> bool {
    !is_structure && is_alive && enemy_or_neutral && !is_self && !under_construction && has_weapon
}

/// Seed ActiveBody subdual cap/heal when INI left them unauthored (cap 0 = immune).
///
/// C++ `canBeSubdued` is `SubdualDamageCap > 0` (ActiveBody.cpp). Host test /
/// residual templates often omit the module; seed cap = maxHealth so
/// `isSubdued()` (`maxHealth <= subdual`) can fire, and ChinaTankECM heal
/// 50 / 500ms so disable lingers after the beam stops.
pub fn seed_host_subdual_if_unauthored(
    cap: &mut f32,
    heal_rate_frames: &mut u32,
    heal_amount: &mut f32,
    max_health: f32,
) {
    if *cap <= 0.0 {
        *cap = max_health.max(1.0);
    }
    if *heal_rate_frames == 0 {
        *heal_rate_frames = ECM_SUBDUAL_HEAL_RATE_FRAMES;
    }
    if *heal_amount <= 0.0 {
        *heal_amount = ECM_SUBDUAL_HEAL_AMOUNT;
    }
}

/// C++ `ActiveBody::internalAddSubdualDamage` — clamp to cap, no HP.
pub fn accumulate_subdual_damage(current: f32, amount: f32, cap: f32) -> f32 {
    if amount <= 0.0 || !amount.is_finite() || cap <= 0.0 {
        return current;
    }
    (current + amount).min(cap)
}

/// C++ `ActiveBody::isSubdued` (`maxHealth <= currentSubdual`).
pub fn is_subdual_full(current: f32, max_health: f32) -> bool {
    max_health > 0.0 && current + 1e-3 >= max_health
}

/// KindOf residual filter for vehicle-disabler path (ground vehicle, not aircraft).
pub fn is_legal_ecm_vehicle_disabler_target(
    is_vehicle: bool,
    is_aircraft: bool,
    is_alive: bool,
    enemy_or_neutral: bool,
    is_self: bool,
    under_construction: bool,
) -> bool {
    is_vehicle && !is_aircraft && is_alive && enemy_or_neutral && !is_self && !under_construction
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_ecm_jam_radius_2d(jammer_pos: (f32, f32), target_pos: (f32, f32), radius: f32) -> bool {
    let dx = jammer_pos.0 - target_pos.0;
    let dy = jammer_pos.1 - target_pos.1;
    dx * dx + dy * dy <= radius * radius
}

/// True when jammer team vs target team is residual-hostile (enemy) or Neutral victim.
///
/// Retail ECMTankMissileJammer: RadiusDamageAffects = ENEMIES NEUTRALS.
pub fn is_ecm_hostile_team(
    jammer_team_is_neutral: bool,
    same_team: bool,
    target_is_neutral: bool,
) -> bool {
    if jammer_team_is_neutral {
        // Neutral jammer residual does not jam anyone (fail-closed).
        return false;
    }
    !same_team || target_is_neutral
}

/// Wave 54 residual honesty: jam radius / missile jammer weapon residual.
pub fn honesty_ecm_jam_radius_weapon_residual_ok() -> bool {
    (HOST_ECM_JAM_RADIUS - 150.0).abs() < 0.01
        && (ECM_MISSILE_JAMMER_PRIMARY_DAMAGE - 100.0).abs() < 0.01
        && (ECM_MISSILE_JAMMER_ATTACK_RANGE - 15.0).abs() < 0.01
        && (ECM_MISSILE_JAMMER_MIN_ATTACK_RANGE - 10.0).abs() < 0.01
        && ECM_MISSILE_JAMMER_DELAY_MS == 650
        && ECM_MISSILE_JAMMER_DELAY_FRAMES == ecm_ms_to_frames(ECM_MISSILE_JAMMER_DELAY_MS)
        && ECM_MISSILE_JAMMER_WEAPON == "ECMTankMissileJammer"
        && ECM_MISSILE_JAMMER_FIRE_FX == "FX_ECMTankMissileJammerPulse"
        && ECM_MISSILE_JAMMER_DAMAGE_TYPE == "SUBDUAL_MISSILE"
}

/// Wave 54 residual honesty: vehicle disabler residual.
pub fn honesty_ecm_vehicle_disabler_residual_ok() -> bool {
    (ECM_VEHICLE_DISABLER_ATTACK_RANGE - 200.0).abs() < 0.01
        && (ECM_VEHICLE_DISABLER_PRIMARY_DAMAGE - 24.0).abs() < 0.01
        && ECM_VEHICLE_DISABLER_DELAY_MS == 100
        && ECM_VEHICLE_DISABLER_DELAY_FRAMES == ecm_ms_to_frames(ECM_VEHICLE_DISABLER_DELAY_MS)
        && ECM_VEHICLE_DISABLER_WEAPON == "ECMTankVehicleDisabler"
        && ECM_VEHICLE_DISABLER_DAMAGE_TYPE == "SUBDUAL_VEHICLE"
        && ECM_DISABLE_STREAM_LASER == "ECMDisableStream"
        && ECM_DISABLE_STREAM_BONE == "WEAPONA01"
        && ECM_VEHICLE_DISABLER_FIRE_SOUND == "FrequencyJammerWeaponLoop"
        && ECM_VEHICLE_DISABLER_FIRE_SOUND_LOOP_MS == 120
}

/// Wave 54 residual honesty: exclusive delay + subdual body residual.
pub fn honesty_ecm_subdual_reload_residual_ok() -> bool {
    ECM_EXCLUSIVE_WEAPON_DELAY_MS == 1_000
        && ECM_EXCLUSIVE_WEAPON_DELAY_FRAMES == ecm_ms_to_frames(ECM_EXCLUSIVE_WEAPON_DELAY_MS)
        && (ECM_SUBDUAL_DAMAGE_CAP - 600.0).abs() < 0.01
        && ECM_SUBDUAL_HEAL_RATE_MS == 500
        && ECM_SUBDUAL_HEAL_RATE_FRAMES == ecm_ms_to_frames(ECM_SUBDUAL_HEAL_RATE_MS)
        && (ECM_SUBDUAL_HEAL_AMOUNT - 50.0).abs() < 0.01
        && (ECM_TANK_VISION_RANGE - 150.0).abs() < 0.01
        && (ECM_TANK_MAX_HEALTH - 300.0).abs() < 0.01
        && ECM_TANK_BUILD_COST == 800
}

/// Wave 54 residual honesty: vehicle list + KindOf filters.
pub fn honesty_ecm_vehicle_list_kindof_residual_ok() -> bool {
    ECM_TANK_VEHICLE_MARKERS.len() >= 4
        && is_ecm_jammer("ChinaTankECM")
        && is_ecm_jammer("Tank_ChinaTankECM")
        && is_ecm_jammer("Nuke_ChinaTankECM")
        && is_ecm_jammer("Infa_ChinaTankECM")
        && is_ecm_tank_vehicle_list("ChinaTankECM")
        && is_ecm_tank_vehicle_list("Tank_ChinaTankECM")
        && !is_ecm_jammer("ChinaTankBattleMaster")
        && is_legal_ecm_vehicle_disabler_target(true, false, true, true, false, false)
        && !is_legal_ecm_vehicle_disabler_target(true, true, true, true, false, false) // aircraft
        && !is_legal_ecm_vehicle_disabler_target(false, false, true, true, false, false)
    // infantry
}

/// Combined Wave 54 ECM residual honesty pack.
/// Wave residual honesty: ECM missile jam scatter peels.
pub fn honesty_ecm_missile_jam_scatter_ok() -> bool {
    (ECM_MISSILE_JAM_SCATTER_RADIUS - 75.0).abs() < 0.01
        && ECM_MISSILE_JAM_MAX_PER_PULSE == 4
        && ECM_MISSILE_JAMMER_DELAY_FRAMES == 20
        && (ECM_MISSILE_JAMMER_PRIMARY_DAMAGE - 100.0).abs() < 0.01
        && ECM_MISSILE_JAMMER_WEAPON == "ECMTankMissileJammer"
}

/// Wave residual honesty: ECMDisableStream laser peels.
pub fn honesty_ecm_disable_stream_ok() -> bool {
    ECM_DISABLE_STREAM_LASER == "ECMDisableStream"
        && ECM_DISABLE_STREAM_BONE == "WEAPONA01"
        && ECM_VEHICLE_DISABLER_WEAPON == "ECMTankVehicleDisabler"
        && ECM_VEHICLE_DISABLER_DELAY_FRAMES == 3
        && ECM_EXCLUSIVE_WEAPON_DELAY_FRAMES == 30
        && ECM_VEHICLE_DISABLER_FIRE_SOUND == "FrequencyJammerWeaponLoop"
}

pub fn honesty_ecm_jam_residual_pack_ok() -> bool {
    honesty_ecm_jam_radius_weapon_residual_ok()
        && honesty_ecm_vehicle_disabler_residual_ok()
        && honesty_ecm_subdual_reload_residual_ok()
        && honesty_ecm_vehicle_list_kindof_residual_ok()
        && honesty_ecm_missile_jam_scatter_ok()
        && honesty_ecm_disable_stream_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecm_jammer_name_matrix() {
        assert!(is_ecm_jammer("ChinaTankECM"));
        assert!(is_ecm_jammer("Tank_ChinaTankECM"));
        assert!(is_ecm_jammer("Nuke_ChinaTankECM"));
        assert!(is_ecm_jammer("Infa_ChinaTankECM"));
        assert!(is_ecm_jammer("TestECMTank"));
        assert!(is_ecm_jammer("FrequencyJammer"));
        assert!(is_ecm_jammer("AmericaMissileJammer"));
        assert!(!is_ecm_jammer("ChinaTankBattleMaster"));
        assert!(!is_ecm_jammer("USA_Ranger"));
        assert!(!is_ecm_jammer("TestTank"));
        assert!(!is_ecm_jammer("ChinaSpeakerTower"));
        assert!(!is_ecm_jammer("AmericaVehicleMedic"));
    }

    #[test]
    fn legal_ecm_jam_target_matrix() {
        // structure, alive, enemy_or_neutral, is_self, under_construction, has_weapon
        assert!(is_legal_ecm_jam_target(
            false, true, true, false, false, true
        ));
        assert!(!is_legal_ecm_jam_target(
            true, true, true, false, false, true
        ));
        assert!(!is_legal_ecm_jam_target(
            false, false, true, false, false, true
        ));
        assert!(!is_legal_ecm_jam_target(
            false, true, false, false, false, true
        ));
        assert!(!is_legal_ecm_jam_target(
            false, true, true, true, false, true
        ));
        assert!(!is_legal_ecm_jam_target(
            false, true, true, false, true, true
        ));
        assert!(!is_legal_ecm_jam_target(
            false, true, true, false, false, false
        ));
    }

    #[test]
    fn ecm_radius_and_team_filters() {
        assert!(HOST_ECM_JAM_RADIUS > 0.0);
        assert!(in_ecm_jam_radius_2d((0.0, 0.0), (50.0, 0.0), 150.0));
        assert!(!in_ecm_jam_radius_2d((0.0, 0.0), (200.0, 0.0), 150.0));
        assert!(is_ecm_hostile_team(false, false, false)); // enemy
        assert!(is_ecm_hostile_team(false, false, true)); // neutral victim
        assert!(!is_ecm_hostile_team(false, true, false)); // same team ally
        assert!(!is_ecm_hostile_team(true, false, false)); // neutral jammer
    }

    #[test]
    fn missile_jam_scatter_peels() {
        assert!(honesty_ecm_missile_jam_scatter_ok());
        assert!(is_ecm_jam_missile_name("TomahawkMissile"));
        assert!(is_ecm_jam_missile_name("SCUDMissile"));
        assert!(!is_ecm_jam_missile_name("AmericaInfantryMissileDefender"));
        assert!(!is_ecm_jam_missile_name("CrusaderTankShell"));
        assert!(is_dumb_projectile_shell_name("TechnicalCannonShell"));
        let (x, z) = ecm_missile_scatter_offset(7);
        assert!((x * x + z * z).sqrt() <= ECM_MISSILE_JAM_SCATTER_RADIUS + 0.01);
        assert!(is_ecm_jam_projectile_flags(true, "TomahawkMissile", false));
        assert!(!is_ecm_jam_projectile_flags(true, "TankShell", false));
        assert!(!is_ecm_jam_projectile_flags(true, "foo", true));
    }

    #[test]
    fn ecm_disable_stream_peels() {
        assert!(honesty_ecm_disable_stream_ok());
        assert_eq!(ECM_DISABLE_STREAM_LASER, "ECMDisableStream");
        assert_eq!(ECM_DISABLE_STREAM_BONE, "WEAPONA01");
    }

    #[test]
    fn ecm_jam_residual_pack_honesty() {
        assert!(honesty_ecm_jam_residual_pack_ok());
        assert_eq!(ecm_ms_to_frames(650), 20);
        assert_eq!(ecm_ms_to_frames(100), 3);
        assert_eq!(ecm_ms_to_frames(1_000), 30);
        assert_eq!(ecm_ms_to_frames(500), 15);
    }

    #[test]
    fn ecm_vehicle_disabler_kindof_filter() {
        assert!(is_legal_ecm_vehicle_disabler_target(
            true, false, true, true, false, false
        ));
        assert!(!is_legal_ecm_vehicle_disabler_target(
            true, true, true, true, false, false
        ));
        assert!(!is_legal_ecm_vehicle_disabler_target(
            false, false, true, true, false, false
        ));
        assert!(!is_legal_ecm_vehicle_disabler_target(
            true, false, false, true, false, false
        ));
        assert!(!is_legal_ecm_vehicle_disabler_target(
            true, false, true, false, false, false
        ));
        assert!(!is_legal_ecm_vehicle_disabler_target(
            true, false, true, true, true, false
        ));
        assert!(!is_legal_ecm_vehicle_disabler_target(
            true, false, true, true, false, true
        ));
    }

    /// C++ ActiveBody.cpp:1292-1294 — jam only after subdual fills maxHealth.
    #[test]
    fn subdual_accumulate_not_instant() {
        let mut cap = 0.0;
        let mut rate = 0;
        let mut heal = 0.0;
        seed_host_subdual_if_unauthored(&mut cap, &mut rate, &mut heal, 400.0);
        assert!((cap - 400.0).abs() < 1e-3);
        assert_eq!(rate, ECM_SUBDUAL_HEAL_RATE_FRAMES);
        let mut pool = 0.0;
        pool = accumulate_subdual_damage(pool, ECM_VEHICLE_DISABLER_PRIMARY_DAMAGE, cap);
        assert!(!is_subdual_full(pool, 400.0));
        for _ in 0..20 {
            pool = accumulate_subdual_damage(pool, ECM_VEHICLE_DISABLER_PRIMARY_DAMAGE, cap);
        }
        assert!(is_subdual_full(pool, 400.0));
    }

    #[test]
    fn ecm_vehicle_list_residual() {
        assert!(is_ecm_tank_vehicle_list("ChinaTankECM"));
        assert!(is_ecm_tank_vehicle_list("Nuke_ChinaTankECM"));
        assert!(!is_ecm_tank_vehicle_list("DeadChinaECMTankHulk"));
        assert!(!is_ecm_tank_vehicle_list("ChinaTankBattleMaster"));
    }

    /// C++ FireWeaponUpdate::isOkayToFire ExclusiveWeaponDelay 1000ms → 30f.
    /// last_shot 0 = never fired → jammer may pulse.
    #[test]
    fn exclusive_weapon_delay_blocks_after_disabler_shot() {
        assert!(!ecm_exclusive_weapon_delay_blocks(0, 0));
        assert!(!ecm_exclusive_weapon_delay_blocks(30, 0));
        assert!(ecm_exclusive_weapon_delay_blocks(3, 3));
        assert!(ecm_exclusive_weapon_delay_blocks(32, 3));
        assert!(!ecm_exclusive_weapon_delay_blocks(33, 3));
        assert_eq!(ECM_EXCLUSIVE_WEAPON_DELAY_FRAMES, 30);
    }
}
