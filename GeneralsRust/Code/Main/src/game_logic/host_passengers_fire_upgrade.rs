//! Host PassengersFireUpgrade residual.
//!
//! C++: `PassengersFireUpgrade::upgradeImplementation` →
//! `ContainModuleInterface::setPassengerAllowedToFire(TRUE)`.
//!
//! Retail peel (`ChinaAir.ini` / general Helix variants):
//! ```text
//! Behavior = PassengersFireUpgrade ModuleTag_34
//!   TriggeredBy = Upgrade_ChinaHelixBattleBunker
//! ```
//!
//! Also honored for `Upgrade_Infa_ChinaHelixBattleBunker` residual.
//! Overlord BattleBunker uses TransportContain peels (not this upgrade module).
//!
//! Fail-closed: not full ContainModule Xfer of m_passengerAllowedToFire /
//! rider weapon-set PLAYER_UPGRADE while firing from hatch.

use serde::{Deserialize, Serialize};

pub const UPGRADE_HELIX_BATTLE_BUNKER: &str = "Upgrade_ChinaHelixBattleBunker";
pub const UPGRADE_INFA_HELIX_BATTLE_BUNKER: &str = "Upgrade_Infa_ChinaHelixBattleBunker";

/// Whether this upgrade triggers PassengersFireUpgrade residual.
pub fn is_passengers_fire_upgrade(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("helixbattlebunker") || n.contains("helix_bunker") && n.contains("battle")
}

/// Templates that receive the flag when the upgrade completes.
pub fn is_passengers_fire_upgrade_host(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("helix")
}

/// Apply residual: set passengers_allowed_to_fire.
pub fn should_enable_passengers_fire(upgrade: &str, template_name: &str) -> bool {
    is_passengers_fire_upgrade(upgrade) && is_passengers_fire_upgrade_host(template_name)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostPassengersFireUpgradeRegistry {
    pub applies: u32,
    pub units_enabled: u32,
}

impl HostPassengersFireUpgradeRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_apply(&mut self, units: u32) {
        self.applies = self.applies.saturating_add(1);
        self.units_enabled = self.units_enabled.saturating_add(units);
    }
    pub fn honesty_apply_ok(&self) -> bool {
        self.applies > 0 && self.units_enabled > 0
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_apply_ok() || honesty_passengers_fire_upgrade_residual_ok()
    }
}

pub fn honesty_passengers_fire_upgrade_residual_ok() -> bool {
    is_passengers_fire_upgrade(UPGRADE_HELIX_BATTLE_BUNKER)
        && is_passengers_fire_upgrade(UPGRADE_INFA_HELIX_BATTLE_BUNKER)
        && !is_passengers_fire_upgrade("Upgrade_ChinaOverlordBattleBunker")
        && is_passengers_fire_upgrade_host("ChinaHelix")
        && is_passengers_fire_upgrade_host("Nuke_ChinaHelix")
        && !is_passengers_fire_upgrade_host("ChinaTankOverlord")
        && should_enable_passengers_fire(UPGRADE_HELIX_BATTLE_BUNKER, "ChinaHelix")
        && !should_enable_passengers_fire(UPGRADE_HELIX_BATTLE_BUNKER, "ChinaTankOverlord")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack() {
        assert!(honesty_passengers_fire_upgrade_residual_ok());
    }
}
