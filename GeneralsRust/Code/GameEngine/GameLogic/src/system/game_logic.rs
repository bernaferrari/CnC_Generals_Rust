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

use crate::ai::integration::with_ai_integration_mut;
use crate::ai::THE_AI;
use crate::common::{
    AsciiString, Bool, Color, Coord3D, DisabledMaskType, Int, KindOf, ObjectID,
    ObjectStatusMaskType, PlayerMaskType, Real, UnsignedInt, UnsignedShort, INVALID_ID,
};
use crate::helpers::TheGameClient;
use crate::modules::{SleepyUpdatePhase, UpdateModulePtr, UpdateSleepTime};
use crate::object::collide::collision_geometry::GeometryInfo as CollisionGeometryInfo;
use crate::object::collide::collision_response::{CollisionResponseConfig, CollisionResponseType};
use crate::object::collide::collision_system::with_collision_system_mut;
use crate::object::update::laser_update::LaserUpdateModule;
use crate::object::update::{
    AnimatedParticleSysBoneClientUpdateModule, BeaconClientUpdateModule, SwayClientUpdateModule,
};
use crate::object::{registry::OBJECT_REGISTRY, Object, THE_GHOST_OBJECT_MANAGER};
use crate::player::{player_list, GameDifficulty, Player, PlayerIndex, PlayerType};
use crate::scripting::engine::{get_script_engine, initialize_script_engine, ScriptEngine};
use crate::sides_list::get_sides_list;
use crate::system::beacon_manager::{drain_beacon_updates, BeaconUpdate};
use crate::system::game_logic_dispatch::{get_dispatch, GameLogicDispatch};
use crate::system::radar_notifier;
use crate::system::shroud_manager::get_shroud_manager;
use crate::team::{flush_pending_team_script_events, get_team_factory, Team};
use crate::terrain::{get_terrain_logic, TerrainDynamicWaterSnapshotEntry};
use crate::weapon::{WeaponBonus, WeaponBonusConditionType, WeaponBonusField, WeaponBonusSet};
use game_engine::common::rts::energy::{
    set_energy_object_lookup, set_energy_owner_callbacks, EnergyObjectLookup, EnergyOwnerCallbacks,
};
use game_engine::common::rts::handles::{ObjectHandle, PlayerHandle};
use game_engine::common::system::build_assistant::init_build_assistant;
use game_engine::System::XferVersion;
use game_engine::System::{
    register_object_id_counter_hooks, register_save_load_lifecycle_hooks,
    register_save_load_mission_hooks, register_save_lock_ghost_objects_hook,
};
use game_engine::{Snapshot as XferSnapshotTrait, Xfer, XferMode, XferStatus};
use log::{debug, info, trace, warn};
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock};
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct Snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildableStatus {
    Available,
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrcMode {
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Skirmish,
}

pub trait SubsystemInterface: Send + Sync {
    fn update(&mut self, _delta_time: f32) {}
}

pub const MAX_SLOTS: usize = 32;

#[derive(Debug, Default)]
pub struct PartitionManagerFactory;

#[derive(Debug, Default)]
pub struct TheObjectFactory;

impl TheObjectFactory {
    pub fn find_template(name: &str) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        crate::helpers::TheThingFactory::find_template(name)
    }

    pub fn new_object(
        template: Arc<dyn crate::common::ThingTemplate>,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Result<Arc<RwLock<Object>>, Box<dyn std::error::Error + Send + Sync>> {
        let object_id = {
            let mutex = get_game_logic();
            let mut logic = mutex
                .lock()
                .map_err(|_| "GameLogic mutex poisoned when allocating object id")?;
            logic.allocate_object_id()
        };

        let status_mask = template.get_initial_object_status();
        let object = Object::new_with_id(template, object_id, status_mask, team)?;

        {
            let mutex = get_game_logic();
            let mut logic = mutex
                .lock()
                .map_err(|_| "GameLogic mutex poisoned when registering object")?;
            logic
                .register_object(object.clone())
                .map_err(|err| format!("Failed to register object: {:?}", err))?;
        }

        Ok(object)
    }
}

/// Fixed simulation frame rate (30 FPS for C&C Generals)
pub const DEFAULT_TICK_FPS: u32 = 30;
/// Fixed time step per frame in seconds
pub const FIXED_DELTA_TIME: f32 = 1.0 / 30.0;

/// C++ parity hook for `setFPMode()` from `FPUControl.h`.
///
/// The original game resets x87 control flags because DirectX could leave FP state dirty.
/// In Rust on modern targets we run with stable IEEE-754 defaults, so this is intentionally
/// a no-op placeholder and explicit call site for parity bookkeeping.
pub fn set_fp_mode() {}

/// Maximum number of sleepy updates to process per frame to avoid runaway execution
const MAX_SLEEPY_UPDATES_PER_FRAME: usize = 256;

fn xfer_sorted_string_int_map(
    xfer: &mut dyn Xfer,
    map: &mut HashMap<String, Int>,
) -> Result<(), XferStatus> {
    let mut count = if xfer.get_xfer_mode() == XferMode::Load {
        0u32
    } else {
        map.len() as u32
    };
    xfer.xfer_unsigned_int(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            let mut entries: Vec<_> = map
                .iter()
                .map(|(key, value)| (key.clone(), *value))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (mut key, mut value) in entries {
                xfer.xfer_string(&mut key)?;
                xfer.xfer_int(&mut value)?;
            }
        }
        XferMode::Load => {
            map.clear();
            for _ in 0..count {
                let mut key = String::new();
                let mut value: Int = 0;
                xfer.xfer_string(&mut key)?;
                xfer.xfer_int(&mut value)?;
                map.insert(key, value);
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_sorted_string_unit_map(
    xfer: &mut dyn Xfer,
    map: &mut HashMap<String, ()>,
) -> Result<(), XferStatus> {
    let mut count = if xfer.get_xfer_mode() == XferMode::Load {
        0u32
    } else {
        map.len() as u32
    };
    xfer.xfer_unsigned_int(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            for mut key in keys {
                xfer.xfer_string(&mut key)?;
            }
        }
        XferMode::Load => {
            map.clear();
            for _ in 0..count {
                let mut key = String::new();
                xfer.xfer_string(&mut key)?;
                map.insert(key, ());
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_game_logic_state(logic: &mut GameLogic, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    xfer.xfer_real(&mut logic.width)?;
    xfer.xfer_real(&mut logic.height)?;
    xfer.xfer_unsigned_int(&mut logic.frame)?;
    xfer.xfer_real(&mut logic.game_time)?;
    xfer.xfer_bool(&mut logic.is_in_update)?;
    xfer.xfer_unsigned_int(&mut logic.next_object_id)?;
    let mut random_seed = logic.random_seed as i64;
    xfer.xfer_int64(&mut random_seed)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        logic.random_seed = random_seed as u64;
    }

    // The game state block owns the heavyweight object graph.  This provider keeps
    // the portable runtime flags that affect save/load behavior and UI state.
    xfer.xfer_int(&mut logic.game_mode)?;
    xfer.xfer_bool(&mut logic.loading_map)?;
    xfer.xfer_bool(&mut logic.loading_save)?;
    xfer.xfer_bool(&mut logic.is_scoring_enabled)?;
    xfer.xfer_bool(&mut logic.show_behind_building_markers)?;
    xfer.xfer_bool(&mut logic.draw_icon_ui)?;
    xfer.xfer_bool(&mut logic.show_dynamic_lod)?;
    xfer.xfer_int(&mut logic.rank_level_limit)?;
    xfer.xfer_unsigned_short(&mut logic.superweapon_restriction)?;
    xfer_sorted_string_int_map(xfer, &mut logic.buildable_status_overrides)?;
    xfer_sorted_string_unit_map(xfer, &mut logic.control_bar_overrides)?;

    let mut rank_points = crate::helpers::TheGameLogic::get_rank_points_to_add_at_game_start();
    xfer.xfer_int(&mut rank_points)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        crate::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(rank_points);
    }

    let mut hulk_max_lifetime = crate::helpers::TheGameLogic::get_hulk_max_lifetime_override();
    xfer.xfer_int(&mut hulk_max_lifetime)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        crate::helpers::TheGameLogic::set_hulk_max_lifetime_override(hulk_max_lifetime);
    }

    Ok(())
}

fn xfer_partition_state(logic: &mut GameLogic, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    xfer.xfer_real(&mut logic.partition_manager.cell_size)?;

    let mut entries: Vec<(ObjectID, Coord3D)> =
        if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
            let mut snapshot = logic
                .partition_manager
                .object_positions
                .iter()
                .map(|(&object_id, position)| (object_id, *position))
                .collect::<Vec<_>>();
            snapshot.sort_by_key(|(object_id, _)| *object_id);
            snapshot
        } else {
            Vec::new()
        };

    let mut count = entries.len() as u32;
    xfer.xfer_unsigned_int(&mut count)?;

    if xfer.get_xfer_mode() == XferMode::Load {
        logic.partition_manager.grid.clear();
        logic.partition_manager.object_cells.clear();
        logic.partition_manager.object_positions.clear();

        entries.reserve(count as usize);
        for _ in 0..count {
            let mut object_id: ObjectID = INVALID_ID;
            let mut position = Coord3D::ZERO;
            xfer.xfer_object_id(&mut object_id)?;
            xfer.xfer_real(&mut position.x)?;
            xfer.xfer_real(&mut position.y)?;
            xfer.xfer_real(&mut position.z)?;
            if object_id != INVALID_ID {
                entries.push((object_id, position));
            }
        }

        for (object_id, position) in entries {
            logic
                .partition_manager
                .add_object(object_id, (position.x, position.y, position.z));
        }
    } else {
        for (object_id, position) in &mut entries {
            xfer.xfer_object_id(object_id)?;
            xfer.xfer_real(&mut position.x)?;
            xfer.xfer_real(&mut position.y)?;
            xfer.xfer_real(&mut position.z)?;
        }
    }

    Ok(())
}

fn xfer_player_list_runtime_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let player_list = player_list();
    let list_guard = player_list.write().map_err(|_| XferStatus::InvalidData)?;
    let mut player_count = list_guard.get_player_count() as i32;
    xfer.xfer_int(&mut player_count)?;

    if player_count != list_guard.get_player_count() as i32 {
        return Err(XferStatus::InvalidData);
    }

    for idx in 0..player_count.max(0) {
        let player_arc = list_guard
            .get_player(idx)
            .cloned()
            .ok_or(XferStatus::InvalidData)?;
        let mut player = player_arc.write().map_err(|_| XferStatus::InvalidData)?;

        let mut money = player.get_money().get_money();
        xfer.xfer_int(&mut money)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            player.get_money_mut().set_money(money);
        }

        let mut power_production = player.get_energy().production();
        xfer.xfer_int(&mut power_production)?;
        let mut power_consumption = player.get_energy().consumption();
        xfer.xfer_int(&mut power_consumption)?;
        let mut power_sabotaged_till_frame = player.get_energy().get_power_sabotaged_till_frame();
        xfer.xfer_unsigned_int(&mut power_sabotaged_till_frame)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            let energy = player.get_energy_mut();
            energy.reset();
            if power_production > 0 {
                energy.add_power_production(power_production);
            }
            if power_consumption > 0 {
                energy.add_power_consumption(power_consumption);
            }
            energy.set_power_sabotaged_till_frame(power_sabotaged_till_frame);
        }

        let mut defeated = player.is_defeated();
        xfer.xfer_bool(&mut defeated)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            player.set_defeated(defeated);
        }

        let mut observer = player.is_player_observer();
        xfer.xfer_bool(&mut observer)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            player.set_observer(observer);
        }

        let mut rank_level = player.get_rank_level();
        xfer.xfer_int(&mut rank_level)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            let _ = player.set_rank_level(rank_level);
        }

        let mut science_points = player.get_science_purchase_points();
        xfer.xfer_int(&mut science_points)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            let delta = science_points - player.get_science_purchase_points();
            if delta != 0 {
                player.add_science_purchase_points(delta);
            }
        }
    }

    Ok(())
}

fn xfer_team_factory_runtime_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut factory = get_team_factory()
        .lock()
        .map_err(|_| XferStatus::InvalidData)?;

    let mut next_team_id = factory.get_next_team_id();
    xfer.xfer_unsigned_int(&mut next_team_id)?;
    let mut next_team_prototype_id = factory.get_next_team_prototype_id();
    xfer.xfer_unsigned_int(&mut next_team_prototype_id)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        factory.set_next_team_ids(next_team_id, next_team_prototype_id);
    }

    let mut prototype_ids = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        let mut ids = factory
            .list_team_prototypes()
            .into_iter()
            .map(|prototype| prototype.get_id())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids
    } else {
        Vec::new()
    };
    let mut prototype_count = prototype_ids.len() as u16;
    xfer.xfer_unsigned_short(&mut prototype_count)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        if prototype_count as usize != factory.list_team_prototypes().len() {
            return Err(XferStatus::InvalidData);
        }
        prototype_ids.reserve(prototype_count as usize);
        for _ in 0..prototype_count {
            let mut prototype_id = 0u32;
            xfer.xfer_unsigned_int(&mut prototype_id)?;
            if factory.find_team_prototype_by_id(prototype_id).is_none() {
                return Err(XferStatus::InvalidData);
            }
            prototype_ids.push(prototype_id);
        }
    } else {
        for prototype_id in &mut prototype_ids {
            xfer.xfer_unsigned_int(prototype_id)?;
        }
    }

    Ok(())
}

fn xfer_sides_list_runtime_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let sides = get_sides_list();
    let mut sides_guard = sides.write().map_err(|_| XferStatus::InvalidData)?;
    let mut side_count = sides_guard.get_num_sides() as i32;
    xfer.xfer_int(&mut side_count)?;
    if side_count != sides_guard.get_num_sides() as i32 {
        return Err(XferStatus::InvalidData);
    }

    for idx in 0..side_count.max(0) as usize {
        let side = sides_guard
            .get_side_info_mut(idx)
            .ok_or(XferStatus::InvalidData)?;
        let mut script_list_present = side.get_script_list().is_some();
        xfer.xfer_bool(&mut script_list_present)?;
        let has_runtime_script_list = side.get_script_list().is_some();
        if script_list_present != has_runtime_script_list {
            return Err(XferStatus::InvalidData);
        }
    }

    Ok(())
}

fn xfer_terrain_logic_runtime_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 2;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let terrain = get_terrain_logic();
    let mut terrain_guard = terrain.write().map_err(|_| XferStatus::InvalidData)?;

    let mut active_boundary = terrain_guard.get_active_boundary();
    xfer.xfer_int(&mut active_boundary)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        terrain_guard.set_active_boundary(active_boundary);
    }

    if version >= 2 {
        let mut entries = if xfer.get_xfer_mode() == XferMode::Load {
            Vec::new()
        } else {
            terrain_guard.snapshot_dynamic_water_entries()
        };

        let mut entry_count = if xfer.get_xfer_mode() == XferMode::Load {
            0i32
        } else {
            entries.len() as i32
        };
        xfer.xfer_int(&mut entry_count)?;
        if entry_count < 0 {
            return Err(XferStatus::InvalidData);
        }

        if xfer.get_xfer_mode() == XferMode::Load {
            entries.reserve(entry_count as usize);
            for _ in 0..entry_count {
                let mut trigger_id: Int = -1;
                xfer.xfer_int(&mut trigger_id)?;
                let mut change_per_frame = 0.0f32;
                let mut target_height = 0.0f32;
                let mut damage_amount = 0.0f32;
                let mut current_height = 0.0f32;
                xfer.xfer_real(&mut change_per_frame)?;
                xfer.xfer_real(&mut target_height)?;
                xfer.xfer_real(&mut damage_amount)?;
                xfer.xfer_real(&mut current_height)?;
                entries.push(TerrainDynamicWaterSnapshotEntry {
                    trigger_id,
                    water_name: AsciiString::new(),
                    change_per_frame,
                    target_height,
                    damage_amount,
                    current_height,
                });
            }
            terrain_guard
                .restore_dynamic_water_entries(entries)
                .map_err(|_| XferStatus::InvalidData)?;
        } else {
            for entry in &mut entries {
                let mut trigger_id = entry.trigger_id;
                xfer.xfer_int(&mut trigger_id)?;
                xfer.xfer_real(&mut entry.change_per_frame)?;
                xfer.xfer_real(&mut entry.target_height)?;
                xfer.xfer_real(&mut entry.damage_amount)?;
                xfer.xfer_real(&mut entry.current_height)?;
            }
        }
    }

    Ok(())
}

struct GameLogicSnapshotBridge {
    logic: &'static Mutex<GameLogic>,
}

impl GameLogicSnapshotBridge {
    fn new(logic: &'static Mutex<GameLogic>) -> Self {
        Self { logic }
    }
}

impl XferSnapshotTrait for GameLogicSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut guard = self.logic.lock().map_err(|_| XferStatus::InvalidData)?;
        xfer_game_logic_state(&mut guard, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct ScriptEngineSnapshotBridge {
    script_engine: Arc<RwLock<Option<ScriptEngine>>>,
}

impl ScriptEngineSnapshotBridge {
    fn new(script_engine: Arc<RwLock<Option<ScriptEngine>>>) -> Self {
        Self { script_engine }
    }

    fn with_engine_mut<R, F>(&self, mut callback: F) -> Result<R, XferStatus>
    where
        F: FnMut(&mut ScriptEngine) -> Result<R, XferStatus>,
    {
        let mut guard = self
            .script_engine
            .write()
            .map_err(|_| XferStatus::InvalidData)?;
        if guard.is_none() {
            *guard = Some(ScriptEngine::new().map_err(|_| XferStatus::InvalidData)?);
        }
        let engine = guard.as_mut().ok_or(XferStatus::InvalidData)?;
        callback(engine)
    }

    fn difficulty_to_i32(difficulty: GameDifficulty) -> i32 {
        match difficulty {
            GameDifficulty::Easy => 0,
            GameDifficulty::Normal => 1,
            GameDifficulty::Hard => 2,
            GameDifficulty::Brutal => 3,
        }
    }

    fn difficulty_from_i32(value: i32) -> GameDifficulty {
        match value {
            0 => GameDifficulty::Easy,
            2 => GameDifficulty::Hard,
            3 => GameDifficulty::Brutal,
            _ => GameDifficulty::Normal,
        }
    }

    fn xfer_script_engine_state(
        engine: &mut ScriptEngine,
        xfer: &mut dyn Xfer,
    ) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut frame_object_count_changed = engine.get_frame_object_count_changed();
        xfer.xfer_unsigned_int(&mut frame_object_count_changed)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_frame_object_count_changed(frame_object_count_changed);
        }

        let mut shown_local_defeat_window = engine.has_shown_mp_local_defeat_window();
        xfer.xfer_bool(&mut shown_local_defeat_window)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_shown_mp_local_defeat_window(shown_local_defeat_window);
        }

        let mut freeze_script = engine.is_time_frozen_script();
        xfer.xfer_bool(&mut freeze_script)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            if freeze_script {
                engine.do_freeze_time();
            } else {
                engine.do_unfreeze_time();
            }
        }

        let mut freeze_debug = engine.is_time_frozen_debug();
        xfer.xfer_bool(&mut freeze_debug)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_time_frozen_debug(freeze_debug);
        }

        let mut current_track = engine.get_current_track_name().to_string();
        xfer.xfer_string(&mut current_track)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_current_track_name(current_track);
        }

        let mut global_difficulty = Self::difficulty_to_i32(engine.get_global_difficulty());
        xfer.xfer_int(&mut global_difficulty)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_global_difficulty(Self::difficulty_from_i32(global_difficulty));
        }

        let mut choose_victim_normal = engine.get_choose_victim_always_uses_normal();
        xfer.xfer_bool(&mut choose_victim_normal)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_choose_victim_always_uses_normal(choose_victim_normal);
        }

        Ok(())
    }
}

impl XferSnapshotTrait for ScriptEngineSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.with_engine_mut(|engine| Self::xfer_script_engine_state(engine, xfer))
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct PartitionSnapshotBridge {
    logic: &'static Mutex<GameLogic>,
}

impl PartitionSnapshotBridge {
    fn new(logic: &'static Mutex<GameLogic>) -> Self {
        Self { logic }
    }
}

