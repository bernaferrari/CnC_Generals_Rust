//! Emergency Repair Special Power
//!
//! General special power that instantly repairs all friendly vehicles and
//! structures in the target area.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::helpers::TheObjectCreationListStore;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use crate::player::player_list;
use std::sync::Arc;
use std::sync::RwLock;

const EMERGENCY_REPAIR_RADIUS: Real = 100.0;
const EMERGENCY_REPAIR_AMOUNT: Real = 0.5; // 50% health restoration

#[derive(Debug, Clone)]
pub struct EmergencyRepairPowerData {
    pub base: SpecialPowerModuleData,
    pub repair_radius: Real,
    pub repair_percentage: Real,
    pub affects_vehicles: Bool,
    pub affects_aircraft: Bool,
    pub affects_buildings: Bool,
    pub ocl_name: AsciiString,
    pub upgrade_ocl: Vec<(AsciiString, AsciiString)>,
}

impl EmergencyRepairPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::EmergencyRepair);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING | SpecialPowerFlags::AFFECTS_FRIENDLY;
        base.recharge_time = 240.0; // 4 minutes (240000 ms)
        base.cost = 0;
        base.range = 0.0;
        base.radius = EMERGENCY_REPAIR_RADIUS;

        let upgrade_ocl = vec![
            (
                "SCIENCE_EmergencyRepair3".into(),
                "SUPERWEAPON_RepairVehicles3".into(),
            ),
            (
                "SCIENCE_EmergencyRepair2".into(),
                "SUPERWEAPON_RepairVehicles2".into(),
            ),
            (
                "Early_SCIENCE_EmergencyRepair3".into(),
                "SUPERWEAPON_RepairVehicles3".into(),
            ),
            (
                "Early_SCIENCE_EmergencyRepair2".into(),
                "SUPERWEAPON_RepairVehicles2".into(),
            ),
        ];

        Self {
            base,
            repair_radius: EMERGENCY_REPAIR_RADIUS,
            repair_percentage: EMERGENCY_REPAIR_AMOUNT,
            affects_vehicles: true,
            affects_aircraft: true,
            affects_buildings: true,
            ocl_name: "SUPERWEAPON_RepairVehicles1".into(),
            upgrade_ocl,
        }
    }
}

pub struct EmergencyRepairPower {
    data: EmergencyRepairPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    repaired_objects: Vec<ObjectID>,
}

