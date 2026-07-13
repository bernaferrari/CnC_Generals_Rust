//! Host PointDefenseLaser intercept residual (Paladin / Avenger anti-missile laser).
//!
//! Residual slice (playability):
//! - Paladin / Avenger (and name residual variants) scan for interceptable
//!   missiles / projectiles in residual fire range and destroy them without a
//!   manual AttackObject order (C++ PointDefenseLaserUpdate scan + fire path).
//! - Retail Paladin: WeaponTemplate PaladinPointDefenseLaser, AttackRange 65,
//!   DelayBetweenShots 1000ms, PrimaryTargetTypes BALLISTIC_MISSILE SMALL_MISSILE,
//!   ScanRange 120. Avenger dual lasers: AttackRange 100, Delay 500ms each.
//!
//! Fail-closed honesty:
//! - Not full PointDefenseLaserUpdate velocity prediction / scan-rate matrix
//! - Not full KindOf mask Primary/SecondaryTargetTypes beyond residual names
//! - Not full laser beam drawable / FireFX particle attach bone path
//! - Not full TERTIARY WeaponStore allocateNewWeapon path
//! - Not network PDL replication (network deferred)

/// Retail PaladinPointDefenseLaser AttackRange residual.
pub const PALADIN_PDL_FIRE_RANGE: f32 = 65.0;

/// Retail Paladin PointDefenseLaserUpdate ScanRange residual.
pub const PALADIN_PDL_SCAN_RANGE: f32 = 120.0;

/// Retail PaladinPointDefenseLaser PrimaryDamage residual.
pub const PALADIN_PDL_DAMAGE: f32 = 100.0;

/// Retail PaladinPointDefenseLaser DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const PALADIN_PDL_DELAY_FRAMES: u32 = 30;

/// Retail AvengerPointDefenseLaser AttackRange residual.
pub const AVENGER_PDL_FIRE_RANGE: f32 = 100.0;

/// Retail Avenger dual-laser DelayBetweenShots 500ms → 15 frames.
/// Residual collapses two lasers into one fire stream with Avenger delay.
pub const AVENGER_PDL_DELAY_FRAMES: u32 = 15;

/// Retail AvengerPointDefenseLaser PrimaryDamage residual.
pub const AVENGER_PDL_DAMAGE: f32 = 100.0;

/// Activate / intercept audio residual (FXList WeaponFX_PaladinPointDefenseLaser).
pub const PDL_INTERCEPT_AUDIO: &str = "PaladinPointDefenseLaserPulse";

/// Whether template is a residual PointDefenseLaser carrier (Paladin / Avenger).
///
/// Fail-closed: name residual (not full INI PointDefenseLaserUpdate module matrix).
pub fn is_point_defense_carrier(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    // Explicit residual test names.
    if n == "testpaladin" || n == "testavenger" || n == "testpdl" {
        return true;
    }
    // AmericaTankPaladin / USA_Paladin / Lazr_AmericaTankPaladin / …
    if n.contains("paladin") {
        return true;
    }
    // AmericaVehicleAvenger / USA_Avenger / SupW_AmericaVehicleAvenger / …
    if n.contains("avenger") {
        return true;
    }
    false
}

/// True when residual carrier uses Avenger dual-laser stats (range/delay).
pub fn is_avenger_carrier(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("avenger") || n == "testavenger"
}

/// Residual fire range for a PDL carrier.
pub fn pdl_fire_range(template_name: &str) -> f32 {
    if is_avenger_carrier(template_name) {
        AVENGER_PDL_FIRE_RANGE
    } else {
        PALADIN_PDL_FIRE_RANGE
    }
}

/// Residual scan range (slightly larger than fire range; Paladin retail 120).
pub fn pdl_scan_range(template_name: &str) -> f32 {
    if is_avenger_carrier(template_name) {
        // Avenger has no separate ScanRange in residual; use fire range * 1.2.
        AVENGER_PDL_FIRE_RANGE * 1.2
    } else {
        PALADIN_PDL_SCAN_RANGE
    }
}

/// Residual damage per intercept shot.
pub fn pdl_damage(template_name: &str) -> f32 {
    if is_avenger_carrier(template_name) {
        AVENGER_PDL_DAMAGE
    } else {
        PALADIN_PDL_DAMAGE
    }
}

/// Residual reload delay in logic frames.
pub fn pdl_delay_frames(template_name: &str) -> u32 {
    if is_avenger_carrier(template_name) {
        AVENGER_PDL_DELAY_FRAMES
    } else {
        PALADIN_PDL_DELAY_FRAMES
    }
}

/// Whether residual target is a primary intercept candidate (missile / projectile).
///
/// Retail PrimaryTargetTypes = BALLISTIC_MISSILE SMALL_MISSILE.
/// Fail-closed: KindOf::Projectile + name residual (missile/rocket/scud/tomahawk).
pub fn is_primary_intercept_target(
    is_projectile_kind: bool,
    is_alive: bool,
    same_team: bool,
    template_name: &str,
) -> bool {
    if !is_alive || same_team {
        return false;
    }
    if is_projectile_kind {
        return true;
    }
    is_missile_name_residual(template_name)
}

