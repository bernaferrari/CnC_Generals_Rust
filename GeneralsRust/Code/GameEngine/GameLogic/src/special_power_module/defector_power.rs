//! Defector Special Power - Converts enemy units to player's side

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::player::player_list;

#[derive(Debug, Clone)]
pub struct DefectorSpecialPowerData {
    pub base: SpecialPowerModuleData,
    /// Maximum number of units to convert
    pub max_units: Int,
    /// Duration of conversion (0 = permanent)
    pub duration: Real,
}

impl DefectorSpecialPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::Defector);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING | SpecialPowerFlags::AFFECTS_ENEMY;

        Self {
            base,
            max_units: 5,
            duration: 0.0, // Permanent by default
        }
    }
}

pub struct DefectorSpecialPower {
    data: DefectorSpecialPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    converted_units: Vec<ObjectID>,
    revert_frame: Option<UnsignedInt>,
}

impl DefectorSpecialPower {
    pub fn new(data: DefectorSpecialPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);
        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            converted_units: Vec::new(),
            revert_frame: None,
        }
    }

    fn convert_units(
        &mut self,
        targeting: &TargetingInfo,
        player_id: ObjectID,
    ) -> Result<(), String> {
        log::info!("Converting enemy units at {:?}", targeting.position);

        self.converted_units.clear();
        self.revert_frame = None;

        let list = player_list()
            .read()
            .map_err(|_| "PlayerList lock poisoned".to_string())?;
        let player = list
            .get_player(player_id as Int)
            .cloned()
            .ok_or_else(|| format!("Invalid player id {}", player_id))?;
        let new_team = player
            .read()
            .map_err(|_| "Player lock poisoned".to_string())?
            .get_default_team()
            .ok_or_else(|| format!("Player {} has no default team", player_id))?;

        let radius = targeting.radius.max(self.data.base.radius).max(0.0);
        let object_ids = crate::helpers::ThePartitionManager::get()
            .map(|mgr| mgr.get_objects_in_range(&targeting.position, radius))
            .unwrap_or_default();

        let mut converted = 0i32;
        for object_id in object_ids {
            if converted >= self.data.max_units {
                break;
            }

            let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };

            let can_convert = {
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };

                if obj_guard.is_destroyed() || obj_guard.is_structure() {
                    false
                } else {
                    obj_guard
                        .get_controlling_player_id()
                        .map(|id| id != player_id)
                        .unwrap_or(false)
                }
            };

            if !can_convert {
                continue;
            }

            let old_owner = obj_arc.read().ok().and_then(|g| g.get_controlling_player());
            let new_owner = new_team.read().ok().and_then(|team_guard| {
                let idx = team_guard.get_controlling_player_id().unwrap_or(player_id) as Int;
                list.get_player(idx).cloned()
            });

            if let Ok(mut obj_write) = obj_arc.write() {
                if self.data.duration > 0.0 {
                    let _ = obj_write.set_temporary_team(Some(new_team.clone()));
                } else {
                    let _ = obj_write.set_team(Some(new_team.clone()));
                }
                obj_write.on_capture(old_owner, new_owner);
            }

            self.converted_units.push(object_id);
            converted += 1;
            self.stats.record_unit_affected();
        }

        if self.data.duration > 0.0 && !self.converted_units.is_empty() {
            let frames = (self.data.duration / crate::system::game_logic::FIXED_DELTA_TIME as Real)
                .ceil()
                .max(0.0) as UnsignedInt;
            self.revert_frame =
                Some(crate::helpers::TheGameLogic::get_frame().saturating_add(frames));
        }

        Ok(())
    }

    fn process_reverts(&mut self, current_frame: UnsignedInt) {
        let Some(revert_frame) = self.revert_frame else {
            return;
        };
        if current_frame < revert_frame {
            return;
        }

        for object_id in std::mem::take(&mut self.converted_units) {
            let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let mut obj_write = match obj_arc.write() {
                Ok(guard) => guard,
                Err(_) => continue,
            };
            let _ = obj_write.restore_original_team();
        }

        self.revert_frame = None;
    }
}

impl SpecialPowerModuleInterface for DefectorSpecialPower {
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
                    reason: "Defector power requires targeting".to_string(),
                }
            }
        };

        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        if let Err(reason) = self.convert_units(targeting, player_id) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);
        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.convert_units(targeting, 0)
    }

    fn update(&mut self, delta_time: Real) {
        self.get_cooldown_state_mut().update(delta_time);
        self.process_reverts(crate::helpers::TheGameLogic::get_frame());
    }
}