impl EmergencyRepairPower {
    pub fn new(data: EmergencyRepairPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            repaired_objects: Vec::new(),
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
            return Err("Emergency Repair requires targeting".to_string());
        }
        if self.data.base.is_instant() && targeting.is_some() {
            return Err("Instant power does not accept targeting".to_string());
        }
        Ok(())
    }

    fn execute_repair(
        &mut self,
        owner_player_id: ObjectID,
        targeting: &TargetingInfo,
    ) -> Result<(), String> {
        log::info!(
            "Emergency Repair activated at position {:?}, radius={}",
            targeting.position,
            self.data.repair_radius
        );

        self.repaired_objects.clear();

        if let Some(ocl_name) = self.select_ocl_name(owner_player_id) {
            if let Some(ocl) = TheObjectCreationListStore::find_object_creation_list(&ocl_name) {
                if let Some(owner) = self.resolve_owner_object(owner_player_id) {
                    let owner_guard = owner
                        .read()
                        .map_err(|_| "EmergencyRepair owner lock poisoned".to_string())?;
                    let ctx = crate::object_creation_list::live_creation_context();
                    let _ = ocl.create_with_angle(
                        &ctx,
                        Some(&*owner_guard),
                        &targeting.position,
                        &targeting.position,
                        INVALID_ANGLE,
                        0,
                    );
                }
            }
        }

        // Find and repair all friendly objects in radius
        self.apply_repairs(owner_player_id, targeting)?;

        // Trigger visual effects
        log::debug!("Triggering repair FX");

        Ok(())
    }

    fn apply_repairs(
        &mut self,
        owner_player_id: ObjectID,
        targeting: &TargetingInfo,
    ) -> Result<(), String> {
        log::debug!("Applying repairs to friendly objects");

        let radius = self.data.repair_radius;
        let object_ids = crate::helpers::ThePartitionManager::get()
            .map(|mgr| mgr.get_objects_in_range(&targeting.position, radius))
            .unwrap_or_default();

        for object_id in object_ids {
            let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };

            let (should_repair, is_structure, heal_amount) = {
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };

                if obj_guard.is_destroyed() {
                    continue;
                }

                let rel = relationship_to_player(&obj_guard, owner_player_id);
                let should_repair = self.should_repair_object(&obj_guard, rel);
                if !should_repair {
                    continue;
                }

                let current_health = obj_guard.get_health();
                let max_health = obj_guard.get_max_health();
                if current_health >= max_health {
                    continue;
                }

                let heal_amount = max_health * self.data.repair_percentage;
                (true, obj_guard.is_structure(), heal_amount)
            };

            if !should_repair {
                continue;
            }

            if let Ok(mut obj_write) = obj_arc.write() {
                let _ = obj_write.heal(heal_amount);
            }

            self.repaired_objects.push(object_id);
            if is_structure {
                self.stats.record_building_affected();
            } else {
                self.stats.record_unit_affected();
            }
        }

        log::debug!("Repaired {} objects", self.repaired_objects.len());

        Ok(())
    }

    fn select_ocl_name(&self, player_id: ObjectID) -> Option<AsciiString> {
        if !self.data.upgrade_ocl.is_empty() {
            if let Some(manager) = super::player_science::get_player_science_manager() {
                if let Ok(mgr) = manager.read() {
                    if let Some(player_science) = mgr.get_player(player_id) {
                        for (science, ocl) in &self.data.upgrade_ocl {
                            if player_science.has_science(science.as_str()) {
                                return Some(ocl.clone());
                            }
                        }
                    }
                }
            }
        }
        if self.data.ocl_name.is_empty() {
            None
        } else {
            Some(self.data.ocl_name.clone())
        }
    }

    fn resolve_owner_object(
        &self,
        player_id: ObjectID,
    ) -> Option<Arc<RwLock<crate::object::Object>>> {
        let list = player_list().read().ok()?;
        let player = list.get_player(player_id as Int).cloned()?;
        let player_guard = player.read().ok()?;
        let owned = player_guard.get_all_objects();
        drop(player_guard);
        for object_id in owned {
            if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                return Some(obj);
            }
        }
        None
    }

    fn should_repair_object(&self, obj: &Object, relationship: Relationship) -> Bool {
        if !matches!(
            relationship,
            Relationship::Friend | Relationship::Ally | Relationship::Allies
        ) {
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

    pub fn get_repaired_objects(&self) -> &[ObjectID] {
        &self.repaired_objects
    }
}

impl SpecialPowerModuleInterface for EmergencyRepairPower {
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
                    reason: "Emergency Repair requires targeting".to_string(),
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

        if let Err(reason) = self.execute_repair(player_id, targeting) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);

        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.execute_repair(0, targeting)
    }
}

fn relationship_to_player(obj: &Object, player_id: ObjectID) -> Relationship {
    let Some(controller) = obj.get_controlling_player_id() else {
        return Relationship::Neutral;
    };
    if controller == player_id {
        Relationship::Friend
    } else {
        Relationship::Enemy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emergency_repair_creation() {
        let data = EmergencyRepairPowerData::new("EmergencyRepair".into());
        let power = EmergencyRepairPower::new(data);

        assert_eq!(power.get_name(), "EmergencyRepair");
        assert!(power.is_ready());
        assert_eq!(power.data.repair_percentage, 0.5);
    }

    #[test]
    fn test_emergency_repair_activation() {
        let data = EmergencyRepairPowerData::new("EmergencyRepair".into());
        let mut power = EmergencyRepairPower::new(data);

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 0.0, 200.0);
        let result = power.try_activate(1, Some(&targeting), 0);

        assert!(result.is_success());
        assert!(power.is_on_cooldown());
    }
}