/// Name residual for ballistic / small missiles without KindOf::Projectile.
pub fn is_missile_name_residual(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    // Avoid false positives: MissileDefender infantry, Patriot Battery structure,
    // Stinger Site structure. Projectile names like PatriotMissileProjectile stay true.
    if n.contains("missiledefender")
        || n.contains("stingersite")
        || n.contains("stinger_site")
        || (n.contains("patriot") && !n.contains("projectile") && !n.contains("missileprojectile"))
    {
        return false;
    }
    // Pure structure batteries named "Patriot" / "PatriotBattery".
    if n == "usa_patriot" || n.ends_with("patriotbattery") || n.ends_with("patriotmissilesystem") {
        return false;
    }
    n.contains("missile")
        || n.contains("rocket")
        || n.contains("scud")
        || n.contains("tomahawk")
        || n.contains("cruise")
        || n == "testmissile"
        || n == "testprojectile"
        || (n.ends_with("shell")
            && (n.contains("nuke") || n.contains("scud") || n.contains("artillery")))
}

/// Secondary residual target (Paladin SecondaryTargetTypes = INFANTRY).
/// Lower priority than missiles; residual damages infantry in fire range.
pub fn is_secondary_intercept_target(
    is_infantry: bool,
    is_alive: bool,
    same_team: bool,
    under_construction: bool,
) -> bool {
    is_infantry && is_alive && !same_team && !under_construction
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_pdl_range_2d(carrier_pos: (f32, f32), target_pos: (f32, f32), range: f32) -> bool {
    let dx = carrier_pos.0 - target_pos.0;
    let dz = carrier_pos.1 - target_pos.1;
    dx * dx + dz * dz <= range * range
}

/// Priority score: lower is better. Primary missiles = 0, secondary infantry = 1.
pub fn intercept_priority(
    is_primary: bool,
    is_secondary: bool,
) -> Option<u8> {
    if is_primary {
        Some(0)
    } else if is_secondary {
        Some(1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdl_carrier_name_matrix() {
        assert!(is_point_defense_carrier("USA_Paladin"));
        assert!(is_point_defense_carrier("AmericaTankPaladin"));
        assert!(is_point_defense_carrier("Lazr_AmericaTankPaladin"));
        assert!(is_point_defense_carrier("TestPaladin"));
        assert!(is_point_defense_carrier("USA_Avenger"));
        assert!(is_point_defense_carrier("AmericaVehicleAvenger"));
        assert!(is_point_defense_carrier("TestAvenger"));
        assert!(!is_point_defense_carrier("USA_Ranger"));
        assert!(!is_point_defense_carrier("USA_Patriot"));
        assert!(!is_point_defense_carrier("ChinaTankBattleMaster"));
        assert!(!is_point_defense_carrier("TestTank"));
    }

    #[test]
    fn avenger_vs_paladin_stats() {
        assert!((pdl_fire_range("USA_Paladin") - PALADIN_PDL_FIRE_RANGE).abs() < 0.01);
        assert!((pdl_fire_range("USA_Avenger") - AVENGER_PDL_FIRE_RANGE).abs() < 0.01);
        assert_eq!(pdl_delay_frames("USA_Paladin"), PALADIN_PDL_DELAY_FRAMES);
        assert_eq!(pdl_delay_frames("USA_Avenger"), AVENGER_PDL_DELAY_FRAMES);
        assert!(is_avenger_carrier("AmericaVehicleAvenger"));
        assert!(!is_avenger_carrier("AmericaTankPaladin"));
    }

    #[test]
    fn missile_name_residual_matrix() {
        assert!(is_missile_name_residual("ScudMissile"));
        assert!(is_missile_name_residual("TomahawkMissile"));
        assert!(is_missile_name_residual("TestMissile"));
        assert!(is_missile_name_residual("TestProjectile"));
        assert!(is_missile_name_residual("PatriotMissileProjectile"));
        assert!(!is_missile_name_residual("AmericaInfantryMissileDefender"));
        assert!(!is_missile_name_residual("USA_Patriot"));
        assert!(!is_missile_name_residual("GLA_StingerSite"));
        assert!(!is_missile_name_residual("USA_Ranger"));
    }

    #[test]
    fn primary_secondary_intercept_matrix() {
        assert!(is_primary_intercept_target(true, true, false, "Anything"));
        assert!(is_primary_intercept_target(
            false,
            true,
            false,
            "ScudMissile"
        ));
        assert!(!is_primary_intercept_target(
            false,
            true,
            true,
            "ScudMissile"
        ));
        assert!(!is_primary_intercept_target(false, false, false, "ScudMissile"));
        assert!(is_secondary_intercept_target(true, true, false, false));
        assert!(!is_secondary_intercept_target(true, true, true, false));
        assert!(!is_secondary_intercept_target(false, true, false, false));
    }

    #[test]
    fn range_and_priority() {
        assert!(in_pdl_range_2d((0.0, 0.0), (50.0, 0.0), 65.0));
        assert!(!in_pdl_range_2d((0.0, 0.0), (80.0, 0.0), 65.0));
        assert_eq!(intercept_priority(true, true), Some(0));
        assert_eq!(intercept_priority(false, true), Some(1));
        assert_eq!(intercept_priority(false, false), None);
    }
}
