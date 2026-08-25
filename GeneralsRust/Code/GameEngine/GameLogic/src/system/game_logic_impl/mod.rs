//! Core GameLogic orchestration layer - The main game loop
//!
//! This module implements the critical game logic core that orchestrates all game systems.
//! It serves as the heartbeat of the game, executing all subsystems in the proper order
//! each frame to maintain game state and simulation determinism.
//!
//! ## Architecture
//!
//! The GameLogic system is organized as a singleton that manages:
//! - Object lifecycle (registration, update, destruction)
//! - Frame-by-frame update orchestration
//! - AI player updates
//! - Command queue processing
//! - Physics and damage resolution
//! - Scripting and victory condition evaluation
//! - Vision and fog-of-war updates
//!
//! ## Update Loop Phases (CRITICAL ORDER)
//!
//! The update loop executes in this exact order every frame, matching
//! `GameLogic::update()` in `GameLogic.cpp` (lines 3548-3803):
//!
//! 1. **Frame Setup**: Sync frame counter to GameClient (C++ line 3595)
//! 2. **Early Scripting**: ScriptEngine update before objects (C++ line 3600)
//! 3. **Time Freeze Check**: Skip if frozen by script/debug (C++ lines 3603-3617)
//! 4. **Terrain Update**: Bridge/water state changes (C++ line 3622)
//! 5. **Pre-Update**: Clear frame events, reset temporary flags
//! 6. **Command Processing**: Process player input commands (C++ line 3669)
//! 7. **Normal Update Modules**: Every-frame update modules (C++ lines 3672-3694)
//! 8. **Sleepy Update Modules**: Delayed update modules (C++ lines 3697-3738)
//!    - StealthUpdate, AIUpdate, behavior modules, etc.
//! 9. **AI Update**: TheAI->UPDATE() (C++ line 3743)
//! 10. **Production/Build**: BuildAssistant update (C++ line 3748)
//! 11. **Damage/Physics Resolution**: Deferred damage, collisions
//! 12. **Partition Manager Update**: Spatial grid rebuild (C++ line 3753)
//! 13. **Death/Cleanup**: processDestroyList() (C++ line 3762)
//! 14. **Weapon Store Update**: Delayed weapon damage (C++ line 3767)
//! 15. **Victory Conditions**: Win/loss evaluation (C++ line 3769)
//! 16. **Disabled Status Check**: Re-enable expired disables (C++ lines 3783-3792)
//! 17. **Vision/Shroud Update**: Fog of war, radar
//! 18. **Frame Increment**: m_frame++ (C++ line 3801, caller-managed)
//!
//! ## Stealth System Integration
//!
//! The stealth system is fully integrated into the update loop via StealthUpdate modules:
//!
//! ### Stealth Activation/Deactivation
//! - StealthUpdate checks conditions every frame (or when sleeping)
//! - Conditions include: not attacking, not moving, not taking damage, not using abilities
//! - When conditions are met, sets OBJECT_STATUS_STEALTHED status bit
//! - When broken (e.g., attacking), clears bit and starts delay timer
//!
//! ### Stealth Breaking on Attack
//! 1. WeaponUpdate fires weapon → sets OBJECT_STATUS_IS_FIRING_WEAPON
//! 2. StealthUpdate::allowedToStealth() checks flag (C++ line 268)
//! 3. Returns false → stealth breaks
//! 4. OBJECT_STATUS_STEALTHED cleared, delay timer starts
//! 5. After stealthDelay frames + weapon stops firing → re-stealth
//!
//! ### Detection System
//! - Enemy units with detection capability call markAsDetected()
//! - Sets OBJECT_STATUS_DETECTED for a duration
//! - Unit becomes visible to enemies even while stealthed
//! - Detection expires after detectionExpiresFrame
//!
//! ### Disguise System (Bomb Truck)
//! - Special stealth units can disguise as other units
//! - disguiseAsObject() changes visual appearance
//! - Broken when detected or attacking
//! - Visual transition with opacity fade
//!
//! ## C++ Reference
//!
//! This implementation ports the following C++ files:
//! - `GameLogic.cpp` - Main game loop and object management (lines 500-800+)
//! - `GameLogic.h` - Interface definitions
//! - `GameLogicDispatch.cpp` - Command dispatch system
//!
//! ## Timing Requirements
//!
//! - **Frame Rate**: Fixed 30 FPS (delta_time = 1.0/30.0 ≈ 0.0333s)
//! - **Determinism**: Same frame order every game for multiplayer sync
//! - **Synchronization**: All systems must complete before next frame

