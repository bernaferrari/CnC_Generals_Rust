//! Veterancy Crate Collision Module
//!
//! FILE: veterancy_crate_collide.rs
//! Author: Converted from Graham Smallwood's C++ implementation, March 2002
//! Desc: A crate that gives a level of experience to all within n distance

use super::*;
use crate::experience::ExperienceRequirements;
use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::object::collide::crate_collide::crate_collide::CrateCollide as LegacyCrateCollide;
use crate::object::collide::*;
use crate::scripting::engine::transfer_object_name;
use std::sync::{Arc, Mutex};

/// Module data specific to veterancy crate collision
#[derive(Debug, Clone)]
pub struct VeterancyCrateCollideModuleData {
    pub base: CrateCollideModuleData,
    /// Range of effect for veterancy bonus (0 = single target only)
    pub range_of_effect: u32,
    /// If true, adds owner's veterancy level to bonus
    pub adds_owner_veterancy: bool,
    /// If true, this is a pilot entering a vehicle
    pub is_pilot: bool,
}

impl Default for VeterancyCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: CrateCollideModuleData::default(),
            range_of_effect: 0,
            adds_owner_veterancy: false,
            is_pilot: false,
        }
    }
}

/// Veterancy Crate Collide Module
///
/// This module implements a crate that grants veterancy experience to units.
/// It can affect a single unit or all units within a specified range.
pub struct VeterancyCrateCollide {
    base: LegacyCrateCollide,
    module_data: VeterancyCrateCollideModuleData,
    owner_object_id: ObjectId,
    version: u32,
}

impl VeterancyCrateCollide {
    /// Create a new VeterancyCrateCollide instance
    ///
    /// # Arguments
    /// * `object_id` - The ID of the object this module belongs to
    /// * `module_data` - Configuration data for the veterancy crate collision behavior
    pub fn new(object_id: ObjectId, module_data: VeterancyCrateCollideModuleData) -> Self {
        Self {
            base: LegacyCrateCollide::new(object_id, module_data.base.clone()),
            module_data,
            owner_object_id: object_id,
            version: 1,
        }
    }

    /// Get the veterancy crate collision module data
    pub fn get_veterancy_crate_collide_module_data(&self) -> &VeterancyCrateCollideModuleData {
        &self.module_data
    }

    /// Get the number of levels this crate will grant
    pub fn get_levels_to_gain(&self) -> i32 {
        if !self.module_data.adds_owner_veterancy {
            return 1;
        }

        // C++ parity: derive levels from the crate owner's veterancy.
        let Some(owner_obj) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return 0;
        };
        let Ok(owner_guard) = owner_obj.read() else {
            return 0;
        };
        owner_guard.get_veterancy_level() as i32
    }

    /// Get the current version of this module for serialization
    pub fn get_version(&self) -> u32 {
        self.version
    }
}

impl CollideModule for VeterancyCrateCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        if let Some(other_obj) = other {
            if self.is_valid_to_execute_internal(other_obj) {
                // Execute the veterancy crate behavior
                let success = self.execute_crate_behavior_internal(other_obj)?;
                if !success {
                    return Err(CollisionError::InvalidObject(
                        "Failed to execute veterancy crate behavior".to_string(),
                    ));
                }
                self.base.finalize_collection(other_obj)?;
            }
        }

        Ok(())
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        self.is_valid_to_execute_internal(other)
    }
}

