//! Host FireWeaponWhenDeadBehavior residual (death weapon AOE on die).
//!
//! C++: when upgrade active and die mux applies, createAndFireTempWeapon at
//! object position. Skips UNDER_CONSTRUCTION and conflicting upgrades.
//!
//! Residual playability slice:
//! - Template peels with known DeathWeapon residual packs
//! - On die: dual-ring primary/secondary splash around corpse
//! - Skips under-construction objects
//!
//! Fail-closed: not full UpgradeMux activation masks / RequiresAllTriggers /
//! WeaponStore temp-weapon projectile path.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DeathWeaponSplash {
    pub primary_damage: f32,
    pub primary_radius: f32,
    pub secondary_damage: f32,
    pub secondary_radius: f32,
    pub weapon_name: &'static str,
}

/// Resolve residual DeathWeapon splash for a template, if any.
pub fn death_weapon_for_template(template_name: &str) -> Option<DeathWeaponSplash> {
    let n = template_name.to_ascii_lowercase();
    // Scud Storm launcher death residual (already partly honesty-tracked).
    if n.contains("scudstorm") && !n.contains("missile") {
        return Some(DeathWeaponSplash {
            primary_damage: 400.0,
            primary_radius: 50.0,
            secondary_damage: 100.0,
            secondary_radius: 80.0,
            weapon_name: "ScudStormDamageWeapon",
        });
    }
    // Nuclear tank death residual (China NukeCannon already has shell path).
    if n.contains("nucleartank") || n.contains("nuclear_tank") {
        return Some(DeathWeaponSplash {
            primary_damage: 50.0,
            primary_radius: 40.0,
            secondary_damage: 10.0,
            secondary_radius: 60.0,
            weapon_name: "NukeTankDeathWeapon",
        });
    }
    // Stinger site / tunnel network common death weapons are OCL; skip.
    // Demo trap proximity explosives.
    if n.contains("demotrap") || n.contains("demo_trap") {
        return Some(DeathWeaponSplash {
            primary_damage: 500.0,
            primary_radius: 25.0,
            secondary_damage: 200.0,
            secondary_radius: 40.0,
            weapon_name: "DemoTrapWeapon",
        });
    }
    // Terrorist death pack when not intentional suicide residual.
    if n.contains("terrorist") {
        return Some(DeathWeaponSplash {
            primary_damage: 700.0,
            primary_radius: 15.0,
            secondary_damage: 0.0,
            secondary_radius: 0.0,
            weapon_name: "SuicideDynamitePack",
        });
    }
    // Bomb truck default death (non-carbomb).
    if n.contains("bombtruck") {
        return Some(DeathWeaponSplash {
            primary_damage: 400.0,
            primary_radius: 40.0,
            secondary_damage: 100.0,
            secondary_radius: 70.0,
            weapon_name: "BombTruckDefaultDeathWeapon",
        });
    }
    // Anthrax bomb / toxin truck death residual peel.
    if n.contains("anthraxbomb") || (n.contains("toxin") && n.contains("truck")) {
        return Some(DeathWeaponSplash {
            primary_damage: 200.0,
            primary_radius: 50.0,
            secondary_damage: 50.0,
            secondary_radius: 80.0,
            weapon_name: "AnthraxBombWeapon",
        });
    }
    None
}

/// Dual-ring damage residual at distance from death epicenter.
pub fn splash_damage_at_distance(splash: &DeathWeaponSplash, dist: f32) -> f32 {
    if splash.primary_radius > 0.0 && dist <= splash.primary_radius {
        return splash.primary_damage;
    }
    if splash.secondary_radius > 0.0 && dist <= splash.secondary_radius {
        return splash.secondary_damage;
    }
    0.0
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostFireWeaponWhenDeadState {
    pub fired: bool,
    pub applications: u32,
    pub total_damage: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrorist_and_scud_have_death_weapons() {
        assert!(death_weapon_for_template("GLAInfantryTerrorist").is_some());
        assert!(death_weapon_for_template("GLAScudStorm").is_some());
        assert!(death_weapon_for_template("AmericaTankCrusader").is_none());
    }

    #[test]
    fn splash_falloff() {
        let s = DeathWeaponSplash {
            primary_damage: 100.0,
            primary_radius: 10.0,
            secondary_damage: 25.0,
            secondary_radius: 30.0,
            weapon_name: "t",
        };
        assert!((splash_damage_at_distance(&s, 5.0) - 100.0).abs() < 0.1);
        assert!((splash_damage_at_distance(&s, 20.0) - 25.0).abs() < 0.1);
        assert_eq!(splash_damage_at_distance(&s, 50.0), 0.0);
    }
}
