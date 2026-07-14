//! Host China MiG combat residual (napalm missiles + BlackNapalm + Nuke MiG).
//!
//! Residual slice (playability):
//! - `ChinaJetMIG` / `China_MiG` / Tank_/Infa_/Boss_ variants spawn with PRIMARY
//!   `NapalmMissileWeapon` (PrimaryDamage **75** / r**5** + SecondaryDamage **40** /
//!   r**30**, range **320**, min **80**, Delay **300**ms → 9 frames). ClipSize **2**
//!   honesty (RETURN_TO_BASE rearm matrix fail-closed).
//! - Impact residual also seeds FireFieldSmall DoT (OCL_FireFieldSmall residual via
//!   host Inferno fire-zone registry).
//! - BlackNapalm PLAYER_UPGRADE residual (`Upgrade_ChinaBlackNapalm`):
//!   `BlackNapalmMissileWeapon` SecondaryDamage **50** + FireFieldUpgradedSmall.
//! - Nuke General `Nuke_ChinaJetMIG`: base `Nuke_MiGMissileWeapon` Primary **100**
//!   / r**5** + Secondary **40** / r**30** + SmallRadiationField residual.
//!   Tactical Nuke MiG PLAYER_UPGRADE residual (`Upgrade_ChinaTacticalNukeMig`):
//!   `Nuke_NukeMissileWeapon` Primary **150** / r**50** + Secondary **50** / r**60**
//!   + SmallRadiationField residual.
//! - AA residual: AntiAirborneVehicle = Yes (can_target_air).
//!
//! Wave 67 residual pack (retail ChinaAir.ini / Weapon.ini / Locomotor.ini):
//! - Weapon residual: DamageType **JET_MISSILES**, DeathType **BURNED**,
//!   Projectile **NapalmMissile**, FireFX **WeaponFX_NapalmMissile**,
//!   AutoReloadsClip **RETURN_TO_BASE**, ClipSize **2**, ClipReload **8000**ms → **240**f.
//! - BlackNapalm residual: SecondaryDamage **50**, DeathType **EXPLODED**,
//!   ClipReload **2000**ms → **60**f (upgrade path).
//! - Body residual: MaxHealth **160**, Vision **200**, Shroud **300**,
//!   BuildCost **1200**, BuildTime **10**s → **300**f, TransportSlotCount **0**,
//!   Geometry BOX **14**/**7**/**5**, Locomotor Speed **160**/Min **60**.
//! - Aircraft Armor residual: Upgrade_ChinaAircraftArmor AddMaxHealth **40**
//!   + ADD_CURRENT_HEALTH_TOO.
//!
//! Fail-closed honesty:
//! - Not full JetAIUpdate RETURN_TO_BASE / ClipReload airfield rearm matrix
//! - Not full HistoricBonus FirestormSmallCreationWeapon multi-missile matrix
//! - Not full MediumRadiationField for Nuke_NukeMissileWeapon residual
//! - Not network MiG / BlackNapalm / TacticalNuke replication (network deferred)

use super::Weapon;
use std::collections::HashSet;

/// Logic frames per second (host fixed step).
pub const MIG_LOGIC_FPS: f32 = 30.0;

/// Retail standard MiG primary weapon.
pub const NAPALM_MISSILE_WEAPON: &str = "NapalmMissileWeapon";
/// Retail BlackNapalm upgraded primary.
pub const BLACK_NAPALM_MISSILE_WEAPON: &str = "BlackNapalmMissileWeapon";
/// Retail Nuke General base MiG primary.
pub const NUKE_MIG_MISSILE_WEAPON: &str = "Nuke_MiGMissileWeapon";
/// Retail Nuke General Tactical Nuke MiG upgraded primary.
pub const NUKE_NUKE_MISSILE_WEAPON: &str = "Nuke_NukeMissileWeapon";
/// Retail Upgrade_ChinaBlackNapalm.
pub const UPGRADE_CHINA_BLACK_NAPALM: &str = "Upgrade_ChinaBlackNapalm";
/// Retail Upgrade_ChinaTacticalNukeMig.
pub const UPGRADE_CHINA_TACTICAL_NUKE_MIG: &str = "Upgrade_ChinaTacticalNukeMig";
/// Retail Upgrade_ChinaAircraftArmor.
pub const UPGRADE_CHINA_AIRCRAFT_ARMOR: &str = "Upgrade_ChinaAircraftArmor";

