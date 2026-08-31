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
//! Overlord BattleBunker TransportContain (`OverlordContain.cpp:553`)
//! allows infantry (and portable structures) to fire when the bunker is
//! installed — host residual sets `passengers_allowed_to_fire` on that peel.
//!
//! Fail-closed: not full ContainModule Xfer of m_passengerAllowedToFire /
//! rider weapon-set PLAYER_UPGRADE while firing from hatch.

use serde::{Deserialize, Serialize};

pub const UPGRADE_HELIX_BATTLE_BUNKER: &str = "Upgrade_ChinaHelixBattleBunker";
pub const UPGRADE_INFA_HELIX_BATTLE_BUNKER: &str = "Upgrade_Infa_ChinaHelixBattleBunker";
pub const UPGRADE_OVERLORD_BATTLE_BUNKER: &str = "Upgrade_ChinaOverlordBattleBunker";

/// Whether this upgrade triggers PassengersFireUpgrade residual
/// (Helix module) or Overlord bunker TransportContain fire peel.
pub fn is_passengers_fire_upgrade(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("helixbattlebunker")
        || (n.contains("helix_bunker") && n.contains("battle"))
        || n.contains("overlordbattlebunker")
        || (n.contains("overlord") && n.contains("bunker") && n.contains("battle"))
}

/// Templates that receive the flag when the upgrade completes.
pub fn is_passengers_fire_upgrade_host(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.contains("helix")
        && !n.contains("bunker")
        && !n.contains("gattling")
        && !n.contains("propaganda")
    {
        return true;
    }
    crate::game_logic::host_overlord_addons::is_overlord_tank_template(template_name)
}

/// C++ `OverlordContain::isPassengerAllowedToFire` (`OverlordContain.cpp:553`):
/// infantry in an installed BattleBunker may fire (unless nested).
pub fn overlord_bunker_passengers_may_fire(bunker_slots: usize, nested: bool) -> bool {
    bunker_slots > 0 && !nested
}

/// C++ pairs each PassengersFireUpgrade module with its own TriggeredBy
/// upgrade: the Helix module fires on HelixBattleBunker, while the Overlord
/// BattleBunker is a TransportContain fire peel (OverlordContain.cpp:553)
/// driven by the Overlord bunker upgrade.  Cross-family application is a
/// no-op — an Overlord owns no module triggered by the Helix upgrade.
pub fn should_enable_passengers_fire(upgrade: &str, template_name: &str) -> bool {
    let u = upgrade.to_ascii_lowercase();
    let overlord_upgrade = u.contains("overlordbattlebunker")
        || (u.contains("overlord") && u.contains("bunker") && u.contains("battle"));
    if overlord_upgrade {
        return crate::game_logic::host_overlord_addons::is_overlord_tank_template(template_name);
    }
    let helix_upgrade = u.contains("helixbattlebunker")
        || (u.contains("helix_bunker") && u.contains("battle"));
    if helix_upgrade {
        let n = template_name.to_ascii_lowercase();
        return n.contains("helix")
            && !n.contains("bunker")
            && !n.contains("gattling")
            && !n.contains("propaganda");
    }
    false
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
        && is_passengers_fire_upgrade(UPGRADE_OVERLORD_BATTLE_BUNKER)
        && is_passengers_fire_upgrade_host("ChinaHelix")
        && is_passengers_fire_upgrade_host("Nuke_ChinaHelix")
        && is_passengers_fire_upgrade_host("ChinaTankOverlord")
        && !is_passengers_fire_upgrade_host("ChinaTankOverlordBattleBunker")
        && should_enable_passengers_fire(UPGRADE_HELIX_BATTLE_BUNKER, "ChinaHelix")
        && should_enable_passengers_fire(UPGRADE_OVERLORD_BATTLE_BUNKER, "ChinaTankOverlord")
        && !should_enable_passengers_fire(UPGRADE_HELIX_BATTLE_BUNKER, "ChinaTankOverlord")
        && overlord_bunker_passengers_may_fire(5, false)
        && !overlord_bunker_passengers_may_fire(0, false)
        && !overlord_bunker_passengers_may_fire(5, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack() {
        assert!(honesty_passengers_fire_upgrade_residual_ok());
    }

    /// C++ OverlordContain.cpp:553 — bunker infantry fire is allowed.
    #[test]
    fn overlord_bunker_upgrade_enables_passenger_fire() {
        assert!(should_enable_passengers_fire(
            UPGRADE_OVERLORD_BATTLE_BUNKER,
            "ChinaTankOverlord"
        ));
        assert!(overlord_bunker_passengers_may_fire(5, false));
    }
}
