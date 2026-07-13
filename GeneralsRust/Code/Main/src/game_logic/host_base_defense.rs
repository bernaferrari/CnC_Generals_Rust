//! Host base-defense structure residual (Patriot / Gattling / Stinger auto-fire).
//!
//! Residual slice (playability):
//! - Base defenses (USA Patriot, China Gattling Cannon, GLA Stinger Site, and
//!   `FSBaseDefense` structures) auto-acquire and damage nearby enemies while
//!   Idle without a manual `AttackObject` / player attack order.
//! - Retail weapon names: `PatriotMissileWeapon` (dmg 30, range 225) and
//!   `GattlingBuildingGun` (dmg 10, range 225).
//! - C++ `AIUpdateInterface` AutoAcquireEnemiesWhenIdle residual for stationary
//!   base defenses (not full turret pitch / continuous-fire matrix / LOS).
//!
//! Fail-closed honesty:
//! - Not full WeaponSet PRIMARY/SECONDARY/TERTIARY anti-air chooser
//! - Not full PointDefenseLaserUpdate missile intercept matrix
//! - Not full AssistedTargetingModule Patriot assist clips
//! - Not full continuous-fire rate ramp / ChainGun upgrade bonuses
//! - Not network base-defense replication (network deferred)

/// Retail Patriot primary weapon template name.
pub const PATRIOT_PRIMARY_WEAPON: &str = "PatriotMissileWeapon";

/// Retail China Gattling Cannon primary weapon template name.
pub const GATTLING_BUILDING_PRIMARY_WEAPON: &str = "GattlingBuildingGun";

/// Whether template is a residual base-defense structure that should auto-fire.
///
/// Fail-closed: name + FSBaseDefense kind residual (not full INI module matrix).
/// Excludes Overlord/Helix/tank-mounted gattling payloads (not structures).
pub fn is_base_defense_structure(
    template_name: &str,
    is_structure: bool,
    is_fs_base_defense: bool,
) -> bool {
    if is_fs_base_defense {
        return true;
    }
    if !is_structure {
        return false;
    }
    let n = template_name.to_ascii_lowercase();
    // Vehicle/portable gattling payloads are not structure base defenses.
    if n.contains("overlord") || n.contains("helix") || n.contains("tank") || n.contains("gunship")
    {
        return false;
    }
    n.contains("patriot")
        || n.contains("gattlingcannon")
        || n.contains("gattling_cannon")
        || n.contains("stingersite")
        || n.contains("stinger_site")
        || n.contains("basedefense")
        || n.contains("base_defense")
        || n.contains("firebase")
}

/// Retail-ish residual weapon name for known host base-defense templates.
pub fn primary_weapon_name_for_defense(template_name: &str) -> Option<&'static str> {
    let n = template_name.to_ascii_lowercase();
    if n.contains("patriot") {
        Some(PATRIOT_PRIMARY_WEAPON)
    } else if n.contains("gattling") {
        Some(GATTLING_BUILDING_PRIMARY_WEAPON)
    } else {
        None
    }
}

/// Legal residual target for base-defense auto-fire.
pub fn is_legal_base_defense_target(
    is_alive: bool,
    same_team: bool,
    is_neutral: bool,
    under_construction: bool,
    is_attackable_or_combat_kind: bool,
) -> bool {
    is_alive
        && !same_team
        && !is_neutral
        && !under_construction
        && is_attackable_or_combat_kind
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_defense_name_matrix() {
        assert!(is_base_defense_structure("USA_Patriot", true, false));
        assert!(is_base_defense_structure("AmericaPatriotBattery", true, false));
        assert!(is_base_defense_structure("Lazr_PatriotMissileSystem", true, false));
        assert!(is_base_defense_structure("China_GattlingCannon", true, false));
        assert!(is_base_defense_structure("ChinaGattlingCannon", true, false));
        assert!(is_base_defense_structure("GLA_StingerSite", true, false));
        assert!(is_base_defense_structure("AnyTower", true, true));
        assert!(!is_base_defense_structure("USA_Barracks", true, false));
        assert!(!is_base_defense_structure("USA_Ranger", false, false));
        assert!(!is_base_defense_structure(
            "ChinaTankOverlordGattlingCannon",
            false,
            false
        ));
        assert!(!is_base_defense_structure(
            "ChinaHelixGattlingCannon",
            false,
            false
        ));
        assert!(!is_base_defense_structure("USA_SupplyCenter", true, false));
    }

    #[test]
    fn defense_weapon_name_lookup() {
        assert_eq!(
            primary_weapon_name_for_defense("USA_Patriot"),
            Some(PATRIOT_PRIMARY_WEAPON)
        );
        assert_eq!(
            primary_weapon_name_for_defense("China_GattlingCannon"),
            Some(GATTLING_BUILDING_PRIMARY_WEAPON)
        );
        assert_eq!(primary_weapon_name_for_defense("GLA_StingerSite"), None);
        assert_eq!(primary_weapon_name_for_defense("USA_Ranger"), None);
    }

    #[test]
    fn legal_target_matrix() {
        assert!(is_legal_base_defense_target(true, false, false, false, true));
        assert!(!is_legal_base_defense_target(false, false, false, false, true));
        assert!(!is_legal_base_defense_target(true, true, false, false, true));
        assert!(!is_legal_base_defense_target(true, false, true, false, true));
        assert!(!is_legal_base_defense_target(true, false, false, true, true));
        assert!(!is_legal_base_defense_target(true, false, false, false, false));
    }
}