/// Standard NapalmMissileWeapon PrimaryDamage.
pub const MIG_PRIMARY_DAMAGE: f32 = 75.0;
/// Standard PrimaryDamageRadius.
pub const MIG_PRIMARY_RADIUS: f32 = 5.0;
/// Standard SecondaryDamage.
pub const MIG_SECONDARY_DAMAGE: f32 = 40.0;
/// BlackNapalm SecondaryDamage residual.
pub const MIG_BLACK_SECONDARY_DAMAGE: f32 = 50.0;
/// Standard SecondaryDamageRadius.
pub const MIG_SECONDARY_RADIUS: f32 = 30.0;
/// Standard AttackRange.
pub const MIG_RANGE: f32 = 320.0;
/// Standard MinimumAttackRange.
pub const MIG_MIN_RANGE: f32 = 80.0;
/// Retail DelayBetweenShots residual (msec).
pub const MIG_DELAY_MS: u32 = 300;
/// DelayBetweenShots 300ms → 9 frames @ 30 FPS.
pub const MIG_DELAY_FRAMES: u32 = 9;
/// ClipSize honesty (RETURN_TO_BASE rearm fail-closed).
pub const MIG_CLIP_SIZE: u32 = 2;
/// Retail ClipReloadTime residual (msec, standard napalm).
pub const MIG_CLIP_RELOAD_MS: u32 = 8_000;
/// ClipReloadTime 8000ms → 240 frames honesty residual.
pub const MIG_CLIP_RELOAD_FRAMES: u32 = 240;
/// Retail BlackNapalm ClipReloadTime residual (msec).
pub const MIG_BLACK_CLIP_RELOAD_MS: u32 = 2_000;
/// BlackNapalm ClipReloadTime 2000ms → 60 frames honesty residual.
pub const MIG_BLACK_CLIP_RELOAD_FRAMES: u32 = 60;
/// Retail DamageType residual (standard napalm).
pub const MIG_DAMAGE_TYPE: &str = "JET_MISSILES";
/// Retail DeathType residual (standard napalm).
pub const MIG_DEATH_TYPE: &str = "BURNED";
/// Retail BlackNapalm DamageType residual.
pub const MIG_BLACK_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail BlackNapalm DeathType residual.
pub const MIG_BLACK_DEATH_TYPE: &str = "EXPLODED";
/// Retail ProjectileObject residual.
pub const MIG_PROJECTILE: &str = "NapalmMissile";
/// Retail FireFX residual.
pub const MIG_FIRE_FX: &str = "WeaponFX_NapalmMissile";
/// Retail AutoReloadsClip residual.
pub const MIG_AUTO_RELOADS_CLIP: &str = "RETURN_TO_BASE";

/// Nuke_MiGMissileWeapon PrimaryDamage.
pub const NUKE_MIG_PRIMARY_DAMAGE: f32 = 100.0;
/// Nuke_NukeMissileWeapon PrimaryDamage.
pub const NUKE_TACTICAL_PRIMARY_DAMAGE: f32 = 150.0;
/// Nuke_NukeMissileWeapon PrimaryDamageRadius.
pub const NUKE_TACTICAL_PRIMARY_RADIUS: f32 = 50.0;
/// Nuke_NukeMissileWeapon SecondaryDamage.
pub const NUKE_TACTICAL_SECONDARY_DAMAGE: f32 = 50.0;
/// Nuke_NukeMissileWeapon SecondaryDamageRadius.
pub const NUKE_TACTICAL_SECONDARY_RADIUS: f32 = 60.0;

