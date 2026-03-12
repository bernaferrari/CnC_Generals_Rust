//! Spy Vision Special Power - Reveals area of map

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::system::shroud_manager::get_shroud_manager;

#[derive(Debug, Clone)]
pub struct SpyVisionSpecialPowerData {
    pub base: SpecialPowerModuleData,
    pub vision_duration: Real,
}

impl SpyVisionSpecialPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::SpyVision);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING | SpecialPowerFlags::RADAR_EFFECT;
        let name_str = base.name.as_str();
        if name_str.eq_ignore_ascii_case("SpecialPowerSpyDrone") {
            base.recharge_time = 90.0; // 90000 ms
            base.radius = 250.0;
        } else {
            base.recharge_time = 60.0; // 60000 ms
            base.radius = 300.0;
        }

        Self {
            base,
            vision_duration: 30.0,
        }
    }
}

pub struct SpyVisionSpecialPower {
    data: SpyVisionSpecialPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    active_vision_end_time: Real,
}

impl SpyVisionSpecialPower {
    pub fn new(data: SpyVisionSpecialPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);
        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            active_vision_end_time: 0.0,
        }
    }

    fn reveal_area(
        &mut self,
        player_id: ObjectID,
        targeting: &TargetingInfo,
        current_frame: UnsignedInt,
    ) -> Result<(), String> {
        log::info!("Revealing area at {:?}", targeting.position);

        let duration_frames = (self.data.vision_duration
            / crate::system::game_logic::FIXED_DELTA_TIME as Real)
            .ceil()
            .max(0.0) as u32;
        let player_mask = 1u32 << (player_id.min((MAX_PLAYER_COUNT - 1) as u32));

        let mut shroud = get_shroud_manager()
            .lock()
            .map_err(|_| "ShroudManager lock poisoned".to_string())?;
        shroud.do_shroud_reveal(
            &targeting.position,
            targeting.radius.max(self.data.base.radius) as f32,
            player_mask,
        );
        shroud.queue_undo_shroud_reveal(
            &targeting.position,
            targeting.radius.max(self.data.base.radius) as f32,
            player_mask,
            duration_frames,
            current_frame as u32,
        );

        self.active_vision_end_time = (current_frame as Real
            * crate::system::game_logic::FIXED_DELTA_TIME as Real)
            + self.data.vision_duration;
        Ok(())
    }
}

impl SpecialPowerModuleInterface for SpyVisionSpecialPower {
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
        targeting: Option<&TargetingInfo>,
        current_frame: UnsignedInt,
    ) -> ActivationResult {
        let targeting = match targeting {
            Some(t) => t,
            None => {
                return ActivationResult::InvalidTarget {
                    reason: "Spy vision power requires targeting".to_string(),
                }
            }
        };

        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        if let Err(reason) = self.reveal_area(player_id, targeting, current_frame) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);
        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.reveal_area(0, targeting, crate::helpers::TheGameLogic::get_frame())
    }
}
