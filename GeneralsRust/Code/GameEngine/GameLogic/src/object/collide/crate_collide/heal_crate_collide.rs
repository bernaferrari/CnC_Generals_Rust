//! Heal Crate Collision Module
//!
//! This crate heals all objects owned by the player who collects it.
//! When a unit picks up this crate, all units belonging to that player
//! are restored to full health.

use super::super::{CollisionError, Coord3D, GameObject};
use super::crate_collide::{CrateCollide, CrateCollideBehavior, CrateCollideModuleData};
use crate::common::*;
use crate::helpers::{TheAudio, TheGameLogic};
use crate::object::collide::crate_collide::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Configuration data for HealCrateCollide
///
/// This module uses only the base CrateCollideModuleData as it doesn't
/// require any additional configuration parameters.
#[derive(Debug, Clone)]
pub struct HealCrateCollideModuleData {
    /// Base crate collision data
    pub base: CrateCollideModuleData,
    /// Optional healing amount multiplier (1.0 = full heal, 0.5 = half heal, etc.)
    pub heal_multiplier: f32,
    /// Whether to heal only units or include structures
    pub heal_structures: bool,
    /// Maximum range for healing effect (0 = unlimited)
    pub heal_range: f32,
}

impl HealCrateCollideModuleData {
    pub fn new() -> Self {
        Self {
            base: CrateCollideModuleData::new(),
            heal_multiplier: 1.0,  // Full heal by default
            heal_structures: true, // Heal structures as well
            heal_range: 0.0,       // Unlimited range by default
        }
    }

    pub fn with_heal_multiplier(mut self, multiplier: f32) -> Self {
        self.heal_multiplier = multiplier.max(0.0).min(1.0); // Clamp between 0 and 1
        self
    }

    pub fn with_heal_structures(mut self, heal_structures: bool) -> Self {
        self.heal_structures = heal_structures;
        self
    }

    pub fn with_heal_range(mut self, range: f32) -> Self {
        self.heal_range = range.max(0.0); // Cannot be negative
        self
    }
}

impl Default for HealCrateCollideModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// Healing statistics for tracking and reporting
#[derive(Debug, Clone)]
pub struct HealingStats {
    /// Number of units healed
    pub units_healed: u32,
    /// Number of structures healed
    pub structures_healed: u32,
    /// Total health restored
    pub total_health_restored: f32,
    /// Time when healing was performed
    pub heal_time: u64,
}

impl HealingStats {
    pub fn new() -> Self {
        Self {
            units_healed: 0,
            structures_healed: 0,
            total_health_restored: 0.0,
            heal_time: 0,
        }
    }
}

/// Heal state information
#[derive(Debug)]
struct HealState {
    /// Whether healing is currently in progress
    is_healing: bool,
    /// ID of the player being healed
    target_player_id: Option<PlayerId>,
    /// Healing start time
    heal_start_time: u64,
    /// Statistics from the last healing operation
    last_healing_stats: Option<HealingStats>,
}

/// Heal Crate Collide implementation
pub struct HealCrateCollide {
    /// Base crate collision functionality
    base_crate: CrateCollide,
    /// Module-specific configuration
    module_data: HealCrateCollideModuleData,
    /// Thread-safe healing state
    state: Arc<Mutex<HealState>>,
}

impl HealCrateCollide {
    pub fn new(object_id: ObjectId, module_data: HealCrateCollideModuleData) -> Self {
        Self {
            base_crate: CrateCollide::new(object_id, module_data.base.clone()),
            module_data,
            state: Arc::new(Mutex::new(HealState {
                is_healing: false,
                target_player_id: None,
                heal_start_time: 0,
                last_healing_stats: None,
            })),
        }
    }

    pub fn get_module_data(&self) -> &HealCrateCollideModuleData {
        &self.module_data
    }

    /// Execute the healing process for all objects owned by the player
    pub fn execute_healing(&mut self, other: &dyn GameObject) -> Result<bool, CollisionError> {
        let player_id = other.get_controlling_player();

        // Start healing state tracking
        {
            let mut state = self.state.lock().map_err(|e| {
                CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
            })?;
            state.is_healing = true;
            state.target_player_id = Some(player_id);
            state.heal_start_time = self.get_current_time()?;
        }

        // Heal all objects owned by the player
        let healing_stats = self.heal_all_player_objects(player_id, &other.get_position())?;

        // Play healing audio effect
        self.play_heal_audio(&other.get_position())?;

        // Store healing statistics
        {
            let mut state = self.state.lock().map_err(|e| {
                CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
            })?;
            state.is_healing = false;
            state.last_healing_stats = Some(healing_stats);
        }

        Ok(true)
    }

    /// Heal all objects owned by a specific player
    fn heal_all_player_objects(
        &self,
        player_id: PlayerId,
        crate_position: &Coord3D,
    ) -> Result<HealingStats, CollisionError> {
        let mut stats = HealingStats::new();
        stats.heal_time = self.get_current_time()?;

        // Get all objects owned by the player
        let player_objects = self.get_all_player_objects(player_id)?;

        for object in player_objects.iter() {
            let object = object.as_ref();
            // Skip dead objects
            if object.is_effectively_dead() {
                continue;
            }

            // Check range if specified
            if self.module_data.heal_range > 0.0 {
                let distance = crate_position.distance_to(&object.get_position());
                if distance > self.module_data.heal_range {
                    continue;
                }
            }

            // Check if we should heal structures
            if self.is_structure(object) && !self.module_data.heal_structures {
                continue;
            }

            // Perform healing
            let health_restored = self.heal_object(object)?;

            if health_restored > 0.0 {
                stats.total_health_restored += health_restored;

                if self.is_structure(object) {
                    stats.structures_healed += 1;
                } else {
                    stats.units_healed += 1;
                }
            }
        }

        Ok(stats)
    }

