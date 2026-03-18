//! Special Power Integration Layer
//!
//! This module provides the integration between special powers and the game engine systems.
//! It acts as a bridge to connect special powers with:
//! - Object Manager (for accessing game objects)
//! - AI Update System (for issuing attack commands)
//! - Player System (for cooldown synchronization and science checks)
//! - Terrain Logic (for pathfinding and edge detection)
//! - Partition Manager (for finding objects in radius)
//! - Game Logic (for frame counting and timing)
//!
//! Port of integration logic from C++ SpecialPowerModule.cpp and related files.

use super::base_power::*;
use super::types::*;
use crate::common::*;
use crate::object::Object;
use crate::player::MoneyInterface;
use game_engine::common::game_common::MAX_TURRETS;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// Frame counter type for synchronization
pub type FrameCount = UnsignedInt;

/// Object manager interface for special power integration
/// Provides access to game objects needed by special powers
pub trait ObjectManagerInterface: Send + Sync {
    /// Get object by ID
    fn get_object(&self, object_id: ObjectID) -> Option<Arc<RwLock<Object>>>;

    /// Check if object is disabled
    fn is_object_disabled(&self, object_id: ObjectID) -> bool;

    /// Get object position
    fn get_object_position(&self, object_id: ObjectID) -> Option<Coord3D>;

    /// Reload all ammunition for an object
    /// Matches C++ Object::reloadAllAmmo(TRUE)
    fn reload_all_ammo(&mut self, object_id: ObjectID, force: bool) -> Result<(), String>;
}

/// AI update interface for weapon firing commands
/// Matches C++ AIUpdateInterface
pub trait AIUpdateInterface: Send + Sync {
    /// Issue attack position command
    /// Matches C++ ai->aiAttackPosition(location, shot_count, CMD_FROM_AI)
    fn ai_attack_position(
        &mut self,
        object_id: ObjectID,
        location: Option<&Coord3D>,
        shot_count: UnsignedInt,
        command_source: CommandSource,
    ) -> Result<(), String>;

    /// Issue attack object command
    /// Matches C++ ai->aiAttackObject(target, shot_count, CMD_FROM_AI)
    fn ai_attack_object(
        &mut self,
        object_id: ObjectID,
        target_id: ObjectID,
        shot_count: UnsignedInt,
        command_source: CommandSource,
    ) -> Result<(), String>;

    /// Set turret target position
    /// Matches C++ ai->setTurretTargetPosition(turret_type, location)
    fn set_turret_target_position(
        &mut self,
        object_id: ObjectID,
        turret_index: usize,
        location: &Coord3D,
    ) -> Result<(), String>;

    /// Set turret target object
    /// Matches C++ ai->setTurretTargetObject(turret_type, target)
    fn set_turret_target_object(
        &mut self,
        object_id: ObjectID,
        turret_index: usize,
        target_id: ObjectID,
    ) -> Result<(), String>;
}

/// Command source enum
/// Matches C++ command source flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSource {
    FromAI,
    FromPlayer,
    FromScript,
}

/// Player interface for special power cooldown management
/// Matches C++ Player class special power methods
pub trait PlayerInterface: Send + Sync {
    /// Get or start special power ready frame for SharedNSync powers
    /// Matches C++ player->getOrStartSpecialPowerReadyFrame(template)
    fn get_or_start_special_power_ready_frame(
        &self,
        power_id: SpecialPowerID,
        current_frame: FrameCount,
    ) -> FrameCount;

    /// Express special power ready frame (set to specific frame)
    /// Matches C++ player->expressSpecialPowerReadyFrame(template, frame)
    fn express_special_power_ready_frame(&mut self, power_id: SpecialPowerID, frame: FrameCount);

    /// Reset or start special power ready frame
    /// Matches C++ player->resetOrStartSpecialPowerReadyFrame(template)
    fn reset_or_start_special_power_ready_frame(
        &mut self,
        power_id: SpecialPowerID,
        current_frame: FrameCount,
        reload_time: FrameCount,
    );

    /// Check if player has specific science
    /// Matches C++ player->hasScience(science_type)
    fn has_science(&self, science_name: &str) -> bool;

    /// Get player index
    fn get_player_index(&self) -> UnsignedInt;

    /// Whether the player builds instantly (debug/free build).
    fn builds_instantly(&self) -> bool;

    /// Access to player's money interface.
    fn get_money(&self) -> &dyn MoneyInterface;

    /// Build time modifier (handicap + power).
    fn get_build_time_modifier(&self) -> f32;

    /// Cost modifier (handicap + cheats).
    fn get_cost_modifier(&self) -> f32;
}

/// Terrain logic interface for edge detection
/// Matches C++ TerrainLogic class
pub trait TerrainLogicInterface: Send + Sync {
    /// Find closest edge point to location
    /// Matches C++ TheTerrainLogic->findClosestEdgePoint(location)
    fn find_closest_edge_point(&self, location: &Coord3D) -> Coord3D;

