//! Convert to Car Bomb Crate Collision Module
//!
//! A crate (actually a terrorist - mobile crate) that converts a car into a car bomb,
//! activating its weapon and then activating its AI.
//! Author: Graham Smallwood, March 2002 (original C++), converted to Rust

use std::sync::{Arc, Mutex, RwLock};

use crate::common::{FieldParse, KindOf, ObjectStatusMaskType, ObjectStatusTypes};
use crate::effects::FXList;
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::Object;
use crate::scripting::engine::transfer_object_name;
use crate::weapon::{WeaponSetFlags, WeaponSetType};

/// Module data for convert to car bomb crate collide behavior
#[derive(Debug, Clone)]
pub struct ConvertToCarBombCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
    /// Range of effect for the conversion (unused in C++ but present)
    pub range_of_effect: u32,
    /// FX list to play when conversion occurs
    pub fx_list: Option<Arc<FXList>>,
}

impl Default for ConvertToCarBombCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
            range_of_effect: 0,
            fx_list: None,
        }
    }
}

impl ConvertToCarBombCrateCollideModuleData {
    /// Build field parser for INI configuration
    pub fn build_field_parse() -> Vec<FieldParse> {
        LegacyCrateCollideModuleData::build_field_parse()
    }
}

/// Convert to Car Bomb crate collide module.
#[derive(Debug)]
pub struct ConvertToCarBombCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<ConvertToCarBombCrateCollideModuleData>>,
}

impl ConvertToCarBombCrateCollide {
    /// Create new car bomb conversion crate collide module.
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: ConvertToCarBombCrateCollideModuleData,
    ) -> Self {
        Self {
            base: LegacyCrateCollide::from_object_handle(object, module_data.base.clone()),
            module_data: Arc::new(Mutex::new(module_data)),
        }
    }

    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        if !self.base.is_valid_to_execute(&other) {
            return Ok(false);
        }

        let other_lock = other.read().map_err(|_| GameError::LockError)?;

        if other_lock.is_effectively_dead() {
            return Ok(false);
        }

        if other_lock.is_kind_of(KindOf::Aircraft) || other_lock.is_kind_of(KindOf::Boat) {
            return Ok(false);
        }

        if other_lock.test_status(ObjectStatusTypes::IsCarBomb) {
            return Ok(false);
        }

        let mut flags = WeaponSetFlags::new();
        flags.set(WeaponSetType::CarBomb);
        let set = other_lock.weapon_set.find_weapon_template_set(&flags);
        let Some(template_set) = set else {
            return Ok(false);
        };
        if !template_set.conditions.test(WeaponSetType::CarBomb) {
            return Ok(false);
        }

        if other_lock.test_weapon_set_flag(WeaponSetType::CarBomb) {
            return Ok(false);
        }

        Ok(true)
    }

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        let obj = self.base.get_object().map_err(GameError::from)?;
        let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
        let other_id = other.read().map_err(|_| GameError::LockError)?.get_id();

        // Require AI goal match to avoid accidental conversion.
        if let Some(ai) = obj_guard.get_ai_update_interface() {
            let goal_id = ai
                .lock()
                .ok()
                .and_then(|ai_guard| ai_guard.get_goal_object())
                .and_then(|goal| goal.read().ok().map(|goal_guard| goal_guard.get_id()));
            if goal_id != Some(other_id) {
                return Ok(false);
            }
        }

        // Booby trap check.
        if other
            .read()
            .map_err(|_| GameError::LockError)?
            .check_and_detonate_booby_trap(Some(&obj_guard))
        {
            let other_dead = other
                .read()
                .map_err(|_| GameError::LockError)?
                .is_effectively_dead();
            if other_dead || obj_guard.is_effectively_dead() {
                return Ok(false);
            }
        }

        // Activate car bomb weapon set.
        {
            let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
            other_guard.set_weapon_set_flag(WeaponSetType::CarBomb);
        }

        // Play conversion FX.
        if let Ok(module_guard) = self.module_data.lock() {
            if let Some(fx) = module_guard.fx_list.as_ref() {
                let _ = fx.do_fx_obj(&other, None);
            }
        }

        // Transfer ownership to terrorist's team.
        {
            let new_team = if let Some(player) = obj_guard.get_controlling_player() {
                if let Ok(player_guard) = player.read() {
                    player_guard.get_default_team()
                } else {
                    None
                }
            } else {
                None
            };
            drop(obj_guard);

            if let Some(team) = new_team {
                let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
                other_guard.defect(Some(team), 0);
            }
        }

        // Transfer terrorist name to the car for script control.
        {
            let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
            let owner_name = obj_guard.get_name().clone();
            if !owner_name.is_empty() {
                transfer_object_name(&owner_name, other_id).ok();
            }
        }

        // Transfer vision and shroud clearing ranges.
        {
            let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
            let vision = obj_guard.get_vision_range();
            let shroud = obj_guard.get_shroud_clearing_range();
            drop(obj_guard);

            let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
            other_guard.set_vision_range(vision);
            other_guard.set_shroud_clearing_range(shroud);
        }

        // Mark as car bomb.
        {
            let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
            other_guard.set_status(ObjectStatusMaskType::IS_CAR_BOMB, true);
        }

        // Copy veterancy level.
        {
            let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
            let level = obj_guard.get_veterancy_level();
            drop(obj_guard);

            let other_guard = other.read().map_err(|_| GameError::LockError)?;
            if let Some(exp) = other_guard.get_experience_tracker() {
                if let Ok(mut exp_guard) = exp.lock() {
                    exp_guard.set_veterancy_level(level);
                }
            }
        }

        other
            .read()
            .map_err(|_| GameError::LockError)?
            .refresh_radar_object_from_state();

        Ok(true)
    }
}

impl LegacyCollideAdapter for ConvertToCarBombCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if ConvertToCarBombCrateCollide::is_valid_to_execute(self, other.clone())?
            && ConvertToCarBombCrateCollide::execute_crate_behavior(self, other.clone())?
        {
            self.base
                .finalize_collection(&other)
                .map_err(GameError::from)?;
        }

        Ok(())
    }

    fn legacy_would_like_to_collide_with(
        &self,
        other: Arc<RwLock<Object>>,
    ) -> Result<bool, GameError> {
        ConvertToCarBombCrateCollide::is_valid_to_execute(self, other)
    }
}

impl CrateCollideModule for ConvertToCarBombCrateCollide {
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        ConvertToCarBombCrateCollide::is_valid_to_execute(self, other)
    }

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        ConvertToCarBombCrateCollide::execute_crate_behavior(self, other)
    }
}

impl game_engine::common::system::Snapshotable for ConvertToCarBombCrateCollide {
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