    /// Heal a specific object
    fn heal_object(&self, object: &dyn GameObject) -> Result<f32, CollisionError> {
        let current_health = self.get_object_health(object)?;
        let max_health = self.get_object_max_health(object)?;

        if current_health >= max_health {
            return Ok(0.0); // Already at full health
        }

        let heal_amount = (max_health - current_health) * self.module_data.heal_multiplier;
        let new_health = current_health + heal_amount;

        self.set_object_health(object, new_health)?;

        // Play healing FX on the object
        self.play_heal_fx_on_object(object)?;

        Ok(heal_amount)
    }

    /// Get the last healing statistics
    pub fn get_last_healing_stats(&self) -> Result<Option<HealingStats>, CollisionError> {
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(state.last_healing_stats.clone())
    }

    /// Check if currently healing
    pub fn is_healing(&self) -> Result<bool, CollisionError> {
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(state.is_healing)
    }

    /// Get the player being healed (if any)
    pub fn get_target_player(&self) -> Result<Option<PlayerId>, CollisionError> {
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(state.target_player_id)
    }

    // Helper methods that would interface with the game engine
    fn get_current_time(&self) -> Result<u64, CollisionError> {
        Ok(TheGameLogic::get_frame() as u64)
    }

    fn get_all_player_objects(
        &self,
        _player_id: PlayerId,
    ) -> Result<Vec<Box<dyn GameObject>>, CollisionError> {
        // Would get all objects owned by the specified player
        Ok(Vec::new())
    }

    fn is_structure(&self, _object: &dyn GameObject) -> bool {
        // Would check if object is a structure/building
        false
    }

    fn get_object_health(&self, _object: &dyn GameObject) -> Result<f32, CollisionError> {
        // Would get current health of the object
        Ok(50.0) // Example current health
    }

    fn get_object_max_health(&self, _object: &dyn GameObject) -> Result<f32, CollisionError> {
        // Would get maximum health of the object
        Ok(100.0) // Example max health
    }

    fn set_object_health(
        &self,
        _object: &dyn GameObject,
        _health: f32,
    ) -> Result<(), CollisionError> {
        // Would set the health of the object
        Ok(())
    }

    fn play_heal_audio(&self, position: &Coord3D) -> Result<(), CollisionError> {
        // C++ parity: use MiscAudio::m_crateHeal at world position.
        if let Some(audio) = TheAudio::get() {
            let event = TheAudio::get_misc_audio().crate_heal.clone();
            let mut audio_event = crate::common::audio::AudioEventRts::new(event.sound_type);
            audio_event.set_position(&(position.x, position.y, position.z));
            audio.add_audio_event(&audio_event);
        }
        Ok(())
    }

    fn play_heal_fx_on_object(&self, _object: &dyn GameObject) -> Result<(), CollisionError> {
        // Would play healing visual effects on the object
        Ok(())
    }
}

impl CrateCollideBehavior for HealCrateCollide {
    fn execute_crate_behavior(&mut self, other: &dyn GameObject) -> Result<bool, CollisionError> {
        self.execute_healing(other)
    }

    fn is_valid_to_execute(&self, other: &dyn GameObject) -> bool {
        // Use base validation - healing doesn't require additional restrictions
        self.base_crate.is_valid_to_execute(other)
    }
}

/// Factory for creating HealCrateCollide modules
pub struct HealCrateCollideFactory;

impl HealCrateCollideFactory {
    pub fn create(object_id: ObjectId) -> HealCrateCollide {
        let data = HealCrateCollideModuleData::new();
        HealCrateCollide::new(object_id, data)
    }

    pub fn create_with_config(
        object_id: ObjectId,
        config: HealCrateCollideModuleData,
    ) -> HealCrateCollide {
        HealCrateCollide::new(object_id, config)
    }

    pub fn create_partial_heal(object_id: ObjectId, heal_percentage: f32) -> HealCrateCollide {
        let data = HealCrateCollideModuleData::new().with_heal_multiplier(heal_percentage);
        HealCrateCollide::new(object_id, data)
    }

    pub fn create_units_only(object_id: ObjectId) -> HealCrateCollide {
        let data = HealCrateCollideModuleData::new().with_heal_structures(false);
        HealCrateCollide::new(object_id, data)
    }

    pub fn create_ranged_heal(object_id: ObjectId, range: f32) -> HealCrateCollide {
        let data = HealCrateCollideModuleData::new().with_heal_range(range);
        HealCrateCollide::new(object_id, data)
    }
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.

impl game_engine::common::system::Snapshotable for HealCrateCollide {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base_crate.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // C++ parity: versioned xfer entry point (current version 1).
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        self.base_crate.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base_crate.load_post_process()
    }
}
