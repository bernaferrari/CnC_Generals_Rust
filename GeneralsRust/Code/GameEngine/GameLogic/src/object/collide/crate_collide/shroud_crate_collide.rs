//! Shroud Crate Collision Module
//!
//! FILE: shroud_crate_collide.rs
//! Author: Converted from Graham Smallwood's C++ implementation, March 2002
//! Desc: A crate that clears the shroud for the picker-upper

use super::*;
use crate::helpers::TheAudio;
use crate::object::collide::crate_collide::crate_collide::CrateCollide as LegacyCrateCollide;
use crate::object::collide::*;

/// Shroud Crate Collide Module
///
/// This module implements a crate that reveals the entire map (clears shroud)
/// for the player who picks it up.
pub struct ShroudCrateCollide {
    base: LegacyCrateCollide,
    version: u32,
}

impl ShroudCrateCollide {
    /// Create a new ShroudCrateCollide instance
    ///
    /// # Arguments
    /// * `object_id` - The ID of the object this module belongs to
    /// * `module_data` - Configuration data for the crate collision behavior
    pub fn new(object_id: ObjectId, module_data: CrateCollideModuleData) -> Self {
        Self {
            base: LegacyCrateCollide::new(object_id, module_data),
            version: 1,
        }
    }

    /// Get the current version of this module for serialization
    pub fn get_version(&self) -> u32 {
        self.version
    }
}

impl CollideModule for ShroudCrateCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        if let Some(other_obj) = other {
            if self.base.is_valid_to_execute(other_obj) {
                // Execute the shroud crate behavior
                let success = self.execute_crate_behavior_internal(other_obj)?;
                if !success {
                    return Err(CollisionError::InvalidObject(
                        "Failed to execute shroud crate behavior".to_string(),
                    ));
                }
                self.base.finalize_collection(other_obj)?;
            }
        }

        Ok(())
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        self.base.is_valid_to_execute(other)
    }
}

impl ShroudCrateCollide {
    /// Internal implementation of crate behavior execution
    ///
    /// This method reveals the entire map for the controlling player of the object
    /// that collided with this crate, and plays a crate pickup sound.
    ///
    /// # Arguments
    /// * `other` - The object that collided with this crate
    ///
    /// # Returns
    /// * `Ok(true)` if the crate behavior was successfully executed
    /// * `Ok(false)` if the behavior could not be executed
    /// * `Err(CollisionError)` if an error occurred during execution
    fn execute_crate_behavior_internal(
        &self,
        other: &dyn GameObject,
    ) -> Result<bool, CollisionError> {
        // Get the controlling player of the object that picked up the crate
        let crate_player = other.get_controlling_player();
        let player_id = crate_player.value() as u32;

        // Reveal the entire map for this player
        let mut shroud_manager = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|e| {
                CollisionError::PartitionManagerError(format!(
                    "Failed to lock shroud manager: {}",
                    e
                ))
            })?;
        shroud_manager
            .reveal_map_for_player(player_id)
            .map_err(CollisionError::PartitionManagerError)?;

        // C++ parity: use MiscAudio::m_crateShroud and bind the event to the picker object ID.
        if let Some(audio) = TheAudio::get() {
            let event = TheAudio::get_misc_audio().crate_shroud.clone();
            let mut audio_event = crate::common::audio::AudioEventRts::new(event.sound_type);
            audio_event.set_object_id(other.get_id());
            audio.add_audio_event(&audio_event);
        }

        Ok(true)
    }
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.

impl game_engine::common::system::Snapshotable for ShroudCrateCollide {
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