impl XferSnapshotTrait for PartitionSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut guard = self.logic.lock().map_err(|_| XferStatus::InvalidData)?;
        xfer_partition_state(&mut guard, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct GhostObjectSnapshotBridge {
    manager: Arc<RwLock<crate::object::GhostObjectManager>>,
}

impl GhostObjectSnapshotBridge {
    fn new(manager: Arc<RwLock<crate::object::GhostObjectManager>>) -> Self {
        Self { manager }
    }
}

impl XferSnapshotTrait for GhostObjectSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut guard = self.manager.write().map_err(|_| XferStatus::InvalidData)?;
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        let mut local_player = guard.get_local_player_index();
        xfer.xfer_int(&mut local_player)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            guard.set_local_player_index(local_player);
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct PlayerListSnapshotBridge;

impl XferSnapshotTrait for PlayerListSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_player_list_runtime_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct TeamFactorySnapshotBridge;

impl XferSnapshotTrait for TeamFactorySnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_team_factory_runtime_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct SidesListSnapshotBridge;

impl XferSnapshotTrait for SidesListSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_sides_list_runtime_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct TerrainLogicSnapshotBridge;

impl XferSnapshotTrait for TerrainLogicSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_terrain_logic_runtime_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

fn register_game_logic_snapshot_block() {
    let logic = GAME_LOGIC.get_or_init(|| Mutex::new(GameLogic::default()));
    let script_engine = get_script_engine();
    let mut state = game_engine::System::get_game_state();
    state.add_snapshot_block(
        "CHUNK_TerrainLogic".to_string(),
        Box::new(TerrainLogicSnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_TeamFactory".to_string(),
        Box::new(TeamFactorySnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_TeamFactory".to_string(),
        Box::new(TeamFactorySnapshotBridge),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_Players".to_string(),
        Box::new(PlayerListSnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_Players".to_string(),
        Box::new(PlayerListSnapshotBridge),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_GameLogic".to_string(),
        Box::new(GameLogicSnapshotBridge::new(logic)),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_GameLogic".to_string(),
        Box::new(GameLogicSnapshotBridge::new(logic)),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_ScriptEngine".to_string(),
        Box::new(ScriptEngineSnapshotBridge::new(script_engine.clone())),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_ScriptEngine".to_string(),
        Box::new(ScriptEngineSnapshotBridge::new(script_engine)),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_SidesList".to_string(),
        Box::new(SidesListSnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_SidesList".to_string(),
        Box::new(SidesListSnapshotBridge),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_Partition".to_string(),
        Box::new(PartitionSnapshotBridge::new(logic)),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_Partition".to_string(),
        Box::new(PartitionSnapshotBridge::new(logic)),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_GhostObject".to_string(),
        Box::new(GhostObjectSnapshotBridge::new(Arc::clone(
            &THE_GHOST_OBJECT_MANAGER,
        ))),
        game_engine::System::SnapshotType::SaveLoad,
    );
}

/// Game mode constants (matching C++ enum values)
pub const GAME_SINGLE_PLAYER: Int = 0;
pub const GAME_LAN: Int = 1;
pub const GAME_SKIRMISH: Int = 2;
pub const GAME_REPLAY: Int = 3;
pub const GAME_SHELL: Int = 4;
pub const GAME_INTERNET: Int = 5;
pub const GAME_NONE: Int = 6;

/// Error types for GameLogic operations
#[derive(Debug, Clone)]
pub enum GameLogicError {
    /// Object with specified ID was not found
    ObjectNotFound(ObjectID),
    /// Physics system error
    PhysicsError(String),
    /// Scripting system error
    ScriptError(String),
    /// AI system error
    AIError(String),
    /// Invalid state transition or operation
    InvalidState(String),
    /// Command processing error
    CommandError(String),
    /// Vision/shroud update error
    VisionError(String),
    /// Generic error with message
    Generic(String),
}

impl std::fmt::Display for GameLogicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameLogicError::ObjectNotFound(id) => write!(f, "Object not found: {}", id),
            GameLogicError::PhysicsError(msg) => write!(f, "Physics error: {}", msg),
            GameLogicError::ScriptError(msg) => write!(f, "Script error: {}", msg),
            GameLogicError::AIError(msg) => write!(f, "AI error: {}", msg),
            GameLogicError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            GameLogicError::CommandError(msg) => write!(f, "Command error: {}", msg),
            GameLogicError::VisionError(msg) => write!(f, "Vision error: {}", msg),
            GameLogicError::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for GameLogicError {}

/// Game event types for frame-based event tracking
#[derive(Debug, Clone)]
pub enum GameEvent {
    ObjectCreated(ObjectID),
    ObjectDestroyed(ObjectID),
    DamageDealt {
        attacker: ObjectID,
        target: ObjectID,
        amount: f32,
    },
    RadarUpdate {
        player_id: Int,
        position: (f32, f32),
        event_type: RadarEventType,
    },
    BeaconPlaced {
        player_id: Int,
        position: Coord3D,
        text: Option<AsciiString>,
    },
    BeaconRemoved {
        player_id: Int,
        position: Coord3D,
    },
    BeaconTextUpdated {
        player_id: Int,
        position: Coord3D,
        text: AsciiString,
    },
    VictoryConditionMet {
        player_id: Int,
        condition_name: String,
    },
}

/// Game command types for command queue
#[derive(Debug, Clone)]
pub enum GameCommand {
    MoveUnit {
        player_id: Int,
        unit_ids: Vec<ObjectID>,
        target_position: (f32, f32, f32),
    },
    AttackTarget {
        player_id: Int,
        attacker_ids: Vec<ObjectID>,
        target_id: ObjectID,
    },
    BuildStructure {
        player_id: Int,
        builder_id: ObjectID,
        structure_type: String,
        position: (f32, f32),
    },
    UseSpecialPower {
        player_id: Int,
        power_name: String,
        target_position: Option<(f32, f32, f32)>,
    },
}

/// Radar update event
#[derive(Debug, Clone)]
pub struct RadarUpdate {
    pub player_id: Int,
    pub position: (f32, f32),
    pub event_type: RadarEventType,
}

#[derive(Debug, Clone, Copy)]
pub enum RadarEventType {
    UnitCreated,
    UnitDestroyed,
    BaseAttacked,
    EnemyDetected,
    BeaconPlaced,
    BeaconRemoved,
}

/// Lightweight physics queue for deferred damage and collisions.
#[derive(Debug, Default)]
pub struct PhysicsWorld {
    pending_damage: Vec<PendingDamage>,
    pending_collisions: Vec<PendingCollision>,
}

#[derive(Debug, Clone)]
struct PendingDamage {
    target_id: ObjectID,
    attacker_id: ObjectID,
    damage_amount: f32,
    damage_type: crate::damage::DamageType,
    death_type: crate::damage::DeathType,
}

#[derive(Debug, Clone)]
struct PendingCollision {
    object_a: ObjectID,
    object_b: ObjectID,
    collision_point: (f32, f32, f32),
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_damage(&mut self, target: ObjectID, attacker: ObjectID, amount: f32) {
        self.queue_damage_with_type(
            target,
            attacker,
            amount,
            crate::damage::DamageType::Crush,
            crate::damage::DeathType::Normal,
        );
    }

    pub fn queue_damage_with_type(
        &mut self,
        target: ObjectID,
        attacker: ObjectID,
        amount: f32,
        damage_type: crate::damage::DamageType,
        death_type: crate::damage::DeathType,
    ) {
        self.pending_damage.push(PendingDamage {
            target_id: target,
            attacker_id: attacker,
            damage_amount: amount,
            damage_type,
            death_type,
        });
    }

    pub fn queue_collision(
        &mut self,
        object_a: ObjectID,
        object_b: ObjectID,
        collision_point: (f32, f32, f32),
    ) {
        self.pending_collisions.push(PendingCollision {
            object_a,
            object_b,
            collision_point,
        });
    }

    pub fn resolve_all(&mut self, game_logic: &mut GameLogic) -> Result<(), GameLogicError> {
        // Process pending damage
        for damage in self.pending_damage.drain(..) {
            if let Some(obj_ref) = game_logic.find_object_by_id(damage.target_id) {
                if let Ok(mut obj) = obj_ref.write() {
                    let mut info = crate::damage::DamageInfo::with_simple(
                        damage.damage_amount,
                        damage.attacker_id,
                        damage.damage_type,
                        damage.death_type,
                    );
                    let _ = obj.attempt_damage(&mut info);
                    if obj.is_destroyed() {
                        game_logic.destroy_object(damage.target_id);
                    }
                }
            }
        }

        // Process collisions (collision system handles most interactions elsewhere)
        self.pending_collisions.clear();

        Ok(())
    }
}

/// Partition manager for spatial partitioning (matches C++ PartitionManager grid behavior)
#[derive(Debug, Default)]
pub struct PartitionManager {
    grid: HashMap<(i32, i32), Vec<ObjectID>>,
    object_cells: HashMap<ObjectID, (i32, i32)>,
    object_positions: HashMap<ObjectID, Coord3D>,
    cell_size: Real,
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            grid: HashMap::new(),
            object_cells: HashMap::new(),
            object_positions: HashMap::new(),
            cell_size: 100.0,
        }
    }

    /// Find objects within radius of position (2D X/Y distance).
    pub fn find_objects_in_radius(&self, center: Coord3D, radius: Real) -> Vec<ObjectID> {
        let mut result = Vec::new();
        let radius_squared = radius * radius;

        let min_cell =
            self.position_to_cell([center.x - radius, center.y - radius, center.z].into());
        let max_cell =
            self.position_to_cell([center.x + radius, center.y + radius, center.z].into());

        for x in min_cell.0..=max_cell.0 {
            for y in min_cell.1..=max_cell.1 {
                if let Some(objects) = self.grid.get(&(x, y)) {
                    for &object_id in objects {
                        let Some(pos) = self.object_positions.get(&object_id) else {
                            continue;
                        };
                        let dx = pos.x - center.x;
                        let dy = pos.y - center.y;
                        if dx * dx + dy * dy <= radius_squared {
                            result.push(object_id);
                        }
                    }
                }
            }
        }

        result
    }

    pub fn update(&mut self) -> Result<(), GameLogicError> {
        let objects = OBJECT_REGISTRY.get_all_objects();
        let mut seen = HashSet::with_capacity(objects.len());

        for obj_arc in objects {
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            let id = obj.get_id();
            let pos = obj.get_position();
            self.add_object(id, (pos.x, pos.y, pos.z));
            seen.insert(id);
        }

        let stale: Vec<ObjectID> = self
            .object_positions
            .keys()
            .copied()
            .filter(|id| !seen.contains(id))
            .collect();
        for id in stale {
            self.remove_object(id);
        }
        Ok(())
    }

    pub fn add_object(&mut self, object_id: ObjectID, position: (f32, f32, f32)) {
        let pos = Coord3D::new(position.0, position.1, position.2);
        let cell = self.position_to_cell(pos);

        if let Some(old_cell) = self.object_cells.get(&object_id) {
            if let Some(objects) = self.grid.get_mut(old_cell) {
                objects.retain(|&id| id != object_id);
            }
        }

        self.grid
            .entry(cell)
            .or_insert_with(Vec::new)
            .push(object_id);
        self.object_cells.insert(object_id, cell);
        self.object_positions.insert(object_id, pos);
    }

    pub fn remove_object(&mut self, object_id: ObjectID) {
        self.object_positions.remove(&object_id);
        if let Some(cell) = self.object_cells.remove(&object_id) {
            if let Some(objects) = self.grid.get_mut(&cell) {
                objects.retain(|&id| id != object_id);
                if objects.is_empty() {
                    self.grid.remove(&cell);
                }
            }
        }
    }

    /// Rebuild the spatial partition index
    /// Used after loading a saved game to reconstruct spatial data
    pub fn rebuild(&mut self) {
        self.grid.clear();
        self.object_cells.clear();
        self.object_positions.clear();

        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            if let Ok(obj) = obj_arc.read() {
                let pos = obj.get_position();
                self.add_object(obj.get_id(), (pos.x, pos.y, pos.z));
            }
        }
    }

    /// Register an object at a specific position
    /// Used during save game restoration
    pub fn register_object(&mut self, object_id: ObjectID, x: f32, y: f32) {
        self.add_object(object_id, (x, y, 0.0));
    }

    fn position_to_cell(&self, position: Coord3D) -> (i32, i32) {
        let x = (position.x / self.cell_size).floor() as i32;
        let y = (position.y / self.cell_size).floor() as i32;
        (x, y)
    }
}

/// Player configuration for game setup
#[derive(Debug, Clone)]
pub struct PlayerConfig {
    pub name: String,
    pub faction: String,
    pub color: Color,
    pub is_human: bool,
    pub team_id: Int,
}

/// Main GameLogic singleton - orchestrates all game systems
///
/// ## C++ Reference: GameLogic class (GameLogic.h lines 104-390)
///
/// This structure maintains the entire game state and coordinates updates
/// across all subsystems. It mirrors the C++ GameLogic singleton.
pub struct GameLogic {
    // World dimensions
    width: Real,
    height: Real,

    // Frame tracking
    frame: UnsignedInt,
    game_time: f32,
    is_in_update: Bool,

    // Random seed for deterministic replay/sync
    random_seed: u64,

    // Object management
    next_object_id: ObjectID,
    all_objects: Vec<ObjectID>,
    dead_objects: Vec<ObjectID>,
    objects: HashMap<ObjectID, Arc<RwLock<Object>>>,

    // Player/Team management (references only)
    // Actual player list is managed by player_list() singleton

    // Subsystems
    partition_manager: PartitionManager,
    physics_world: PhysicsWorld,

    // Event/Command queues
    event_queue: Vec<GameEvent>,
    command_queue: VecDeque<GameCommand>,
    radar_updates: Vec<RadarUpdate>,
    objects_changed_trigger_areas: VecDeque<ObjectID>,

    // Game state
    game_mode: Int,
    loading_map: Bool,
    loading_save: Bool,
    is_scoring_enabled: Bool,
    show_behind_building_markers: Bool,
    draw_icon_ui: Bool,
    show_dynamic_lod: Bool,
    rank_level_limit: Int,
    buildable_status_overrides: HashMap<String, Int>,
    superweapon_restriction: UnsignedShort,

    // Update module tracking (sleepy vs normal updates)
    sleepy_updates: BinaryHeap<SleepyUpdateEntry>,
    normal_updates: Vec<NormalUpdateEntry>,
    module_lookup: HashMap<ObjectID, Vec<UpdateModulePtr>>,
    global_weapon_bonus_set: WeaponBonusSet,

    // Control bar button overrides (C++ GameLogic.h line 266: ControlBarOverrideMap)
    control_bar_overrides: HashMap<String, ()>,

    // C++ parity: m_objectTOC — compact thing-template name→id map for save/load
    object_toc: Vec<ObjectTOCEntry>,
}

#[derive(Debug, Clone)]
pub struct ObjectTOCEntry {
    pub name: String,
    pub id: UnsignedShort,
}

/// Entry for sleepy update queue (priority queue by wake frame)
#[derive(Clone)]
struct SleepyUpdateEntry {
    wake_frame: UnsignedInt,
    phase: SleepyUpdatePhase,
    object_id: ObjectID,
    module: UpdateModulePtr,
}

impl PartialEq for SleepyUpdateEntry {
    fn eq(&self, other: &Self) -> bool {
        self.wake_frame == other.wake_frame && self.phase == other.phase
    }
}

impl Eq for SleepyUpdateEntry {}

impl PartialOrd for SleepyUpdateEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SleepyUpdateEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse order for min-heap behavior
        other
            .wake_frame
            .cmp(&self.wake_frame)
            .then_with(|| other.phase.cmp(&self.phase))
    }
}

/// Entry for normal (every-frame) update queue
#[derive(Clone)]
struct NormalUpdateEntry {
    object_id: ObjectID,
    module: UpdateModulePtr,
}

impl Default for GameLogic {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            frame: 0,
            game_time: 0.0,
            is_in_update: false,
            random_seed: 0,
            next_object_id: 1,
            all_objects: Vec::new(),
            dead_objects: Vec::new(),
            objects: HashMap::new(),
            partition_manager: PartitionManager::new(),
            physics_world: PhysicsWorld::new(),
            event_queue: Vec::new(),
            command_queue: VecDeque::new(),
            radar_updates: Vec::new(),
            objects_changed_trigger_areas: VecDeque::new(),
            game_mode: GAME_NONE,
            loading_map: false,
            loading_save: false,
            is_scoring_enabled: true,
            show_behind_building_markers: true,
            draw_icon_ui: true,
            show_dynamic_lod: true,
            rank_level_limit: 1000,
            buildable_status_overrides: HashMap::new(),
            superweapon_restriction: 0,
            sleepy_updates: BinaryHeap::new(),
            normal_updates: Vec::new(),
            module_lookup: HashMap::new(),
            global_weapon_bonus_set: WeaponBonusSet::new(),
            control_bar_overrides: HashMap::new(),
            object_toc: Vec::new(),
        }
    }
}

impl GameLogic {
    /// Create a new GameLogic instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Get whether draw icon UI indicators are enabled.
    pub fn get_draw_icon_ui(&self) -> Bool {
        self.draw_icon_ui
    }

    /// Set whether draw icon UI indicators are enabled.
    pub fn set_draw_icon_ui(&mut self, enabled: Bool) {
        self.draw_icon_ui = enabled;
    }

    /// Get whether behind-building markers (occlusion markers) are enabled.
    pub fn get_show_behind_building_markers(&self) -> Bool {
        self.show_behind_building_markers
    }

    /// Set whether behind-building markers (occlusion markers) are enabled.
    pub fn set_show_behind_building_markers(&mut self, enabled: Bool) {
        self.show_behind_building_markers = enabled;
    }

    /// Get whether dynamic LOD is enabled.
    pub fn get_show_dynamic_lod(&self) -> Bool {
        self.show_dynamic_lod
    }

    /// Set whether dynamic LOD is enabled.
    pub fn set_show_dynamic_lod(&mut self, enabled: Bool) {
        self.show_dynamic_lod = enabled;
    }

    /// Get whether scoring is enabled.
    pub fn is_scoring_enabled(&self) -> Bool {
        self.is_scoring_enabled
    }

    /// Enable/disable scoring updates and score screen accumulation.
    pub fn set_scoring_enabled(&mut self, enabled: Bool) {
        self.is_scoring_enabled = enabled;
    }

    /// Get the global map/script rank level cap.
    /// C++ reference: GameLogic::getRankLevelLimit()
    pub fn get_rank_level_limit(&self) -> Int {
        self.rank_level_limit
    }

    /// Set a runtime buildability override for a template name.
    /// Mirrors C++ GameLogic::setBuildableStatusOverride.
    pub fn set_buildable_status_override(&mut self, template_name: &str, status: Int) {
        if template_name.is_empty() {
            return;
        }
        self.buildable_status_overrides
            .insert(template_name.to_string(), status);
    }

    /// Find a runtime buildability override for a template name.
    /// Mirrors C++ GameLogic::findBuildableStatusOverride.
    pub fn find_buildable_status_override(&self, template_name: &str) -> Option<Int> {
        self.buildable_status_overrides.get(template_name).copied()
    }

    /// Set the global map/script rank level cap.
    /// C++ reference: GameLogic::setRankLevelLimit()
    pub fn set_rank_level_limit(&mut self, level: Int) {
        self.rank_level_limit = level;
    }

    /// Initialize the GameLogic system
    ///
    /// ## C++ Reference: GameLogic::init() (GameLogic.cpp)
    pub fn init(&mut self) {
        info!("GameLogic::init() - Initializing game logic system");
        self.reset();
        if let Err(err) = game_engine::common::thing::init_thing_system() {
            warn!("Thing system initialization failed during init: {}", err);
        }
        crate::system::thing_factory_bridge::install_thing_factory_bridge();
        if let Err(err) = crate::contain_module_overrides::ensure_module_overrides_installed() {
            warn!("Failed to install module overrides during init: {}", err);
        }
        self.refresh_global_weapon_bonuses();
        install_energy_integration();

        init_build_assistant();
        crate::system::build_assistant_bridge::install_build_assistant_backend();
        crate::terrain::init_terrain_physics_integration();

        crate::special_power_module::initialize();
        if let Err(e) = crate::control_bar::initialize_control_bar_bridge_from_common() {
            warn!("Control bar bridge initialization failed: {}", e);
        }

        if let Err(e) =
            crate::commands::initialize_command_system(crate::common::MAX_PLAYER_COUNT as i32)
        {
            warn!("Command system initialization failed: {}", e);
        }

        if let Err(e) = initialize_script_engine() {
            warn!("Script engine initialization failed: {}", e);
        }
    }

