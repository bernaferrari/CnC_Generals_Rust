//! Cash Bounty Special Power
//!
//! Matches C++ CashBountyPower behavior by setting the player's cash bounty percentage.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;

#[derive(Debug, Clone)]
pub struct CashBountyPowerData {
    pub base: SpecialPowerModuleData,
    /// Cash bounty percentage (0.0 - 1.0)
    pub bounty_percent: Real,
}

impl CashBountyPowerData {
    pub fn new(name: AsciiString, bounty_percent: Real) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::CashBounty);
        base.flags = SpecialPowerFlags::INSTANT | SpecialPowerFlags::PLAYER_SPECIFIC;
        base.recharge_time = 0.0;

        Self {
            base,
            bounty_percent,
        }
    }
}

pub struct CashBountyPower {
    data: CashBountyPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
}

impl CashBountyPower {
    pub fn new(data: CashBountyPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);
        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
        }
    }

    fn can_afford(&self, player_id: ObjectID) -> Bool {
        if self.data.base.cost <= 0 {
            return true;
        }

        self.get_player_money(player_id)
            .map(|money| money >= self.data.base.cost)
            .unwrap_or(false)
    }

    fn deduct_cost(&mut self, player_id: ObjectID) -> Bool {
        if self.data.base.cost <= 0 {
            return true;
        }

        let player_list = crate::player::player_list();
        let Ok(list_guard) = player_list.read() else {
            return false;
        };
        let Some(player_arc) = list_guard.get_player(player_id as PlayerIndex) else {
            return false;
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return false;
        };

        if !player_guard
            .get_money_mut()
            .subtract_money(self.data.base.cost)
        {
            return false;
        }

        if self.data.base.cost > 0 {
            player_guard
                .get_score_keeper_mut()
                .add_money_spent(self.data.base.cost as u32);
        }

        true
    }

    fn get_player_money(&self, player_id: ObjectID) -> Option<Int> {
        let player_list = crate::player::player_list();
        let list_guard = player_list.read().ok()?;
        let player_arc = list_guard.get_player(player_id as PlayerIndex)?;
        let player_guard = player_arc.read().ok()?;
        Some(player_guard.get_money().get_money())
    }

    fn check_prerequisites(&self, player_id: ObjectID) -> Bool {
        self.data.base.check_prerequisites(player_id)
    }

    fn apply_bounty(&mut self, player_id: ObjectID) -> Result<(), String> {
        let player_list = crate::player::player_list();
        let Ok(list_guard) = player_list.read() else {
            return Err("Failed to lock player list".to_string());
        };
        let Some(player_arc) = list_guard.get_player(player_id as PlayerIndex) else {
            return Err("Player not found".to_string());
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return Err("Failed to lock player".to_string());
        };

        let bounty = self.data.bounty_percent;
        if bounty > player_guard.get_cash_bounty() {
            player_guard.set_cash_bounty(bounty);
        }
        Ok(())
    }
}

impl SpecialPowerModuleInterface for CashBountyPower {
    fn get_data(&self) -> &SpecialPowerModuleData {
        &self.data.base
    }
    fn get_data_mut(&mut self) -> &mut SpecialPowerModuleData {
        &mut self.data.base
    }
    fn get_cooldown_state(&self) -> &CooldownState {
        &self.cooldown
    }
    fn get_cooldown_state_mut(&mut self) -> &mut CooldownState {
        &mut self.cooldown
    }
    fn get_stats(&self) -> &SpecialPowerStats {
        &self.stats
    }
    fn get_stats_mut(&mut self) -> &mut SpecialPowerStats {
        &mut self.stats
    }

    fn try_activate(
        &mut self,
        player_id: ObjectID,
        _targeting: Option<&TargetingInfo>,
        current_frame: UnsignedInt,
    ) -> ActivationResult {
        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        if !self.can_afford(player_id) {
            let available = self.get_player_money(player_id).unwrap_or(0);
            return ActivationResult::InsufficientFunds {
                cost: self.data.base.cost,
                available,
            };
        }

        if !self.check_prerequisites(player_id) {
            return ActivationResult::MissingPrerequisites {
                required: self.data.base.required_science.clone(),
            };
        }

        if !self.deduct_cost(player_id) {
            return ActivationResult::Failed {
                reason: "Failed to deduct cost".to_string(),
            };
        }

        if let Err(reason) = self.apply_bounty(player_id) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);
        ActivationResult::Success
    }

    fn execute(&mut self, _targeting: &TargetingInfo) -> Result<(), String> {
        Ok(()) // Instant power, no targeting needed
    }
}
