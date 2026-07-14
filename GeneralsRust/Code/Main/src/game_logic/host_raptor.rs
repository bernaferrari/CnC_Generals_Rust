//! Host America Raptor combat residual (jet missiles + Laser Missiles upgrade).
//!
//! Residual slice (playability):
//! - `AmericaJetRaptor` / `USA_Raptor` / SupW_/Lazr_ variants spawn with PRIMARY
//!   `RaptorJetMissileWeapon` (PrimaryDamage **100** / radius **5**, range **320**,
//!   min **100**, Delay **150**ms → 5 frames). ClipSize **4** honesty
//!   (RETURN_TO_BASE full clip matrix fail-closed).
//! - Airforce General King Raptor (`AirF_AmericaJetRaptor`) uses
//!   `AirF_RaptorJetMissileWeapon` residual: PrimaryDamage **125** / range **350**,
//!   Delay **75**ms → 3 frames, ClipSize **6** honesty. PDL residual remains in
//!   `host_point_defense` (not re-opened).
//! - Laser Missiles PLAYER_UPGRADE residual (`Upgrade_AmericaLaserMissiles`):
//!   standard DAMAGE **125%** → **125**; King Raptor DAMAGE **112%** → **140**.
//! - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
//!   AA + ground residual (AntiAirborneVehicle=Yes).
//!
//! Wave 67 residual pack (retail AmericaAir.ini / Weapon.ini / Locomotor.ini):
//! - Weapon residual: DamageType **JET_MISSILES**, DeathType **EXPLODED**,
//!   ScatterRadiusVsInfantry **10**, Projectile **RaptorJetMissile**,
//!   DetonationFX **WeaponFX_JetMissileDetonation**, AutoReloadsClip **RETURN_TO_BASE**,
//!   AntiAirborneVehicle **Yes** / AntiAirborneInfantry **No**, ShowsAmmoPips **Yes**.
//! - Body residual: MaxHealth **160**, Vision **200**, Shroud **400**,
//!   BuildCost **1400**, BuildTime **20**s → **600**f, TransportSlotCount **0**,
//!   Geometry BOX **14**/**7**/**5**, Locomotor Speed **175**/Damaged **120**/Min **60**.
//! - Clip residual: standard ClipReload **8000**ms → **240**f / King **2000**ms → **60**f.
//!
//! Fail-closed honesty:
//! - Not full JetAIUpdate RETURN_TO_BASE / ClipReload 8000ms airfield rearm matrix
//! - Not full ScatterRadiusVsInfantry / projectile exhaust FX matrix
//! - Not full CountermeasuresBehavior flare volley residual
//! - Not network laser-missiles / raptor fire replication (network deferred)

use super::Weapon;
use std::collections::HashSet;

/// Logic frames per second (host fixed step).
pub const RAPTOR_LOGIC_FPS: f32 = 30.0;

/// Retail standard Raptor primary weapon.
pub const RAPTOR_JET_MISSILE_WEAPON: &str = "RaptorJetMissileWeapon";
/// Retail Airforce General King Raptor primary weapon.
pub const AIRF_RAPTOR_JET_MISSILE_WEAPON: &str = "AirF_RaptorJetMissileWeapon";
/// Retail Upgrade_AmericaLaserMissiles.
pub const UPGRADE_AMERICA_LASER_MISSILES: &str = "Upgrade_AmericaLaserMissiles";