use crate::ai::THE_AI;
use crate::ai::integration::with_ai_integration_mut;
use crate::common::{
    AsciiString, Bool, Color, Coord3D, DisabledMaskType, INVALID_ID, Int, KindOf, ObjectID,
    ObjectStatusMaskType, ObjectStatusTypes, PlayerMaskType, Real, UnsignedInt, UnsignedShort,
};
use crate::helpers::{TheGameClient, get_camera_view_bridge};
use crate::modules::{SleepyUpdatePhase, UpdateModulePtr, UpdateSleepTime};
use crate::object::collide::collision_geometry::GeometryInfo as CollisionGeometryInfo;
use crate::object::collide::collision_response::{CollisionResponseConfig, CollisionResponseType};
use crate::object::collide::collision_system::with_collision_system_mut;
use crate::object::drawable::DrawableExt;
use crate::object::{Object, THE_GHOST_OBJECT_MANAGER, registry::OBJECT_REGISTRY};
use crate::player::{GameDifficulty, Player, PlayerIndex, PlayerType, player_list};
use crate::scripting::engine::{ScriptEngine, get_script_engine, initialize_script_engine};
use crate::sides_list::get_sides_list;
use crate::system::beacon_manager::{BeaconUpdate, drain_beacon_updates};
use crate::system::game_logic_dispatch::{GameLogicDispatch, get_dispatch};
use crate::system::radar_notifier;
use crate::system::shroud_manager::get_shroud_manager;
use crate::team::{Team, flush_pending_team_script_events, get_team_factory};
use crate::terrain::{TerrainDynamicWaterSnapshotEntry, get_terrain_logic};
use crate::weapon::{WeaponBonus, WeaponBonusConditionType, WeaponBonusField, WeaponBonusSet};
use game_engine::System::SaveGame::register_partition_manager_update;
use game_engine::System::XferVersion;
use game_engine::System::{
    register_object_id_counter_hooks, register_save_load_lifecycle_hooks,
    register_save_load_mission_hooks, register_save_lock_ghost_objects_hook,
};
use game_engine::common::rts::energy::{
    EnergyObjectLookup, EnergyOwnerCallbacks, set_energy_object_lookup, set_energy_owner_callbacks,
};
use game_engine::common::rts::handles::{ObjectHandle, PlayerHandle};
use game_engine::common::system::build_assistant::init_build_assistant;
use game_engine::{Snapshot as XferSnapshotTrait, Xfer, XferMode, XferStatus};
use log::{debug, info, trace, warn};
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock};
use std::time::Instant;

include!("types.rs");
include!("xfer_crc.rs");
include!("xfer_helpers.rs");
include!("xfer_object_load.rs");
include!("xfer_campaign.rs");
include!("xfer_player_list.rs");
include!("xfer_team_factory.rs");

include!("snapshot.rs");
include!("events.rs");
include!("physics.rs");
include!("partition.rs");
include!("impl_init.rs");
include!("impl_update.rs");
include!("impl_sleepy.rs");
include!("impl_lifecycle.rs");
include!("impl_save.rs");
include!("globals.rs");

#[cfg(test)]
include!("tests.rs");
#[cfg(test)]
include!("select_object_tests.rs");
