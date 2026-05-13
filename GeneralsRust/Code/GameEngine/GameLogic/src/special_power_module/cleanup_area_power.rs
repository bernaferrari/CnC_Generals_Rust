//! Cleanup Area Special Power
//!
//! Matches C++ CleanupAreaPower behavior by delegating to CleanupHazardUpdate.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;

#[derive(Debug, Clone)]
pub struct CleanupAreaPowerData {
    pub base: SpecialPowerModuleData,
    pub cleanup_move_range: Real,
}

impl CleanupAreaPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::CleanupArea);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING | SpecialPowerFlags::AFFECTS_TERRAIN;
        base.recharge_time = 0.0;
        base.radius = 110.0;

        Self {
            base,
            cleanup_move_range: 0.0,
        }
    }
}

pub struct CleanupAreaPower {
    data: CleanupAreaPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    owner_object_id: Option<ObjectID>,
}

impl CleanupAreaPower {
    pub fn new(data: CleanupAreaPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);
        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            owner_object_id: None,
        }
    }

    /// Set cleanup area parameters on the owning object's CleanupHazardUpdate module.
    fn cleanup_area(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        let owner_id = self
            .owner_object_id
            .ok_or_else(|| "CleanupAreaPower requires an owning object".to_string())?;
        let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(owner_id) else {
            return Err("CleanupAreaPower owner object not found".to_string());
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Err("CleanupAreaPower owner lock poisoned".to_string());
        };
        if obj_guard.is_disabled() {
            return Ok(());
        }

        let Some(module) = obj_guard.find_update_module("CleanupHazardUpdate") else {
            return Err("CleanupAreaPower requires CleanupHazardUpdate module".to_string());
        };

        let move_range = self.data.cleanup_move_range;
        let mut applied = false;
        module.with_module(|module| {
            if let Some(cleanup_hazard) = module.get_cleanup_hazard_control_interface() {
                cleanup_hazard.set_cleanup_area_parameters(
                    targeting.position.x,
                    targeting.position.y,
                    targeting.position.z,
                    move_range,
                );
                applied = true;
            }
        });

        if !applied {
            return Err("CleanupHazardUpdate module not available".to_string());
        }

        Ok(())
    }

    pub fn set_owner(&mut self, owner_id: ObjectID) {
        self.owner_object_id = Some(owner_id);
    }
}

impl SpecialPowerModuleInterface for CleanupAreaPower {
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
        _player_id: ObjectID,
        targeting: Option<&TargetingInfo>,
        current_frame: UnsignedInt,
    ) -> ActivationResult {
        if let Some(owner_id) = self.owner_object_id {
            if let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(owner_id) {
                if let Ok(guard) = obj.read() {
                    if guard.is_disabled() {
                        return ActivationResult::Disabled;
                    }
                }
            }
        } else {
            return ActivationResult::Failed {
                reason: "CleanupAreaPower requires an owning object".to_string(),
            };
        }

        let targeting = match targeting {
            Some(t) => t,
            None => {
                return ActivationResult::InvalidTarget {
                    reason: "Cleanup area power requires targeting".to_string(),
                }
            }
        };

        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        if let Err(reason) = self.execute(targeting) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);
        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.cleanup_area(targeting)
    }
}
