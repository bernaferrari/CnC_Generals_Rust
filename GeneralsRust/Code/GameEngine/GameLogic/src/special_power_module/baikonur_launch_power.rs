//! Baikonur Launch Special Power - Nuclear missile launch

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::helpers::{TheAudio, TheGameLogic, TheThingFactory};
use crate::player::PlayerIndex;

#[derive(Debug, Clone)]
pub struct BaikonurLaunchPowerData {
    pub base: SpecialPowerModuleData,
    pub detonation_object: AsciiString,
}

impl BaikonurLaunchPowerData {
    pub fn new(name: AsciiString, detonation_object: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::BaikonurLaunch);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::AFFECTS_TERRAIN
            | SpecialPowerFlags::SUPERWEAPON;

        Self {
            base,
            detonation_object,
        }
    }
}

pub struct BaikonurLaunchPower {
    data: BaikonurLaunchPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    owner_id: Option<ObjectID>,
    target_position: Coord3D,
}

impl BaikonurLaunchPower {
    pub fn new(data: BaikonurLaunchPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);
        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            owner_id: None,
            target_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }

    pub fn set_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }

    fn open_launch_door(&self) {
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
            return;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return;
        };
        owner_guard.set_model_condition_state(ModelConditionFlags::DOOR_1_OPENING);
    }

    fn owner_is_disabled(&self) -> bool {
        let Some(owner_id) = self.owner_id else {
            return false;
        };
        let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
            return false;
        };
        owner
            .read()
            .map(|guard| guard.is_disabled())
            .unwrap_or(false)
    }

    fn resolve_team(&self) -> Result<std::sync::Arc<std::sync::RwLock<crate::team::Team>>, String> {
        let owner_id = self
            .owner_id
            .ok_or_else(|| "Baikonur launch requires owning object".to_string())?;
        let owner = TheGameLogic::find_object_by_id(owner_id)
            .ok_or_else(|| "Baikonur launch owner object not found".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "Baikonur launch owner lock poisoned".to_string())?;
        owner_guard
            .get_team()
            .ok_or_else(|| "Baikonur launch owner has no team".to_string())
    }

    fn spawn_detonation(&mut self) -> Result<(), String> {
        log::info!("Detonating Baikonur strike at {:?}", self.target_position);

        // Create detonation object at target location.
        // Matches C++ BaikonurLaunchPower::doSpecialPowerAtLocation.
        let template = match TheThingFactory::find_template(self.data.detonation_object.as_str()) {
            Some(template) => template,
            None => {
                log::warn!(
                    "Could not find detonation object template: {}",
                    self.data.detonation_object
                );
                return Ok(());
            }
        };

        let team_arc = self.resolve_team()?;
        let team_guard = team_arc
            .read()
            .map_err(|_| "Team lock poisoned".to_string())?;
        let factory = TheThingFactory::get().map_err(|e| e.to_string())?;
        let detonation = factory
            .new_object(template.clone(), &*team_guard)
            .map_err(|e| e.to_string())?;

        detonation
            .write()
            .map_err(|_| "Detonation object lock poisoned".to_string())?
            .set_position(&self.target_position)?;

        Ok(())
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
            return Err("Nuclear launch requires targeting".to_string());
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

    fn start_launch(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        if self.owner_is_disabled() {
            return Ok(());
        }
        self.target_position = targeting.position;
        self.spawn_detonation()
    }
}

impl SpecialPowerModuleInterface for BaikonurLaunchPower {
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
        if targeting.is_none() {
            // Matches C++ BaikonurLaunchPower::doSpecialPower (no target).
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

            if let Some(owner_id) = self.owner_id {
                if let Some(owner) = TheGameLogic::find_object_by_id(owner_id) {
                    if let Ok(owner_guard) = owner.read() {
                        if owner_guard.is_disabled() {
                            return ActivationResult::Disabled;
                        }
                    }
                }
            }

            self.open_launch_door();

            if !self.deduct_cost(player_id) {
                return ActivationResult::Failed {
                    reason: "Failed to deduct cost".to_string(),
                };
            }

            self.play_sound();
            self.cooldown.start_cooldown(current_frame);
            self.stats
                .record_activation(current_frame, self.data.base.cost);
            return ActivationResult::Success;
        }

        if let Err(reason) = self.validate_targeting(targeting) {
            return ActivationResult::InvalidTarget { reason };
        }

        let targeting = match targeting {
            Some(t) => t,
            None => {
                return ActivationResult::InvalidTarget {
                    reason: "Nuclear launch requires targeting".to_string(),
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

        if let Some(owner_id) = self.owner_id {
            if let Some(owner) = TheGameLogic::find_object_by_id(owner_id) {
                if let Ok(owner_guard) = owner.read() {
                    if owner_guard.is_disabled() {
                        return ActivationResult::Disabled;
                    }
                }
            }
        }

        if let Err(reason) = self.start_launch(targeting) {
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
        self.start_launch(targeting)
    }

    fn update(&mut self, delta_time: Real) {
        self.cooldown.update(delta_time);
    }
}