/// Residual projectile speed.
pub const MIG_PROJECTILE_SPEED: f32 = 1000.0;
/// Residual fire audio.
pub const MIG_FIRE_AUDIO: &str = "MigJetNapalmWeapon";

// --- Body residual (ChinaJetMIG) ---

/// Retail MaxHealth residual.
pub const MIG_MAX_HEALTH: f32 = 160.0;
/// Retail Aircraft Armor AddMaxHealth residual.
pub const MIG_AIRCRAFT_ARMOR_ADD_MAX_HEALTH: f32 = 40.0;
/// Retail Aircraft Armor ChangeType residual.
pub const MIG_AIRCRAFT_ARMOR_CHANGE_TYPE: &str = "ADD_CURRENT_HEALTH_TOO";
/// Retail VisionRange residual.
pub const MIG_VISION_RANGE: f32 = 200.0;
/// Retail ShroudClearingRange residual.
pub const MIG_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail BuildCost residual.
pub const MIG_BUILD_COST: u32 = 1_200;
/// Retail BuildTime residual (seconds).
pub const MIG_BUILD_TIME_SEC: f32 = 10.0;
/// BuildTime 10s → 300 frames @ 30 FPS.
pub const MIG_BUILD_TIME_FRAMES: u32 = 300;
/// Retail TransportSlotCount residual.
pub const MIG_TRANSPORT_SLOT_COUNT: u32 = 0;
/// Retail Geometry BOX MajorRadius residual.
pub const MIG_GEOMETRY_MAJOR: f32 = 14.0;
/// Retail Geometry BOX MinorRadius residual.
pub const MIG_GEOMETRY_MINOR: f32 = 7.0;
/// Retail GeometryHeight residual.
pub const MIG_GEOMETRY_HEIGHT: f32 = 5.0;
/// Retail MIGLocomotor Speed residual.
pub const MIG_LOCOMOTOR_SPEED: f32 = 160.0;
/// Retail MIGLocomotor MinSpeed residual.
pub const MIG_LOCOMOTOR_MIN_SPEED: f32 = 60.0;
/// Retail ExperienceValue residual.
pub const MIG_EXPERIENCE_VALUE: [u32; 4] = [50, 50, 100, 150];
/// Retail ExperienceRequired residual.
pub const MIG_EXPERIENCE_REQUIRED: [u32; 4] = [0, 100, 200, 400];

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn mig_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * MIG_LOGIC_FPS / 1000.0).round() as u32
}

/// Residual loadout kind for damage / field selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigLoadout {
    /// Standard napalm dual-radius + FireFieldSmall.
    Standard,
    /// BlackNapalm dual-radius + FireFieldUpgradedSmall.
    BlackNapalm,
    /// Nuke General base dual-radius + SmallRadiationField.
    NukeBase,
    /// Nuke Tactical upgrade dual-radius + SmallRadiationField.
    NukeTactical,
}

fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether template is a residual living MiG jet.
///
/// Fail-closed: name residual. Excludes missiles / weapons / hulks / cargo / napalm striker.
pub fn is_mig_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "chinamig"
        || n == "china_mig"
        || n == "testmig"
        || n == "chinajetmig"
        || n == "nukechinajetmig"
        || n == "tankchinajetmig"
        || n == "infachinajetmig"
        || n == "bossjetmig"
    {
        return true;
    }
    // Exclude non-living residual objects.
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.contains("exhaust")
        || n.contains("locomotor")
        || n.starts_with("upgrade")
        || n.contains("crate")
        || n.contains("cargo")
        || n.contains("napalmstriker")
        || n.contains("firestorm")
        || n.contains("firefield")
    {
        return false;
    }
    // Living jet residual: *JetMIG* / *JetMiG* / bare *MIG* aircraft chassis.
    n.contains("jetmig") || n.ends_with("mig") || n.contains("chinamig")
}

