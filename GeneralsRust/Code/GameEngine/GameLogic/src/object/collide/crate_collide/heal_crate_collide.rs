//! Heal Crate Collision Module
//!
//! A crate that heals every object owned by the player who collects it.

use super::super::{CollisionError, Coord3D, GameObject};
use super::crate_collide::{CrateCollide, CrateCollideBehavior, CrateCollideModuleData};
use crate::common::*;
use crate::helpers::TheAudio;
use crate::object::collide::crate_collide::*;

/// Configuration data for HealCrateCollide.
///
/// C++ exposes only inherited CrateCollide module data for this module.
#[derive(Debug, Clone)]
pub struct HealCrateCollideModuleData {
    /// Base crate collision data
    pub base: CrateCollideModuleData,
}

impl HealCrateCollideModuleData {
    pub fn new() -> Self {
        Self {
            base: CrateCollideModuleData::new(),
        }
    }
}

impl Default for HealCrateCollideModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// Heal Crate Collide implementation.
pub struct HealCrateCollide {
    /// Base crate collision functionality
    base_crate: CrateCollide,
    /// Module-specific configuration
    module_data: HealCrateCollideModuleData,
}

impl HealCrateCollide {
    pub fn new(object_id: ObjectId, module_data: HealCrateCollideModuleData) -> Self {
        Self {
            base_crate: CrateCollide::new(object_id, module_data.base.clone()),
            module_data,
        }
    }

    pub fn get_module_data(&self) -> &HealCrateCollideModuleData {
        &self.module_data
    }

    /// Execute the C++ heal-crate behavior.
    pub fn execute_healing(&mut self, other: &dyn GameObject) -> Result<bool, CollisionError> {
        let Some(other_handle) = other.as_object_handle() else {
            return Ok(false);
        };
        let Some(player) = other_handle
            .read()
            .map_err(|_| CollisionError::InvalidObject("Failed to lock collector".to_string()))?
            .get_controlling_player()
        else {
            return Ok(false);
        };

        if let Ok(mut player_guard) = player.write() {
            player_guard.heal_all_objects();
        }

        self.play_heal_audio(&other.get_position());
        Ok(true)
    }

    fn play_heal_audio(&self, position: &Coord3D) {
        if let Some(audio) = TheAudio::get() {
            let event = TheAudio::get_misc_audio().crate_heal.clone();
            let mut audio_event = crate::common::audio::AudioEventRts::new(event.sound_type);
            audio_event.set_position(&(position.x, position.y, position.z));
            audio.add_audio_event(&audio_event);
        }
    }
}

impl CrateCollideBehavior for HealCrateCollide {
    fn execute_crate_behavior(&mut self, other: &dyn GameObject) -> Result<bool, CollisionError> {
        self.execute_healing(other)
    }

    fn is_valid_to_execute(&self, other: &dyn GameObject) -> bool {
        self.base_crate.is_valid_to_execute(other)
    }
}

/// Factory for creating HealCrateCollide modules.
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
}

impl game_engine::common::system::Snapshotable for HealCrateCollide {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base_crate.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        self.base_crate.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base_crate.load_post_process()
    }
}