    /// Find farthest edge point from location
    /// Matches C++ TheTerrainLogic->findFarthestEdgePoint(location)
    fn find_farthest_edge_point(&self, location: &Coord3D) -> Coord3D;

    /// Check if location is passable
    fn is_passable(&self, location: &Coord3D) -> bool;
}

/// Partition manager interface for object queries
/// Matches C++ PartitionManager
pub trait PartitionManagerInterface: Send + Sync {
    /// Find objects in radius
    fn find_objects_in_radius(
        &self,
        center: &Coord3D,
        radius: Real,
        filter: Option<ObjectFilter>,
    ) -> Vec<ObjectID>;

    /// Find position around a location
    /// Matches C++ ThePartitionManager->findPositionAround()
    fn find_position_around(
        &self,
        location: &Coord3D,
        max_radius: Real,
        flags: FindPositionFlags,
    ) -> Option<Coord3D>;
}

/// Object filter for partition queries
#[derive(Debug, Clone, Copy)]
pub enum ObjectFilter {
    All,
    Infantry,
    Vehicles,
    Structures,
    Aircraft,
    Enemy,
    Friendly,
}

// Find position flags
// Matches C++ FindPositionOptions flags
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct FindPositionFlags: u32 {
        const CLEAR_CELLS_ONLY = 1 << 0;
        const PASSABLE = 1 << 1;
        const NO_WATER = 1 << 2;
    }
}

/// Game logic interface for frame counting
/// Matches C++ GameLogic
pub trait GameLogicInterface: Send + Sync {
    /// Get current game frame
    /// Matches C++ TheGameLogic->getFrame()
    fn get_frame(&self) -> FrameCount;

    /// Get frames per second
    fn get_fps(&self) -> Real;

    /// Convert seconds to frames
    fn seconds_to_frames(&self, seconds: Real) -> FrameCount {
        (seconds * self.get_fps()) as FrameCount
    }

    /// Convert frames to seconds
    fn frames_to_seconds(&self, frames: FrameCount) -> Real {
        frames as Real / self.get_fps()
    }
}

/// Default game-logic bridge backed by the global `TheGameLogic` singleton.
pub struct TheGameLogicBridge;

impl GameLogicInterface for TheGameLogicBridge {
    fn get_frame(&self) -> FrameCount {
        crate::helpers::TheGameLogic::get_frame()
    }

    fn get_fps(&self) -> Real {
        crate::common::LOGICFRAMES_PER_SECOND as Real
    }
}

/// Object creation list interface
/// Matches C++ ObjectCreationList::create() methods
pub trait ObjectCreationListInterface: Send + Sync {
    /// Create objects from OCL at location
    /// Matches C++ ObjectCreationList::create(ocl, owner, creation_pos, target_pos, angle)
    fn create_ocl(
        &mut self,
        ocl_name: &str,
        owner_id: ObjectID,
        creation_pos: &Coord3D,
        target_pos: &Coord3D,
        angle: Real,
    ) -> Result<Vec<ObjectID>, String>;
}

/// Special power integration context
/// Aggregates all integration interfaces needed by special powers
pub struct SpecialPowerIntegrationContext {
    pub object_manager: Option<Arc<RwLock<dyn ObjectManagerInterface>>>,
    pub ai_update: Option<Arc<RwLock<dyn AIUpdateInterface>>>,
    pub player: Option<Arc<RwLock<dyn PlayerInterface>>>,
    pub terrain_logic: Option<Arc<RwLock<dyn TerrainLogicInterface>>>,
    pub partition_manager: Option<Arc<RwLock<dyn PartitionManagerInterface>>>,
    pub game_logic: Option<Arc<RwLock<dyn GameLogicInterface>>>,
    pub ocl_system: Option<Arc<RwLock<dyn ObjectCreationListInterface>>>,
}

impl SpecialPowerIntegrationContext {
    pub fn new() -> Self {
        Self {
            object_manager: None,
            ai_update: None,
            player: None,
            terrain_logic: None,
            partition_manager: None,
            game_logic: None,
            ocl_system: None,
        }
    }

    /// Get current game frame
    pub fn get_current_frame(&self) -> FrameCount {
        if let Some(game_logic) = &self.game_logic {
            if let Ok(logic) = game_logic.read() {
                return logic.get_frame();
            }
        }
        0 // Default to frame 0 if not available
    }

    /// Check if object is disabled
    pub fn is_object_disabled(&self, object_id: ObjectID) -> bool {
        if let Some(obj_mgr) = &self.object_manager {
            if let Ok(mgr) = obj_mgr.read() {
                return mgr.is_object_disabled(object_id);
            }
        }
        false
    }

