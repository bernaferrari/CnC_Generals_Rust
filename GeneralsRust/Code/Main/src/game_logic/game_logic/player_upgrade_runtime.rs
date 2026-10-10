//! C++ player upgrade ledger transitions with explicitly supplied upgrade rules.
use super::*;

impl Player {
    /// Refund one paid production entry, publishing its final economy values.
    /// C++ Money::deposit (Money.cpp:44-59, Money.h:49) requests Deposit for a
    /// nonzero amount before crediting the controlling player's money.
    /// Keep the host's existing charged-cost, saturation, delta, and statistics
    /// policy; a refund does not pass through collected-supply accounting.
    pub(crate) fn refund_production_cost(&mut self, refund: &Resources) {
        if refund.supplies > 0 {
            crate::game_logic::host_economy_log::record_money_audio(
                self.id,
                crate::game_logic::host_economy_log::HostMoneyAudio::Deposit,
            );
        }
        if crate::gameworld_shadow::gameworld_economy_authority_live() {
            self.pending_supply_delta += refund.supplies as i64;
        } else {
            self.resources.supplies = self.resources.supplies.saturating_add(refund.supplies);
        }
        self.power_available -= refund.power;
        crate::game_logic::host_economy_log::record(
            self.id,
            self.effective_supplies(),
            self.power_available,
        );
    }

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
        self.refund_production_cost(refund);
        true
    }

    /// Mark research finished. OBJECT upgrades stay off the player completed set.
    pub fn complete_researched_upgrade(
        &mut self,
        upgrade_name: &str,
        upgrade_type: gamelogic::upgrade::UpgradeType,
    ) {
        self.complete_researched_upgrade_with_definition(upgrade_name, upgrade_type, None);
    }

    pub(crate) fn complete_researched_upgrade_with_definition(
        &mut self,
        upgrade_name: &str,
        upgrade_type: gamelogic::upgrade::UpgradeType,
        definition: Option<&gamelogic::upgrade::UpgradeTemplate>,
    ) {
        if let Some(queued) = self.find_queued_upgrade_name(upgrade_name) {
            self.queued_upgrades.remove(&queued);
        }
        // C++ ProductionUpdate.cpp:874-879 / 931 — purchased, not granted.
        if let Some(definition) = definition {
            self.record_upgrade_production_complete(definition);
        }
        if upgrade_type == gamelogic::upgrade::UpgradeType::Object {
            return;
        }
        self.add_completed_upgrade_with_definition(upgrade_name, definition);
    }

    pub fn has_unlocked_upgrade(&self, upgrade_name: &str) -> bool {
        let expected = normalize_upgrade_name(upgrade_name);
        self.unlocked_sciences
            .iter()
            .chain(self.completed_upgrades.iter())
            .any(|unlocked| normalize_upgrade_name(unlocked) == expected)
    }
}