/// Whether template is Nuke General MiG residual chassis.
pub fn is_nuke_mig_template(template_name: &str) -> bool {
    if !is_mig_template(template_name) {
        return false;
    }
    let n = alnum_lower(template_name);
    n.starts_with("nuke") || n.contains("nukemig")
}

/// Whether residual fire should apply MiG residual path.
pub fn should_apply_mig_residual(is_mig: bool) -> bool {
    is_mig
}

/// BlackNapalm PLAYER_UPGRADE residual present.
pub fn has_black_napalm_upgrade(applied_upgrades: &HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = alnum_lower(u);
        l.contains("blacknapalm") || l == "upgrade_chinablacknapalm"
    })
}

/// Tactical Nuke MiG PLAYER_UPGRADE residual present.
pub fn has_tactical_nuke_mig_upgrade(applied_upgrades: &HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = alnum_lower(u);
        l.contains("tacticalnukemig")
            || l.contains("nukemig")
            || l == "upgrade_chinatacticalnukemig"
    })
}

/// Resolve residual loadout from chassis + upgrades.
pub fn mig_loadout(is_nuke_chassis: bool, applied_upgrades: &HashSet<String>) -> MigLoadout {
    if is_nuke_chassis {
        if has_tactical_nuke_mig_upgrade(applied_upgrades) {
            MigLoadout::NukeTactical
        } else {
            MigLoadout::NukeBase
        }
    } else if has_black_napalm_upgrade(applied_upgrades) {
        MigLoadout::BlackNapalm
    } else {
        MigLoadout::Standard
    }
}

/// Primary damage residual for loadout (weapon.damage seed).
pub fn mig_primary_damage(loadout: MigLoadout) -> f32 {
    match loadout {
        MigLoadout::Standard | MigLoadout::BlackNapalm => MIG_PRIMARY_DAMAGE,
        MigLoadout::NukeBase => NUKE_MIG_PRIMARY_DAMAGE,
        MigLoadout::NukeTactical => NUKE_TACTICAL_PRIMARY_DAMAGE,
    }
}

/// Primary radius residual for loadout.
pub fn mig_primary_radius(loadout: MigLoadout) -> f32 {
    match loadout {
        MigLoadout::NukeTactical => NUKE_TACTICAL_PRIMARY_RADIUS,
        _ => MIG_PRIMARY_RADIUS,
    }
}

/// Secondary damage residual for loadout.
pub fn mig_secondary_damage(loadout: MigLoadout) -> f32 {
    match loadout {
        MigLoadout::Standard | MigLoadout::NukeBase => MIG_SECONDARY_DAMAGE,
        MigLoadout::BlackNapalm => MIG_BLACK_SECONDARY_DAMAGE,
        MigLoadout::NukeTactical => NUKE_TACTICAL_SECONDARY_DAMAGE,
    }
}

/// Secondary radius residual for loadout.
pub fn mig_secondary_radius(loadout: MigLoadout) -> f32 {
    match loadout {
        MigLoadout::NukeTactical => NUKE_TACTICAL_SECONDARY_RADIUS,
        _ => MIG_SECONDARY_RADIUS,
    }
}

/// Whether loadout seeds FireField residual (standard / black).
pub fn mig_spawns_fire_field(loadout: MigLoadout) -> bool {
    matches!(loadout, MigLoadout::Standard | MigLoadout::BlackNapalm)
}

/// Whether loadout uses upgraded FireField (BlackNapalm).
pub fn mig_fire_field_upgraded(loadout: MigLoadout) -> bool {
    matches!(loadout, MigLoadout::BlackNapalm)
}

