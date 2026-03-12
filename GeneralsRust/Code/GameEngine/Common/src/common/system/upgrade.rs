// FILE: upgrade.rs ////////////////////////////////////////////////////////////
// Game upgrade system
///////////////////////////////////////////////////////////////////////////////

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Upgrade {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cost: u32,
    pub prerequisites: Vec<String>,
    pub unlocks: Vec<String>,
}

pub struct UpgradeSystem {
    upgrades: HashMap<String, Upgrade>,
    purchased_upgrades: Vec<String>,
}

impl UpgradeSystem {
    pub fn new() -> Self {
        Self {
            upgrades: HashMap::new(),
            purchased_upgrades: Vec::new(),
        }
    }

    pub fn register_upgrade(&mut self, upgrade: Upgrade) {
        self.upgrades.insert(upgrade.id.clone(), upgrade);
    }

    pub fn can_purchase(&self, upgrade_id: &str) -> bool {
        if let Some(upgrade) = self.upgrades.get(upgrade_id) {
            upgrade
                .prerequisites
                .iter()
                .all(|prereq| self.purchased_upgrades.contains(prereq))
        } else {
            false
        }
    }

    pub fn purchase_upgrade(&mut self, upgrade_id: &str) -> bool {
        if self.can_purchase(upgrade_id)
            && !self.purchased_upgrades.contains(&upgrade_id.to_string())
        {
            self.purchased_upgrades.push(upgrade_id.to_string());
            true
        } else {
            false
        }
    }

    pub fn has_upgrade(&self, upgrade_id: &str) -> bool {
        self.purchased_upgrades.contains(&upgrade_id.to_string())
    }
}

impl Default for UpgradeSystem {
    fn default() -> Self {
        Self::new()
    }
}
