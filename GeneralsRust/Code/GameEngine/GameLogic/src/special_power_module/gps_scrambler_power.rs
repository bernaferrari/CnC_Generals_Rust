//! GPS Scrambler Special Power
//!
//! GLA special power that disables enemy radar and prevents them from using
//! special powers for a duration.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::helpers::TheGameLogic;
use crate::player::player_list;

const GPS_SCRAMBLER_DURATION: Real = 60.0;

#[derive(Debug, Clone)]
pub struct GpsScramblerPowerData {
    pub base: SpecialPowerModuleData,
    pub scramble_duration: Real,
    pub affects_all_enemies: Bool,
}

impl GpsScramblerPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::OCL);
        base.flags = SpecialPowerFlags::INSTANT | SpecialPowerFlags::AFFECTS_ENEMY;
        base.recharge_time = 240.0; // 4 minutes (240000 ms)
        base.cost = 0;
        base.range = 0.0;
        base.radius = 100.0;
        let name_str = base.name.as_str();
        if name_str.eq_ignore_ascii_case("Slth_SuperweaponGPSScrambler") {
            base.recharge_time = 180.0; // 180000 ms
        }

        Self {
            base,
            scramble_duration: GPS_SCRAMBLER_DURATION,
            affects_all_enemies: true,
        }
    }
}

pub struct GpsScramblerPower {
    data: GpsScramblerPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    active_until_frame: UnsignedInt,
    affected_players: Vec<ObjectID>,
}

impl GpsScramblerPower {
    pub fn new(data: GpsScramblerPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            active_until_frame: 0,
            affected_players: Vec::new(),
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

    fn execute_scramble(
        &mut self,
        player_id: ObjectID,
        current_frame: UnsignedInt,
    ) -> Result<(), String> {
        log::info!("GPS Scrambler activated by player {}", player_id);

        self.affected_players.clear();

        let frames_per_second = 30;
        self.active_until_frame = current_frame
            + (self.data.scramble_duration * frames_per_second as Real) as UnsignedInt;

        // Apply scrambling to all enemy players
        self.apply_scrambling_effect(player_id)?;

        Ok(())
    }

    fn apply_scrambling_effect(&mut self, activating_player_id: ObjectID) -> Result<(), String> {
        log::debug!("Applying GPS scrambling to enemy players");

        let Ok(list) = player_list().read() else {
            return Err("Failed to lock player list".to_string());
        };
        let Some(activating_player) = list.get_player(activating_player_id as Int) else {
            return Err(format!(
                "Activating player {} not found",
                activating_player_id
            ));
        };

        let Ok(activating_guard) = activating_player.read() else {
            return Err("Failed to lock activating player".to_string());
        };
        let enemy_players = activating_guard.get_enemy_players();

        drop(activating_guard);

        for enemy_id in enemy_players {
            let Some(enemy_player) = list.get_player(enemy_id) else {
                continue;
            };
            let Ok(mut enemy_guard) = enemy_player.write() else {
                continue;
            };

            enemy_guard.disable_radar();
            self.affected_players.push(enemy_id as ObjectID);
        }

        log::debug!(
            "GPS Scrambler affecting {} players",
            self.affected_players.len()
        );

        Ok(())
    }

    pub fn update_scrambler(&mut self, current_frame: UnsignedInt) {
        if current_frame >= self.active_until_frame && !self.affected_players.is_empty() {
            log::debug!("GPS Scrambler effect ended, restoring radar");

            if let Ok(list) = player_list().read() {
                for player_id in self.affected_players.iter().copied() {
                    let Some(player) = list.get_player(player_id as Int) else {
                        continue;
                    };
                    if let Ok(mut guard) = player.write() {
                        guard.enable_radar();
                    }
                }
            }

            self.affected_players.clear();
        }
    }

    pub fn is_active(&self) -> Bool {
        !self.affected_players.is_empty()
    }
}

impl SpecialPowerModuleInterface for GpsScramblerPower {
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
        // GPS Scrambler doesn't require targeting (instant global effect)

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

        if let Err(reason) = self.execute_scramble(player_id, current_frame) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);

        ActivationResult::Success
    }

    fn execute(&mut self, _targeting: &TargetingInfo) -> Result<(), String> {
        self.execute_scramble(1, 0)
    }

    fn update(&mut self, _delta_time: Real) {
        self.update_scrambler(TheGameLogic::get_frame());
    }
}