/// Whether loadout seeds SmallRadiationField residual (Nuke chassis).
pub fn mig_spawns_radiation(loadout: MigLoadout) -> bool {
    matches!(loadout, MigLoadout::NukeBase | MigLoadout::NukeTactical)
}

/// Weapon template name residual for loadout.
pub fn mig_weapon_name(loadout: MigLoadout) -> &'static str {
    match loadout {
        MigLoadout::Standard => NAPALM_MISSILE_WEAPON,
        MigLoadout::BlackNapalm => BLACK_NAPALM_MISSILE_WEAPON,
        MigLoadout::NukeBase => NUKE_MIG_MISSILE_WEAPON,
        MigLoadout::NukeTactical => NUKE_NUKE_MISSILE_WEAPON,
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual MiG primary Weapon.
pub fn mig_weapon(loadout: MigLoadout) -> Weapon {
    Weapon {
        damage: mig_primary_damage(loadout),
        range: MIG_RANGE,
        min_range: MIG_MIN_RANGE,
        reload_time: delay_frames_to_reload_secs(MIG_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(MIG_CLIP_SIZE),
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: MIG_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// Dual-radius residual damage at distance from impact.
pub fn mig_damage_at(distance_from_impact: f32, loadout: MigLoadout) -> f32 {
    let primary_r = mig_primary_radius(loadout);
    let secondary_r = mig_secondary_radius(loadout);
    if distance_from_impact <= primary_r {
        mig_primary_damage(loadout)
    } else if distance_from_impact <= secondary_r {
        mig_secondary_damage(loadout)
    } else {
        0.0
    }
}

/// Legal residual splash / fire target.
pub fn is_legal_mig_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Apply Aircraft Armor residual: +AddMaxHealth current+max (ADD_CURRENT_HEALTH_TOO).
pub fn apply_mig_aircraft_armor_health(max_health: &mut f32, current: &mut f32, maximum: &mut f32) {
    *max_health = (*max_health + MIG_AIRCRAFT_ARMOR_ADD_MAX_HEALTH).max(0.0);
    *maximum = (*maximum + MIG_AIRCRAFT_ARMOR_ADD_MAX_HEALTH).max(0.0);
    *current = (*current + MIG_AIRCRAFT_ARMOR_ADD_MAX_HEALTH).max(0.0);
}

// --- Wave 67 residual honesty packs ---

/// Wave 67 residual honesty: MiG weapon / loadout residual peel.
pub fn honesty_mig_weapon_residual_ok() -> bool {
    NAPALM_MISSILE_WEAPON == "NapalmMissileWeapon"
        && BLACK_NAPALM_MISSILE_WEAPON == "BlackNapalmMissileWeapon"
        && (MIG_PRIMARY_DAMAGE - 75.0).abs() < 0.01
        && (MIG_PRIMARY_RADIUS - 5.0).abs() < 0.01
        && (MIG_SECONDARY_DAMAGE - 40.0).abs() < 0.01
        && (MIG_BLACK_SECONDARY_DAMAGE - 50.0).abs() < 0.01
        && (MIG_SECONDARY_RADIUS - 30.0).abs() < 0.01
        && (MIG_RANGE - 320.0).abs() < 0.01
        && (MIG_MIN_RANGE - 80.0).abs() < 0.01
        && MIG_DELAY_MS == 300
        && MIG_DELAY_FRAMES == mig_ms_to_frames(MIG_DELAY_MS)
        && MIG_DELAY_FRAMES == 9
        && MIG_CLIP_SIZE == 2
        && MIG_CLIP_RELOAD_MS == 8_000
        && MIG_CLIP_RELOAD_FRAMES == mig_ms_to_frames(MIG_CLIP_RELOAD_MS)
        && MIG_BLACK_CLIP_RELOAD_MS == 2_000
        && MIG_BLACK_CLIP_RELOAD_FRAMES == mig_ms_to_frames(MIG_BLACK_CLIP_RELOAD_MS)
        && MIG_DAMAGE_TYPE == "JET_MISSILES"
        && MIG_DEATH_TYPE == "BURNED"
        && MIG_BLACK_DAMAGE_TYPE == "EXPLOSION"
        && MIG_BLACK_DEATH_TYPE == "EXPLODED"
        && MIG_PROJECTILE == "NapalmMissile"
        && MIG_FIRE_FX == "WeaponFX_NapalmMissile"
        && MIG_AUTO_RELOADS_CLIP == "RETURN_TO_BASE"
        && MIG_FIRE_AUDIO == "MigJetNapalmWeapon"
        && (NUKE_MIG_PRIMARY_DAMAGE - 100.0).abs() < 0.01
        && (NUKE_TACTICAL_PRIMARY_DAMAGE - 150.0).abs() < 0.01
        && (NUKE_TACTICAL_PRIMARY_RADIUS - 50.0).abs() < 0.01
        && (NUKE_TACTICAL_SECONDARY_DAMAGE - 50.0).abs() < 0.01
        && (NUKE_TACTICAL_SECONDARY_RADIUS - 60.0).abs() < 0.01
        && {
            let w = mig_weapon(MigLoadout::Standard);
            (w.damage - 75.0).abs() < 0.01
                && w.can_target_air
                && w.ammo == Some(2)
                && mig_spawns_fire_field(MigLoadout::Standard)
                && mig_fire_field_upgraded(MigLoadout::BlackNapalm)
                && mig_spawns_radiation(MigLoadout::NukeBase)
        }
}

/// Wave 67 residual honesty: MiG body / aircraft-armor residual peel.
pub fn honesty_mig_body_residual_ok() -> bool {
    (MIG_MAX_HEALTH - 160.0).abs() < 0.01
        && (MIG_AIRCRAFT_ARMOR_ADD_MAX_HEALTH - 40.0).abs() < 0.01
        && MIG_AIRCRAFT_ARMOR_CHANGE_TYPE == "ADD_CURRENT_HEALTH_TOO"
        && UPGRADE_CHINA_AIRCRAFT_ARMOR == "Upgrade_ChinaAircraftArmor"
        && (MIG_VISION_RANGE - 200.0).abs() < 0.01
        && (MIG_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && MIG_BUILD_COST == 1_200
        && (MIG_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && MIG_BUILD_TIME_FRAMES == (MIG_BUILD_TIME_SEC * MIG_LOGIC_FPS).round() as u32
        && MIG_BUILD_TIME_FRAMES == 300
        && MIG_TRANSPORT_SLOT_COUNT == 0
        && (MIG_GEOMETRY_MAJOR - 14.0).abs() < 0.01
        && (MIG_GEOMETRY_MINOR - 7.0).abs() < 0.01
        && (MIG_GEOMETRY_HEIGHT - 5.0).abs() < 0.01
        && (MIG_LOCOMOTOR_SPEED - 160.0).abs() < 0.01
        && (MIG_LOCOMOTOR_MIN_SPEED - 60.0).abs() < 0.01
        && MIG_EXPERIENCE_VALUE == [50, 50, 100, 150]
        && MIG_EXPERIENCE_REQUIRED == [0, 100, 200, 400]
        && {
            let mut max_h = 160.0_f32;
            let mut cur = 100.0_f32;
            let mut maximum = 160.0_f32;
            apply_mig_aircraft_armor_health(&mut max_h, &mut cur, &mut maximum);
            (max_h - 200.0).abs() < 0.01 && (cur - 140.0).abs() < 0.01
        }
}

/// Combined Wave 67 MiG residual honesty pack.
pub fn honesty_mig_residual_pack_ok() -> bool {
    honesty_mig_weapon_residual_ok() && honesty_mig_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mig_name_matrix() {
        assert!(is_mig_template("ChinaJetMIG"));
        assert!(is_mig_template("China_MiG"));
        assert!(is_mig_template("TestMiG"));
        assert!(is_mig_template("Nuke_ChinaJetMIG"));
        assert!(is_mig_template("Tank_ChinaJetMIG"));
        assert!(is_mig_template("Infa_ChinaJetMIG"));
        assert!(is_mig_template("Boss_JetMIG"));
        assert!(is_nuke_mig_template("Nuke_ChinaJetMIG"));
        assert!(!is_nuke_mig_template("ChinaJetMIG"));
        assert!(!is_mig_template("NapalmMissileWeapon"));
        assert!(!is_mig_template("ChinaJetMIGHulk"));
        assert!(!is_mig_template("ChinaJetCargoPlane"));
        assert!(!is_mig_template("ChinaJetMIGNapalmStriker"));
        assert!(!is_mig_template("AmericaJetRaptor"));
    }

    #[test]
    fn loadout_and_dual_radius() {
        let mut tags = HashSet::new();
        assert_eq!(mig_loadout(false, &tags), MigLoadout::Standard);
        tags.insert(UPGRADE_CHINA_BLACK_NAPALM.to_string());
        assert_eq!(mig_loadout(false, &tags), MigLoadout::BlackNapalm);

        let mut nuke_tags = HashSet::new();
        assert_eq!(mig_loadout(true, &nuke_tags), MigLoadout::NukeBase);
        nuke_tags.insert(UPGRADE_CHINA_TACTICAL_NUKE_MIG.to_string());
        assert_eq!(mig_loadout(true, &nuke_tags), MigLoadout::NukeTactical);

        let w = mig_weapon(MigLoadout::Standard);
        assert!((w.damage - 75.0).abs() < 0.01);
        assert!((w.range - 320.0).abs() < 0.01);
        assert!((w.min_range - 80.0).abs() < 0.01);
        assert!((w.reload_time - 9.0 / 30.0).abs() < 0.02);
        assert!(w.can_target_air);

        assert!((mig_damage_at(0.0, MigLoadout::Standard) - 75.0).abs() < 0.01);
        assert!((mig_damage_at(4.0, MigLoadout::Standard) - 75.0).abs() < 0.01);
        assert!((mig_damage_at(15.0, MigLoadout::Standard) - 40.0).abs() < 0.01);
        assert!((mig_damage_at(15.0, MigLoadout::BlackNapalm) - 50.0).abs() < 0.01);
        assert!((mig_damage_at(40.0, MigLoadout::Standard)).abs() < 0.01);

        assert!((mig_damage_at(0.0, MigLoadout::NukeBase) - 100.0).abs() < 0.01);
        assert!((mig_damage_at(0.0, MigLoadout::NukeTactical) - 150.0).abs() < 0.01);
        assert!((mig_damage_at(55.0, MigLoadout::NukeTactical) - 50.0).abs() < 0.01);

        assert!(mig_spawns_fire_field(MigLoadout::Standard));
        assert!(mig_fire_field_upgraded(MigLoadout::BlackNapalm));
        assert!(mig_spawns_radiation(MigLoadout::NukeBase));
        assert!(!mig_spawns_fire_field(MigLoadout::NukeBase));
    }

    #[test]
    fn mig_residual_pack_honesty_wave67() {
        assert!(honesty_mig_weapon_residual_ok());
        assert!(honesty_mig_body_residual_ok());
        assert!(honesty_mig_residual_pack_ok());
        assert_eq!(mig_ms_to_frames(300), 9);
        assert_eq!(mig_ms_to_frames(8_000), 240);
        assert_eq!(MIG_BUILD_TIME_FRAMES, 300);
        assert_eq!(MIG_AUTO_RELOADS_CLIP, "RETURN_TO_BASE");
        assert_eq!(MIG_PROJECTILE, "NapalmMissile");
        assert!((MIG_AIRCRAFT_ARMOR_ADD_MAX_HEALTH - 40.0).abs() < 0.01);
    }
}
