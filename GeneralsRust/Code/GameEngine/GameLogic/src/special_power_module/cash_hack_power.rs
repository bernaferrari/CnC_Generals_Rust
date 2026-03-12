//! Cash Hack Special Power - Steals money from enemy

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::player::player_list;
use crate::special_power_module::player_money::get_player_money_manager;

#[derive(Debug, Clone)]
pub struct CashHackSpecialPowerData {
    pub base: SpecialPowerModuleData,
    pub steal_amount: Int,
    pub steal_percentage: Real,
}

impl CashHackSpecialPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::CashHack);
        base.flags = SpecialPowerFlags::INSTANT;
        base.recharge_time = 240.0; // 4 minutes (240000 ms)
        let name_str = base.name.as_str();
        if name_str.eq_ignore_ascii_case("SpecialAbilityBlackLotusStealCashHack") {
            base.recharge_time = 2.0; // 2000 ms
        }

        Self {
            base,
            steal_amount: 1000,
            steal_percentage: 0.0,
        }
    }
}

pub struct CashHackSpecialPower {
    data: CashHackSpecialPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
}

impl CashHackSpecialPower {
    pub fn new(data: CashHackSpecialPowerData) -> Self {
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

    fn steal_money(
        &mut self,
        player_id: ObjectID,
        target_player_id: ObjectID,
        current_frame: UnsignedInt,
    ) -> Result<(), String> {
        let manager = get_player_money_manager()
            .ok_or_else(|| "PlayerMoneyManager not initialized".to_string())?;
        let mut mgr = manager
            .write()
            .map_err(|_| "Failed to lock PlayerMoneyManager".to_string())?;

        let target_money = mgr.get_money(target_player_id);
        if target_money <= 0 {
            return Ok(());
        }

        let mut amount = if self.data.steal_percentage > 0.0 {
            ((target_money as Real) * self.data.steal_percentage) as Int
        } else {
            self.data.steal_amount
        };
        if amount <= 0 {
            return Ok(());
        }
        if amount > target_money {
            amount = target_money;
        }

        if !mgr.transfer_money(target_player_id, player_id, amount, current_frame) {
            return Err("Cash hack transfer failed".to_string());
        }

        Ok(())
    }

    fn determine_target_player(&self, player_id: ObjectID) -> Option<ObjectID> {
        let manager = get_player_money_manager()?;
        let mgr = manager.read().ok()?;

        let list = player_list().read().ok()?;
        let me = list.get_player(player_id as Int)?;

        let mut best_enemy: Option<(ObjectID, Int)> = None;
        let mut best_other: Option<(ObjectID, Int)> = None;

        for other_id in 0..crate::common::MAX_PLAYER_COUNT as ObjectID {
            if other_id == player_id {
                continue;
            }
            let Some(them) = list.get_player(other_id as Int) else {
                continue;
            };
            let money = mgr.get_money(other_id);
            if money <= 0 {
                continue;
            }

            let rel = match (me.read(), them.read()) {
                (Ok(me_guard), Ok(them_guard)) => me_guard.get_relationship(&them_guard),
                _ => Relationship::Neutral,
            };

            match rel {
                Relationship::Enemy => {
                    let should_replace = match best_enemy.as_ref() {
                        None => true,
                        Some((_, best)) => money > *best,
                    };
                    if should_replace {
                        best_enemy = Some((other_id, money));
                    }
                }
                _ => {
                    let should_replace = match best_other.as_ref() {
                        None => true,
                        Some((_, best)) => money > *best,
                    };
                    if should_replace {
                        best_other = Some((other_id, money));
                    }
                }
            }
        }

        best_enemy.or(best_other).map(|(id, _)| id)
    }
}

impl SpecialPowerModuleInterface for CashHackSpecialPower {
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

        let Some(target_player_id) = self.determine_target_player(player_id) else {
            return ActivationResult::Failed {
                reason: "No valid cash-hack target".to_string(),
            };
        };

        if !self.deduct_cost(player_id) {
            return ActivationResult::Failed {
                reason: "Failed to deduct cost".to_string(),
            };
        }

        if let Err(reason) = self.steal_money(player_id, target_player_id, current_frame) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);
        ActivationResult::Success
    }

    fn execute(&mut self, _targeting: &TargetingInfo) -> Result<(), String> {
        Ok(())
    }
}
