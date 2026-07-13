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
//! Fail-closed honesty:
//! - Not full JetAIUpdate RETURN_TO_BASE / ClipReload airfield rearm matrix
//! - Not full HistoricBonus FirestormSmallCreationWeapon multi-missile matrix
//! - Not full MediumRadiationField for Nuke_NukeMissileWeapon residual
//! - Not network MiG / BlackNapalm / TacticalNuke replication (network deferred)

use super::Weapon;
use std::collections::HashSet;

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
/// DelayBetweenShots 300ms → 9 frames @ 30 FPS.
pub const MIG_DELAY_FRAMES: u32 = 9;
/// ClipSize honesty (RETURN_TO_BASE rearm fail-closed).
pub const MIG_CLIP_SIZE: u32 = 2;
/// ClipReloadTime 8000ms → 240 frames honesty residual.
pub const MIG_CLIP_RELOAD_FRAMES: u32 = 240;

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
pub fn mig_loadout(
    is_nuke_chassis: bool,
    applied_upgrades: &HashSet<String>,
) -> MigLoadout {
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
}