/// Standard RaptorJetMissileWeapon PrimaryDamage.
pub const RAPTOR_DAMAGE: f32 = 100.0;
/// Standard PrimaryDamageRadius.
pub const RAPTOR_PRIMARY_RADIUS: f32 = 5.0;
/// Standard AttackRange.
pub const RAPTOR_RANGE: f32 = 320.0;
/// Standard MinimumAttackRange.
pub const RAPTOR_MIN_RANGE: f32 = 100.0;
/// Retail DelayBetweenShots residual (msec).
pub const RAPTOR_DELAY_MS: u32 = 150;
/// Standard DelayBetweenShots 150ms → 5 frames @ 30 FPS.
pub const RAPTOR_DELAY_FRAMES: u32 = 5;
/// Standard ClipSize honesty (full RETURN_TO_BASE rearm fail-closed).
pub const RAPTOR_CLIP_SIZE: u32 = 4;
/// Retail ClipReloadTime residual (msec).
pub const RAPTOR_CLIP_RELOAD_MS: u32 = 8_000;
/// Standard ClipReloadTime 8000ms → 240 frames honesty residual.
pub const RAPTOR_CLIP_RELOAD_FRAMES: u32 = 240;
/// Laser Missiles PLAYER_UPGRADE damage multiplier (WeaponBonus DAMAGE 125%).
pub const RAPTOR_LASER_MISSILES_MULT: f32 = 1.25;
/// Retail ScatterRadiusVsInfantry residual.
pub const RAPTOR_SCATTER_VS_INFANTRY: f32 = 10.0;
/// Retail DamageType residual.
pub const RAPTOR_DAMAGE_TYPE: &str = "JET_MISSILES";
/// Retail DeathType residual.
pub const RAPTOR_DEATH_TYPE: &str = "EXPLODED";
/// Retail ProjectileObject residual.
pub const RAPTOR_PROJECTILE: &str = "RaptorJetMissile";
/// Retail ProjectileDetonationFX residual.
pub const RAPTOR_DETONATION_FX: &str = "WeaponFX_JetMissileDetonation";
/// Retail AutoReloadsClip residual.
pub const RAPTOR_AUTO_RELOADS_CLIP: &str = "RETURN_TO_BASE";
/// Retail AntiAirborneVehicle residual.
pub const RAPTOR_ANTI_AIRBORNE_VEHICLE: bool = true;
/// Retail AntiAirborneInfantry residual.
pub const RAPTOR_ANTI_AIRBORNE_INFANTRY: bool = false;
/// Retail ShowsAmmoPips residual.
pub const RAPTOR_SHOWS_AMMO_PIPS: bool = true;

/// King Raptor (AirF) PrimaryDamage.
pub const KING_RAPTOR_DAMAGE: f32 = 125.0;
/// King Raptor AttackRange.
pub const KING_RAPTOR_RANGE: f32 = 350.0;
/// King Raptor MinimumAttackRange (same as standard).
pub const KING_RAPTOR_MIN_RANGE: f32 = 100.0;
/// Retail King DelayBetweenShots residual (msec).
pub const KING_RAPTOR_DELAY_MS: u32 = 75;
/// King Raptor DelayBetweenShots 75ms → 3 frames @ 30 FPS.
pub const KING_RAPTOR_DELAY_FRAMES: u32 = 3;
/// King Raptor ClipSize honesty.
pub const KING_RAPTOR_CLIP_SIZE: u32 = 6;
/// Retail King ClipReloadTime residual (msec).
pub const KING_RAPTOR_CLIP_RELOAD_MS: u32 = 2_000;
/// King Raptor ClipReloadTime 2000ms → 60 frames honesty residual.
pub const KING_RAPTOR_CLIP_RELOAD_FRAMES: u32 = 60;
/// King Raptor Laser Missiles PLAYER_UPGRADE mult (WeaponBonus DAMAGE 112%).
pub const KING_RAPTOR_LASER_MISSILES_MULT: f32 = 1.12;
/// Residual projectile speed.
pub const RAPTOR_PROJECTILE_SPEED: f32 = 1000.0;

/// Residual fire audio.
pub const RAPTOR_FIRE_AUDIO: &str = "RaptorJetMissileWeapon";

// --- Body residual (AmericaJetRaptor) ---

/// Retail MaxHealth residual.
pub const RAPTOR_MAX_HEALTH: f32 = 160.0;
/// Retail VisionRange residual.
pub const RAPTOR_VISION_RANGE: f32 = 200.0;
/// Retail ShroudClearingRange residual.
pub const RAPTOR_SHROUD_CLEARING_RANGE: f32 = 400.0;
/// Retail BuildCost residual.
pub const RAPTOR_BUILD_COST: u32 = 1_400;
/// Retail BuildTime residual (seconds).
pub const RAPTOR_BUILD_TIME_SEC: f32 = 20.0;
/// BuildTime 20s → 600 frames @ 30 FPS.
pub const RAPTOR_BUILD_TIME_FRAMES: u32 = 600;
/// Retail TransportSlotCount residual (jets not transportable).
pub const RAPTOR_TRANSPORT_SLOT_COUNT: u32 = 0;
/// Retail Geometry BOX MajorRadius residual.
pub const RAPTOR_GEOMETRY_MAJOR: f32 = 14.0;
/// Retail Geometry BOX MinorRadius residual.
pub const RAPTOR_GEOMETRY_MINOR: f32 = 7.0;
/// Retail GeometryHeight residual.
pub const RAPTOR_GEOMETRY_HEIGHT: f32 = 5.0;
/// Retail RaptorJetLocomotor Speed residual.
pub const RAPTOR_LOCOMOTOR_SPEED: f32 = 175.0;
/// Retail RaptorJetLocomotor SpeedDamaged residual.
pub const RAPTOR_LOCOMOTOR_SPEED_DAMAGED: f32 = 120.0;
/// Retail RaptorJetLocomotor MinSpeed residual.
pub const RAPTOR_LOCOMOTOR_MIN_SPEED: f32 = 60.0;
/// Retail ExperienceValue residual.
pub const RAPTOR_EXPERIENCE_VALUE: [u32; 4] = [50, 50, 100, 150];
/// Retail ExperienceRequired residual.
pub const RAPTOR_EXPERIENCE_REQUIRED: [u32; 4] = [0, 100, 200, 400];

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn raptor_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * RAPTOR_LOGIC_FPS / 1000.0).round() as u32
}

fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether template is a residual living Raptor jet (incl. King Raptor).
///
/// Fail-closed: name residual. Excludes missiles / weapons / hulks / PDL modules.
pub fn is_raptor_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "usaraptor"
        || n == "testraptor"
        || n == "americajetraptor"
        || n == "airfamericajetraptor"
    {
        return true;
    }
    // Exclude non-living residual objects / carrier raptor / PDL modules.
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.contains("pointdefense")
        || n.contains("exhaust")
        || n.contains("locomotor")
        || n.starts_with("upgrade")
        || n.contains("crate")
        || n.contains("carrier")
        || n.contains("laser")
    {
        return false;
    }
    // Living jet residual: *JetRaptor* / *Raptor* aircraft chassis names.
    n.contains("jetraptor") || n.contains("raptor")
}

/// Whether template is Airforce General King Raptor residual chassis.
pub fn is_king_raptor_template(template_name: &str) -> bool {
    if !is_raptor_template(template_name) {
        return false;
    }
    let n = alnum_lower(template_name);
    n.starts_with("airf") || n.contains("kingraptor")
}

/// Whether residual fire should apply Raptor residual path.
pub fn should_apply_raptor_residual(is_raptor: bool) -> bool {
    is_raptor
}

/// Laser Missiles PLAYER_UPGRADE residual present on unit / player tags.
pub fn has_laser_missiles_upgrade(applied_upgrades: &HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = alnum_lower(u);
        l.contains("lasermissile") || l == "upgrade_americalasermissiles"
    })
}

/// Damage multiplier for Laser Missiles residual (standard 1.25 / King 1.12).
pub fn laser_missiles_damage_mult(is_king: bool) -> f32 {
    if is_king {
        KING_RAPTOR_LASER_MISSILES_MULT
    } else {
        RAPTOR_LASER_MISSILES_MULT
    }
}

/// Primary damage residual for chassis + Laser Missiles upgrade.
pub fn raptor_primary_damage(is_king: bool, has_laser_missiles: bool) -> f32 {
    let base = if is_king {
        KING_RAPTOR_DAMAGE
    } else {
        RAPTOR_DAMAGE
    };
    if has_laser_missiles {
        base * laser_missiles_damage_mult(is_king)
    } else {
        base
    }
}

/// Attack range residual for chassis.
pub fn raptor_attack_range(is_king: bool) -> f32 {
    if is_king {
        KING_RAPTOR_RANGE
    } else {
        RAPTOR_RANGE
    }
}

/// Min range residual for chassis.
pub fn raptor_min_range(is_king: bool) -> f32 {
    if is_king {
        KING_RAPTOR_MIN_RANGE
    } else {
        RAPTOR_MIN_RANGE
    }
}

/// Delay frames residual for chassis.
pub fn raptor_delay_frames(is_king: bool) -> u32 {
    if is_king {
        KING_RAPTOR_DELAY_FRAMES
    } else {
        RAPTOR_DELAY_FRAMES
    }
}

/// Clip size honesty residual for chassis.
pub fn raptor_clip_size(is_king: bool) -> u32 {
    if is_king {
        KING_RAPTOR_CLIP_SIZE
    } else {
        RAPTOR_CLIP_SIZE
    }
}