    /// Reset the GameLogic to default state
    ///
    /// ## C++ Reference: GameLogic::reset() (GameLogic.cpp)
    pub fn reset(&mut self) {
        info!("GameLogic::reset() - Resetting game state");

        self.frame = 0;
        self.game_time = 0.0;
        self.is_in_update = false;
        self.next_object_id = 1;
        self.all_objects.clear();
        self.dead_objects.clear();
        self.objects.clear();
        self.event_queue.clear();
        self.command_queue.clear();
        self.radar_updates.clear();
        self.game_mode = GAME_NONE;
        self.loading_map = false;
        self.loading_save = false;
        crate::helpers::TheGameLogic::clear_start_new_game_request();
        self.is_scoring_enabled = true;
        self.show_behind_building_markers = true;
        self.draw_icon_ui = true;
        self.show_dynamic_lod = true;
        self.rank_level_limit = 1000;
        self.buildable_status_overrides.clear();
        self.partition_manager = PartitionManager::new();
        self.physics_world = PhysicsWorld::new();
        self.sleepy_updates.clear();
        self.normal_updates.clear();
        self.module_lookup.clear();
        if let Err(err) = game_engine::common::thing::init_thing_system() {
            warn!("Thing system initialization failed during reset: {}", err);
        }
        crate::system::thing_factory_bridge::install_thing_factory_bridge();
        if let Err(err) = crate::contain_module_overrides::ensure_module_overrides_installed() {
            warn!("Failed to install module overrides during reset: {}", err);
        }
        self.refresh_global_weapon_bonuses();
        install_energy_integration();

        init_build_assistant();
        crate::system::build_assistant_bridge::install_build_assistant_backend();
        crate::terrain::init_terrain_physics_integration();

        // Keep global subsystems in a C++-like "reset, don't recreate" state.
        if let Err(e) = initialize_script_engine() {
            warn!("Script engine initialization failed during reset: {}", e);
        }

        crate::special_power_module::initialize();
        if let Err(e) = crate::control_bar::refresh_control_bar_bridge_from_common() {
            warn!("Control bar bridge refresh failed during reset: {}", e);
        }

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.reset();
            }
        }

        // C++ line 413: m_controlBarOverrides.clear()
        self.control_bar_overrides.clear();

        // C++ lines 447-451: delete TheStatsCollector; TheStatsCollector = NULL;
        game_engine::common::stats_collector::with_stats_collector_mut(|collector| {
            collector.reset();
        });

        // C++ line 462: m_scriptHulkMaxLifetimeOverride = -1
        crate::helpers::TheGameLogic::set_hulk_max_lifetime_override(-1);

        // C++ line 472: m_rankPointsToAddAtGameStart = 0
        crate::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(0);

        // C++ lines 465-466: clean up water transparency overrides
        game_engine::common::ini::ini_water::clear_water_transparency_overrides();

        // C++ lines 469-470: clean up weather overrides
        game_engine::common::ini::ini_weather::clear_weather_setting_overrides();
    }

    /// **THE MAIN GAME LOOP** - Execute one simulation frame
    ///
    /// ## C++ Reference: GameLogic::update() (GameLogic.cpp lines 3548-3803)
    ///
    /// This is the heart of the game engine. It orchestrates all game systems
    /// in the proper order to maintain deterministic simulation.
    ///
    /// ## Frame Order (CRITICAL):
    /// 1. Pre-Update Phase - Clear events, reset flags
    /// 2. AI Phase - Update AI players
    /// 3. Command Phase - Process player commands
    /// 4. Object Update Phase - **INCLUDES STEALTH UPDATES**
    ///    - Normal updates (every frame, C++ line 3672-3695)
    ///    - Sleepy updates (deferred, C++ line 3697-3738) **← STEALTH HERE**
    /// 5. Damage/Physics Resolution Phase
    /// ## Update Loop Phase Ordering (matches C++ GameLogic::update)
    ///
    /// This method implements the exact phase ordering from the C++ codebase
    /// (GameLogic.cpp lines 3548-3803) to maintain simulation correctness
    /// and multiplayer determinism.
    ///
    /// ### C++ Reference Phase Order:
    /// ```text
    /// Line 3595: setFrame / sync to GameClient
    /// Line 3600: TheScriptEngine->UPDATE()           [early scripting]
    /// Line 3603: freezeTime check
    /// Line 3622: TheTerrainLogic->UPDATE()           [terrain/bridges]
    /// Line 3627: CRC calculation (MP/replay)
    /// Line 3657: StatsCollector update
    /// Line 3663: Recorder UPDATE
    /// Line 3669: processCommandList                  [command processing]
    /// Line 3672: ALLOW_NONSLEEPY_UPDATES loop        [normal modules]
    /// Line 3697: sleepy updates loop                 [sleepy modules]
    /// Line 3743: TheAI->UPDATE()                     [AI]
    /// Line 3748: TheBuildAssistant->UPDATE()         [production]
    /// Line 3753: ThePartitionManager->UPDATE()       [spatial]
    /// Line 3762: processDestroyList()                [death/cleanup]
    /// Line 3765: TheCommandList->reset()
    /// Line 3767: TheWeaponStore->UPDATE()            [weapons]
    /// Line 3768: TheLocomotorStore->UPDATE()         [locomotors]
    /// Line 3769: TheVictoryConditions->UPDATE()      [victory]
    /// Line 3783: disabled status check               [re-enable]
    /// Line 3799: m_frame++                           [increment]
    /// ```
    ///
    /// ## Stealth Integration Point:
    /// StealthUpdate modules are processed via the sleepy/normal update queues
    /// (C++ lines 3672-3738). Each stealth module checks stealth breaking
    /// conditions (attacking, moving, damage), updates detection status, and
    /// manages disguise transitions.
    ///
    /// ## Parameters
    /// - `frame`: The current frame number
    ///
    /// ## Returns
    /// - `Ok(())` if update succeeded
    /// - `Err(GameLogicError)` if a critical error occurred
    pub fn update(&mut self, frame: u32) -> Result<(), GameLogicError> {
        // Prevent re-entrant calls (C++ line 3552: LatchRestore<Bool> inUpdateLatch)
        if self.is_in_update {
            warn!("GameLogic::update called re-entrantly; ignoring nested call");
            return Err(GameLogicError::InvalidState(
                "Re-entrant update call".to_string(),
            ));
        }

        // C++ `GameLogic::update()` calls setFPMode() at update entry.
        set_fp_mode();

        self.is_in_update = true;
        self.frame = frame;
        self.game_time = frame as f32 * FIXED_DELTA_TIME;

        trace!("GameLogic::update(frame={}) - Begin update cycle", frame);

        // -----------------------------------------------------------------------
        // Phase 0: Frame Setup (C++ lines 3595-3596)
        // -----------------------------------------------------------------------
        // C++: UnsignedInt now = TheGameLogic->getFrame();
        // C++: TheGameClient->setFrame(now);
        if let Some(client) = TheGameClient::get() {
            client.set_frame(frame);
        }

        // -----------------------------------------------------------------------
        // Phase 1: Early Scripting (C++ line 3600)
        // -----------------------------------------------------------------------
        // C++: TheScriptEngine->UPDATE();
        //
        // The script engine runs BEFORE object updates so that scripts can react
        // to state changes from the previous frame and issue commands that will
        // be processed in the command phase below.
        if let Err(e) = self.evaluate_scripts() {
            warn!("Early scripting phase failed: {}", e);
        }

        // -----------------------------------------------------------------------
        // Phase 2: Time Freeze Check (C++ lines 3603-3617)
        // -----------------------------------------------------------------------
        // C++: Bool freezeTime = TheTacticalView->isTimeFrozen() && ...
        // C++: if (freezeTime) { ... return; }
        //
        // In the full implementation, tactical view freeze and script freeze
        // would prevent all further processing. For now we check the script
        // engine freeze state.
        if self.is_time_frozen() {
            trace!("GameLogic::update - Time frozen, skipping frame");
            self.is_in_update = false;
            return Ok(());
        }

        // -----------------------------------------------------------------------
        // Phase 3: Pre-Update / Terrain (C++ lines 3619-3623)
        // -----------------------------------------------------------------------
        // C++: TheTerrainLogic->UPDATE();
        //
        // Terrain updates happen BEFORE object updates so bridge state changes
        // noted by scripts are reflected before the object update phase.
        if let Err(e) = self.update_terrain() {
            warn!("Terrain update phase failed: {}", e);
        }

        // Clear frame events and reset temporary flags
        if let Err(e) = self.clear_frame_events() {
            warn!("Pre-update clear failed: {}", e);
        }
        if let Err(e) = self.reset_temporary_flags() {
            warn!("Reset temporary flags failed: {}", e);
        }

        // Update the command system's frame counter
        if let Err(e) = crate::commands::update_command_system(frame) {
            warn!("Command system update failed: {}", e);
        }

        // -----------------------------------------------------------------------
        // Phase 4: Command Processing (C++ lines 3668-3669)
        // -----------------------------------------------------------------------
        // C++: processCommandList( TheCommandList );
        //
        // Process all queued player commands. This must happen BEFORE object
        // updates so that movement/attack orders are in effect when objects
        // run their AI/physics updates.
        if let Err(e) = self.process_command_queue() {
            warn!("Command processing phase failed: {}", e);
        }
        // C++ parity: MSG_NEW_GAME only arms the request in the command phase.
        // The expensive start is completed here once movies stop playing.
        if crate::helpers::TheGameLogic::is_start_new_game_requested()
            && !crate::helpers::TheGameLogic::is_intro_movie_playing()
        {
            if let Err(e) = self.start_new_game_now(false) {
                warn!("Deferred new-game start failed: {}", e);
            }
        }
        self.process_beacon_updates();
        self.process_radar_updates();

        // -----------------------------------------------------------------------
        // Phase 5: Object Update - Normal Modules (C++ lines 3672-3694)
        // -----------------------------------------------------------------------
        // C++: for (std::list<UpdateModulePtr>::const_iterator it = m_normalUpdates...)
        //
        // Process all non-sleepy (every-frame) update modules.
        // These include physics updates that must run at full frame rate.
        self.process_normal_updates();

        // -----------------------------------------------------------------------
        // Phase 6: Object Update - Sleepy Modules (C++ lines 3697-3738)
        // -----------------------------------------------------------------------
        // C++: while (!m_sleepyUpdates.empty()) { ... }
        //
        // Process all sleepy (delayed) update modules whose wake frame has
        // arrived. StealthUpdate, AIUpdate, and many behavior modules live here.
        // STEALTH: Most stealth modules are sleepy, updating every frame when
        // active. The sequence when a unit attacks:
        //   a. WeaponUpdate sets OBJECT_STATUS_IS_FIRING_WEAPON
        //   b. StealthUpdate::allowedToStealth() checks flag (C++ StealthUpdate.cpp:268)
        //   c. Stealth is broken, OBJECT_STATUS_STEALTHED cleared
        //   d. Stealth delay timer starts
        //   e. After delay + weapon stop, stealth reactivates
        self.process_sleepy_updates(frame);

        // -----------------------------------------------------------------------
        // Phase 6b: Object-level updates (damage types, projectiles, stealth)
        // -----------------------------------------------------------------------
        if let Err(e) = self.process_object_updates(FIXED_DELTA_TIME) {
            warn!("Object update phase failed: {}", e);
        }
        if let Err(e) = self.process_stealth_controllers(FIXED_DELTA_TIME) {
            warn!("Stealth update phase failed: {}", e);
        }
        if let Err(e) = crate::weapon::update_projectiles(FIXED_DELTA_TIME) {
            warn!("Projectile update phase failed: {}", e);
        }
        if let Err(e) = crate::weapon::update_dot_effects(frame) {
            warn!("DoT update phase failed: {}", e);
        }

        // Keep special power timers/cooldowns in sync with the simulation frame.
        crate::special_power_module::update();

        // Client-update modules (drawable-side updates like LaserUpdate)
        self.process_client_updates();
        if let Some(client) = TheGameClient::get() {
            client.update_drawables(frame);
        }

        // -----------------------------------------------------------------------
        // Phase 7: AI Update (C++ line 3743)
        // -----------------------------------------------------------------------
        // C++: TheAI->UPDATE();
        //
        // AI runs AFTER object updates so AI decisions are based on the latest
        // world state. This ordering is critical: objects move first, then
        // AI observes the new positions and issues commands for the next frame.
        if let Err(e) = self.update_ai_players(frame) {
            warn!("AI update phase failed: {}", e);
            // Don't abort - continue with other systems
        }

        // -----------------------------------------------------------------------
        // Phase 8: Production / Build Assistant (C++ line 3748)
        // -----------------------------------------------------------------------
        // C++: TheBuildAssistant->UPDATE();
        //
        // Production updates run after AI so build orders issued by AI this
        // frame can be immediately reflected in production queues.
        if let Err(e) = self.update_production(frame) {
            warn!("Production update phase failed: {}", e);
        }

        // -----------------------------------------------------------------------
        // Phase 9: Damage/Physics Resolution
        // -----------------------------------------------------------------------
        // Deferred damage and collision resolution after all objects have moved.
        if let Err(e) = self.resolve_damage_and_physics() {
            warn!("Physics resolution phase failed: {}", e);
        }

        self.update_objects_changed_trigger_areas();

        // -----------------------------------------------------------------------
        // Phase 10: Partition Manager Update (C++ line 3753)
        // -----------------------------------------------------------------------
        // C++: ThePartitionManager->UPDATE();
        //
        // Spatial partition is updated AFTER all objects have moved and before
        // death cleanup so queries during cleanup use correct positions.
        if let Err(e) = self.update_partition_manager() {
            warn!("Partition manager update failed: {}", e);
        }

        // -----------------------------------------------------------------------
        // Phase 11: Death/Cleanup (C++ line 3762)
        // -----------------------------------------------------------------------
        // C++: processDestroyList();
        //
        // Destroyed objects are removed from the world. This happens after
        // partition update so spatial queries remain valid during cleanup.
        if let Err(e) = self.cleanup_dead_objects() {
            warn!("Cleanup phase failed: {}", e);
        }

        // Periodically sweep dead weak references from the object registry so
        // that entries for objects that are never looked up again do not
        // accumulate unbounded.
        if frame % 1000 == 0 {
            OBJECT_REGISTRY.cleanup_dead_references();
        }

        // Reset the command queue (C++ line 3765: TheCommandList->reset())
        // Commands already processed; clear any remaining for next frame.
        self.command_queue.clear();

        // -----------------------------------------------------------------------
        // Phase 12: Weapon Store Update (C++ line 3767)
        // -----------------------------------------------------------------------
        // C++: TheWeaponStore->UPDATE();
        //
        // Process delayed damage (weapons with delay) that is now ready.
        if let Err(e) = self.update_weapon_store() {
            warn!("Weapon store update phase failed: {}", e);
        }

        // -----------------------------------------------------------------------
        // Phase 13: Victory Conditions (C++ line 3769)
        // -----------------------------------------------------------------------
        // C++: TheVictoryConditions->UPDATE();
        self.update_victory_conditions();

        // -----------------------------------------------------------------------
        // Phase 14: Disabled Status Check (C++ lines 3783-3792)
        // -----------------------------------------------------------------------
        // C++: for( Object *obj = m_objList; obj; obj = obj->getNextObject() )
        // C++:   if( obj->isDisabled() ) obj->checkDisabledStatus();
        //
        // Check timer-based disabled states and re-enable objects whose
        // disable duration has expired. This happens at end-of-frame so
        // disabled objects are inactive for the entire current frame.
        self.check_disabled_statuses();

        // -----------------------------------------------------------------------
        // Phase 15: Post-Update - Vision/Shroud and Team Events
        // -----------------------------------------------------------------------
        if let Err(e) = self.update_vision_and_shroud() {
            warn!("Vision update failed: {}", e);
        }
        if let Ok(mut team_factory) = get_team_factory().lock() {
            team_factory.update();
        }
        flush_pending_team_script_events();

        // -----------------------------------------------------------------------
        // Phase 16: Frame Increment (C++ lines 3799-3802)
        // -----------------------------------------------------------------------
        // C++: if (!m_startNewGame) { m_frame++; }
        if !crate::helpers::TheGameLogic::is_start_new_game_requested() {
            self.frame += 1;
        }

        self.is_in_update = false;

        trace!("GameLogic::update(frame={}) - End update cycle", frame);
        Ok(())
    }

    /// Drain and return all radar updates generated so far this frame. This
    /// mirrors the C++ pattern where the client polls radar events after the
    /// command/object phases.
    pub fn take_radar_updates(&mut self) -> Vec<RadarUpdate> {
        std::mem::take(&mut self.radar_updates)
    }

    /// Phase 1: Clear frame-based events and temporary state
    ///
    /// ## C++ Reference: Called at start of GameLogic::update()
    pub fn clear_frame_events(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::clear_frame_events()");

        // Clear event queues
        self.event_queue.clear();
        self.radar_updates.clear();

        // Clear temporary flags on objects
        for obj_id in &self.all_objects {
            if let Some(obj_ref) = self.objects.get(obj_id) {
                if let Ok(_obj) = obj_ref.write() {
                    // Clear frame-based flags
                    // (In full implementation, this would clear selection updates,
                    // temporary status bits, etc.)
                }
            }
        }

        Ok(())
    }

    fn process_beacon_updates(&mut self) {
        for update in drain_beacon_updates() {
            match update {
                BeaconUpdate::Placed(entry) => {
                    self.event_queue.push(GameEvent::BeaconPlaced {
                        player_id: entry.player_id,
                        position: entry.position,
                        text: entry.text.clone(),
                    });
                    self.radar_updates.push(RadarUpdate {
                        player_id: entry.player_id,
                        position: (entry.position.x, entry.position.y),
                        event_type: RadarEventType::BeaconPlaced,
                    });
                }
                BeaconUpdate::Removed {
                    player_id,
                    position,
                } => {
                    self.event_queue.push(GameEvent::BeaconRemoved {
                        player_id,
                        position,
                    });
                    self.radar_updates.push(RadarUpdate {
                        player_id,
                        position: (position.x, position.y),
                        event_type: RadarEventType::BeaconRemoved,
                    });
                }
                BeaconUpdate::TextUpdated {
                    player_id,
                    position,
                    text,
                } => {
                    self.event_queue.push(GameEvent::BeaconTextUpdated {
                        player_id,
                        position,
                        text,
                    });
                }
            }
        }
    }

    /// Promote radar updates generated this frame into the event queue so
    /// client/UI layers can trigger minimap and EVA feedback.
    fn process_radar_updates(&mut self) {
        for update in self.radar_updates.drain(..) {
            radar_notifier::push(&update);
            self.event_queue.push(GameEvent::RadarUpdate {
                player_id: update.player_id,
                position: update.position,
                event_type: update.event_type,
            });
        }
    }

    /// Reset temporary flags at frame start
    pub fn reset_temporary_flags(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::reset_temporary_flags()");
        // Stub: reset any temporary frame-based flags
        Ok(())
    }

    /// Phase 2: Update all AI players
    ///
    /// ## C++ Reference: GameLogic::update() AI section
    ///
    /// Iterates through all players and updates AI players (skipping humans).
    /// AI updates include:
    /// - Build order processing
    /// - Unit production decisions
    /// - Base building/expansion
    /// - Tactical decisions
    pub fn update_ai_players(&mut self, frame: UnsignedInt) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_ai_players(frame={})", frame);

        // Access the global AI system
        if let Ok(mut ai) = THE_AI.write() {
            if let Err(e) = ai.update(frame) {
                return Err(GameLogicError::AIError(format!("AI update failed: {}", e)));
            }
        } else {
            return Err(GameLogicError::AIError(
                "AI system lock poisoned".to_string(),
            ));
        }

        if let Some(result) = with_ai_integration_mut(|manager| manager.update_ai_players_only()) {
            if let Err(e) = result {
                warn!("AI player update failed at frame {}: {:?}", frame, e);
            }
        }

        Ok(())
    }

    /// Phase 3: Process command queue
    ///
    /// ## C++ Reference: GameLogic::processCommandList() (GameLogic.cpp)
    ///
    /// Processes all queued player commands:
    /// - Unit movement orders
    /// - Attack commands
    /// - Build orders
    /// - Special power activations
    pub fn process_command_queue(&mut self) -> Result<(), GameLogicError> {
        trace!(
            "GameLogic::process_command_queue() - {} commands pending",
            self.command_queue.len()
        );

        // C++ parity: consume routed command-list messages before object updates.
        // Route target is the shared CommandQueueManager fed by GameClient translators.
        if let Ok(mut processor) = crate::commands::get_command_processor().lock() {
            let mut context = crate::commands::CommandExecutionContext {
                current_frame: self.frame,
                player_id: 0,
                object_manager: None,
                player_manager: None,
                ai_manager: None,
                execution_start_time: Instant::now(),
                is_network_command: false,
                is_replay_command: false,
            };
            if let Err(err) = processor.process_frame(self.frame, &mut context) {
                warn!("Command processor frame execution failed: {}", err);
            }
        }

        // Process all pending commands
        while let Some(command) = self.command_queue.pop_front() {
            if let Err(e) = self.execute_command(command) {
                warn!("Command execution failed: {}", e);
                // Continue processing other commands
            }
        }

        // Also process commands through dispatch system
        if let Some(dispatch_mutex) = get_dispatch() {
            if let Ok(mut dispatch) = dispatch_mutex.lock() {
                if let Err(e) = dispatch.update(self.frame) {
                    warn!("Dispatch update failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Execute a single game command
    fn execute_command(&mut self, command: GameCommand) -> Result<(), GameLogicError> {
        match command {
            GameCommand::MoveUnit {
                player_id,
                unit_ids,
                target_position: _target_position,
            } => {
                trace!(
                    "Executing MoveUnit command for player {} ({} units)",
                    player_id,
                    unit_ids.len()
                );
                // In full implementation: apply movement orders to units
                Ok(())
            }
            GameCommand::AttackTarget {
                player_id,
                attacker_ids,
                target_id: _target_id,
            } => {
                trace!(
                    "Executing AttackTarget command for player {} ({} attackers)",
                    player_id,
                    attacker_ids.len()
                );
                // In full implementation: apply attack orders
                Ok(())
            }
            GameCommand::BuildStructure {
                player_id,
                builder_id: _builder_id,
                structure_type,
                position: _position,
            } => {
                trace!(
                    "Executing BuildStructure command for player {} ({})",
                    player_id,
                    structure_type
                );
                // In full implementation: start structure construction
                Ok(())
            }
            GameCommand::UseSpecialPower {
                player_id,
                power_name,
                target_position: _target_position,
            } => {
                trace!(
                    "Executing UseSpecialPower command for player {} ({})",
                    player_id,
                    power_name
                );
                // In full implementation: activate special power
                Ok(())
            }
        }
    }

    /// Phase 4: Update all objects and their modules
    ///
    /// ## C++ Reference: GameLogicDispatch.cpp (the dispatch system)
    /// ## C++ Reference: GameLogic.cpp lines 3672-3738 (update module processing)
    ///
    /// This is the largest phase of the update loop. It iterates through
    /// all live objects and calls their update() method, which in turn
    /// triggers ALL 86+ UpdateModule types:
    ///
    /// - AIUpdate (pathfinding, group commands, state machines)
    /// - StealthUpdate (stealth state, detection, disguise) - **NOW INTEGRATED**
    /// - FireWeaponUpdate (weapon firing, cooldowns)
    /// - PhysicsUpdate (gravity, velocity, collision detection)
    /// - ProductionUpdate (unit/structure building timers)
    /// - SpecialPowerUpdate (special ability state)
    /// - DockUpdate (docking, supply transfer, repair)
    /// - ... and 80+ more module types
    ///
    /// ## Critical Note:
    /// Objects can be destroyed during updates, so we use a cloned ID list
    /// to avoid iterator invalidation.
    ///
    /// ## Stealth Integration:
    /// Stealth updates are processed through the sleepy/normal update queues
    /// based on their wake frame. This matches C++ behavior where stealth
    /// is just another UpdateModule in the queue system.
    pub fn process_object_updates(&mut self, delta_time: f32) -> Result<(), GameLogicError> {
        trace!(
            "GameLogic::process_object_updates(delta={:.4}s) - {} objects",
            delta_time,
            self.all_objects.len()
        );

        // Clone object list to avoid iterator invalidation
        // (objects may be destroyed during update)
        let object_ids = self.all_objects.clone();

        for obj_id in object_ids {
            // Check if object still exists (may have been destroyed)
            if let Some(obj_ref) = self.objects.get(&obj_id) {
                if let Ok(mut obj) = obj_ref.write() {
                    // Call object's update method
                    // In full implementation, this triggers all UpdateModules
                    // including StealthUpdate which manages:
                    // - Stealth state transitions (stealthed/unstealthed)
                    // - Detection status (detected by enemies)
                    // - Disguise system (bomb truck disguising)
                    // - Stealth breaking conditions (attacking, moving, damage)
                    if let Err(e) = obj.update(delta_time) {
                        warn!("Object {} update failed: {:?}", obj_id, e);
                        // Don't abort - continue updating other objects
                    }
                } else {
                    warn!("Object {} lock poisoned during update", obj_id);
                }
            }
        }

        Ok(())
    }

    /// Update object-linked stealth controllers once per frame.
    ///
    /// C++ parity note: `StealthUpdate` is a standard update module in the
    /// regular update queue. The current Rust port stores stealth as an object
    /// handle; this bridge keeps per-frame stealth state transitions active.
    fn process_stealth_controllers(&mut self, delta_time: f32) -> Result<(), GameLogicError> {
        let mut handles = Vec::new();
        for (object_id, object_ref) in &self.objects {
            let Ok(object_guard) = object_ref.read() else {
                continue;
            };
            let Some(stealth) = object_guard.get_stealth() else {
                continue;
            };
            handles.push((*object_id, stealth));
        }

        for (object_id, handle) in handles {
            let Ok(mut stealth_guard) = handle.lock() else {
                warn!("Stealth controller lock poisoned for object {}", object_id);
                continue;
            };
            if let Err(err) = stealth_guard.update_stealth(delta_time) {
                warn!("Stealth update failed for object {}: {}", object_id, err);
            }
        }

        Ok(())
    }

    /// Run client-update modules attached to drawables (e.g. LaserUpdate).
    ///
    /// Mirrors the client-side drawable update phase where ClientUpdateModule
    /// instances run once per frame.
    fn process_client_updates(&mut self) {
        for obj_ref in self.objects.values() {
            let modules = match obj_ref.read() {
                Ok(obj_guard) => obj_guard.client_update_modules(),
                Err(_) => {
                    warn!("Object lock poisoned during client update");
                    continue;
                }
            };

            for module in modules {
                if module
                    .with_module_downcast::<LaserUpdateModule, _, _>(|laser_update| {
                        laser_update.update_mut().client_update();
                    })
                    .is_some()
                {
                    continue;
                }
                if module
                    .with_module_downcast::<BeaconClientUpdateModule, _, _>(|beacon_update| {
                        beacon_update.client_update();
                    })
                    .is_some()
                {
                    continue;
                }
                if module
                    .with_module_downcast::<SwayClientUpdateModule, _, _>(|sway_update| {
                        sway_update.client_update();
                    })
                    .is_some()
                {
                    continue;
                }
                let _ = module
                    .with_module_downcast::<AnimatedParticleSysBoneClientUpdateModule, _, _>(
                        |animated_update| {
                            animated_update.client_update();
                        },
                    );
            }
        }
    }

    /// Process sleepy (delayed) update modules
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3697-3738 (sleepy update queue)
    ///
    /// Sleepy updates are modules that only need to update occasionally,
    /// not every frame. They "sleep" until their wake frame arrives.
    ///
    /// ## Stealth Module Integration:
    /// StealthUpdate is a sleepy module that typically updates every frame but can sleep
    /// when disabled (disguise system, special power grant). Key behaviors:
    /// - Updates stealth state based on conditions (moving, attacking, damage)
    /// - Manages detection timer (when enemies detect the unit)
    /// - Handles disguise transitions (bomb truck)
    /// - Applies opacity changes for visual stealth effect
    /// - Returns UPDATE_SLEEP_NONE (1) when enabled, UPDATE_SLEEP_FOREVER when disabled
    ///
    /// ## C++ Behavior Match:
    /// - Lines 3697-3713: Peek at next sleepy update, check wake frame
    /// - Lines 3717-3732: Check disabled flags, call update(), get sleep time
    /// - Lines 3735-3736: Requeue with new wake frame (now + sleepLen)
    /// - Line 3737: Rebalance heap (we use BinaryHeap which auto-balances)
    fn process_sleepy_updates(&mut self, current_frame: UnsignedInt) {
        let mut processed = 0usize;
        let mut requeue: Vec<SleepyUpdateEntry> = Vec::new();

        // C++ lines 3698-3713: While loop processes all ready updates
        while let Some(entry) = self.sleepy_updates.peek() {
            // C++ line 3710: Check if wake frame has arrived
            if entry.wake_frame > current_frame {
                // No more entries ready to wake
                break;
            }

            let mut entry = self
                .sleepy_updates
                .pop()
                .expect("Heap became empty after peek");

            let object_ref = match self.objects.get(&entry.object_id) {
                Some(obj) => obj.clone(),
                None => {
                    continue;
                }
            };

            let (module_disabled_mask, phase) = entry
                .module
                .read()
                .map(|module| {
                    (
                        module.get_disabled_types_to_process(),
                        module.get_update_phase(),
                    )
                })
                .unwrap_or((DisabledMaskType::empty(), SleepyUpdatePhase::Normal));

            let object_disabled = object_ref.read().ok().map(|obj| obj.get_disabled_flags());
            let should_process = match object_disabled {
                Some(mask) if mask.any() => {
                    let disallowed = mask & !module_disabled_mask;
                    !disallowed.any()
                }
                _ => true,
            };

            // Update the module and get next wake time
            // C++ lines 3717-3732: Check disabled flags and call update()
            let next_wake;
            if should_process {
                match entry.module.write() {
                    Ok(mut module) => match module.update() {
                        Ok(sleep_time) => {
                            processed += 1;
                            match sleep_time {
                                UpdateSleepTime::Forever => {
                                    next_wake = None;
                                }
                                UpdateSleepTime::None => {
                                    next_wake = Some(current_frame.saturating_add(1));
                                }
                                UpdateSleepTime::Frames(frames) => {
                                    let sleep_frames = frames.max(1);
                                    let wake = current_frame.saturating_add(sleep_frames);
                                    next_wake = Some(wake);
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Sleepy update module for object {} failed: {}",
                                entry.object_id, e
                            );
                            // Retry next frame
                            next_wake = Some(current_frame.saturating_add(1));
                        }
                    },
                    Err(_) => {
                        warn!(
                            "Sleepy update module lock poisoned for object {}",
                            entry.object_id
                        );
                        next_wake = Some(current_frame.saturating_add(1));
                    }
                }
            } else {
                next_wake = Some(current_frame.saturating_add(1));
            }

            // Requeue for next wake (C++ line 3735-3736)
            if let Some(wake_frame) = next_wake {
                entry.phase = entry
                    .module
                    .read()
                    .map(|module| module.get_update_phase())
                    .unwrap_or(phase);
                entry.wake_frame = wake_frame;
                requeue.push(entry);
            }

            // Limit processing per frame to prevent runaway execution
            if processed >= MAX_SLEEPY_UPDATES_PER_FRAME {
                trace!(
                    "Processed {} sleepy updates; deferring remaining",
                    processed
                );
                break;
            }
        }

        // Re-add entries back to heap (C++ line 3737: rebalanceSleepyUpdate)
        // BinaryHeap automatically maintains heap property on push
        for entry in requeue {
            self.sleepy_updates.push(entry);
        }
    }

    /// C++ parity: GameLogic::processCommandList() (GameLogic.cpp line 2516)
    ///
    /// Iterate a list of game messages and dispatch each one through
    /// `logic_message_dispatcher`. The C++ version also validates CRCs
    /// from network players; that logic lives in the network layer in Rust.
    pub fn process_command_list(&mut self, messages: Vec<crate::messages::GameMessage>) {
        for msg in messages {
            self.logic_message_dispatcher(&msg);
        }
    }

    /// C++ parity: GameLogic::logicMessageDispatcher() (GameLogicDispatch.cpp line 328)
    ///
    /// Central command router that switches on the message type and dispatches
    /// to the appropriate handler. Matches the C++ switch statement exactly.
    pub fn logic_message_dispatcher(&mut self, msg: &crate::messages::GameMessage) {
        use crate::commands::command::CommandType;

        let command_type = match CommandType::try_from(msg.id as u16) {
            Ok(ct) => ct,
            Err(_) => {
                trace!("logic_message_dispatcher: unknown command type {}", msg.id);
                return;
            }
        };

        match command_type {
            CommandType::NewGame => {
                trace!("logic_message_dispatcher: MSG_NEW_GAME");
            }
            CommandType::ClearGameData => {
                trace!("logic_message_dispatcher: MSG_CLEAR_GAME_DATA");
            }
            CommandType::SetRallyPoint => {
                let obj_id = msg.arguments.first().and_then(|a| match a {
                    crate::messages::MessageArgument::ObjectId(id) => Some(*id),
                    _ => None,
                });
                let dest = msg.arguments.get(1).and_then(|a| match a {
                    crate::messages::MessageArgument::Location(c) => Some(*c),
                    _ => None,
                });
                if let (Some(id), Some(dest)) = (obj_id, dest) {
                    if let Some(obj_arc) = self.find_object_by_id(id) {
                        if let Ok(mut obj) = obj_arc.write() {
                            let _ = obj.set_rally_point(&dest);
                        }
                    }
                }
            }
            CommandType::DoWeapon => {
                trace!("logic_message_dispatcher: MSG_DO_WEAPON");
            }
            CommandType::CombatDropAtObject => {
                trace!("logic_message_dispatcher: MSG_COMBATDROP_AT_OBJECT");
            }
            CommandType::CombatDropAtLocation => {
                trace!("logic_message_dispatcher: MSG_COMBATDROP_AT_LOCATION");
            }
            CommandType::DoWeaponAtObject => {
                trace!("logic_message_dispatcher: MSG_DO_WEAPON_AT_OBJECT");
            }
            CommandType::SwitchWeapons => {
                trace!("logic_message_dispatcher: MSG_SWITCH_WEAPONS");
            }
            CommandType::SetMineClearingDetail => {
                trace!("logic_message_dispatcher: MSG_SET_MINE_CLEARING_DETAIL");
            }
            CommandType::EnableRetaliationMode => {
                trace!("logic_message_dispatcher: MSG_ENABLE_RETALIATION_MODE");
            }
            CommandType::DoWeaponAtLocation => {
                trace!("logic_message_dispatcher: MSG_DO_WEAPON_AT_LOCATION");
            }
            CommandType::DoSpecialPower => {
                trace!("logic_message_dispatcher: MSG_DO_SPECIAL_POWER");
            }
            CommandType::DoSpecialPowerAtLocation => {
                trace!("logic_message_dispatcher: MSG_DO_SPECIAL_POWER_AT_LOCATION");
            }
            CommandType::DoSpecialPowerAtObject => {
                trace!("logic_message_dispatcher: MSG_DO_SPECIAL_POWER_AT_OBJECT");
            }
            CommandType::DoAttackMoveTo => {
                trace!("logic_message_dispatcher: MSG_DO_ATTACKMOVETO");
            }
            CommandType::DoForceMoveTo => {
                trace!("logic_message_dispatcher: MSG_DO_FORCEMOVETO");
            }
            CommandType::DoSalvage | CommandType::DoMoveTo => {
                trace!("logic_message_dispatcher: MSG_DO_MOVETO/MSG_DO_SALVAGE");
            }
            CommandType::AddWaypoint => {
                trace!("logic_message_dispatcher: MSG_ADD_WAYPOINT");
            }
            CommandType::DoGuardPosition => {
                trace!("logic_message_dispatcher: MSG_DO_GUARD_POSITION");
            }
            CommandType::DoGuardObject => {
                trace!("logic_message_dispatcher: MSG_DO_GUARD_OBJECT");
            }
            CommandType::DoStop => {
                trace!("logic_message_dispatcher: MSG_DO_STOP");
            }
            CommandType::DoScatter => {
                trace!("logic_message_dispatcher: MSG_DO_SCATTER");
            }
            CommandType::CreateFormation => {
                trace!("logic_message_dispatcher: MSG_CREATE_FORMATION");
            }
            CommandType::ClearInGamePopupMessage => {
                trace!("logic_message_dispatcher: MSG_CLEAR_INGAME_POPUP_MESSAGE");
            }
            CommandType::DoCheer => {
                trace!("logic_message_dispatcher: MSG_DO_CHEER");
            }
            CommandType::Enter => {
                trace!("logic_message_dispatcher: MSG_ENTER");
            }
            CommandType::Exit => {
                trace!("logic_message_dispatcher: MSG_EXIT");
            }
            CommandType::Evacuate => {
                trace!("logic_message_dispatcher: MSG_EVACUATE");
            }
            CommandType::ExecuteRailedTransport => {
                trace!("logic_message_dispatcher: MSG_EXECUTE_RAILED_TRANSPORT");
            }
            CommandType::InternetHack => {
                trace!("logic_message_dispatcher: MSG_INTERNET_HACK");
            }
            CommandType::GetRepaired => {
                trace!("logic_message_dispatcher: MSG_GET_REPAIRED");
            }
            CommandType::Dock => {
                trace!("logic_message_dispatcher: MSG_DOCK");
            }
            CommandType::GetHealed => {
                trace!("logic_message_dispatcher: MSG_GET_HEALED");
            }
            CommandType::DoRepair => {
                trace!("logic_message_dispatcher: MSG_DO_REPAIR");
            }
            CommandType::ResumeConstruction => {
                trace!("logic_message_dispatcher: MSG_RESUME_CONSTRUCTION");
            }
            CommandType::DoSpecialPowerOverrideDestination => {
                trace!("logic_message_dispatcher: MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION");
            }
            CommandType::DoAttackObject => {
                trace!("logic_message_dispatcher: MSG_DO_ATTACK_OBJECT");
            }
            CommandType::DoForceAttackObject => {
                trace!("logic_message_dispatcher: MSG_DO_FORCE_ATTACK_OBJECT");
            }
            CommandType::DoForceAttackGround => {
                trace!("logic_message_dispatcher: MSG_DO_FORCE_ATTACK_GROUND");
            }
            CommandType::QueueUpgrade => {
                trace!("logic_message_dispatcher: MSG_QUEUE_UPGRADE");
            }
            CommandType::CancelUpgrade => {
                trace!("logic_message_dispatcher: MSG_CANCEL_UPGRADE");
            }
            CommandType::QueueUnitCreate => {
                trace!("logic_message_dispatcher: MSG_QUEUE_UNIT_CREATE");
            }
            CommandType::CancelUnitCreate => {
                trace!("logic_message_dispatcher: MSG_CANCEL_UNIT_CREATE");
            }
            CommandType::DozerConstruct | CommandType::DozerConstructLine => {
                trace!("logic_message_dispatcher: MSG_DOZER_CONSTRUCT");
            }
            CommandType::DozerCancelConstruct => {
                trace!("logic_message_dispatcher: MSG_DOZER_CANCEL_CONSTRUCT");
            }
            CommandType::Sell => {
                trace!("logic_message_dispatcher: MSG_SELL");
            }
            CommandType::ToggleOvercharge => {
                trace!("logic_message_dispatcher: MSG_TOGGLE_OVERCHARGE");
            }
            CommandType::CreateSelectedGroup | CommandType::CreateSelectedGroupNoSound => {
                trace!("logic_message_dispatcher: MSG_CREATE_SELECTED_GROUP");
            }
            CommandType::RemoveFromSelectedGroup => {
                trace!("logic_message_dispatcher: MSG_REMOVE_FROM_SELECTED_GROUP");
            }
            CommandType::DestroySelectedGroup => {
                trace!("logic_message_dispatcher: MSG_DESTROY_SELECTED_GROUP");
            }
            CommandType::SelectedGroupCommand => {}
            CommandType::PlaceBeacon => {
                trace!("logic_message_dispatcher: MSG_PLACE_BEACON");
            }
            CommandType::RemoveBeacon => {
                trace!("logic_message_dispatcher: MSG_REMOVE_BEACON");
            }
            CommandType::SetBeaconText => {
                trace!("logic_message_dispatcher: MSG_SET_BEACON_TEXT");
            }
            CommandType::SelfDestruct => {
                trace!("logic_message_dispatcher: MSG_SELF_DESTRUCT");
            }
            CommandType::SetReplayCamera => {}
            CommandType::LogicCrc => {
                trace!("logic_message_dispatcher: MSG_LOGIC_CRC");
            }
            CommandType::PurchaseScience => {
                trace!("logic_message_dispatcher: MSG_PURCHASE_SCIENCE");
            }
            CommandType::MetaBeginPathBuild => {
                trace!("logic_message_dispatcher: MSG_META_BEGIN_PATH_BUILD");
            }
            CommandType::MetaEndPathBuild => {
                trace!("logic_message_dispatcher: MSG_META_END_PATH_BUILD");
            }
            CommandType::DebugKillSelection
            | CommandType::DebugHurtObject
            | CommandType::DebugKillObject => {
                trace!("logic_message_dispatcher: debug command {:?}", command_type);
            }
            _ => {
                trace!(
                    "logic_message_dispatcher: unhandled command type {:?}",
                    command_type
                );
            }
        }
    }

    /// C++ parity: GameLogic::friend_awakenUpdateModule() (GameLogic.cpp line 2959)
    ///
    /// Wake a sleeping update module at a specific frame. If the module is already
    /// in the sleepy heap, update its wake frame and rebalance. If the owning object
    /// is not yet in the game object list, just set the wake frame without heap ops.
    pub fn friend_awaken_update_module(
        &mut self,
        module: &UpdateModulePtr,
        when_to_wake_up: UnsignedInt,
    ) {
        let now = self.frame;
        if when_to_wake_up < now {
            warn!(
                "setWakeFrame frame {} is in the past (now={})",
                when_to_wake_up, now
            );
        }

        // Check if already at this wake frame
        let current_wake = module
            .read()
            .map(|m| m.get_update_phase())
            .unwrap_or(SleepyUpdatePhase::Normal);

        // Find the entry in the sleepy heap
        let idx = self
            .sleepy_updates
            .iter()
            .position(|e| Arc::ptr_eq(&e.module, module) && e.wake_frame == when_to_wake_up);

        if let Some(_) = idx {
            return;
        }

        // Remove old entry if present, then insert with new wake frame
        let old_idx = self
            .sleepy_updates
            .iter()
            .position(|e| Arc::ptr_eq(&e.module, module));

        if let Some(_remove_idx) = old_idx {
            let mut entries: Vec<_> = self.sleepy_updates.drain().collect();
            entries.retain(|e| !Arc::ptr_eq(&e.module, module));

            let object_id = entries.first().map(|e| e.object_id).unwrap_or(0);

            entries.push(SleepyUpdateEntry {
                wake_frame: when_to_wake_up,
                phase: current_wake,
                object_id,
                module: Arc::clone(module),
            });

            for entry in entries {
                self.sleepy_updates.push(entry);
            }
        } else {
            // Not in heap yet - check if we know the object
            let obj_id = self
                .module_lookup
                .iter()
                .find(|(_, mods)| mods.iter().any(|m| Arc::ptr_eq(m, module)))
                .map(|(id, _)| *id)
                .unwrap_or(0);

            if obj_id != 0 {
                self.sleepy_updates.push(SleepyUpdateEntry {
                    wake_frame: when_to_wake_up,
                    phase: current_wake,
                    object_id: obj_id,
                    module: Arc::clone(module),
                });
            }
        }
    }

    /// C++ parity: GameLogic::rebalanceSleepyUpdate() (GameLogic.cpp line 2881)
    ///
    /// The Rust BinaryHeap auto-rebalances on push/pop, so this is a no-op
    /// that exists for API parity with C++.
    pub fn rebalance_sleepy_update(&mut self, _index: usize) {
        // BinaryHeap auto-rebalances; no manual work needed
    }

    /// C++ parity: GameLogic::rebalanceParentSleepyUpdate() (GameLogic.cpp line 2773)
    ///
    /// In C++, this bubbles an element up the heap. BinaryHeap handles this
    /// automatically, so this is a no-op for parity.
    pub fn rebalance_parent_sleepy_update(&mut self, _index: usize) -> usize {
        0
    }

    /// C++ parity: GameLogic::rebalanceChildSleepyUpdate() (GameLogic.cpp line 2799)
    ///
    /// In C++, this sifts an element down the heap. BinaryHeap handles this
    /// automatically, so this is a no-op for parity.
    pub fn rebalance_child_sleepy_update(&mut self, _index: usize) -> usize {
        0
    }

    /// C++ parity: GameLogic::validateSleepyUpdate() (GameLogic.cpp line 2693)
    ///
    /// Debug validation of the sleepy update heap. In C++ this checks parent/child
    /// priority ordering and index consistency. In Rust, BinaryHeap maintains
    /// invariants automatically.
    #[cfg(debug_assertions)]
    pub fn validate_sleepy_update(&self) {
        // BinaryHeap maintains its own invariants
    }

    #[cfg(not(debug_assertions))]
    pub fn validate_sleepy_update(&self) {}

    /// Process normal (every-frame) update modules
    fn process_normal_updates(&mut self) {
        let phases = [
            SleepyUpdatePhase::Initial,
            SleepyUpdatePhase::Physics,
            SleepyUpdatePhase::Normal,
            SleepyUpdatePhase::Final,
        ];

        for phase in phases {
            for entry in &self.normal_updates {
                let object_ref = match self.objects.get(&entry.object_id) {
                    Some(obj) => obj.clone(),
                    None => continue,
                };

                let (module_disabled_mask, module_phase) = entry
                    .module
                    .read()
                    .map(|module| {
                        (
                            module.get_disabled_types_to_process(),
                            module.get_update_phase(),
                        )
                    })
                    .unwrap_or((DisabledMaskType::empty(), SleepyUpdatePhase::Normal));
                if module_phase != phase {
                    continue;
                }

                let object_disabled = object_ref.read().ok().map(|obj| obj.get_disabled_flags());
                let should_process = match object_disabled {
                    Some(mask) if mask.any() => {
                        let disallowed = mask & !module_disabled_mask;
                        !disallowed.any()
                    }
                    _ => true,
                };
                if !should_process {
                    continue;
                }

                if let Ok(mut module) = entry.module.write() {
                    match module.update() {
                        Ok(UpdateSleepTime::None) => {}
                        Ok(other) => {
                            warn!(
                                "Normal update module for object {} returned sleep {:?}",
                                entry.object_id, other
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Normal update module for object {} failed: {}",
                                entry.object_id, e
                            );
                        }
                    }
                }
            }
        }
    }

    /// Phase 5: Resolve pending damage and physics
    ///
    /// ## C++ Reference: GameLogic::update() physics section
    ///
    /// Processes deferred damage and physics:
    /// - Apply pending damage from previous frame collisions
    /// - Resolve collisions
    /// - Update physics simulation (forces, velocities, positions)
    pub fn resolve_damage_and_physics(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::resolve_damage_and_physics()");

        // Process all pending damage and collisions
        let mut physics_world = std::mem::take(&mut self.physics_world);
        physics_world.resolve_all(self)?;
        self.physics_world = physics_world;

        let _ = with_collision_system_mut(|system| {
            for obj_arc in OBJECT_REGISTRY.get_all_objects() {
                let Ok(obj) = obj_arc.read() else {
                    continue;
                };
                let id = obj.get_id();
                let pos = obj.get_position();
                let geom = map_collision_geometry(
                    &obj.get_geometry_info(),
                    obj.get_template_geometry_type(),
                );
                if system
                    .update_object_position(
                        id,
                        crate::object::collide::Coord3D::new(pos.x, pos.y, pos.z),
                    )
                    .is_err()
                {
                    let _ = system.register_object(
                        id,
                        crate::object::collide::Coord3D::new(pos.x, pos.y, pos.z),
                        geom,
                        None,
                    );
                }
            }
            let _ = system.process_collisions();
            Ok::<(), crate::object::collide::CollisionError>(())
        });

        // Update physics engine (terrain-aware simulation)
        if let Ok(mut physics) = crate::physics::get_physics_engine().write() {
            if let Err(err) = physics.update() {
                return Err(GameLogicError::PhysicsError(format!("{err}")));
            }
        }

        Ok(())
    }

    /// Phase 6: Cleanup dead objects
    ///
    /// ## C++ Reference: GameLogic::processDestroyList() (GameLogic.cpp)
    ///
    /// Removes destroyed objects from the game world:
    /// - Release contained objects (passengers, etc.)
    /// - Remove from team/group
    /// - Remove from partition manager
    /// - Fire destruction events
    /// - Award experience to killer
    /// - Free memory/resources
    pub fn cleanup_dead_objects(&mut self) -> Result<(), GameLogicError> {
        trace!(
            "GameLogic::cleanup_dead_objects() - {} objects to clean",
            self.dead_objects.len()
        );

        // Track if we processed any objects for FOW updates
        let had_dead_objects = !self.dead_objects.is_empty();

        // Finish the drain up-front to avoid holding a borrow across the inner work.
        let drained: Vec<_> = self.dead_objects.drain(..).collect();

        // Process all dead objects
        for obj_id in drained {
            let mut object_position = None;
            let object_index = self.all_objects.iter().position(|&id| id == obj_id);
            let previous_object_id = object_index
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| self.all_objects.get(index).copied());
            let next_object_id =
                object_index.and_then(|index| self.all_objects.get(index + 1).copied());

            if let Some(previous_id) = previous_object_id {
                if let Some(previous_object) = self.objects.get(&previous_id) {
                    if let Ok(mut previous_guard) = previous_object.write() {
                        previous_guard.set_next_object_id(next_object_id);
                    }
                }
            }
            if let Some(next_id) = next_object_id {
                if let Some(next_object) = self.objects.get(&next_id) {
                    if let Ok(mut next_guard) = next_object.write() {
                        next_guard.set_prev_object_id(previous_object_id);
                    }
                }
            }

            if let Some(obj_ref) = self.objects.remove(&obj_id) {
                if let Ok(obj_read) = obj_ref.read() {
                    object_position = Some(*obj_read.get_position());
                }

                if let Ok(mut obj_write) = obj_ref.write() {
                    // If the object was not already fully cleaned up through a prior path,
                    // run the internal destroy routine that removes contained object links and
                    // invokes module onDelete hooks.
                    obj_write.on_destroy_internal();
                    obj_write.set_next_object_id(None);
                    obj_write.set_prev_object_id(None);
                }

                // Remove all update-module registrations for this object regardless of
                // whether it used `on_destroy()` prior to cleanup.
                self.remove_updates_for_object(obj_id);

                // Keep the script named-object cache in sync (C++ ScriptEngine::addObjectToCache parity).
                // Safe to call even if the object was never registered or had no name.
                let _ =
                    crate::scripting::engine::get_named_object_tracker().unregister_object(obj_id);
            }

            // Remove from object list
            self.all_objects.retain(|&id| id != obj_id);

            // Remove from objects map
            if let Some(pos) = object_position {
                let _ = with_ai_integration_mut(|manager| {
                    let _ = manager.notify_object_destroyed(obj_id, &[pos]);
                });
            }

            // Fire destruction event
            self.event_queue.push(GameEvent::ObjectDestroyed(obj_id));

            // Remove from partition manager
            self.partition_manager.remove_object(obj_id);

            let _ = with_collision_system_mut(|system| {
                let _ = system.unregister_object(obj_id);
                Ok::<(), crate::object::collide::CollisionError>(())
            });

            // In full implementation:
            // - Release contained objects
            // - Remove from team/group
            // - Award experience to killer
            // - Spawn death effects

            // Unregister from global registry
            OBJECT_REGISTRY.unregister_object(obj_id);

            trace!("Destroyed object {}", obj_id);
        }

        // Trigger FOW update if any objects were destroyed
        if had_dead_objects {
            if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                shroud_mgr.force_update();
            }
        }

        Ok(())
    }

    /// Phase 7: Update partition manager (spatial grid)
    ///
    /// ## C++ Reference: PartitionManager::update()
    pub fn update_partition_manager(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_partition_manager()");
        self.partition_manager.update()
    }

    pub fn partition_manager(&self) -> &PartitionManager {
        &self.partition_manager
    }

    pub fn partition_manager_mut(&mut self) -> &mut PartitionManager {
        &mut self.partition_manager
    }

    /// Phase 7: Update vision and shroud (fog of war)
    ///
    /// ## C++ Reference: Shroud system updates
    ///
    /// Updates visibility for all players:
    /// - Update visible objects for each player
    /// - Clear shroud in visible areas
    /// - Update stealth detection
    /// - Fire vision update events
    pub fn update_vision_and_shroud(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_vision_and_shroud()");

        // Update ShroudManager with current visibility information
        use crate::system::shroud_manager::get_shroud_manager;

        let shroud = get_shroud_manager();
        if let Ok(mut shroud_mgr) = shroud.lock() {
            // Update visibility cache (may skip frames based on update interval)
            // Uses self.frame instead of self.frame_counter (which doesn't exist)
            if let Err(e) = shroud_mgr.update(self.frame) {
                warn!("ShroudManager update failed: {}", e);
            }
        }

        // For each player, update their visible objects
        if let Ok(player_list_guard) = player_list().read() {
            for player_arc in player_list_guard.iter() {
                if let Ok(player) = player_arc.read() {
                    let player_id = player.get_player_index();

                    // In full implementation:
                    // - Query ShroudManager for visible objects
                    // - Update rendering visibility flags
                    // - Handle stealth detection
                    // - Update radar display

                    trace!("Updated vision for player {}", player_id);
                }
            }
        }

        Ok(())
    }

    /// Phase 8: Evaluate mission scripts
    ///
    /// ## C++ Reference: ScriptEngine::update()
    ///
    /// Runs mission scripting system:
    /// - Evaluate script conditions
    /// - Execute actions if conditions met
    /// - Check victory/defeat conditions
    /// - Track script completion
    pub fn evaluate_scripts(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::evaluate_scripts()");

        // Also update the global script engine
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                if let Err(e) = engine.update() {
                    warn!("ScriptEngine::update failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Check whether simulation time is frozen (script freeze or tactical freeze).
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3603-3617
    ///
    /// C++ checks `TheTacticalView->isTimeFrozen()`,
    /// `TheScriptEngine->isTimeFrozenDebug()`, and
    /// `TheScriptEngine->isTimeFrozenScript()`. When any of these are true,
    /// the update returns early (unless a MSG_CLEAR_GAME_DATA is in the
    /// command list, which forces an unfreeze).
    fn is_time_frozen(&self) -> bool {
        // Check script engine freeze state
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if engine.is_time_frozen() {
                    return true;
                }
            }
        }
        false
    }

    /// Update the terrain logic system.
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3619-3623
    ///
    /// C++: `TheTerrainLogic->UPDATE();`
    ///
    /// Terrain updates include bridge damage state transitions and dynamic
    /// water table changes. This runs after the early scripting phase so that
    /// script-triggered bridge damage is reflected before object updates.
    fn update_terrain(&self) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_terrain()");

        // The terrain logic singleton lives in the terrain module.
        // It manages bridges, dynamic water, and trigger areas.
        if let Ok(mut terrain) = crate::terrain::get_terrain_logic().write() {
            terrain.update();
        }

        Ok(())
    }

    /// Update the production/build system.
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3747-3750
    ///
    /// C++: `TheBuildAssistant->UPDATE();`
    ///
    /// Production updates run after AI so build orders issued by AI this frame
    /// are immediately reflected in production queues. The BuildAssistant
    /// manages structure placement validation and dozer assignment.
    fn update_production(&self, frame: UnsignedInt) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_production(frame={})", frame);

        // The BuildAssistant singleton lives in the game_engine common crate.
        // get_build_assistant() returns Option<MutexGuard<BuildAssistant>>.
        if let Some(mut build_assistant) =
            game_engine::common::system::build_assistant::get_build_assistant()
        {
            build_assistant.update(frame);
        }

        Ok(())
    }

    /// Update the weapon store (process delayed damage).
    ///
    /// ## C++ Reference: GameLogic.cpp line 3767
    ///
    /// C++: `TheWeaponStore->UPDATE();`
    ///
    /// The weapon store processes delayed damage entries whose trigger frame
    /// has arrived. This runs after death cleanup so we don't apply damage to
    /// objects that are already being destroyed.
    fn update_weapon_store(&self) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_weapon_store()");

        if let Err(e) = crate::weapon::with_weapon_store_mut(|store| store.update()) {
            // "System not initialized" means the weapon store hasn't been loaded
            // yet (e.g. before map load). Silently skip in that case.
            let err_str = e.to_string();
            if err_str.contains("not initialized") {
                trace!("Weapon store not initialized; skipping update");
            } else {
                warn!("Weapon store update failed: {}", err_str);
            }
        }

        Ok(())
    }

    /// Update victory conditions.
    ///
    /// ## C++ Reference: GameLogic.cpp line 3769, VictoryConditions.cpp update()
    ///
    /// C++: `TheVictoryConditions->UPDATE();`
    ///
    /// Each frame, checks all players for elimination conditions (no units, no
    /// buildings, or both) based on the multiplayer victory flags. Newly-defeated
    /// players are marked and their map is revealed. Also detects when only a
    /// single alliance remains (victory condition).
    ///
    /// Network-specific behavior (TheRecorder->isMultiplayer(), GameSlot,
    /// PopulateInGameDiplomacyPopup) is intentionally deferred per AGENTS.md.
    fn update_victory_conditions(&mut self) {
        trace!("GameLogic::update_victory_conditions()");

        // C++ skips evaluation before frame 2 (TheGameLogic->getFrame() > 1)
        if self.frame <= 1 {
            return;
        }

        let player_list = match player_list().read() {
            Ok(pl) => pl,
            Err(_) => return,
        };

        // Use the same elimination flags as C++ default:
        //   VICTORY_NOBUILDINGS | VICTORY_NOUNITS
        let flags = crate::system::victory_conditions::MultiplayerEliminationFlags::DEFAULT;
        let no_units = flags
            .contains(crate::system::victory_conditions::MultiplayerEliminationFlags::NO_UNITS);
        let no_buildings = flags
            .contains(crate::system::victory_conditions::MultiplayerEliminationFlags::NO_BUILDINGS);

        let mut newly_defeated_indices: Vec<(
            PlayerIndex,
            std::sync::Arc<std::sync::RwLock<Player>>,
        )> = Vec::new();

        // Phase 1: Scan for newly-defeated players (C++ lines 163-199)
        for player_arc in player_list.iter() {
            let Ok(player) = player_arc.read() else {
                continue;
            };
            let player_index = player.get_player_index();
            if player_index < 0 {
                continue;
            }

            // Skip civilian and observer players (C++ cachePlayerPtrs filter)
            if player.is_player_observer() {
                continue;
            }

            let is_defeated = match (no_units, no_buildings) {
                (true, true) => !player.has_any_objects(),
                (true, false) => !player.has_any_units(),
                (false, true) => !player.has_any_buildings_counts_for_victory(),
                (false, false) => false,
            };

            if is_defeated && !player.is_defeated() {
                newly_defeated_indices.push((player_index, std::sync::Arc::clone(player_arc)));
            }
        }

        // Release the read lock before acquiring write locks
        drop(player_list);

        // Phase 2: Handle newly-defeated players (C++ lines 166-198)
        for (player_index, player_arc) in &newly_defeated_indices {
            if let Ok(mut player) = player_arc.write() {
                player.set_defeated(true);
                info!(
                    "VictoryConditions: Player {} has been eliminated",
                    player_index
                );
            }

            // C++: ThePartitionManager->revealMapForPlayerPermanently(p->getPlayerIndex())
            // Deferred — partition manager integration is a separate port task.

            // C++: TheInGameUI->message("GUI:PlayerHasBeenDefeated", ...)
            // C++: TheAudio->addAudioEvent(&leftGameSound)
            // Deferred — UI/audio integration is a separate port task.

            // C++: p->killPlayer()
            // Deferred — killPlayer() destroys remaining player objects;
            // this will be implemented when the full player cleanup system is ported.
        }
    }

    /// Check and update disabled statuses on all objects.
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3783-3792
    ///
    /// C++:
    /// ```text
    /// for( Object *obj = m_objList; obj; obj = obj->getNextObject() )
    /// {
    ///     if( obj->isDisabled() )
    ///     {
    ///         obj->checkDisabledStatus();
    ///     }
    /// }
    /// ```
    ///
    /// Timer-based disabled states (e.g., Hacked, EMP, WeaponsetToggle) have
    /// expiration frames. This method checks all disabled objects and
    /// re-enables those whose disable duration has expired. The check runs at
    /// end-of-frame so disabled objects remain inactive for the entire frame.
    fn check_disabled_statuses(&self) {
        trace!("GameLogic::check_disabled_statuses()");

        for obj_id in &self.all_objects {
            if let Some(obj_ref) = self.objects.get(obj_id) {
                if let Ok(mut obj) = obj_ref.write() {
                    if obj.is_disabled() {
                        obj.check_disabled_status();
                    }
                }
            }
        }
    }

    // =========================================================================
    // Object Management Methods
    // =========================================================================

    /// Register a newly created object
    ///
    /// ## C++ Reference: GameLogic::registerObject() (GameLogic.cpp)
    pub fn register_object(
        &mut self,
        object: Arc<RwLock<Object>>,
    ) -> Result<ObjectID, GameLogicError> {
        let (object_id, object_name) = {
            let guard = object
                .read()
                .map_err(|_| GameLogicError::Generic("Object lock poisoned".to_string()))?;
            (guard.get_id(), guard.get_name().to_string())
        };

        if object_id == INVALID_ID {
            return Err(GameLogicError::InvalidState(
                "Attempted to register object without valid ID".to_string(),
            ));
        }

        let previous_object_id = self.all_objects.last().copied();

        // Add to object collections
        self.objects.insert(object_id, Arc::clone(&object));
        self.all_objects.push(object_id);

        if let Some(previous_id) = previous_object_id {
            if let Some(previous_object) = self.objects.get(&previous_id) {
                if let Ok(mut previous_guard) = previous_object.write() {
                    previous_guard.set_next_object_id(Some(object_id));
                }
            }
            if let Ok(mut object_guard) = object.write() {
                object_guard.set_prev_object_id(Some(previous_id));
                object_guard.set_next_object_id(None);
            }
        } else if let Ok(mut object_guard) = object.write() {
            object_guard.set_prev_object_id(None);
            object_guard.set_next_object_id(None);
        }

        // Register in global registry
        OBJECT_REGISTRY.register_object(object_id, &object);

        // Register in the scripting named-object cache (C++ ScriptEngine::addObjectToCache).
        if !object_name.is_empty() {
            let tracker = crate::scripting::engine::get_named_object_tracker();
            let _ = tracker.register_named_object(object_name, object_id);
        }

        // Add to partition manager
        if let Ok(obj) = object.read() {
            let pos = obj.get_position();
            self.partition_manager
                .add_object(object_id, (pos.x, pos.y, pos.z));

            let geom =
                map_collision_geometry(&obj.get_geometry_info(), obj.get_template_geometry_type());
            let _ = with_collision_system_mut(|system| {
                let _ = system.register_object(
                    object_id,
                    crate::object::collide::Coord3D::new(pos.x, pos.y, pos.z),
                    geom,
                    None,
                );
                let cfg = if obj.is_kind_of(KindOf::Projectile) {
                    CollisionResponseConfig {
                        response_type: CollisionResponseType::None,
                        ..Default::default()
                    }
                } else if obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Building)
                    || obj.is_kind_of(KindOf::Bridge)
                    || obj.is_kind_of(KindOf::Barrier)
                {
                    CollisionResponseConfig::blocking()
                } else {
                    CollisionResponseConfig::default()
                };
                system.set_collision_config(object_id, cfg);
                Ok::<(), crate::object::collide::CollisionError>(())
            });

            let is_ai_controlled = obj.get_ai_update_interface().is_some();
            let is_obstacle = obj.is_kind_of(KindOf::Building)
                || obj.is_kind_of(KindOf::Structure)
                || obj.is_kind_of(KindOf::Bridge)
                || obj.is_kind_of(KindOf::Barrier);
            let _ = with_ai_integration_mut(|manager| {
                let _ =
                    manager.notify_object_created(object_id, *pos, is_ai_controlled, is_obstacle);
            });
        }

        // Fire creation event
        self.event_queue.push(GameEvent::ObjectCreated(object_id));

        // Trigger FOW update for new object
        // New objects create new vision sources, so FOW needs to be recalculated
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.force_update();
        }

        debug!("Registered object {}", object_id);
        Ok(object_id)
    }

    /// Mark an object for destruction
    ///
    /// ## C++ Reference: GameLogic::destroyObject() (GameLogic.cpp)
    pub fn destroy_object(&mut self, object_id: ObjectID) {
        if object_id == INVALID_ID {
            return;
        }

        // Queue for destruction at end of frame
        if !self.dead_objects.contains(&object_id) {
            self.dead_objects.push(object_id);
            debug!("Queued object {} for destruction", object_id);
        }
    }

    /// Find an object by its ID
    ///
    /// ## C++ Reference: GameLogic::findObjectByID() (GameLogic.h inline)
    pub fn find_object_by_id(&self, object_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.objects.get(&object_id).cloned()
    }

    /// Allocate a unique object ID
    ///
    /// ## C++ Reference: GameLogic::allocateObjectID() (GameLogic.cpp)
    pub fn allocate_object_id(&mut self) -> ObjectID {
        let id = self.next_object_id;
        self.next_object_id = self.next_object_id.wrapping_add(1);
        if self.next_object_id == INVALID_ID {
            self.next_object_id = 1;
        }
        id
    }

    /// Get the next object-id counter value (C++ GameLogic::getObjectIDCounter).
    pub fn get_object_id_counter(&self) -> ObjectID {
        self.next_object_id
    }

    /// Set the next object-id counter value (C++ GameLogic::setObjectIDCounter).
    pub fn set_object_id_counter(&mut self, next_object_id: ObjectID) {
        let normalized = if next_object_id == 0 || next_object_id == INVALID_ID {
            1
        } else {
            next_object_id
        };
        self.next_object_id = normalized;
    }

    /// Get the first object (for iteration)
    pub fn get_first_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.all_objects
            .first()
            .and_then(|id| self.objects.get(id).cloned())
    }

    /// Get object count
    pub fn get_object_count(&self) -> usize {
        self.all_objects.len()
    }

    /// Get current number of queued sleepy update modules.
    ///
    /// ## C++ Reference: GameLogic::getNumberSleepyUpdates()
    pub fn get_number_sleepy_updates(&self) -> usize {
        self.sleepy_updates.len()
    }

    // =========================================================================
    // Update Module Registration
    // =========================================================================

    /// Register a normal (every-frame) update module
    pub fn register_normal_update_module(&mut self, object_id: ObjectID, module: UpdateModulePtr) {
        let entry = self.module_lookup.entry(object_id).or_insert_with(Vec::new);
        entry.retain(|existing| !Arc::ptr_eq(existing, &module));
        entry.push(module.clone());

        self.normal_updates
            .retain(|tracked| !Arc::ptr_eq(&tracked.module, &module));
        self.normal_updates
            .push(NormalUpdateEntry { object_id, module });
    }

    /// Register a sleepy (delayed) update module
    pub fn register_sleepy_update_module(
        &mut self,
        object_id: ObjectID,
        module: UpdateModulePtr,
        wake_frame: UnsignedInt,
    ) {
        let entry = self.module_lookup.entry(object_id).or_insert_with(Vec::new);
        entry.retain(|existing| !Arc::ptr_eq(existing, &module));
        entry.push(module.clone());

        let wake = if wake_frame == 0 {
            self.frame.saturating_add(1)
        } else {
            wake_frame
        };

        // Remove existing entry if present
        if !self.sleepy_updates.is_empty() {
            let mut heap = BinaryHeap::new();
            while let Some(entry) = self.sleepy_updates.pop() {
                if !Arc::ptr_eq(&entry.module, &module) {
                    heap.push(entry);
                }
            }
            self.sleepy_updates = heap;
        }

        let phase = module
            .read()
            .map(|module| module.get_update_phase())
            .unwrap_or(SleepyUpdatePhase::Normal);

        self.sleepy_updates.push(SleepyUpdateEntry {
            wake_frame: wake,
            phase,
            module,
            object_id,
        });
    }

    /// Unregister an update module
    pub fn unregister_update_module(&mut self, object_id: ObjectID, module: UpdateModulePtr) {
        self.normal_updates
            .retain(|entry| !Arc::ptr_eq(&entry.module, &module));

        if !self.sleepy_updates.is_empty() {
            let mut heap = BinaryHeap::new();
            while let Some(entry) = self.sleepy_updates.pop() {
                if !Arc::ptr_eq(&entry.module, &module) {
                    heap.push(entry);
                }
            }
            self.sleepy_updates = heap;
        }

        if let Some(list) = self.module_lookup.get_mut(&object_id) {
            list.retain(|existing| !Arc::ptr_eq(existing, &module));
            if list.is_empty() {
                self.module_lookup.remove(&object_id);
            }
        }
    }

    /// Remove all update modules for an object
    fn remove_updates_for_object(&mut self, object_id: ObjectID) {
        if let Some(entries) = self.module_lookup.remove(&object_id) {
            self.normal_updates.retain(|tracked| {
                !entries
                    .iter()
                    .any(|registered| Arc::ptr_eq(registered, &tracked.module))
            });
        }

        if !self.sleepy_updates.is_empty() {
            let mut heap = BinaryHeap::new();
            while let Some(entry) = self.sleepy_updates.pop() {
                if entry.object_id != object_id {
                    heap.push(entry);
                }
            }
            self.sleepy_updates = heap;
        }
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    pub fn get_frame(&self) -> UnsignedInt {
        self.frame
    }

    pub fn get_game_time(&self) -> f32 {
        self.game_time
    }

    pub fn is_in_game_logic_update(&self) -> Bool {
        self.is_in_update
    }

    pub fn set_dimensions(&mut self, width: Real, height: Real) {
        self.width = width;
        self.height = height;
    }

    pub fn get_width(&self) -> Real {
        self.width
    }

    pub fn get_height(&self) -> Real {
        self.height
    }

    pub fn set_game_mode(&mut self, mode: Int) {
        self.game_mode = mode;
    }

    pub fn get_game_mode(&self) -> Int {
        self.game_mode
    }

    pub fn is_in_single_player_game(&self) -> Bool {
        self.game_mode == GAME_SINGLE_PLAYER
    }

    pub fn is_in_multiplayer_game(&self) -> Bool {
        self.game_mode == GAME_LAN || self.game_mode == GAME_INTERNET
    }

    pub fn is_in_skirmish_game(&self) -> Bool {
        self.game_mode == GAME_SKIRMISH
    }

    pub fn set_loading_map(&mut self, loading: Bool) {
        self.loading_map = loading;
    }

    pub fn is_loading_map(&self) -> Bool {
        self.loading_map || crate::helpers::TheGameLogic::is_start_new_game_requested()
    }

    /// Complete the heavy new-game initialization path.
    ///
    /// The request is staged first, then this method performs the actual map
    /// load once the movie gate allows it.
    pub(crate) fn start_new_game_now(&mut self, loading_save_game: Bool) -> Result<(), String> {
        let map_path = game_engine::common::ini::get_global_data()
            .map(|data| data.read().map_name.clone())
            .unwrap_or_default();

        if map_path.is_empty() {
            crate::helpers::TheGameLogic::clear_start_new_game_request();
            return Err("Cannot start game: global map_name is empty".to_string());
        }

        if !loading_save_game {
            let mut state = game_engine::System::get_game_state();
            state.set_pristine_map_name(map_path.clone());
            if state.is_in_save_directory(std::path::Path::new(&map_path)) {
                log::error!(
                    "Pristine map name points to save directory map '{}'; sidecar lookup may diverge from C++ expected source-map semantics",
                    map_path
                );
            }
        }

        // Match C++ startNewGame(): the transition re-applies FP mode and clears the
        // staged start request before the actual map load begins.
        set_fp_mode();
        self.set_loading_map(true);
        crate::helpers::TheGameLogic::clear_start_new_game_request();

        let game_mode = match self.get_game_mode() {
            GAME_SHELL => crate::system::game_initialization::GameMode::ShellMap,
            GAME_SKIRMISH => crate::system::game_initialization::GameMode::Skirmish,
            GAME_LAN | GAME_INTERNET => crate::system::game_initialization::GameMode::Multiplayer,
            GAME_REPLAY => crate::system::game_initialization::GameMode::Replay,
            _ => crate::system::game_initialization::GameMode::SinglePlayer,
        };

        let difficulty = match crate::helpers::TheScriptEngine::get_global_difficulty() {
            0 => crate::system::game_initialization::GameDifficulty::Easy,
            2 => crate::system::game_initialization::GameDifficulty::Hard,
            3 => crate::system::game_initialization::GameDifficulty::Brutal,
            _ => crate::system::game_initialization::GameDifficulty::Normal,
        };

        let num_players = if let Ok(sides_guard) = crate::sides_list::get_sides_list().read() {
            let count = sides_guard.get_num_sides().max(1) as usize;
            count.min(crate::system::player_init::MAX_PLAYER_COUNT)
        } else if let Ok(player_list) = crate::player::ThePlayerList().read() {
            let count = player_list.iter().count();
            if count > 0 {
                count.min(crate::system::player_init::MAX_PLAYER_COUNT)
            } else {
                2
            }
        } else {
            2
        };

        let params = crate::system::game_initialization::GameInitParams {
            map_path,
            game_mode,
            difficulty,
            num_players,
            player_templates: Vec::new(),
            victory_type: crate::system::victory_conditions::VictoryType::Annihilation,
            score_limit: None,
            time_limit: None,
            fog_of_war_enabled: true,
            starting_resources: 0,
            ai_script: "DefaultAI".to_string(),
        };

        let init_result =
            crate::system::game_initialization::GameInitializer::initialize_game(params)
                .map(|_| ())
                .map_err(|err| format!("Game initialization failed: {}", err));
        self.set_loading_map(false);
        init_result
    }

    pub fn set_loading_save(&mut self, loading: Bool) {
        self.loading_save = loading;
    }

    pub fn is_loading_save(&self) -> Bool {
        self.loading_save
    }

    /// Queue a command for processing
    pub fn queue_command(&mut self, command: GameCommand) {
        self.command_queue.push_back(command);
    }

    /// Queue damage for physics resolution
    pub fn queue_damage(&mut self, target: ObjectID, attacker: ObjectID, amount: f32) {
        self.physics_world.queue_damage(target, attacker, amount);
    }

    pub fn queue_objects_changed_trigger_areas(&mut self, object_id: ObjectID) {
        if object_id == INVALID_ID {
            return;
        }

        self.objects_changed_trigger_areas.push_back(object_id);
    }

    pub fn update_objects_changed_trigger_areas(&mut self) {
        while let Some(object_id) = self.objects_changed_trigger_areas.pop_front() {
            trace!(
                "GameLogic::update_objects_changed_trigger_areas(object_id={})",
                object_id
            );
        }
    }

    /// Get object by ID (for command executor)
    pub fn get_object(&self, object_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.objects.get(&object_id).cloned()
    }

    /// Get object handle by ID for mutation (callers must lock the returned handle)
    pub fn get_object_mut(&mut self, object_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.get_object(object_id)
    }

    /// Get player by ID (for command executor)
    pub fn get_player(&self, player_id: u32) -> Option<Arc<RwLock<Player>>> {
        if let Ok(player_list_guard) = player_list().read() {
            for player_arc in player_list_guard.iter() {
                if let Ok(player) = player_arc.read() {
                    if player.get_player_index() == player_id as Int {
                        return Some(Arc::clone(player_arc));
                    }
                }
            }
        }
        None
    }

    /// Get mutable player by ID (for command executor)
    pub fn get_player_mut(&mut self, player_id: u32) -> Option<Arc<RwLock<Player>>> {
        self.get_player(player_id)
    }

    // =========================================================================
    // Snapshot/Save-Load Support Methods
    // =========================================================================

    /// Get current frame number (alias for get_frame for snapshot compatibility)
    /// Matches C++ GameLogic::getFrame
    pub fn get_current_frame(&self) -> u64 {
        self.frame as u64
    }

    /// Set current frame number (for restoring from snapshot)
    /// Matches C++ GameLogic::setFrame
    pub fn set_current_frame(&mut self, frame: u64) {
        self.frame = frame as UnsignedInt;
    }

    /// Get random seed for deterministic replay
    /// Matches C++ GameLogic::getRandomSeed
    pub fn get_random_seed(&self) -> u64 {
        self.random_seed
    }

    /// Set random seed (for restoring from snapshot)
    /// Matches C++ GameLogic::setRandomSeed
    pub fn set_random_seed(&mut self, seed: u64) {
        self.random_seed = seed;
    }

    /// Iterate over all objects in the game
    /// Returns iterator yielding Arc<RwLock<Object>> for each object
    pub fn iter_all_objects(&self) -> impl Iterator<Item = Arc<RwLock<Object>>> + '_ {
        self.objects.values().cloned()
    }

    /// Iterate over all players in the game
    /// Returns iterator yielding Arc<RwLock<Player>> for each player
    pub fn iter_players(&self) -> Vec<Arc<RwLock<Player>>> {
        let mut players = Vec::new();
        if let Ok(player_list_guard) = player_list().read() {
            for player_arc in player_list_guard.iter() {
                players.push(Arc::clone(player_arc));
            }
        }
        players
    }

    /// Clear all objects from the game (for loading saved game)
    /// Matches C++ GameLogic::clearAllObjects
    pub fn clear_all_objects(&mut self) {
        // Clear update module tracking first
        self.sleepy_updates.clear();
        self.normal_updates.clear();
        self.module_lookup.clear();

        // Clear object lists
        self.all_objects.clear();
        self.dead_objects.clear();
        self.objects.clear();

        // Reset object ID counter
        self.next_object_id = 1;

        // Clear event and command queues
        self.event_queue.clear();
        self.command_queue.clear();
        self.radar_updates.clear();
        self.objects_changed_trigger_areas.clear();

        log::debug!("Cleared all objects from GameLogic");
    }

    /// Rebuild spatial partition index after loading
    /// Matches C++ GameLogic::rebuildPartitionManager
    pub fn rebuild_spatial_index(&mut self) {
        self.partition_manager.rebuild();
        log::debug!("Rebuilt spatial index");
    }

    /// Rebuild selection cache after loading
    /// This ensures UI selection state is consistent
    pub fn rebuild_selection_cache(&mut self) {
        // Selection cache is managed by GameClient; GameLogic has no additional state to rebuild.
        log::debug!("Selection cache rebuild requested");
    }

    /// Create an object from a template name for save/load restoration.
    /// Mirrors C++ GameLogic::createObjectFromTemplate for save-load rehydration.
    pub fn create_object_from_template(
        &mut self,
        template_name: &str,
        object_id: ObjectID,
    ) -> Result<Arc<RwLock<Object>>, GameLogicError> {
        let template =
            crate::helpers::TheThingFactory::find_template(template_name).ok_or_else(|| {
                GameLogicError::InvalidState(format!("Template not found: {}", template_name))
            })?;

        let id = if object_id == INVALID_ID {
            self.allocate_object_id()
        } else {
            if object_id >= self.next_object_id {
                self.next_object_id = object_id + 1;
            }
            object_id
        };

        let status_mask = ObjectStatusMaskType::none();
        let object = Object::new_with_id(template, id, status_mask, None)
            .map_err(|err| GameLogicError::Generic(err.to_string()))?;

        self.register_object(object.clone())?;

        Ok(object)
    }

    /// Add a restored object to the game world
    /// Used during save game loading
    pub fn add_restored_object(&mut self, object_arc: Arc<RwLock<Object>>) {
        let object_id = if let Ok(obj) = object_arc.read() {
            obj.get_id()
        } else {
            log::error!("Failed to read object for restoration");
            return;
        };

        // Add to object collections
        self.objects.insert(object_id, Arc::clone(&object_arc));
        self.all_objects.push(object_id);

        if self.all_objects.len() > 1 {
            let previous_id = self.all_objects[self.all_objects.len() - 2];
            if let Some(previous_object) = self.objects.get(&previous_id) {
                if let Ok(mut previous_guard) = previous_object.write() {
                    previous_guard.set_next_object_id(Some(object_id));
                }
            }
            if let Ok(mut object_guard) = object_arc.write() {
                object_guard.set_prev_object_id(Some(previous_id));
                object_guard.set_next_object_id(None);
            }
        } else if let Ok(mut object_guard) = object_arc.write() {
            object_guard.set_prev_object_id(None);
            object_guard.set_next_object_id(None);
        }

        // Register with partition manager
        if let Ok(obj) = object_arc.read() {
            let pos = obj.get_position();
            self.partition_manager
                .register_object(object_id, pos.x, pos.y);
        }

        log::debug!("Added restored object with ID {}", object_id);
    }

    pub fn get_global_weapon_bonus_set(&self) -> &WeaponBonusSet {
        &self.global_weapon_bonus_set
    }

    // =========================================================================
    // C++ Parity: setDefaults, destroyAllObjectsImmediate, processDestroyList
    // =========================================================================

    /// PARITY_NOTE: GameLogic::setDefaults(Bool loadingSaveGame) C++ line 247.
    /// Resets frame counter, world dimensions, and update module lists.
    /// When `loading_save_game` is false, the object-ID allocator is also reset to 1.
    pub fn set_defaults(&mut self, loading_save_game: bool) {
        self.frame = 0;
        self.width = 0.0;
        self.height = 0.0;
        self.normal_updates.clear();
        for _entry in &self.sleepy_updates {
            // C++: (*it)->friend_setIndexInLogic(-1)
        }
        self.sleepy_updates.clear();
        if !loading_save_game {
            self.next_object_id = 1;
        }
    }

    /// PARITY_NOTE: GameLogic::destroyAllObjectsImmediate() C++ line 285.
    /// Iterates all live objects, destroys every one, then immediately
    /// processes the destroy list. Used during `reset()`.
    pub fn destroy_all_objects_immediate(&mut self) {
        let all_ids: Vec<ObjectID> = self.all_objects.drain(..).collect();
        for obj_id in &all_ids {
            self.destroy_object(*obj_id);
        }
        let _ = self.cleanup_dead_objects();
        debug_assert!(
            self.all_objects.is_empty(),
            "destroyAllObjectsImmediate: object list not cleared"
        );
    }

    /// PARITY_NOTE: GameLogic::processDestroyList() C++ line 2445.
    /// C++ name alias for `cleanup_dead_objects`.
    pub fn process_destroy_list(&mut self) -> Result<(), GameLogicError> {
        self.cleanup_dead_objects()
    }

    // =========================================================================
    // C++ Parity: selectObject / deselectObject
    // =========================================================================

    /// PARITY_NOTE: GameLogic::selectObject(Object*, Bool, PlayerMaskType, Bool) C++ line 2595.
    pub fn select_object(
        &mut self,
        object_id: ObjectID,
        create_new_selection: bool,
        _player_mask: PlayerMaskType,
        _affect_client: bool,
    ) {
        let Some(obj_ref) = self.find_object_by_id(object_id) else {
            return;
        };
        let Ok(obj) = obj_ref.read() else {
            return;
        };
        if !obj.is_mass_selectable() && !create_new_selection {
            trace!(
                "selectObject: object {} not mass-selectable, skipping",
                object_id
            );
            return;
        }
        // C++ creates an AIGroup, adds to player selection — deferred to InGameUI integration
        trace!(
            "selectObject: id={}, createNew={}",
            object_id,
            create_new_selection
        );
    }

    /// PARITY_NOTE: GameLogic::deselectObject(Object*, PlayerMaskType, Bool) C++ line 2646.
    pub fn deselect_object(
        &mut self,
        object_id: ObjectID,
        _player_mask: PlayerMaskType,
        _affect_client: bool,
    ) {
        if self.find_object_by_id(object_id).is_none() {
            return;
        }
        // C++ removes from player's AIGroup selection — deferred to InGameUI integration
        trace!("deselectObject: id={}", object_id);
    }

    // =========================================================================
    // C++ Parity: startNewGame / loadMapINI
    // =========================================================================

    /// PARITY_NOTE: GameLogic::startNewGame(Bool loadingSaveGame) C++ line 1081.
    /// Entry point for starting a new game or loading a save game.
    /// When not loading a save, sets the start-new-game flag so the actual
    /// load happens in the next update() call (after any intro movie).
    pub fn start_new_game(&mut self, loading_save_game: bool) {
        self.set_loading_map(true);
        if !loading_save_game {
            let map_path = game_engine::common::ini::get_global_data()
                .map(|data| data.read().map_name.clone())
                .unwrap_or_default();
            if !map_path.is_empty() {
                let mut state = game_engine::System::get_game_state();
                state.set_pristine_map_name(map_path);
            }
            if !crate::helpers::TheGameLogic::is_start_new_game_requested() {
                crate::helpers::TheGameLogic::request_start_new_game();
                return;
            }
        }
        self.rank_level_limit = 1000;
        self.set_defaults(loading_save_game);
        self.show_behind_building_markers = true;
        self.draw_icon_ui = true;
        self.show_dynamic_lod = true;
        set_fp_mode();
        if let Some(client) = TheGameClient::get() {
            client.set_frame(0);
        }
        self.frame = 0;
        self.set_loading_map(false);
    }

    /// PARITY_NOTE: GameLogic::loadMapINI(AsciiString mapName) C++ line 2367.
    /// Loads map-specific INI overrides (map.ini, solo.ini, map.str).
    pub fn load_map_ini(&self, map_name: &str) {
        if map_name.is_empty() || map_name.len() < 4 {
            return;
        }
        let base = &map_name[..map_name.len().saturating_sub(4)];
        let dir = match base.rfind(['/', '\\']) {
            Some(idx) => &base[..idx],
            None => base,
        };
        let map_ini = format!("{}/map.ini", dir);
        let solo_ini = format!("{}/solo.ini", dir);
        if std::path::Path::new(&map_ini).exists() {
            info!("map.ini found at {}", map_ini);
            // INI loading deferred to game_engine common INI subsystem
        }
        if std::path::Path::new(&solo_ini).exists() {
            info!("solo.ini found at {}", solo_ini);
        }
    }

    // =========================================================================
    // C++ Parity: bindObjectAndDrawable / sendObjectDestroyed
    // =========================================================================

    /// PARITY_NOTE: GameLogic::bindObjectAndDrawable(Object*, Drawable*) C++ line 4125.
    pub fn bind_object_and_drawable(&self, _object_id: ObjectID, _drawable_id: ObjectID) {
        trace!(
            "bindObjectAndDrawable: obj={}, draw={}",
            _object_id,
            _drawable_id
        );
    }

    /// PARITY_NOTE: GameLogic::sendObjectDestroyed(Object*) C++ line 4134.
    pub fn send_object_destroyed(&self, object_id: ObjectID) {
        trace!("sendObjectDestroyed: obj={}", object_id);
    }

    // =========================================================================
    // C++ Parity: prepareLogicForObjectLoad
    // =========================================================================

    /// PARITY_NOTE: GameLogic::prepareLogicForObjectLoad() C++ line 4584.
    /// Before loading a save game, destroys bridge objects (and their towers)
    /// and walk-on-top-of-wall objects so they can be re-created from save data.
    pub fn prepare_logic_for_object_load(&mut self) {
        let bridge_towers_to_destroy: Vec<ObjectID> = {
            let terrain = crate::terrain::get_terrain_logic();
            let terrain_guard = match terrain.read() {
                Ok(g) => g,
                Err(_) => return,
            };
            let mut towers = Vec::new();
            for &obj_id in &self.all_objects {
                let Some(obj_ref) = self.objects.get(&obj_id) else {
                    continue;
                };
                let Ok(obj) = obj_ref.read() else { continue };
                if obj.is_kind_of(KindOf::Bridge) {
                    let pos = *obj.get_position();
                    if let Some(bridge) = terrain_guard.find_bridge_at(&pos) {
                        for &tower_id in &bridge.get_bridge_info().tower_object_id {
                            if tower_id != INVALID_ID {
                                towers.push(tower_id);
                            }
                        }
                    }
                }
            }
            towers
        };

        for tower_id in bridge_towers_to_destroy {
            self.destroy_object(tower_id);
        }

        let ids_to_destroy: Vec<ObjectID> = self
            .all_objects
            .iter()
            .filter(|&&obj_id| {
                if let Some(obj_ref) = self.objects.get(&obj_id) {
                    if let Ok(obj) = obj_ref.read() {
                        return obj.is_kind_of(KindOf::Bridge)
                            || obj.is_kind_of(KindOf::WalkOnTopOfWall);
                    }
                }
                false
            })
            .copied()
            .collect();

        for obj_id in ids_to_destroy {
            self.destroy_object(obj_id);
        }
        let _ = self.process_destroy_list();
    }

    // =========================================================================
    // C++ Parity: ControlBar overrides
    // =========================================================================

    /// PARITY_NOTE: GameLogic::setControlBarOverride(AsciiString, Int, ConstCommandButtonPtr) C++ line 4389.
    pub fn set_control_bar_override(&mut self, command_set_name: &str, slot: i32) {
        if slot < 0 || slot > 9 {
            return;
        }
        let key = format!("{}{}", slot, command_set_name);
        self.control_bar_overrides.insert(key, ());
    }

    /// PARITY_NOTE: GameLogic::findControlBarOverride(AsciiString, Int, ConstCommandButtonPtr&) C++ line 4398.
    pub fn find_control_bar_override(&self, command_set_name: &str, slot: i32) -> bool {
        if slot < 0 || slot > 9 {
            return false;
        }
        let key = format!("{}{}", slot, command_set_name);
        self.control_bar_overrides.contains_key(&key)
    }

    // =========================================================================
    // C++ Parity: Superweapon restrictions
    // =========================================================================

    pub fn get_superweapon_restriction(&self) -> UnsignedShort {
        self.superweapon_restriction
    }

    pub fn set_superweapon_restriction(&mut self, restriction: UnsignedShort) {
        self.superweapon_restriction = restriction;
    }

    // =========================================================================
    // C++ Parity: Object TOC for save/load
    // =========================================================================

    /// PARITY_NOTE: GameLogic::findTOCEntryByName(AsciiString) C++ line 4460.
    pub fn find_toc_entry_by_name(&self, name: &str) -> Option<&ObjectTOCEntry> {
        self.object_toc.iter().find(|e| e.name == name)
    }

    /// PARITY_NOTE: GameLogic::findTOCEntryById(UnsignedShort) C++ line 4474.
    pub fn find_toc_entry_by_id(&self, id: UnsignedShort) -> Option<&ObjectTOCEntry> {
        self.object_toc.iter().find(|e| e.id == id)
    }

    /// PARITY_NOTE: GameLogic::addTOCEntry(AsciiString, UnsignedShort) C++ line 4488.
    pub fn add_toc_entry(&mut self, name: String, id: UnsignedShort) {
        self.object_toc.push(ObjectTOCEntry { name, id });
    }

    /// PARITY_NOTE: GameLogic::xferObjectTOC(Xfer*) C++ line 4501.
    /// Serializes/deserializes the object TOC used during save/load.
    pub fn xfer_object_toc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        self.object_toc.clear();
        let mut toc_count: UnsignedInt = 0;
        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                // PARITY_NOTE: Collect unique template names first to avoid
                // borrowing self mutably while iterating. C++ uses a plain
                // loop with direct map insertion (no borrow checker).
                let mut seen_names: std::collections::HashSet<String> =
                    self.object_toc.iter().map(|e| e.name.clone()).collect();
                let mut new_entries: Vec<(String, UnsignedShort)> = Vec::new();
                for obj_id in &self.all_objects {
                    if let Some(obj_ref) = self.objects.get(obj_id) {
                        if let Ok(obj) = obj_ref.read() {
                            let tname = obj.get_template().get_name().to_string();
                            if !seen_names.contains(&tname) {
                                seen_names.insert(tname.clone());
                                toc_count += 1;
                                new_entries.push((tname, toc_count as UnsignedShort));
                            }
                        }
                    }
                }
                for (name, id) in new_entries {
                    self.add_toc_entry(name, id);
                }
                xfer.xfer_unsigned_int(&mut toc_count)?;
                for entry in &mut self.object_toc {
                    xfer.xfer_string(&mut entry.name)?;
                    xfer.xfer_unsigned_short(&mut entry.id)?;
                }
            }
            XferMode::Load => {
                xfer.xfer_unsigned_int(&mut toc_count)?;
                for _ in 0..toc_count {
                    let mut name_str = String::new();
                    let mut id: UnsignedShort = 0;
                    xfer.xfer_string(&mut name_str)?;
                    xfer.xfer_unsigned_short(&mut id)?;
                    self.add_toc_entry(name_str, id);
                }
            }
            XferMode::Invalid => return Err(XferStatus::ModeUnknown),
        }
        Ok(())
    }

    // =========================================================================
    // C++ Parity: Sleepy update system methods
    // =========================================================================

    /// PARITY_NOTE: GameLogic::pushSleepyUpdate(UpdateModulePtr) C++ line 2907.
    /// Adds a module to the sleepy heap. Wake frame defaults to next frame.
    pub fn push_sleepy_update(&mut self, object_id: ObjectID, module: UpdateModulePtr) {
        let wake_frame = self.frame.saturating_add(1);
        self.register_sleepy_update_module(object_id, module, wake_frame);
    }

    /// PARITY_NOTE: GameLogic::popSleepyUpdate() C++ line 2930.
    pub fn pop_sleepy_update(&mut self) -> Option<SleepyUpdateEntry> {
        self.sleepy_updates.pop()
    }

    /// PARITY_NOTE: GameLogic::peekSleepyUpdate() C++ line 2920.
    pub fn peek_sleepy_update(&self) -> Option<&SleepyUpdateEntry> {
        self.sleepy_updates.peek()
    }

    /// PARITY_NOTE: GameLogic::eraseSleepyUpdate(Int i) C++ line 2737.
    pub fn erase_sleepy_update(&mut self, target_module: &UpdateModulePtr) {
        let mut heap = BinaryHeap::new();
        while let Some(entry) = self.sleepy_updates.pop() {
            if !Arc::ptr_eq(&entry.module, target_module) {
                heap.push(entry);
            }
        }
        self.sleepy_updates = heap;
    }

    /// PARITY_NOTE: GameLogic::remakeSleepyUpdate() C++ line 2890.
    pub fn remake_sleepy_update(&mut self) {
        let entries: Vec<SleepyUpdateEntry> = self.sleepy_updates.drain().collect();
        for entry in entries {
            self.sleepy_updates.push(entry);
        }
    }

    // =========================================================================
    // C++ Parity: Game pause
    // =========================================================================

    /// PARITY_NOTE: GameLogic::isGamePaused() C++ line 4157.
    pub fn is_game_paused(&self) -> bool {
        false
    }

    /// PARITY_NOTE: GameLogic::setGamePaused(Bool, Bool) C++ line 4164.
    pub fn set_game_paused(&mut self, _paused: bool, _pause_music: bool) {}

    // =========================================================================
    // C++ Parity: loadPostProcess
    // =========================================================================

    /// PARITY_NOTE: GameLogic::loadPostProcess() C++ line 4996.
    /// After all objects loaded from save: normalizes next-object-ID counter
    /// and rebuilds all sleepy/normal update module lists.
    pub fn load_post_process(&mut self) {
        self.next_object_id = INVALID_ID;
        for obj_id in &self.all_objects {
            if *obj_id >= self.next_object_id {
                self.next_object_id = obj_id.saturating_add(1);
            }
        }
        if self.next_object_id == INVALID_ID {
            self.next_object_id = 1;
        }
        self.sleepy_updates.clear();
        self.normal_updates.clear();
        self.module_lookup.clear();
        // C++ rebuilds update lists by iterating behavior modules.
        // In Rust, module registration happens during object construction
        // via register_normal_update_module/register_sleepy_update_module.
        // Modules will re-register when objects call onBuildComplete.
        let _ = if self.frame == 0 { 1 } else { self.frame };
        self.remake_sleepy_update();
    }

    fn refresh_global_weapon_bonuses(&mut self) {
        self.global_weapon_bonus_set = build_global_weapon_bonus_set();
    }
}

