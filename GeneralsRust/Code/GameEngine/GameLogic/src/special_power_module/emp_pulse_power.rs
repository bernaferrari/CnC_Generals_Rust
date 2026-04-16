//! EMP Pulse Special Power
//!
//! China special power that disables all vehicles and structures in the target
//! area for a duration without dealing damage.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::object::Object;

const EMP_RADIUS: Real = 200.0;
const EMP_DURATION: Real = 30.0;

#[derive(Debug, Clone)]
pub struct EmpPulsePowerData {
    pub base: SpecialPowerModuleData,
    pub emp_radius: Real,
    pub disable_duration: Real,
    pub affects_vehicles: Bool,
    pub affects_aircraft: Bool,
    pub affects_buildings: Bool,
}

impl EmpPulsePowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::OCL);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::AFFECTS_FRIENDLY // Can hit own units
            | SpecialPowerFlags::SUPERWEAPON;
        base.recharge_time = 360.0; // 6 minutes (360000 ms)
        base.cost = 0;
        base.range = 0.0;
        base.radius = EMP_RADIUS;

        Self {
            base,
            emp_radius: EMP_RADIUS,
            disable_duration: EMP_DURATION,
            affects_vehicles: true,
            affects_aircraft: true,
            affects_buildings: true,
        }
    }
}

pub struct EmpPulsePower {
    data: EmpPulsePowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    affected_objects: Vec<ObjectID>,
}

impl EmpPulsePower {
    pub fn new(data: EmpPulsePowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            affected_objects: Vec::new(),
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

    fn validate_targeting(&self, targeting: Option<&TargetingInfo>) -> Result<(), String> {
        if self.data.base.requires_targeting() && targeting.is_none() {
            return Err("EMP Pulse requires targeting".to_string());
        }
        if self.data.base.is_instant() && targeting.is_some() {
            return Err("Instant power does not accept targeting".to_string());
        }
        Ok(())
    }

    fn execute_emp(
        &mut self,
        owner_player_id: ObjectID,
        targeting: &TargetingInfo,
        current_frame: UnsignedInt,
    ) -> Result<(), String> {
        log::info!(
            "EMP Pulse activated at position {:?}, radius={}",
            targeting.position,
            self.data.emp_radius
        );

        self.affected_objects.clear();

        // Apply EMP effect to all valid targets in radius
        self.apply_emp_effect(owner_player_id, targeting, current_frame)?;

        // Trigger visual/audio effects
        log::debug!("Triggering EMP pulse FX");

        Ok(())
    }

    fn apply_emp_effect(
        &mut self,
        owner_player_id: ObjectID,
        targeting: &TargetingInfo,
        current_frame: UnsignedInt,
    ) -> Result<(), String> {
        log::debug!("Applying EMP effect to objects in radius");

        let radius = self.data.emp_radius;
        let object_ids = crate::helpers::ThePartitionManager::get()
            .map(|mgr| mgr.get_objects_in_range(&targeting.position, radius))
            .unwrap_or_default();

        let frames = (self.data.disable_duration
            / crate::system::game_logic::FIXED_DELTA_TIME as Real)
            .ceil() as UnsignedInt;
        let disable_until = current_frame.saturating_add(frames);

        for object_id in object_ids {
            let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };

            let (should_affect, is_structure) = {
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if obj_guard.is_destroyed() {
                    continue;
                }
                let rel = relationship_to_player(&obj_guard, owner_player_id);
                let should_affect = self.should_affect_object(&obj_guard, rel);
                (should_affect, obj_guard.is_structure())
            };

            if !should_affect {
                continue;
            }

            if let Ok(mut obj_write) = obj_arc.write() {
                obj_write.set_disabled_until(DisabledType::DisabledEmp, disable_until);
            }

            self.affected_objects.push(object_id);
            if is_structure {
                self.stats.record_building_affected();
            } else {
                self.stats.record_unit_affected();
            }
        }

        log::debug!("EMP affected {} objects", self.affected_objects.len());

        Ok(())
    }

    fn should_affect_object(&self, obj: &Object, relationship: Relationship) -> Bool {
        match relationship {
            Relationship::Allies => {
                if !self
                    .data
                    .base
                    .flags
                    .contains(SpecialPowerFlags::AFFECTS_FRIENDLY)
                {
                    return false;
                }
            }
            Relationship::Enemies => {
                if !self
                    .data
                    .base
                    .flags
                    .contains(SpecialPowerFlags::AFFECTS_ENEMY)
                {
                    return false;
                }
            }
            Relationship::Neutral => {
                if !self
                    .data
                    .base
                    .flags
                    .contains(SpecialPowerFlags::AFFECTS_NEUTRAL)
                {
                    return false;
                }
            }
        }

        if obj.is_kind_of(KindOf::Infantry) {
            return false;
        }

        if obj.is_structure() {
            return self.data.affects_buildings;
        }

        if obj.is_kind_of(KindOf::Aircraft) {
            return self.data.affects_aircraft;
        }

        if obj.is_kind_of(KindOf::Vehicle) {
            return self.data.affects_vehicles;
        }

        false
    }

    pub fn get_affected_objects(&self) -> &[ObjectID] {
        &self.affected_objects
    }
}

impl SpecialPowerModuleInterface for EmpPulsePower {
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
                    reason: "EMP Pulse requires targeting".to_string(),
                };
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

        if !self.deduct_cost(player_id) {
            return ActivationResult::Failed {
                reason: "Failed to deduct cost".to_string(),
            };
        }

        if let Err(reason) = self.execute_emp(player_id, targeting, current_frame) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);

        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.execute_emp(0, targeting, crate::helpers::TheGameLogic::get_frame())
    }
}

fn relationship_to_player(obj: &Object, player_id: ObjectID) -> Relationship {
    let Some(controller) = obj.get_controlling_player_id() else {
        return Relationship::Neutral;
    };
    if controller == player_id {
        Relationship::Allies
    } else {
        Relationship::Enemies
    }
}
