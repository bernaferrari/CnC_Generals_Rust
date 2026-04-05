//! Demoralize Special Power - Causes fear effect on enemy units

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::effects::FXList;
use crate::helpers::TheAudio;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DemoralizeSpecialPowerData {
    pub base: SpecialPowerModuleData,
    pub base_range: Real,
    pub bonus_range_per_captured: Real,
    pub max_range: Real,
    pub base_duration_frames: UnsignedInt,
    pub bonus_duration_per_captured_frames: UnsignedInt,
    pub max_duration_frames: UnsignedInt,
    pub fx_list: Option<Arc<FXList>>,
}

impl DemoralizeSpecialPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::Demoralize);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::AFFECTS_NEUTRAL;

        Self {
            base,
            base_range: 0.0,
            bonus_range_per_captured: 0.0,
            max_range: 0.0,
            base_duration_frames: 0,
            bonus_duration_per_captured_frames: 0,
            max_duration_frames: 0,
            fx_list: None,
        }
    }
}

pub struct DemoralizeSpecialPower {
    data: DemoralizeSpecialPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    owner_object_id: ObjectID,
}

impl DemoralizeSpecialPower {
    pub fn new(data: DemoralizeSpecialPowerData, owner_object_id: ObjectID) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);
        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            owner_object_id,
        }
    }

    pub fn set_owner_object_id(&mut self, owner_id: ObjectID) {
        self.owner_object_id = owner_id;
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

    fn validate_targeting(&self, targeting: Option<&TargetingInfo>) -> Result<(), String> {
        if self.data.base.requires_targeting() && targeting.is_none() {
            return Err("Demoralize power requires targeting".to_string());
        }
        if self.data.base.is_instant() && targeting.is_some() {
            return Err("Instant power does not accept targeting".to_string());
        }
        Ok(())
    }

    fn play_sound(&self) {
        if !self.data.base.sound_effect.is_empty() {
            if let Some(audio) = TheAudio::get() {
                let event =
                    crate::common::audio::AudioEventRts::new(self.data.base.sound_effect.as_str());
                audio.add_audio_event(&event);
            }
        }
    }

    fn compute_effect_parameters(&self) -> (Real, UnsignedInt) {
        let mut range = self.data.base_range;
        let mut duration = self.data.base_duration_frames;

        if self.owner_object_id != INVALID_ID {
            if let Some(owner) =
                crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id)
            {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(contain) = owner_guard.get_contain() {
                        if let Ok(contain_guard) = contain.lock() {
                            let captured_count = contain_guard.get_contained_count() as u32;
                            duration = duration.saturating_add(
                                self.data
                                    .bonus_duration_per_captured_frames
                                    .saturating_mul(captured_count),
                            );
                            if duration > self.data.max_duration_frames {
                                duration = self.data.max_duration_frames;
                            }

                            range += self.data.bonus_range_per_captured * captured_count as Real;
                            if range > self.data.max_range {
                                range = self.data.max_range;
                            }
                        }
                    }
                }
            }
        }

        (range, duration)
    }

    fn apply_fear(
        &mut self,
        _owner_player_id: ObjectID,
        targeting: &TargetingInfo,
    ) -> Result<(), String> {
        if self.owner_object_id == INVALID_ID {
            return Err("Demoralize power requires an owning object".to_string());
        }
        let owner_id = self.owner_object_id;
        let owner = crate::helpers::TheGameLogic::find_object_by_id(owner_id)
            .ok_or_else(|| "Demoralize power owning object not found".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "Demoralize owner lock poisoned".to_string())?;
        if owner_guard.is_disabled() {
            return Ok(());
        }
        let owner_off_map = owner_guard.is_off_map();

        let (radius, duration_frames) = self.compute_effect_parameters();
        let object_ids = crate::helpers::ThePartitionManager::get()
            .map(|mgr| mgr.get_objects_in_range(&targeting.position, radius))
            .unwrap_or_default();

        for object_id in object_ids {
            let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };

            let should_affect = {
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if obj_guard.is_destroyed() {
                    continue;
                }
                if !obj_guard.is_kind_of(KindOf::Infantry) {
                    continue;
                }
                if obj_guard.is_off_map() != owner_off_map {
                    continue;
                }
                matches!(
                    owner_guard.relationship_to(&obj_guard),
                    Relationship::Enemies | Relationship::Neutral
                )
            };

            if !should_affect {
                continue;
            }

            if let Ok(obj_guard) = obj_arc.read() {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_demoralized(duration_frames);
                    }
                }
            }

            self.stats.record_unit_affected();
        }

        if let Some(fx_list) = &self.data.fx_list {
            let _ = fx_list.do_fx_at_position(&targeting.position);
        }

        Ok(())
    }
}

impl SpecialPowerModuleInterface for DemoralizeSpecialPower {
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
        if let Err(reason) = self.validate_targeting(targeting) {
            return ActivationResult::InvalidTarget { reason };
        }

        let targeting = match targeting {
            Some(t) => t,
            None => {
                return ActivationResult::InvalidTarget {
                    reason: "Demoralize power requires targeting".to_string(),
                }
            }
        };

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

        if self.owner_object_id == INVALID_ID {
            return ActivationResult::Failed {
                reason: "Demoralize power requires an owning object".to_string(),
            };
        }
        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_disabled() {
                    return ActivationResult::Disabled;
                }
            }
        }

        if let Err(reason) = self.execute_demoralize(player_id, targeting) {
            return ActivationResult::Failed { reason };
        }

        if !self.deduct_cost(player_id) {
            return ActivationResult::Failed {
                reason: "Failed to deduct cost".to_string(),
            };
        }

        self.play_sound();
        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);
        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.execute_demoralize(0, targeting)
    }

    fn execute_demoralize(
        &mut self,
        player_id: ObjectID,
        targeting: &TargetingInfo,
    ) -> Result<(), String> {
        let mut effective_target = targeting.clone();
        if let Some(target_id) = targeting.target_object {
            let Some(target_obj) = crate::helpers::TheGameLogic::find_object_by_id(target_id)
            else {
                return Ok(());
            };
            let Ok(target_guard) = target_obj.read() else {
                return Ok(());
            };
            effective_target.position = *target_guard.get_position();
        }
        self.apply_fear(player_id, &effective_target)
    }
}
