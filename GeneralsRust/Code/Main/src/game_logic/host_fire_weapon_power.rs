//! Host FireWeaponPower residual (special power fires weapon N times).
//!
//! C++: `FireWeaponPower::doSpecialPower[AtLocation]` reloads ammo then
//! `aiAttackPosition(loc, maxShotsToFire)`. Used by Spectre howitzer markers etc.
//!
//! Residual playability slice:
//! - MaxShotsToFire default **1**, retail peels **3**
//! - On activate: queue attack-position residual with shot count
//! - Disabled objects skip
//!
//! Fail-closed: not full turret slot matrix / SpecialPowerModule base recharge
//! beyond host special power registry.

use serde::{Deserialize, Serialize};

/// Retail MaxShotsToFire residual for common FireWeaponPower peels.
pub const FIRE_WEAPON_POWER_DEFAULT_SHOTS: u32 = 1;
pub const FIRE_WEAPON_POWER_SPECTRE_SHOTS: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFireWeaponPowerRequest {
    pub shots_remaining: u32,
    pub target_x: f32,
    pub target_z: f32,
    pub has_location: bool,
}

impl HostFireWeaponPowerRequest {
    pub fn at_self(shots: u32) -> Self {
        Self {
            shots_remaining: shots.max(1),
            target_x: 0.0,
            target_z: 0.0,
            has_location: false,
        }
    }

    pub fn at_location(shots: u32, x: f32, z: f32) -> Self {
        Self {
            shots_remaining: shots.max(1),
            target_x: x,
            target_z: z,
            has_location: true,
        }
    }
}

/// Max shots residual for template peels.
pub fn max_shots_for_template(template_name: &str) -> u32 {
    let n = template_name.to_ascii_lowercase();
    if n.contains("spectre") || n.contains("howitzer") || n.contains("gunship") {
        return FIRE_WEAPON_POWER_SPECTRE_SHOTS;
    }
    if n.contains("paladin") || n.contains("pointdefense") {
        return FIRE_WEAPON_POWER_DEFAULT_SHOTS;
    }
    FIRE_WEAPON_POWER_DEFAULT_SHOTS
}

pub fn wants_fire_weapon_power(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("spectre")
        || n.contains("howitzer")
        || n.contains("fireweaponpower")
        || n.contains("leaflet")
        || n.contains("emp_pulse")
        || n.contains("emppulse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectre_gets_three_shots() {
        assert_eq!(max_shots_for_template("SpectreHowitzerMarker"), 3);
        assert_eq!(max_shots_for_template("AmericaTankPaladin"), 1);
    }
}
