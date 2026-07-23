//! Host UpgradeDie residual.
//!
//! C++: `UpgradeDie::onDie` finds producer via `getProducerID`, then
//! `producer->removeUpgrade(UpgradeToRemove)`.
//!
//! Retail peels (AmericaVehicle.ini / AirforceGeneral / LaserGeneral):
//! - AmericaVehicleBattleDrone → Upgrade_AmericaBattleDrone
//! - AmericaVehicleScoutDrone → Upgrade_AmericaScoutDrone
//! - AmericaVehicleHellfireDrone → Upgrade_AmericaHellfireDrone
//!
//! Used so a dead scout/battle/hellfire drone frees the master's object-upgrade
//! slot and the player can rebuild another drone.

use serde::{Deserialize, Serialize};

use super::host_slave_drones::{
    is_battle_drone_template, is_hellfire_drone_template, is_scout_drone_template,
    UPGRADE_AMERICA_BATTLE_DRONE, UPGRADE_AMERICA_HELLFIRE_DRONE, UPGRADE_AMERICA_SCOUT_DRONE,
};

/// Per-object UpgradeDie residual payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostUpgradeDieData {
    /// C++ UpgradeDieModuleData::m_upgradeName (`UpgradeToRemove`).
    pub upgrade_to_remove: String,
    pub fired: bool,
}

impl HostUpgradeDieData {
    pub fn new(upgrade: impl Into<String>) -> Self {
        Self {
            upgrade_to_remove: upgrade.into(),
            fired: false,
        }
    }
}

/// Honesty counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostUpgradeDieRegistry {
    pub removals: u32,
    pub missing_producer: u32,
    pub missing_upgrade: u32,
}

impl HostUpgradeDieRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_removal(&mut self) {
        self.removals = self.removals.saturating_add(1);
    }

    pub fn record_missing_producer(&mut self) {
        self.missing_producer = self.missing_producer.saturating_add(1);
    }

    pub fn record_missing_upgrade(&mut self) {
        self.missing_upgrade = self.missing_upgrade.saturating_add(1);
    }

    pub fn honesty_removal_ok(&self) -> bool {
        self.removals > 0
    }
}

/// Retail UpgradeToRemove peel from drone template name.
pub fn upgrade_to_remove_for_template(template_name: &str) -> Option<&'static str> {
    if is_scout_drone_template(template_name) {
        return Some(UPGRADE_AMERICA_SCOUT_DRONE);
    }
    if is_battle_drone_template(template_name) {
        return Some(UPGRADE_AMERICA_BATTLE_DRONE);
    }
    if is_hellfire_drone_template(template_name) {
        return Some(UPGRADE_AMERICA_HELLFIRE_DRONE);
    }
    None
}

/// True when template has UpgradeDie residual.
pub fn has_upgrade_die_template(template_name: &str) -> bool {
    upgrade_to_remove_for_template(template_name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drone_upgrade_peels() {
        assert_eq!(
            upgrade_to_remove_for_template("AmericaVehicleScoutDrone"),
            Some(UPGRADE_AMERICA_SCOUT_DRONE)
        );
        assert_eq!(
            upgrade_to_remove_for_template("AmericaVehicleBattleDrone"),
            Some(UPGRADE_AMERICA_BATTLE_DRONE)
        );
        assert_eq!(
            upgrade_to_remove_for_template("AmericaVehicleHellfireDrone"),
            Some(UPGRADE_AMERICA_HELLFIRE_DRONE)
        );
        assert!(upgrade_to_remove_for_template("AmericaTankCrusader").is_none());
    }
}