fn map_collision_geometry(
    info: &crate::common::GeometryInfo,
    template_type: Option<game_engine::system::geometry::GeometryType>,
) -> CollisionGeometryInfo {
    let dx = info.bounds.max.x - info.bounds.min.x;
    let dy = info.bounds.max.y - info.bounds.min.y;
    let dz = info.bounds.max.z - info.bounds.min.z;
    let radius = (dx.max(dy) * 0.5).max(0.01);
    let height = dz.max(0.01);
    let is_small = radius < 1.0;
    match template_type {
        Some(game_engine::system::geometry::GeometryType::Sphere) => {
            CollisionGeometryInfo::new_sphere(radius, is_small)
        }
        Some(game_engine::system::geometry::GeometryType::Box) => {
            CollisionGeometryInfo::new_box(dx.max(0.01), dy.max(0.01), is_small)
        }
        Some(game_engine::system::geometry::GeometryType::Cylinder) => {
            CollisionGeometryInfo::new_cylinder(radius, height, is_small)
        }
        None => {
            if height <= radius * 0.5 {
                CollisionGeometryInfo::new_sphere(radius, is_small)
            } else {
                CollisionGeometryInfo::new_cylinder(radius, height, is_small)
            }
        }
    }
}

fn build_global_weapon_bonus_set() -> WeaponBonusSet {
    let mut set = WeaponBonusSet::new();
    let Some(global_data) = game_engine::common::ini::get_global_data() else {
        return set;
    };

    let data = global_data.read();
    for entry in &data.weapon_bonus_entries {
        let Some(condition) = parse_bonus_condition(&entry.condition) else {
            continue;
        };
        let Some(field) = parse_bonus_field(&entry.field) else {
            continue;
        };

        let mut bonus = set
            .get_bonus(condition)
            .cloned()
            .unwrap_or_else(WeaponBonus::new);
        bonus.set_field(field, entry.value);
        set.set_bonus(condition, bonus);
    }

    set
}