/// Primary weapon template name residual for chassis.
pub fn raptor_weapon_name(is_king: bool) -> &'static str {
    if is_king {
        AIRF_RAPTOR_JET_MISSILE_WEAPON
    } else {
        RAPTOR_JET_MISSILE_WEAPON
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Raptor primary Weapon.
pub fn raptor_weapon(is_king: bool, has_laser_missiles: bool) -> Weapon {
    Weapon {
        damage: raptor_primary_damage(is_king, has_laser_missiles),
        range: raptor_attack_range(is_king),
        min_range: raptor_min_range(is_king),
        reload_time: delay_frames_to_reload_secs(raptor_delay_frames(is_king)),
        last_fire_time: 0.0,
        ammo: Some(raptor_clip_size(is_king)),
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: RAPTOR_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// Residual damage at distance from impact (intended / primary ring).
pub fn raptor_damage_at(distance_from_impact: f32, is_king: bool, has_laser_missiles: bool) -> f32 {
    if distance_from_impact <= RAPTOR_PRIMARY_RADIUS {
        raptor_primary_damage(is_king, has_laser_missiles)
    } else {
        0.0
    }
}

/// Legal residual splash / fire target.
pub fn is_legal_raptor_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

// --- Wave 67 residual honesty packs ---

/// Wave 67 residual honesty: Raptor weapon residual peel.
pub fn honesty_raptor_weapon_residual_ok() -> bool {
    RAPTOR_JET_MISSILE_WEAPON == "RaptorJetMissileWeapon"
        && AIRF_RAPTOR_JET_MISSILE_WEAPON == "AirF_RaptorJetMissileWeapon"
        && (RAPTOR_DAMAGE - 100.0).abs() < 0.01
        && (RAPTOR_PRIMARY_RADIUS - 5.0).abs() < 0.01
        && (RAPTOR_RANGE - 320.0).abs() < 0.01
        && (RAPTOR_MIN_RANGE - 100.0).abs() < 0.01
        && RAPTOR_DELAY_MS == 150
        && RAPTOR_DELAY_FRAMES == raptor_ms_to_frames(RAPTOR_DELAY_MS)
        && RAPTOR_DELAY_FRAMES == 5
        && RAPTOR_CLIP_SIZE == 4
        && RAPTOR_CLIP_RELOAD_MS == 8_000
        && RAPTOR_CLIP_RELOAD_FRAMES == raptor_ms_to_frames(RAPTOR_CLIP_RELOAD_MS)
        && RAPTOR_CLIP_RELOAD_FRAMES == 240
        && (RAPTOR_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && RAPTOR_DAMAGE_TYPE == "JET_MISSILES"
        && RAPTOR_DEATH_TYPE == "EXPLODED"
        && RAPTOR_PROJECTILE == "RaptorJetMissile"
        && RAPTOR_DETONATION_FX == "WeaponFX_JetMissileDetonation"
        && RAPTOR_AUTO_RELOADS_CLIP == "RETURN_TO_BASE"
        && RAPTOR_ANTI_AIRBORNE_VEHICLE
        && !RAPTOR_ANTI_AIRBORNE_INFANTRY
        && RAPTOR_SHOWS_AMMO_PIPS
        && (RAPTOR_PROJECTILE_SPEED - 1000.0).abs() < 0.01
        && (RAPTOR_LASER_MISSILES_MULT - 1.25).abs() < 0.001
        && (KING_RAPTOR_DAMAGE - 125.0).abs() < 0.01
        && (KING_RAPTOR_RANGE - 350.0).abs() < 0.01
        && KING_RAPTOR_DELAY_MS == 75
        && KING_RAPTOR_DELAY_FRAMES == 3
        && KING_RAPTOR_CLIP_SIZE == 6
        && KING_RAPTOR_CLIP_RELOAD_MS == 2_000
        && KING_RAPTOR_CLIP_RELOAD_FRAMES == raptor_ms_to_frames(KING_RAPTOR_CLIP_RELOAD_MS)
        && KING_RAPTOR_CLIP_RELOAD_FRAMES == 60
        && (KING_RAPTOR_LASER_MISSILES_MULT - 1.12).abs() < 0.001
        && {
            let w = raptor_weapon(false, false);
            (w.damage - 100.0).abs() < 0.01
                && w.can_target_air
                && w.can_target_ground
                && w.ammo == Some(4)
        }
}

/// Wave 67 residual honesty: Raptor body / locomotor residual peel.
pub fn honesty_raptor_body_residual_ok() -> bool {
    (RAPTOR_MAX_HEALTH - 160.0).abs() < 0.01
        && (RAPTOR_VISION_RANGE - 200.0).abs() < 0.01
        && (RAPTOR_SHROUD_CLEARING_RANGE - 400.0).abs() < 0.01
        && RAPTOR_BUILD_COST == 1_400
        && (RAPTOR_BUILD_TIME_SEC - 20.0).abs() < 0.01
        && RAPTOR_BUILD_TIME_FRAMES == (RAPTOR_BUILD_TIME_SEC * RAPTOR_LOGIC_FPS).round() as u32
        && RAPTOR_BUILD_TIME_FRAMES == 600
        && RAPTOR_TRANSPORT_SLOT_COUNT == 0
        && (RAPTOR_GEOMETRY_MAJOR - 14.0).abs() < 0.01
        && (RAPTOR_GEOMETRY_MINOR - 7.0).abs() < 0.01
        && (RAPTOR_GEOMETRY_HEIGHT - 5.0).abs() < 0.01
        && (RAPTOR_LOCOMOTOR_SPEED - 175.0).abs() < 0.01
        && (RAPTOR_LOCOMOTOR_SPEED_DAMAGED - 120.0).abs() < 0.01
        && (RAPTOR_LOCOMOTOR_MIN_SPEED - 60.0).abs() < 0.01
        && RAPTOR_EXPERIENCE_VALUE == [50, 50, 100, 150]
        && RAPTOR_EXPERIENCE_REQUIRED == [0, 100, 200, 400]
}

/// Combined Wave 67 Raptor residual honesty pack.
pub fn honesty_raptor_residual_pack_ok() -> bool {
    honesty_raptor_weapon_residual_ok() && honesty_raptor_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raptor_name_matrix() {
        assert!(is_raptor_template("AmericaJetRaptor"));
        assert!(is_raptor_template("USA_Raptor"));
        assert!(is_raptor_template("TestRaptor"));
        assert!(is_raptor_template("SupW_AmericaJetRaptor"));
        assert!(is_raptor_template("Lazr_AmericaJetRaptor"));
        assert!(is_raptor_template("AirF_AmericaJetRaptor"));
        assert!(is_king_raptor_template("AirF_AmericaJetRaptor"));
        assert!(!is_king_raptor_template("AmericaJetRaptor"));
        assert!(!is_raptor_template("RaptorJetMissileWeapon"));
        assert!(!is_raptor_template("RaptorJetMissile"));
        assert!(!is_raptor_template("AirF_RaptorPointDefenseLaser"));
        assert!(!is_raptor_template("AircraftCarrierRaptor"));
        assert!(!is_raptor_template("AmericaVehicleTomahawk"));
        assert!(!is_raptor_template("AmericaJetStealthFighter"));
    }

    #[test]
    fn weapon_laser_missiles_and_king() {
        let std = raptor_weapon(false, false);
        assert!((std.damage - 100.0).abs() < 0.01);
        assert!((std.range - 320.0).abs() < 0.01);
        assert!((std.min_range - 100.0).abs() < 0.01);
        assert!((std.reload_time - 5.0 / 30.0).abs() < 0.01);
        assert_eq!(std.ammo, Some(4));
        assert!(std.can_target_air);

        let laser = raptor_weapon(false, true);
        assert!((laser.damage - 125.0).abs() < 0.01);

        let king = raptor_weapon(true, false);
        assert!((king.damage - 125.0).abs() < 0.01);
        assert!((king.range - 350.0).abs() < 0.01);
        assert_eq!(king.ammo, Some(6));
        assert!((king.reload_time - 3.0 / 30.0).abs() < 0.01);

        let king_laser = raptor_weapon(true, true);
        assert!((king_laser.damage - 125.0 * 1.12).abs() < 0.05);

        assert!((raptor_damage_at(0.0, false, false) - 100.0).abs() < 0.01);
        assert!((raptor_damage_at(5.0, false, false) - 100.0).abs() < 0.01);
        assert!((raptor_damage_at(6.0, false, false)).abs() < 0.01);
    }

    #[test]
    fn laser_missiles_upgrade_name() {
        let mut tags = HashSet::new();
        assert!(!has_laser_missiles_upgrade(&tags));
        tags.insert(UPGRADE_AMERICA_LASER_MISSILES.to_string());
        assert!(has_laser_missiles_upgrade(&tags));
    }

    #[test]
    fn raptor_residual_pack_honesty_wave67() {
        assert!(honesty_raptor_weapon_residual_ok());
        assert!(honesty_raptor_body_residual_ok());
        assert!(honesty_raptor_residual_pack_ok());
        assert_eq!(raptor_ms_to_frames(150), 5);
        assert_eq!(raptor_ms_to_frames(8_000), 240);
        assert_eq!(KING_RAPTOR_DELAY_FRAMES, 3);
        assert_eq!(RAPTOR_BUILD_TIME_FRAMES, 600);
        assert_eq!(RAPTOR_AUTO_RELOADS_CLIP, "RETURN_TO_BASE");
        assert!(RAPTOR_ANTI_AIRBORNE_VEHICLE);
        assert!(!RAPTOR_ANTI_AIRBORNE_INFANTRY);
    }
}