impl VeterancyCrateCollide {
    fn owner_goal_matches(&self, target_id: ObjectId) -> bool {
        let Some(owner_obj) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return false;
        };
        let ai_update = match owner_obj.read() {
            Ok(owner_guard) => owner_guard.get_ai_update_interface(),
            Err(_) => None,
        };
        let Some(ai_update) = ai_update else {
            return false;
        };
        // Avoid deadlocking crate collection when the AI update/machine lock is currently held.
        let Ok(ai_guard) = ai_update.try_lock() else {
            return false;
        };
        let goal_id = ai_guard
            .get_goal_object()
            .and_then(|goal| goal.read().ok().map(|goal_guard| goal_guard.get_id()));
        goal_id == Some(target_id)
    }

    fn owner_player_id(&self) -> Option<PlayerId> {
        let owner_obj = TheGameLogic::find_object_by_id(self.owner_object_id)?;
        let owner_guard = owner_obj.read().ok()?;
        owner_guard.get_player_id()
    }

    /// Enhanced validation for veterancy crate execution
    ///
    /// This method checks if the crate can be executed for the given object,
    /// including special checks for pilots and aircraft.
    fn is_valid_to_execute_internal(&self, other: &dyn GameObject) -> bool {
        // Base validation first
        if !self.base.is_valid_to_execute(other) {
            return false;
        }

        if other.is_effectively_dead() {
            return false;
        }

        if other.is_significantly_above_terrain() {
            return false;
        }

        let levels_to_gain = self.get_levels_to_gain();

        if levels_to_gain <= 0 {
            return false;
        }

        let Some(other_handle) = other.as_object_handle() else {
            return false;
        };
        let Ok(other_guard) = other_handle.read() else {
            return false;
        };
        let Some(tracker) = other_guard.get_experience_tracker() else {
            return false;
        };
        let Ok(tracker_guard) = tracker.lock() else {
            return false;
        };
        if !tracker_guard.is_trainable() || !tracker_guard.can_gain_exp_for_level(levels_to_gain) {
            return false;
        }

        // Pilot-specific checks
        if self.module_data.is_pilot {
            if self.owner_player_id() != Some(other.get_controlling_player()) {
                return false;
            }

            // Can't upgrade a helicopter or plane
            if other.is_using_airborne_locomotor() {
                return false;
            }
        }

        true
    }

    /// Internal implementation of crate behavior execution
    ///
    /// This method grants veterancy experience to the target object or all objects
    /// within the specified range, depending on the module configuration.
    fn execute_crate_behavior_internal(
        &self,
        other: &dyn GameObject,
    ) -> Result<bool, CollisionError> {
        // C++ parity: crate owner AI goal must intentionally target `other`.
        if !self.owner_goal_matches(other.get_id()) {
            return Ok(false);
        }

        let levels_to_gain = self.get_levels_to_gain();
        let range = self.module_data.range_of_effect as f32;

        let mut affected_objects = Vec::new();
        let controlling_player = other.get_controlling_player();
        let match_above_terrain = other.is_significantly_above_terrain();

        if range == 0.0 {
            affected_objects.push(other.get_id());
        } else if let Some(partition_manager) = ThePartitionManager::get() {
            let center = other.get_position();
            let center_pos = crate::common::Coord3D::new(center.x, center.y, center.z);
            let candidates = partition_manager.get_objects_in_range(&center_pos, range);
            for object_id in candidates {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if obj_guard.is_destroyed() {
                    continue;
                }
                if obj_guard.get_controlling_player_id() != Some(controlling_player.value() as u32)
                {
                    continue;
                }
                if obj_guard.is_significantly_above_terrain() != match_above_terrain {
                    continue;
                }
                affected_objects.push(object_id);
            }
        }

        let requirements = ExperienceRequirements::default_requirements();
        for object_id in affected_objects {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(mut obj_guard) = obj_arc.write() else {
                continue;
            };
            let Some(tracker) = obj_guard.get_experience_tracker() else {
                continue;
            };
            let Ok(mut tracker_guard) = tracker.lock() else {
                continue;
            };
            if !tracker_guard.can_gain_exp_for_level(levels_to_gain) {
                continue;
            }
            let old_level = tracker_guard.get_veterancy_level();
            if tracker_guard.gain_exp_for_level(
                levels_to_gain,
                !self.module_data.is_pilot,
                requirements.as_array(),
            ) {
                let new_level = tracker_guard.get_veterancy_level();
                if old_level != new_level {
                    obj_guard.on_veterancy_level_changed(old_level, new_level, true);
                }
            }
        }

        // Transfer object name for pilots (for script control)
        if self.module_data.is_pilot {
            let owner_name = TheGameLogic::find_object_by_id(self.owner_object_id)
                .and_then(|obj| obj.read().ok().map(|obj| obj.get_name().clone()))
                .unwrap_or_else(|| format!("Object{}", self.owner_object_id).into());
            transfer_object_name(&owner_name, other.get_id())
                .map_err(|e| CollisionError::InvalidObject(e.to_string()))?;
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::TheThingFactory;
    use crate::object_manager::get_object_manager;
    use crate::player::{player_list, Player, PlayerIndex};
    use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
    use std::sync::{Arc, RwLock};

    fn ensure_template_exists(name: &str) {
        let needs_init = get_thing_factory().unwrap().is_none();
        if needs_init {
            init_thing_factory().unwrap();
        }
        let mut factory_guard = get_thing_factory().unwrap();
        if let Some(factory) = factory_guard.as_mut() {
            if factory.find_template(name, false).is_none() {
                factory.new_template(name);
            }
        }
    }

    fn setup_player_with_team(
        player_index: PlayerIndex,
        team_name: &str,
    ) -> Arc<RwLock<crate::team::Team>> {
        {
            let player_list = player_list();
            let mut list_guard = player_list.write().expect("Player list lock poisoned");
            list_guard.clear();
        }

        let team_arc = Arc::new(RwLock::new(crate::team::Team::new(
            crate::common::AsciiString::from(team_name),
            (player_index as u32).saturating_add(1),
        )));

        if let Ok(mut team_guard) = team_arc.write() {
            team_guard.set_controlling_player_id(Some(player_index as u32));
        }

        let player_arc = Arc::new(RwLock::new(Player::new(player_index)));
        if let Ok(mut player_guard) = player_arc.write() {
            player_guard.set_default_team(Some(Arc::clone(&team_arc)));
        }
        {
            let player_list = player_list();
            let mut list_guard = player_list.write().expect("Player list lock poisoned");
            list_guard.add_player(player_arc);
        }

        team_arc
    }

    fn create_object_with_team(
        template_name: &str,
        team_arc: &Arc<RwLock<crate::team::Team>>,
        position: Coord3D,
    ) -> Arc<RwLock<crate::object::Object>> {
        ensure_template_exists(template_name);
        let team_guard = team_arc.read().expect("Team lock poisoned");

        let thing_factory = TheThingFactory::get().expect("ThingFactory unavailable");
        let template = TheThingFactory::find_template(template_name).expect("Template missing");
        let obj = thing_factory
            .new_object(template, &*team_guard)
            .expect("Failed to create object");

        if let Ok(mut obj_guard) = obj.write() {
            let object_position = crate::common::Coord3D::new(position.x, position.y, position.z);
            let _ = obj_guard.set_position(&object_position);
        }

        if let Ok(mut manager) = get_object_manager().write() {
            let object_id = obj.read().map(|o| o.get_id()).unwrap_or(0);
            let object_position = crate::common::Coord3D::new(position.x, position.y, position.z);
            manager.update_object_position(object_id, object_position);
        }

        obj
    }

    #[test]
    fn test_veterancy_crate_creation() {
        let _lock = crate::test_sync::lock();

        let module_data = VeterancyCrateCollideModuleData {
            range_of_effect: 10,
            adds_owner_veterancy: true,
            is_pilot: false,
            ..Default::default()
        };

        let veterancy_crate = VeterancyCrateCollide::new(1, module_data);

        assert_eq!(veterancy_crate.get_version(), 1);
        assert_eq!(
            veterancy_crate
                .get_veterancy_crate_collide_module_data()
                .range_of_effect,
            10
        );
        assert!(
            veterancy_crate
                .get_veterancy_crate_collide_module_data()
                .adds_owner_veterancy
        );
    }

    #[test]
    fn test_veterancy_crate_levels_to_gain() {
        let _lock = crate::test_sync::lock();

        let module_data = VeterancyCrateCollideModuleData {
            adds_owner_veterancy: true,
            ..Default::default()
        };

        let veterancy_crate = VeterancyCrateCollide::new(1, module_data);

        // No owner object exists in this unit test setup.
        assert_eq!(veterancy_crate.get_levels_to_gain(), 0);

        let module_data_no_owner = VeterancyCrateCollideModuleData {
            adds_owner_veterancy: false,
            ..Default::default()
        };

        let veterancy_crate_no_owner = VeterancyCrateCollide::new(1, module_data_no_owner);
        assert_eq!(veterancy_crate_no_owner.get_levels_to_gain(), 1);
    }

    #[test]
    fn test_veterancy_crate_execute_behavior() {
        let _lock = crate::test_sync::lock();

        let team_arc = setup_player_with_team(1, "PlayerTeam");
        let other = create_object_with_team("Infantry", &team_arc, Coord3D::new(10.0, 20.0, 0.0));

        let module_data = VeterancyCrateCollideModuleData {
            range_of_effect: 0, // Single target
            ..Default::default()
        };

        let veterancy_crate = VeterancyCrateCollide::new(1, module_data);

        let result = veterancy_crate.execute_crate_behavior_internal(&other);
        assert!(result.is_ok());
        // C++ parity guard: crate owner AI goal must explicitly match target object.
        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn test_veterancy_crate_area_effect() {
        let _lock = crate::test_sync::lock();

        let team_arc = setup_player_with_team(1, "PlayerTeam");
        let other = create_object_with_team("Infantry", &team_arc, Coord3D::new(10.0, 20.0, 0.0));
        let _friend = create_object_with_team("Infantry", &team_arc, Coord3D::new(15.0, 20.0, 0.0));

        let module_data = VeterancyCrateCollideModuleData {
            range_of_effect: 15, // Area effect
            ..Default::default()
        };

        let veterancy_crate = VeterancyCrateCollide::new(1, module_data);

        let result = veterancy_crate.execute_crate_behavior_internal(&other);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
    }
}

impl game_engine::common::system::Snapshotable for VeterancyCrateCollide {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // C++ parity: versioned xfer entry point (current version 1).
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}