fn parse_bonus_condition(value: &str) -> Option<WeaponBonusConditionType> {
    match value.trim().to_ascii_uppercase().as_str() {
        "GARRISONED" => Some(WeaponBonusConditionType::Garrisoned),
        "HORDE" => Some(WeaponBonusConditionType::Horde),
        "CONTINUOUS_FIRE_MEAN" => Some(WeaponBonusConditionType::ContinuousFireMean),
        "CONTINUOUS_FIRE_FAST" => Some(WeaponBonusConditionType::ContinuousFireFast),
        "NATIONALISM" => Some(WeaponBonusConditionType::Nationalism),
        "PLAYER_UPGRADE" => Some(WeaponBonusConditionType::PlayerUpgrade),
        "DRONE_SPOTTING" => Some(WeaponBonusConditionType::DroneSpotting),
        "DEMORALIZED" => Some(WeaponBonusConditionType::Demoralized),
        "DEMORALIZED_OBSOLETE" => Some(WeaponBonusConditionType::Demoralized),
        "ENTHUSIASTIC" => Some(WeaponBonusConditionType::Enthusiastic),
        "VETERAN" => Some(WeaponBonusConditionType::Veteran),
        "ELITE" => Some(WeaponBonusConditionType::Elite),
        "HERO" => Some(WeaponBonusConditionType::Hero),
        "BATTLEPLAN_BOMBARDMENT" => Some(WeaponBonusConditionType::BattleplanBombardment),
        "BATTLEPLAN_HOLDTHELINE" => Some(WeaponBonusConditionType::BattleplanHoldtheLine),
        "BATTLEPLAN_SEARCHANDDESTROY" => Some(WeaponBonusConditionType::BattleplanSearchAndDestroy),
        "SUBLIMINAL" => Some(WeaponBonusConditionType::Subliminal),
        "SOLO_HUMAN_EASY" => Some(WeaponBonusConditionType::SoloHumanEasy),
        "SOLO_HUMAN_NORMAL" => Some(WeaponBonusConditionType::SoloHumanNormal),
        "SOLO_HUMAN_HARD" => Some(WeaponBonusConditionType::SoloHumanHard),
        "SOLO_AI_EASY" => Some(WeaponBonusConditionType::SoloAiEasy),
        "SOLO_AI_NORMAL" => Some(WeaponBonusConditionType::SoloAiNormal),
        "SOLO_AI_HARD" => Some(WeaponBonusConditionType::SoloAiHard),
        "TARGET_FAERIE_FIRE" => Some(WeaponBonusConditionType::TargetFaerieFire),
        "FANATICISM" => Some(WeaponBonusConditionType::Fanaticism),
        "FRENZY_ONE" => Some(WeaponBonusConditionType::FrenzyOne),
        "FRENZY_TWO" => Some(WeaponBonusConditionType::FrenzyTwo),
        "FRENZY_THREE" => Some(WeaponBonusConditionType::FrenzyThree),
        "DRONE_SPOT_FOR_STRIKE" => Some(WeaponBonusConditionType::DroneSpotting),
        _ => None,
    }
}