    /// Execute fire weapon command at location
    /// Integrates with AI and object systems
    pub fn execute_fire_weapon_at_location(
        &self,
        owner_id: ObjectID,
        location: &Coord3D,
        max_shots: UnsignedInt,
    ) -> Result<(), String> {
        // Check if disabled
        if self.is_object_disabled(owner_id) {
            return Err("Object is disabled".to_string());
        }

        // Reload ammunition
        if let Some(obj_mgr) = &self.object_manager {
            if let Ok(mut mgr) = obj_mgr.write() {
                mgr.reload_all_ammo(owner_id, true)?;
            }
        }

        // Issue attack command
        if let Some(ai) = &self.ai_update {
            if let Ok(mut ai_sys) = ai.write() {
                ai_sys.ai_attack_position(
                    owner_id,
                    Some(location),
                    max_shots,
                    CommandSource::FromAI,
                )?;

                // Order turrets to attack as well
                for i in 0..MAX_TURRETS {
                    let _ = ai_sys.set_turret_target_position(owner_id, i, location);
                }
            }
        }

        Ok(())
    }

    /// Execute fire weapon command at object
    pub fn execute_fire_weapon_at_object(
        &self,
        owner_id: ObjectID,
        target_id: ObjectID,
        max_shots: UnsignedInt,
    ) -> Result<(), String> {
        if self.is_object_disabled(owner_id) {
            return Err("Object is disabled".to_string());
        }

        // Reload ammunition
        if let Some(obj_mgr) = &self.object_manager {
            if let Ok(mut mgr) = obj_mgr.write() {
                mgr.reload_all_ammo(owner_id, true)?;
            }
        }

        // Issue attack command
        if let Some(ai) = &self.ai_update {
            if let Ok(mut ai_sys) = ai.write() {
                ai_sys.ai_attack_object(owner_id, target_id, max_shots, CommandSource::FromAI)?;

                // Order turrets to attack as well
                for i in 0..MAX_TURRETS {
                    let _ = ai_sys.set_turret_target_object(owner_id, i, target_id);
                }
            }
        }

        Ok(())
    }
}

impl Default for SpecialPowerIntegrationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Global integration context (singleton).
static INTEGRATION_CONTEXT: OnceLock<Arc<RwLock<SpecialPowerIntegrationContext>>> = OnceLock::new();

/// Initialize the global integration context
pub fn initialize_integration_context() {
    let _ = INTEGRATION_CONTEXT
        .get_or_init(|| Arc::new(RwLock::new(SpecialPowerIntegrationContext::new())));
}

/// Get the global integration context
pub fn get_integration_context() -> Option<Arc<RwLock<SpecialPowerIntegrationContext>>> {
    INTEGRATION_CONTEXT.get().cloned()
}

/// Set object manager
pub fn set_object_manager(manager: Arc<RwLock<dyn ObjectManagerInterface>>) {
    if let Some(context) = get_integration_context() {
        if let Ok(mut ctx) = context.write() {
            ctx.object_manager = Some(manager);
        }
    }
}

/// Set AI update system
pub fn set_ai_update(ai: Arc<RwLock<dyn AIUpdateInterface>>) {
    if let Some(context) = get_integration_context() {
        if let Ok(mut ctx) = context.write() {
            ctx.ai_update = Some(ai);
        }
    }
}

/// Set game logic
pub fn set_game_logic(logic: Arc<RwLock<dyn GameLogicInterface>>) {
    if let Some(context) = get_integration_context() {
        if let Ok(mut ctx) = context.write() {
            ctx.game_logic = Some(logic);
        }
    }
}

/// Set player interface
pub fn set_player(player: Arc<RwLock<dyn PlayerInterface>>) {
    if let Some(context) = get_integration_context() {
        if let Ok(mut ctx) = context.write() {
            ctx.player = Some(player);
        }
    }
}

/// Set terrain logic
pub fn set_terrain_logic(logic: Arc<RwLock<dyn TerrainLogicInterface>>) {
    if let Some(context) = get_integration_context() {
        if let Ok(mut ctx) = context.write() {
            ctx.terrain_logic = Some(logic);
        }
    }
}

/// Set partition manager
pub fn set_partition_manager(manager: Arc<RwLock<dyn PartitionManagerInterface>>) {
    if let Some(context) = get_integration_context() {
        if let Ok(mut ctx) = context.write() {
            ctx.partition_manager = Some(manager);
        }
    }
}

/// Set object creation list (OCL) system.
pub fn set_ocl_system(system: Arc<RwLock<dyn ObjectCreationListInterface>>) {
    if let Some(context) = get_integration_context() {
        if let Ok(mut ctx) = context.write() {
            ctx.ocl_system = Some(system);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration_context_creation() {
        let context = SpecialPowerIntegrationContext::new();
        assert!(context.object_manager.is_none());
        assert!(context.ai_update.is_none());
    }

    #[test]
    fn test_command_source() {
        assert_eq!(CommandSource::FromAI, CommandSource::FromAI);
        assert_ne!(CommandSource::FromAI, CommandSource::FromPlayer);
    }

    #[test]
    fn test_find_position_flags() {
        let flags = FindPositionFlags::CLEAR_CELLS_ONLY | FindPositionFlags::PASSABLE;
        assert!(flags.contains(FindPositionFlags::CLEAR_CELLS_ONLY));
        assert!(flags.contains(FindPositionFlags::PASSABLE));
        assert!(!flags.contains(FindPositionFlags::NO_WATER));
    }
}
