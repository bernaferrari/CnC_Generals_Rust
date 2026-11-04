//! Host UpgradeDie residual.
//!
//! C++: `UpgradeDie::onDie` finds producer via `getProducerID`, then
//! `producer->removeUpgrade(UpgradeToRemove)`.
//!
//! The payload binds at create from the Object's authored UpgradeDie module
//! (`UpgradeToRemove`), never from the template name.

use serde::{Deserialize, Serialize};


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