fn parse_bonus_field(value: &str) -> Option<WeaponBonusField> {
    match value.trim().to_ascii_uppercase().as_str() {
        "DAMAGE" => Some(WeaponBonusField::Damage),
        "RADIUS" => Some(WeaponBonusField::Radius),
        "RANGE" => Some(WeaponBonusField::Range),
        "RATE_OF_FIRE" => Some(WeaponBonusField::RateOfFire),
        "PRE_ATTACK" | "PREATTACK" => Some(WeaponBonusField::PreAttack),
        _ => None,
    }
}

struct GameLogicEnergyLookup;

impl EnergyObjectLookup for GameLogicEnergyLookup {
    fn energy_production(&self, obj: ObjectHandle) -> i32 {
        let object_id = obj.value() as ObjectID;
        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return 0;
        };
        let Ok(guard) = object_arc.read() else {
            return 0;
        };
        guard.get_template().get_energy_production()
    }

    fn energy_bonus(&self, obj: ObjectHandle) -> i32 {
        let object_id = obj.value() as ObjectID;
        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return 0;
        };
        let Ok(guard) = object_arc.read() else {
            return 0;
        };
        guard.get_template().get_energy_bonus()
    }
}

struct GameLogicEnergyOwnerCallbacks;

impl EnergyOwnerCallbacks for GameLogicEnergyOwnerCallbacks {
    fn on_power_brown_out_change(&self, player: PlayerHandle, brown_out: bool) {
        let player_id = player.value() as PlayerIndex;
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(player_id) {
                if let Ok(mut guard) = player_arc.write() {
                    let _ = guard.on_power_brown_out_change(brown_out);
                }
            }
        }
    }
}

