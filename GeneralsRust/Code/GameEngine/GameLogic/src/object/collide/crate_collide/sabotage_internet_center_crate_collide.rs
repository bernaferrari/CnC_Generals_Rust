//! Sabotage Internet Center Crate Collide Module
//!
//! A crate (actually a saboteur - mobile crate) that temporarily disables an internet center.
//! Author: Kris Morness, July 2003 (original C++), converted to Rust

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock};

// Import types that would be defined in other modules
use crate::ai::*;
use crate::common::*;
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;

/// Module data for sabotage internet center crate collide behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabotageInternetCenterCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
    /// Duration of sabotage effect in frames
    pub sabotage_frames: u32,
}

impl Default for SabotageInternetCenterCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
            sabotage_frames: 0,
        }
    }
}

impl SabotageInternetCenterCrateCollideModuleData {
    /// Build field parser for INI configuration
    pub fn build_field_parse() -> Vec<FieldParse> {
        let mut fields = LegacyCrateCollideModuleData::build_field_parse();
        fields.extend(vec![FieldParse::new(
            "SabotageDuration",
            FieldType::DurationUnsignedInt,
            "sabotage_frames",
        )]);
        fields
    }
}

/// Sabotage Internet Center Crate Collide module
#[derive(Debug)]
pub struct SabotageInternetCenterCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<SabotageInternetCenterCrateCollideModuleData>>,
}

impl SabotageInternetCenterCrateCollide {
    /// Create new sabotage internet center crate collide module
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: SabotageInternetCenterCrateCollideModuleData,
    ) -> Self {
        Self {
            base: LegacyCrateCollide::from_object_handle(object, module_data.base.clone()),
            module_data: Arc::new(Mutex::new(module_data)),
        }
    }

    /// Check if this is a valid target for execution
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        // First check base validation
        if !self.base.is_valid_to_execute(&other) {
            return Ok(false);
        }

        let other_lock = other.read().map_err(|_| GameError::LockError)?;

        // Can't sabotage dead structures
        if other_lock.is_effectively_dead() {
            return Ok(false);
        }

        // We can only sabotage internet centers
        if !other_lock.is_kind_of(KindOf::FSInternetCenter) {
            return Ok(false);
        }

        // Can only sabotage enemy buildings
        let relationship = self
            .base
            .get_object()
            .map_err(GameError::from)?
            .read()
            .map_err(|_| GameError::LockError)?
            .relationship_to(&other_lock);

        if relationship != Relationship::Enemies {
            return Ok(false);
        }

        Ok(true)
    }

    /// Execute the crate behavior
    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        // Check to make sure that the other object is also the goal object in the AIUpdateInterface
        // in order to prevent an unintentional conversion simply by having the terrorist walk too close
        let object = self.base.get_object().map_err(GameError::from)?;
        let object_lock = object.read().map_err(|_| GameError::LockError)?;
        let other_id = other.read().map_err(|_| GameError::LockError)?.get_id();

        // Check AI goal object - only execute if this is the intentional target
        if let Some(ai_update) = object_lock.get_ai_update_interface() {
            let goal_id = ai_update
                .lock()
                .ok()
                .and_then(|ai_guard| ai_guard.get_goal_object())
                .and_then(|goal_obj| goal_obj.read().ok().map(|goal_guard| goal_guard.get_id()));
            if goal_id != Some(other_id) {
                log::debug!(
                    "SabotageInternetCenter: Skipping - target {} is not current goal {:?}",
                    other_id,
                    goal_id
                );
                return Ok(false);
            }
        }
        drop(object_lock);

        // Try infiltration event
        TheRadar::try_infiltration_event(other.clone())?;

        // Do sabotage feedback FX
        self.base
            .do_sabotage_feedback_fx(&other, SabotageVictimType::InternetCenter)?;

        // Play eva sound if locally controlled
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                TheEva::set_should_play(EvaEvent::BuildingSabotaged)?;
            }
        }

        // Calculate disable frame
        let module_data = self.module_data.lock().map_err(|_| GameError::LockError)?;
        let disable_frame = TheGameLogic::get_frame() + module_data.sabotage_frames;
        drop(module_data);

        // Disable all internet center spy visions (they stack) without visually disabling the other centers
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if let Some(controlling_player) = other_lock.get_controlling_player() {
                let player_guard = controlling_player
                    .read()
                    .map_err(|_| GameError::LockError)?;
                player_guard.iterate_objects(|obj| {
                    disable_internet_center_spy_vision(obj, disable_frame)
                })?;
            }
        }

        // Disable the internet center
        {
            let mut other_lock = other.write().map_err(|_| GameError::LockError)?;
            other_lock.set_disabled_until(DisabledType::DisabledHacked, disable_frame);
        }

        // Disable all the hackers inside
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if let Some(contain) = other_lock.get_contain() {
                let contain_guard = contain.lock().map_err(|_| GameError::LockError)?;
                let contained_ids: Vec<ObjectID> = contain_guard.get_contained_objects().to_vec();
                drop(contain_guard);
                for object_id in contained_ids {
                    if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                        disable_hacker(obj, disable_frame)?;
                    }
                }
            }
        }

        Ok(true)
    }

    /// Check if this is a sabotage building crate collide
    pub fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl LegacyCollideAdapter for SabotageInternetCenterCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if SabotageInternetCenterCrateCollide::is_valid_to_execute(self, other.clone())? {
            let success =
                SabotageInternetCenterCrateCollide::execute_crate_behavior(self, other.clone())?;
            self.base
                .finish_execution_attempt(&other, success)
                .map_err(GameError::from)?;
        }

        Ok(())
    }

    fn legacy_would_like_to_collide_with(
        &self,
        other: Arc<RwLock<Object>>,
    ) -> Result<bool, GameError> {
        SabotageInternetCenterCrateCollide::is_valid_to_execute(self, other)
    }
}

impl CrateCollideModule for SabotageInternetCenterCrateCollide {
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        SabotageInternetCenterCrateCollide::is_valid_to_execute(self, other)
    }

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        SabotageInternetCenterCrateCollide::execute_crate_behavior(self, other)
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

/// Disable hacker callback function
fn disable_hacker(obj: Arc<RwLock<Object>>, frame: u32) -> Result<(), GameError> {
    let mut obj_lock = obj.write().map_err(|_| GameError::LockError)?;
    obj_lock.set_disabled_until(DisabledType::DisabledHacked, frame);
    Ok(())
}

/// Disable internet center spy vision callback function  
fn disable_internet_center_spy_vision(
    obj: Arc<RwLock<Object>>,
    frame: u32,
) -> Result<(), GameError> {
    let obj_lock = obj.read().map_err(|_| GameError::LockError)?;

    if obj_lock.is_kind_of(KindOf::FSInternetCenter) {
        let disabled = obj_lock
            .find_update_module("SpyVisionUpdate")
            .and_then(|module| {
                module.with_module(|module| {
                    module.get_spy_vision_control_interface().map(|spy_vision| {
                        spy_vision.set_disabled_until_frame(frame);
                    })
                })
            })
            .is_some();

        if !disabled {
            for module in obj_lock.get_behavior_modules() {
                if let Ok(mut module_guard) = module.lock() {
                    if let Some(spy_vision) = module_guard.get_spy_vision_control_interface() {
                        spy_vision.set_disabled_until_frame(frame);
                    }
                }
            }
        }
    }

    Ok(())
}

impl game_engine::common::system::Snapshotable for SabotageInternetCenterCrateCollide {
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
