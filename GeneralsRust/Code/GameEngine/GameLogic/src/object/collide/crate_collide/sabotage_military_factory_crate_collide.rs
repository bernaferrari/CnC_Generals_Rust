//! Sabotage Military Factory Crate Collide Module
//!
//! A crate (actually a saboteur - mobile crate) that temporarily disables military factories.
//! Author: Kris Morness, June 2003 (original C++), converted to Rust

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
use crate::object::*;

/// Module data for sabotage military factory crate collide behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabotageMilitaryFactoryCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
    /// Duration of sabotage effect in frames
    pub sabotage_frames: u32,
}

impl Default for SabotageMilitaryFactoryCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
            sabotage_frames: 0,
        }
    }
}

impl SabotageMilitaryFactoryCrateCollideModuleData {
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

/// Sabotage Military Factory Crate Collide module
#[derive(Debug)]
pub struct SabotageMilitaryFactoryCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<SabotageMilitaryFactoryCrateCollideModuleData>>,
}

impl SabotageMilitaryFactoryCrateCollide {
    /// Create new sabotage military factory crate collide module
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: SabotageMilitaryFactoryCrateCollideModuleData,
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

        // Can't sabotage aircraft carriers
        if other_lock.is_kind_of(KindOf::AircraftCarrier) {
            return Ok(false);
        }

        // We can only sabotage military factory buildings
        if !other_lock.is_kind_of(KindOf::FSBarracks)
            && !other_lock.is_kind_of(KindOf::FSWarfactory)
            && !other_lock.is_kind_of(KindOf::FSAirfield)
        {
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

        if let Some(ai) = object_lock.get_ai_update_interface() {
            let goal_id = ai
                .lock()
                .ok()
                .and_then(|ai_guard| ai_guard.get_goal_object())
                .and_then(|goal| goal.read().ok().map(|goal_guard| goal_guard.get_id()));
            if goal_id != Some(other_id) {
                return Ok(false);
            }
        }
        drop(object_lock);

        // Try infiltration event
        TheRadar::try_infiltration_event(other.clone())?;

        // Do sabotage feedback FX
        self.base
            .do_sabotage_feedback_fx(&other, SabotageVictimType::MilitaryFactory)?;

        // Play eva sound if locally controlled
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                TheEva::set_should_play(EvaEvent::BuildingSabotaged)?;
            }
        }

        // Calculate disable frame and disable the factory
        let module_data = self.module_data.lock().map_err(|_| GameError::LockError)?;
        let disable_frame = TheGameLogic::get_frame() + module_data.sabotage_frames;
        drop(module_data);

        {
            let mut other_lock = other.write().map_err(|_| GameError::LockError)?;
            other_lock.set_disabled_until(DisabledType::DisabledHacked, disable_frame);
        }

        Ok(true)
    }

    /// Check if this is a sabotage building crate collide
    pub fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl LegacyCollideAdapter for SabotageMilitaryFactoryCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if SabotageMilitaryFactoryCrateCollide::is_valid_to_execute(self, other.clone())?
            && SabotageMilitaryFactoryCrateCollide::execute_crate_behavior(self, other.clone())?
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
        SabotageMilitaryFactoryCrateCollide::is_valid_to_execute(self, other)
    }
}

impl CrateCollideModule for SabotageMilitaryFactoryCrateCollide {
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        SabotageMilitaryFactoryCrateCollide::is_valid_to_execute(self, other)
    }

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        SabotageMilitaryFactoryCrateCollide::execute_crate_behavior(self, other)
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl game_engine::common::system::Snapshotable for SabotageMilitaryFactoryCrateCollide {
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