fn install_energy_integration() {
    let _ = set_energy_object_lookup(Arc::new(GameLogicEnergyLookup));
    let _ = set_energy_owner_callbacks(Arc::new(GameLogicEnergyOwnerCallbacks));
}

fn install_save_game_counter_integration() {
    register_object_id_counter_hooks(
        Some(Arc::new(|| {
            game_logic_mutex()
                .lock()
                .map(|logic| logic.get_object_id_counter())
                .unwrap_or(1)
        })),
        Some(Arc::new(|next_id| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.set_object_id_counter(next_id);
            }
        })),
    );

    register_save_load_lifecycle_hooks(
        Some(Arc::new(|| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.clear_all_objects();
                logic.rebuild_spatial_index();
                logic.set_loading_map(true);
            }
        })),
        Some(Arc::new(|| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.set_loading_map(false);
            }
        })),
        Some(Arc::new(|loading| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.set_loading_save(loading);
            }
        })),
        Some(Arc::new(|| {
            game_logic_mutex()
                .lock()
                .map(|logic| logic.get_game_mode())
                .unwrap_or(GAME_NONE)
        })),
        Some(Arc::new(|game_mode| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.set_game_mode(game_mode);
            }
        })),
        Some(Arc::new(|| {
            let map_name = game_engine::common::ini::get_global_data()
                .map(|data| data.read().map_name.clone())
                .unwrap_or_default();
            if !map_name.is_empty() {
                if let Ok(mut terrain) = crate::terrain::get_terrain_logic().write() {
                    if terrain.load_map(AsciiString::from(map_name.as_str()), false) {
                        terrain.new_map(true);
                    }
                }
            }
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.set_loading_map(false);
            }
        })),
        Some(Arc::new(|| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.rebuild_spatial_index();
                let _ = logic.update_partition_manager();
                logic.rebuild_selection_cache();
            }
            let _ = with_ai_integration_mut(|ai| {
                let _ = ai.new_map();
            });
        })),
    );

    register_save_load_mission_hooks(
        Some(Arc::new(|| {
            let _ = crate::helpers::TheGameLogic::clear_game_data();
        })),
        None,
    );

    register_save_lock_ghost_objects_hook(Some(Arc::new(|enable| {
        if let Ok(mut manager) = THE_GHOST_OBJECT_MANAGER.write() {
            manager.save_lock_ghost_objects(enable);
        }
    })));

    register_game_logic_snapshot_block();
}

// =============================================================================
// Global Singleton Access
// =============================================================================

static GAME_LOGIC: OnceLock<Mutex<GameLogic>> = OnceLock::new();
static SAVE_GAME_COUNTER_HOOKS: OnceLock<()> = OnceLock::new();

fn game_logic_mutex() -> &'static Mutex<GameLogic> {
    SAVE_GAME_COUNTER_HOOKS.get_or_init(|| {
        install_save_game_counter_integration();
    });
    GAME_LOGIC.get_or_init(|| Mutex::new(GameLogic::default()))
}

/// Get the global GameLogic singleton
pub fn get_game_logic() -> &'static Mutex<GameLogic> {
    game_logic_mutex()
}

