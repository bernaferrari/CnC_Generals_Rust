//! C++ player upgrade ledger transitions with explicitly supplied upgrade rules.
use super::*;

impl Player {
    pub fn queue_upgrade(
        &mut self,
        upgrade_name: &str,
        cost: &Resources,
        upgrade_type: gamelogic::upgrade::UpgradeType,
    ) -> bool {
        // C++ ProductionUpdate.cpp:250-272 — PLAYER refuses if complete or
        // already in production. OBJECT is per-producer (`giveUpgrade`), not
        // a player-wide unlock (one add-on per unit, not per player).
        let object_scoped = upgrade_type == gamelogic::upgrade::UpgradeType::Object;
        if !object_scoped
            && (self.has_unlocked_upgrade(upgrade_name) || self.has_queued_upgrade(upgrade_name))
        {
            return false;
        }
        if !self.spend_resources(cost) {
            return false;
        }
        self.queued_upgrades.insert(upgrade_name.to_string());
        true
    }

    /// Cancel a queued upgrade and refund the requested resources.
    pub fn cancel_queued_upgrade(&mut self, upgrade_name: &str, refund: &Resources) -> bool {
        let Some(queued_name) = self.find_queued_upgrade_name(upgrade_name) else {
            return false;
        };
        self.queued_upgrades.remove(&queued_name);
        self.apply_supply_gain(refund.supplies);
        self.power_available -= refund.power;
        crate::game_logic::host_economy_log::record(
            self.id,
            self.effective_supplies(),
            self.power_available,
        );
        true
    }

    /// Mark research finished. OBJECT upgrades stay off the player completed set.
    pub fn complete_researched_upgrade(
        &mut self,
        upgrade_name: &str,
        upgrade_type: gamelogic::upgrade::UpgradeType,
    ) {
        if let Some(queued) = self.find_queued_upgrade_name(upgrade_name) {
            self.queued_upgrades.remove(&queued);
        }
        // C++ ProductionUpdate.cpp:874-879 / 931 — purchased, not granted.
        self.record_upgrade_production_complete(upgrade_name);
        if upgrade_type == gamelogic::upgrade::UpgradeType::Object {
            return;
        }
        self.add_completed_upgrade(upgrade_name);
    }

    /// Complete all queued player upgrades into the unlocked upgrade/science set.
    pub fn complete_queued_upgrades(
        &mut self,
        center: &gamelogic::upgrade::center::UpgradeCenter,
    ) -> Vec<String> {
        let mut completed: Vec<String> = self.queued_upgrades.drain().collect();
        completed.sort();
        for upgrade in &completed {
            if center
                .find_upgrade(upgrade)
                .map(|template| {
                    template.get_upgrade_type() == gamelogic::upgrade::UpgradeType::Object
                })
                .unwrap_or_else(|| {
                    crate::game_logic::host_upgrades::is_object_scoped_upgrade_residual(upgrade)
                })
            {
                continue;
            }
            self.add_completed_upgrade(upgrade);
        }
        completed
    }

    pub fn has_unlocked_upgrade(&self, upgrade_name: &str) -> bool {
        let expected = normalize_upgrade_name(upgrade_name);
        self.unlocked_sciences
            .iter()
            .chain(self.completed_upgrades.iter())
            .any(|unlocked| normalize_upgrade_name(unlocked) == expected)
    }
}