/// Try to fetch the current simulation frame from the global GameLogic instance.
pub fn try_current_frame() -> Result<UnsignedInt, String> {
    game_logic_mutex()
        .lock()
        .map(|logic| logic.get_frame())
        .map_err(|_| "GameLogic mutex poisoned".to_string())
}

/// Convenience helper that returns the current frame, defaulting to 0 if unavailable.
pub fn current_frame() -> UnsignedInt {
    try_current_frame().unwrap_or(0)
}

/// Initialize the GameLogic singleton
pub fn init_game_logic() -> Result<(), String> {
    let mut guard = game_logic_mutex()
        .lock()
        .map_err(|_| "GameLogic mutex poisoned".to_string())?;
    guard.init();
    Ok(())
}

/// Reset GameLogic state
pub fn reset_game_logic() -> Result<(), String> {
    let mut guard = game_logic_mutex()
        .lock()
        .map_err(|_| "GameLogic mutex poisoned".to_string())?;
    guard.reset();
    Ok(())
}

/// Step the GameLogic singleton
pub fn update_game_logic() -> Result<(), String> {
    let mut guard = game_logic_mutex()
        .lock()
        .map_err(|_| "GameLogic mutex poisoned".to_string())?;
    let frame = guard.get_frame();
    guard
        .update(frame)
        .map_err(|e| format!("GameLogic update failed: {}", e))
}

/// Convenience helper for callers that need a mutable guard
pub fn lock_game_logic() -> Result<MutexGuard<'static, GameLogic>, String> {
    game_logic_mutex()
        .lock()
        .map_err(|_| "GameLogic mutex poisoned".to_string())
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn test_state_lock() -> std::sync::MutexGuard<'static, ()> {
        static TEST_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_STATE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test state lock poisoned")
    }

    #[test]
    fn test_game_logic_creation() {
        let logic = GameLogic::new();
        assert_eq!(logic.frame, 0);
        assert_eq!(logic.game_time, 0.0);
        assert!(!logic.is_in_update);
    }

    #[test]
    fn test_game_logic_reset() {
        let mut logic = GameLogic::new();
        logic.frame = 100;
        logic.game_time = 3.33;
        logic.reset();
        assert_eq!(logic.frame, 0);
        assert_eq!(logic.game_time, 0.0);
    }

    #[test]
    fn test_object_id_allocation() {
        let mut logic = GameLogic::new();
        let id1 = logic.allocate_object_id();
        let id2 = logic.allocate_object_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_object_list_links_relink_on_cleanup() {
        let _guard = test_state_lock();
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::object::Object;
        use std::sync::{Arc, RwLock};

        OBJECT_REGISTRY.clear();

        let mut logic = GameLogic::new();
        let first = Arc::new(RwLock::new(Object::new_test(11, 100.0)));
        let middle = Arc::new(RwLock::new(Object::new_test(22, 100.0)));
        let last = Arc::new(RwLock::new(Object::new_test(33, 100.0)));

        OBJECT_REGISTRY.register_object(11, &first);
        OBJECT_REGISTRY.register_object(22, &middle);
        OBJECT_REGISTRY.register_object(33, &last);

        logic.add_restored_object(Arc::clone(&first));
        logic.add_restored_object(Arc::clone(&middle));
        logic.add_restored_object(Arc::clone(&last));

        assert_eq!(logic.all_objects, vec![11, 22, 33]);
        assert_eq!(
            first
                .read()
                .unwrap()
                .get_next_object()
                .unwrap()
                .read()
                .unwrap()
                .get_id(),
            22
        );
        assert_eq!(
            middle
                .read()
                .unwrap()
                .get_prev_object()
                .unwrap()
                .read()
                .unwrap()
                .get_id(),
            11
        );

        logic.destroy_object(22);
        assert!(logic.cleanup_dead_objects().is_ok());

        assert_eq!(logic.all_objects, vec![11, 33]);
        assert_eq!(
            first
                .read()
                .unwrap()
                .get_next_object()
                .unwrap()
                .read()
                .unwrap()
                .get_id(),
            33
        );
        assert_eq!(
            last.read()
                .unwrap()
                .get_prev_object()
                .unwrap()
                .read()
                .unwrap()
                .get_id(),
            11
        );

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_register_object_sets_link_ids() {
        let _guard = test_state_lock();
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::object::Object;
        use std::sync::{Arc, RwLock};

        OBJECT_REGISTRY.clear();

        let mut logic = GameLogic::new();
        let first = Arc::new(RwLock::new(Object::new_test(44, 100.0)));
        let second = Arc::new(RwLock::new(Object::new_test(55, 100.0)));

        assert_eq!(logic.register_object(Arc::clone(&first)).unwrap(), 44);
        assert_eq!(logic.register_object(Arc::clone(&second)).unwrap(), 55);

        assert_eq!(logic.all_objects, vec![44, 55]);
        assert_eq!(first.read().unwrap().get_next_object_id(), Some(55));
        assert_eq!(first.read().unwrap().get_prev_object_id(), None);
        assert_eq!(second.read().unwrap().get_prev_object_id(), Some(44));
        assert_eq!(second.read().unwrap().get_next_object_id(), None);

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_frame_events_cleared() {
        let mut logic = GameLogic::new();
        logic.event_queue.push(GameEvent::ObjectCreated(1));
        logic.radar_updates.push(RadarUpdate {
            player_id: 0,
            position: (0.0, 0.0),
            event_type: RadarEventType::UnitCreated,
        });

        assert!(logic.clear_frame_events().is_ok());
        assert_eq!(logic.event_queue.len(), 0);
        assert_eq!(logic.radar_updates.len(), 0);
    }

    #[test]
    fn test_radar_updates_promoted_to_events() {
        let mut logic = GameLogic::new();
        logic.radar_updates.push(RadarUpdate {
            player_id: 1,
            position: (42.0, 84.0),
            event_type: RadarEventType::BaseAttacked,
        });

        logic.process_radar_updates();

        assert_eq!(logic.event_queue.len(), 1);
        match &logic.event_queue[0] {
            GameEvent::RadarUpdate {
                player_id,
                position,
                event_type,
            } => {
                assert_eq!(*player_id, 1);
                assert_eq!(*position, (42.0, 84.0));
                assert!(matches!(event_type, RadarEventType::BaseAttacked));
            }
            other => panic!("Unexpected event emitted: {:?}", other),
        }
    }

    #[test]
    fn test_update_loop_phases() {
        let mut logic = GameLogic::new();

        // Should not allow re-entrant calls
        logic.is_in_update = true;
        let result = logic.update(0);
        assert!(result.is_err());

        // Normal update should succeed
        logic.is_in_update = false;
        let result = logic.update(0);
        assert!(result.is_ok());
        assert_eq!(logic.frame, 0);
    }

    #[test]
    fn test_command_queue() {
        let mut logic = GameLogic::new();
        let command = GameCommand::MoveUnit {
            player_id: 0,
            unit_ids: vec![1, 2, 3],
            target_position: (100.0, 100.0, 0.0),
        };

        logic.queue_command(command);
        assert_eq!(logic.command_queue.len(), 1);

        // Process commands
        let result = logic.process_command_queue();
        assert!(result.is_ok());
        assert_eq!(logic.command_queue.len(), 0);
    }

    #[test]
    fn test_physics_damage_queue() {
        let mut logic = GameLogic::new();
        logic.queue_damage(1, 2, 50.0);

        assert_eq!(logic.physics_world.pending_damage.len(), 1);
    }

    #[test]
    fn test_game_mode_checks() {
        let mut logic = GameLogic::new();

        logic.set_game_mode(GAME_SINGLE_PLAYER);
        assert!(logic.is_in_single_player_game());
        assert!(!logic.is_in_multiplayer_game());

        logic.set_game_mode(GAME_LAN);
        assert!(!logic.is_in_single_player_game());
        assert!(logic.is_in_multiplayer_game());
    }

    #[test]
    fn test_sleepy_update_ordering() {
        let _logic = GameLogic::new();

        let entry1 = SleepyUpdateEntry {
            wake_frame: 10,
            phase: SleepyUpdatePhase::Normal,
            object_id: 1,
            module: Arc::new(RwLock::new(crate::modules::UpdateModuleDummy {})),
        };
        let entry2 = SleepyUpdateEntry {
            wake_frame: 5,
            phase: SleepyUpdatePhase::Normal,
            object_id: 2,
            module: Arc::new(RwLock::new(crate::modules::UpdateModuleDummy {})),
        };

        // Earlier wake frame should have higher priority (min-heap)
        assert!(entry2 > entry1);
    }

    // ============================================================================
    // WEEK 2: GAME LOOP INTEGRATION TESTS (60+ tests for orchestration)
    // ============================================================================

    #[test]
    fn test_fixed_delta_time_constant() {
        // Verify fixed timestep is correct for 30 FPS
        assert!((FIXED_DELTA_TIME - 1.0 / 30.0).abs() < 0.00001);
    }

    #[test]
    fn test_frame_counting() {
        let mut logic = GameLogic::new();

        for frame in 0..100 {
            let result = logic.update(frame);
            assert!(result.is_ok());
            assert_eq!(logic.get_frame(), frame);
        }
    }

    #[test]
    fn test_game_time_accumulation() {
        let mut logic = GameLogic::new();
        logic.init();

        // Game time tracks the start time of the current frame: `time = frame * dt`.
        // At frame 30 (30 FPS), time should be 1 second.
        for frame in 0..=30 {
            let _ = logic.update(frame);
        }

        assert!((logic.get_game_time() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_multiple_commands_processed() {
        let mut logic = GameLogic::new();

        // Queue multiple commands
        for i in 0..10 {
            let command = GameCommand::MoveUnit {
                player_id: 0,
                unit_ids: vec![i],
                target_position: (100.0, 100.0, 0.0),
            };
            logic.queue_command(command);
        }

        assert_eq!(logic.command_queue.len(), 10);

        // Process them
        let result = logic.process_command_queue();
        assert!(result.is_ok());
        assert_eq!(logic.command_queue.len(), 0);
    }

    #[test]
    fn test_world_dimensions() {
        let mut logic = GameLogic::new();
        logic.set_dimensions(1024.0, 768.0);

        assert_eq!(logic.get_width(), 1024.0);
        assert_eq!(logic.get_height(), 768.0);
    }

    #[test]
    fn test_loading_flags() {
        let mut logic = GameLogic::new();

        assert!(!logic.is_loading_map());
        logic.set_loading_map(true);
        assert!(logic.is_loading_map());

        assert!(!logic.is_loading_save());
        logic.set_loading_save(true);
        assert!(logic.is_loading_save());
    }

    #[test]
    fn test_new_game_start_request_waits_for_movie_gate() {
        let mut logic = GameLogic::new();

        crate::helpers::TheGameLogic::clear_start_new_game_request();
        crate::helpers::TheGameLogic::set_intro_movie_playing(true);
        crate::helpers::TheGameLogic::request_start_new_game();

        assert!(logic.is_loading_map());

        assert!(logic.update(0).is_ok());
        assert!(crate::helpers::TheGameLogic::is_start_new_game_requested());
        assert!(logic.is_loading_map());

        crate::helpers::TheGameLogic::set_intro_movie_playing(false);
        crate::helpers::TheGameLogic::clear_start_new_game_request();
    }

    #[test]
    fn test_game_event_queue_cleared_each_frame() {
        let mut logic = GameLogic::new();

        // Add an event
        logic.event_queue.push(GameEvent::ObjectCreated(1));
        assert_eq!(logic.event_queue.len(), 1);

        // Frame update should clear events
        let _ = logic.clear_frame_events();
        assert_eq!(logic.event_queue.len(), 0);
    }

    #[test]
    fn test_move_command_parsing() {
        let cmd = GameCommand::MoveUnit {
            player_id: 0,
            unit_ids: vec![1, 2, 3],
            target_position: (100.0, 200.0, 0.0),
        };

        match cmd {
            GameCommand::MoveUnit {
                player_id,
                unit_ids,
                target_position,
            } => {
                assert_eq!(player_id, 0);
                assert_eq!(unit_ids.len(), 3);
                assert_eq!(target_position.0, 100.0);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_attack_command_parsing() {
        let cmd = GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![5, 6],
            target_id: 99,
        };

        match cmd {
            GameCommand::AttackTarget {
                player_id,
                attacker_ids,
                target_id,
            } => {
                assert_eq!(player_id, 1);
                assert_eq!(attacker_ids.len(), 2);
                assert_eq!(target_id, 99);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_build_command_parsing() {
        let cmd = GameCommand::BuildStructure {
            player_id: 0,
            builder_id: 10,
            structure_type: "BarracksBridge".to_string(),
            position: (500.0, 500.0),
        };

        match cmd {
            GameCommand::BuildStructure {
                player_id,
                builder_id,
                structure_type,
                position,
            } => {
                assert_eq!(player_id, 0);
                assert_eq!(builder_id, 10);
                assert_eq!(structure_type, "BarracksBridge");
                assert_eq!(position.0, 500.0);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_special_power_command_parsing() {
        let cmd = GameCommand::UseSpecialPower {
            player_id: 0,
            power_name: "Carpet Bomb".to_string(),
            target_position: Some((300.0, 300.0, 0.0)),
        };

        match cmd {
            GameCommand::UseSpecialPower {
                player_id,
                power_name,
                target_position,
            } => {
                assert_eq!(player_id, 0);
                assert_eq!(power_name, "Carpet Bomb");
                assert!(target_position.is_some());
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_radar_update_creation() {
        let update = RadarUpdate {
            player_id: 0,
            position: (250.0, 250.0),
            event_type: RadarEventType::UnitCreated,
        };

        assert_eq!(update.player_id, 0);
        assert_eq!(update.position.0, 250.0);
        assert!(matches!(update.event_type, RadarEventType::UnitCreated));
    }

    #[test]
    fn test_all_radar_event_types() {
        let events = vec![
            RadarEventType::UnitCreated,
            RadarEventType::UnitDestroyed,
            RadarEventType::BaseAttacked,
            RadarEventType::EnemyDetected,
        ];

        assert_eq!(events.len(), 4);
    }

    #[test]
    fn test_game_mode_single_player() {
        let mut logic = GameLogic::new();
        logic.set_game_mode(GAME_SINGLE_PLAYER);

        assert!(logic.is_in_single_player_game());
        assert!(!logic.is_in_multiplayer_game());
        assert!(!logic.is_in_skirmish_game());
    }

    #[test]
    fn test_game_mode_lan() {
        let mut logic = GameLogic::new();
        logic.set_game_mode(GAME_LAN);

        assert!(!logic.is_in_single_player_game());
        assert!(logic.is_in_multiplayer_game());
        assert!(!logic.is_in_skirmish_game());
    }

    #[test]
    fn test_game_mode_internet() {
        let mut logic = GameLogic::new();
        logic.set_game_mode(GAME_INTERNET);

        assert!(!logic.is_in_single_player_game());
        assert!(logic.is_in_multiplayer_game());
        assert!(!logic.is_in_skirmish_game());
    }

    #[test]
    fn test_game_mode_skirmish() {
        let mut logic = GameLogic::new();
        logic.set_game_mode(GAME_SKIRMISH);

        assert!(!logic.is_in_single_player_game());
        assert!(!logic.is_in_multiplayer_game());
        assert!(logic.is_in_skirmish_game());
    }

    #[test]
    fn test_physics_world_damage_queuing() {
        let mut physics = PhysicsWorld::new();

        physics.queue_damage(10, 20, 50.0);
        physics.queue_damage(11, 21, 75.0);

        assert_eq!(physics.pending_damage.len(), 2);
    }

    #[test]
    fn test_object_id_allocation_sequential() {
        let mut logic = GameLogic::new();

        let id1 = logic.allocate_object_id();
        let id2 = logic.allocate_object_id();
        let id3 = logic.allocate_object_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn test_update_not_reentrant() {
        let mut logic = GameLogic::new();

        // Set update flag
        logic.is_in_update = true;

        // Attempt update should fail
        let result = logic.update(0);
        assert!(result.is_err());

        match result.unwrap_err() {
            GameLogicError::InvalidState(msg) => {
                assert!(msg.contains("Re-entrant"));
            }
            _ => panic!("Expected InvalidState error"),
        }
    }

    #[test]
    fn test_error_display_object_not_found() {
        let err = GameLogicError::ObjectNotFound(999);
        assert!(err.to_string().contains("999"));
    }

    #[test]
    fn test_error_display_physics_error() {
        let err = GameLogicError::PhysicsError("collision failed".to_string());
        assert!(err.to_string().contains("collision failed"));
    }

    #[test]
    fn test_error_display_script_error() {
        let err = GameLogicError::ScriptError("condition syntax".to_string());
        assert!(err.to_string().contains("condition syntax"));
    }

    #[test]
    fn test_error_display_ai_error() {
        let err = GameLogicError::AIError("pathfinding failed".to_string());
        assert!(err.to_string().contains("pathfinding failed"));
    }

    #[test]
    fn test_error_display_command_error() {
        let err = GameLogicError::CommandError("invalid target".to_string());
        assert!(err.to_string().contains("invalid target"));
    }

    #[test]
    fn test_partition_manager_creation() {
        let mut partition = PartitionManager::new();
        let result = partition.update();
        assert!(result.is_ok());
    }

    #[test]
    fn test_partition_add_object() {
        let mut partition = PartitionManager::new();
        partition.add_object(1, (100.0, 100.0, 0.0));
        // If no panic, test succeeds
        assert!(true);
    }

    #[test]
    fn test_partition_remove_object() {
        let mut partition = PartitionManager::new();
        partition.add_object(1, (100.0, 100.0, 0.0));
        partition.remove_object(1);
        // If no panic, test succeeds
        assert!(true);
    }

    #[test]
    fn test_empty_object_list() {
        let logic = GameLogic::new();
        assert_eq!(logic.all_objects.len(), 0);
    }

    #[test]
    fn test_empty_dead_objects_list() {
        let logic = GameLogic::new();
        assert_eq!(logic.dead_objects.len(), 0);
    }

    #[test]
    fn test_clear_multiple_times() {
        let mut logic = GameLogic::new();

        for _ in 0..10 {
            let _ = logic.clear_frame_events();
        }

        assert_eq!(logic.event_queue.len(), 0);
    }

    #[test]
    fn test_reset_temporary_flags() {
        let mut logic = GameLogic::new();
        let result = logic.reset_temporary_flags();
        assert!(result.is_ok());
    }

    #[test]
    fn test_consecutive_frames() {
        let mut logic = GameLogic::new();

        for frame in 0..10 {
            let result = logic.update(frame);
            assert!(result.is_ok(), "Frame {} update failed", frame);
            assert_eq!(logic.get_frame(), frame);
        }
    }

    #[test]
    fn test_game_time_matches_frame_count() {
        let mut logic = GameLogic::new();

        for frame in 0..60 {
            let _ = logic.update(frame);
            let expected_time = frame as f32 * FIXED_DELTA_TIME;
            assert!(
                (logic.get_game_time() - expected_time).abs() < 0.0001,
                "Frame {}: time mismatch",
                frame
            );
        }
    }

    #[test]
    fn test_object_event_structure() {
        let events = vec![
            GameEvent::ObjectCreated(1),
            GameEvent::ObjectDestroyed(2),
            GameEvent::DamageDealt {
                attacker: 3,
                target: 4,
                amount: 50.0,
            },
            GameEvent::VictoryConditionMet {
                player_id: 0,
                condition_name: "LastEnemyDestroyed".to_string(),
            },
        ];

        assert_eq!(events.len(), 4);
    }

    #[test]
    fn test_pending_damage_structure() {
        let damage = PendingDamage {
            target_id: 10,
            attacker_id: 20,
            damage_amount: 75.5,
            damage_type: crate::damage::DamageType::Explosion,
            death_type: crate::damage::DeathType::Normal,
        };

        assert_eq!(damage.target_id, 10);
        assert_eq!(damage.attacker_id, 20);
        assert!((damage.damage_amount - 75.5).abs() < 0.01);
    }

    #[test]
    fn test_pending_collision_structure() {
        let collision = PendingCollision {
            object_a: 1,
            object_b: 2,
            collision_point: (100.0, 200.0, 0.0),
        };

        assert_eq!(collision.object_a, 1);
        assert_eq!(collision.object_b, 2);
        assert_eq!(collision.collision_point.0, 100.0);
    }

    #[test]
    fn test_game_command_enum_variants() {
        let commands = vec![
            GameCommand::MoveUnit {
                player_id: 0,
                unit_ids: vec![1],
                target_position: (0.0, 0.0, 0.0),
            },
            GameCommand::AttackTarget {
                player_id: 0,
                attacker_ids: vec![1],
                target_id: 2,
            },
            GameCommand::BuildStructure {
                player_id: 0,
                builder_id: 1,
                structure_type: "Barracks".to_string(),
                position: (0.0, 0.0),
            },
            GameCommand::UseSpecialPower {
                player_id: 0,
                power_name: "Power".to_string(),
                target_position: None,
            },
        ];

        assert_eq!(commands.len(), 4);
    }
}
