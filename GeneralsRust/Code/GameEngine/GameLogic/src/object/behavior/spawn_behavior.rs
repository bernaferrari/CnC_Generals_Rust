//! SpawnBehavior - Rust conversion of C++ SpawnBehavior
//!
//! Behavior will create and monitor a group of spawned units and replace as needed
//! Original Authors: Graham Smallwood, January 2002; Colin Day, October 2002
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, Byte, Coord3D, DisabledType, INVALID_ID, Int, KindOf,
    LOGICFRAMES_PER_SECOND, ModuleData, ObjectID, PlayerMaskType, Real, TheObjectFactory,
    UnsignedInt, VeterancyLevel,
};
use crate::object::behavior::behavior_module::{
    BehaviorModuleData, xfer_behavior_module_base_versions,
};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Wave 346: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

// Forward declarations
use crate::MAKE_OBJECT_STATUS_MASK;
use crate::attack::{
    ATTACKRESULT_INVALID_SHOT, ATTACKRESULT_NOT_POSSIBLE, ATTACKRESULT_POSSIBLE,
    ATTACKRESULT_POSSIBLE_AFTER_MOVING, AbleToAttackType, CanAttackResult,
};
use crate::common::CommandSourceType;
use crate::common::DamageTypeFlags;
use crate::common::{
    FROM_CENTER_2D, OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_MASKED, OBJECT_STATUS_RECONSTRUCTING,
    OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION, TheGameLogic, TheInGameUI,
    TheMessageStream, ThePartitionManager,
};
use crate::damage::{BodyDamageType, DamageInfo, DamageType};
use crate::experience::ExperienceTracker;
use crate::messages::{GameMessage, MSG_CREATE_SELECTED_GROUP};
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BehaviorModule, BehaviorModuleInterface,
    BodyModuleInterface, DOOR_NONE_AVAILABLE, DamageModuleInterface, DieModuleInterface,
    ExitDoorType, ExitInterface, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE,
    MODULEINTERFACE_UPDATE, ModuleInterface, SlavedUpdateInterface,
    SpawnBehaviorInterface as ModuleSpawnBehaviorInterface, UPDATE_SLEEP, UPDATE_SLEEP_FOREVER,
    UPDATE_SLEEP_NONE, UpdateModule, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::drawable::DrawableExt;
use crate::object::{Object, ObjectStatusTypes};
use crate::player::{CMD_FROM_AI, Player};
use crate::team::Team;
use crate::template::ObjectTemplate;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, SpawnControlInterface,
};
pub type DieMuxData = crate::object::die::DieMuxData;
use crate::object::die::{
    parse_death_type_flags_tokens, parse_object_status_mask_tokens,
    parse_veterancy_level_flags_tokens,
};
use game_engine::common::ini::{FieldParse, INI, INIError};
use std::str::FromStr;

// Constants
const SPAWN_UPDATE_RATE: Int = (LOGICFRAMES_PER_SECOND / 2) as Int; // Low priority update rate
const SPAWN_DELAY_MIN_FRAMES: Int = 16; // Minimum delay between successive exits
const NONE_SPAWNED_YET: UnsignedInt = 0xFFFFFFFF;
const BIG_DISTANCE: Real = 99999999.9;

/// C++ SpawnBehavior::update first-pass burst queue (SpawnBehavior.cpp:187-208).
/// Computes `birthFrame += listIndex * SPAWN_DELAY_MIN_FRAMES` for runtime-produced
/// InitialBurst slots so the hive exits stagger instead of dumping on one frame.
fn initial_burst_replacement_times(
    now: UnsignedInt,
    spawn_number: Int,
    initial_burst: Int,
    runtime_produced: bool,
) -> Vec<Int> {
    let mut times = Vec::with_capacity(spawn_number.max(0) as usize);
    let mut burst_init_count = initial_burst;
    for list_index in 0..spawn_number {
        if initial_burst > 0 {
            let mut birth_frame = now;
            if runtime_produced && burst_init_count > 0 {
                burst_init_count -= 1;
                birth_frame = birth_frame
                    .saturating_add((list_index * SPAWN_DELAY_MIN_FRAMES) as UnsignedInt);
            }
            times.push(birth_frame as Int);
        } else {
            times.push(list_index);
        }
    }
    times
}

/// C++ SpawnBehavior::createSpawn (SpawnBehavior.cpp:600):
/// `if (md->m_canReclaimOrphans && md->m_isOneShotData == FALSE)`.
fn should_attempt_orphan_reclaim(can_reclaim_orphans: bool, is_one_shot: bool) -> bool {
    can_reclaim_orphans && !is_one_shot
}

/// C++ reclaimOrphanSpawn comment: skip authored list redundancy (consecutive dups).
/// Written C++ `if (prevName.compare(*tempName)) continue` is inverted strcmp
/// (skips *different* names). Live path follows the comment: skip same-as-prev.
fn orphan_template_is_redundant(prev_name: &str, template_name: &str) -> bool {
    prev_name == template_name
}

/// C++ computeAggregateStates (SpawnBehavior.cpp:982-985):
/// `setInitialHealth(100.0f * actualHealth)` — `Int` cast truncates toward zero,
/// no 0..100 clamp. Clamping hid over-population health and changed hive HP.
fn aggregate_initial_health_percent(
    acr_health: Real,
    avg_health_max_sum: Real,
    spawn_count: Int,
    spawn_count_max: Int,
) -> i32 {
    if spawn_count <= 0 {
        return 0;
    }
    let avg_health_max = avg_health_max_sum / spawn_count as Real;
    let perfect_total_health = avg_health_max * spawn_count_max as Real;
    if perfect_total_health == 0.0 {
        return 0;
    }
    let actual_health = acr_health / perfect_total_health;
    (100.0 * actual_health) as i32
}

/// C++ SpawnBehavior::maySpawnSelfTaskAI (SpawnBehavior.cpp:252-278).
/// When the parent has AI, last command must be CMD_FROM_AI.
/// When the parent has no AI (hive/building), still evaluate the ratio so
/// spawnlings keep self-task AI instead of standing idle (hq-ifx4u).
fn may_spawn_self_task_ai_decision(
    spawn_count: UnsignedInt,
    max_self_taskers_ratio: Real,
    self_tasking_spawn_count: UnsignedInt,
    parent_last_command_source: Option<CommandSourceType>,
) -> bool {
    if spawn_count == 0 || max_self_taskers_ratio == 0.0 {
        return false;
    }
    if let Some(src) = parent_last_command_source {
        if src != CMD_FROM_AI {
            return false;
        }
    }
    let cur = self_tasking_spawn_count as Real / spawn_count as Real;
    cur < max_self_taskers_ratio
}

/// C++ SpawnBehavior::shouldTryToSpawn (SpawnBehavior.cpp:809-813):
/// reconstructing + OneShot latches the module off forever.
fn hole_rebuild_one_shot_should_latch(reconstructing: bool, is_one_shot: bool) -> bool {
    reconstructing && is_one_shot
}

/// Module data for SpawnBehavior
#[derive(Debug, Clone)]
pub struct SpawnBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub spawn_number_data: Int,
    pub spawn_start_number_data: Int,
    pub spawn_replace_delay_data: Int,
    pub initial_burst: Int,
    pub is_one_shot_data: Bool,
    pub can_reclaim_orphans: Bool,
    pub aggregate_health: Bool,
    pub exit_by_budding: Bool,
    pub spawned_require_spawner: Bool,
    pub slaves_have_free_will: Bool,
    pub damage_types_to_propagate_to_slaves: DamageTypeFlags,
    pub spawn_template_name_data: Vec<AsciiString>,
    pub die_mux_data: DieMuxData,
}

impl SpawnBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            spawn_number_data: 0,
            spawn_start_number_data: 0,
            spawn_replace_delay_data: 0,
            initial_burst: 0,
            is_one_shot_data: false,
            can_reclaim_orphans: false,
            aggregate_health: false,
            exit_by_budding: false,
            spawned_require_spawner: false,
            slaves_have_free_will: false,
            damage_types_to_propagate_to_slaves: DamageTypeFlags::empty(),
            spawn_template_name_data: Vec::new(),
            die_mux_data: DieMuxData::default(),
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPAWN_BEHAVIOR_FIELDS)
    }
}

crate::impl_behavior_module_data_via_base!(SpawnBehaviorModuleData, base);

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_spawn_number(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.spawn_number_data = INI::parse_int(token)?;
    Ok(())
}

fn parse_spawn_replace_delay(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.spawn_replace_delay_data = parse_duration_frames(tokens)? as Int;
    Ok(())
}

fn parse_one_shot(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.is_one_shot_data = INI::parse_bool(token)?;
    Ok(())
}

fn parse_can_reclaim_orphans(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.can_reclaim_orphans = INI::parse_bool(token)?;
    Ok(())
}

fn parse_aggregate_health(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.aggregate_health = INI::parse_bool(token)?;
    Ok(())
}

fn parse_exit_by_budding(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.exit_by_budding = INI::parse_bool(token)?;
    Ok(())
}

fn parse_spawn_template_name(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().copied().filter(|t| *t != "=") {
        for name in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            data.spawn_template_name_data.push(AsciiString::from(name));
        }
    }
    Ok(())
}

fn parse_spawned_require_spawner(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.spawned_require_spawner = INI::parse_bool(token)?;
    Ok(())
}

fn parse_damage_types_to_slaves(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut flags = DamageTypeFlags::empty();
    for token in tokens {
        for entry in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if entry.eq_ignore_ascii_case("ALL") {
                flags = DamageTypeFlags::all_flags();
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = DamageTypeFlags::empty();
                continue;
            }

            let (remove, name) = if let Some(stripped) = entry.strip_prefix('-') {
                (true, stripped.trim())
            } else if let Some(stripped) = entry.strip_prefix('+') {
                (false, stripped.trim())
            } else {
                (false, entry)
            };

            if let Ok(damage_type) = DamageType::from_str(name) {
                let flag = DamageTypeFlags::from_bits_truncate(1 << damage_type as u64);
                if remove {
                    flags.remove(flag);
                } else {
                    flags.insert(flag);
                }
            }
        }
    }

    data.damage_types_to_propagate_to_slaves = flags;
    Ok(())
}

fn parse_initial_burst(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.initial_burst = INI::parse_int(token)?;
    Ok(())
}

fn parse_slaves_have_free_will(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.slaves_have_free_will = INI::parse_bool(token)?;
    Ok(())
}

fn parse_death_types(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.death_types = parse_death_type_flags_tokens(tokens)?;
    Ok(())
}

fn parse_veterancy_levels(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.veterancy_levels = parse_veterancy_level_flags_tokens(tokens)?;
    Ok(())
}

fn parse_exempt_status(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.exempt_status = parse_object_status_mask_tokens(tokens)?;
    Ok(())
}

fn parse_required_status(
    _ini: &mut INI,
    data: &mut SpawnBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.required_status = parse_object_status_mask_tokens(tokens)?;
    Ok(())
}

const SPAWN_BEHAVIOR_FIELDS: &[FieldParse<SpawnBehaviorModuleData>] = &[
    FieldParse {
        token: "SpawnNumber",
        parse: parse_spawn_number,
    },
    FieldParse {
        token: "SpawnReplaceDelay",
        parse: parse_spawn_replace_delay,
    },
    FieldParse {
        token: "OneShot",
        parse: parse_one_shot,
    },
    FieldParse {
        token: "CanReclaimOrphans",
        parse: parse_can_reclaim_orphans,
    },
    FieldParse {
        token: "AggregateHealth",
        parse: parse_aggregate_health,
    },
    FieldParse {
        token: "ExitByBudding",
        parse: parse_exit_by_budding,
    },
    FieldParse {
        token: "SpawnTemplateName",
        parse: parse_spawn_template_name,
    },
    FieldParse {
        token: "SpawnedRequireSpawner",
        parse: parse_spawned_require_spawner,
    },
    FieldParse {
        token: "PropagateDamageTypesToSlavesWhenExisting",
        parse: parse_damage_types_to_slaves,
    },
    FieldParse {
        token: "InitialBurst",
        parse: parse_initial_burst,
    },
    FieldParse {
        token: "SlavesHaveFreeWill",
        parse: parse_slaves_have_free_will,
    },
    FieldParse {
        token: "DeathTypes",
        parse: parse_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_required_status,
    },
];

/// Interface for spawn behavior
pub trait SpawnBehaviorInterface: Send + Sync {
    fn may_spawn_self_task_ai(&self, max_self_taskers_ratio: Real) -> bool;
    fn on_spawn_death(
        &mut self,
        dead_spawn: ObjectID,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn get_closest_slave(&self, pos: &Coord3D) -> Option<Arc<RwLock<Object>>>;
    fn order_slaves_to_attack_target(
        &mut self,
        target: &Object,
        max_shots_to_fire: Int,
        cmd_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn order_slaves_to_attack_position(
        &mut self,
        pos: &Coord3D,
        max_shots_to_fire: Int,
        cmd_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn get_can_any_slaves_attack_specific_target(
        &self,
        attack_type: AbleToAttackType,
        target: &Object,
        cmd_source: CommandSourceType,
    ) -> CanAttackResult;
    fn get_can_any_slaves_use_weapon_against_target(
        &self,
        attack_type: AbleToAttackType,
        victim: &Object,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
    ) -> CanAttackResult;
    fn can_any_slaves_attack(&self) -> bool;
    fn order_slaves_to_go_idle(
        &mut self,
        cmd_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn order_slaves_disabled_until(
        &mut self,
        disabled_type: DisabledType,
        frame: UnsignedInt,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn order_slaves_to_clear_disabled(
        &mut self,
        disabled_type: DisabledType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn give_slaves_stealth_upgrade(
        &mut self,
        grant_stealth: Bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn are_all_slaves_stealthed(&self) -> bool;
    fn reveal_slaves(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn do_slaves_have_freedom(&self) -> bool;
}

/// Main SpawnBehavior implementation
#[derive(Debug)]
#[allow(dead_code)]
pub struct SpawnBehavior {
    // Base module data
    object_id: ObjectID,
    module_data: Arc<SpawnBehaviorModuleData>,

    // Spawn management
    spawn_template: Option<Arc<ObjectTemplate>>,
    template_name_iterator: usize,
    one_shot_countdown: Int,
    frames_to_wait: Int,
    first_batch_count: Int,
    initial_burst_countdown: UnsignedInt,
    initial_burst_times_inited: Bool,

    // Spawn tracking
    replacement_times: VecDeque<Int>,
    spawn_ids: Vec<ObjectID>,
    active: Bool,

    // Aggregate health tracking
    aggregate_health: Bool,
    spawn_count: UnsignedInt,
    self_tasking_spawn_count: UnsignedInt,
}

impl SpawnBehavior {
    pub fn new(
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = {
            let data_ref = module_data
                .as_any()
                .downcast_ref::<SpawnBehaviorModuleData>()
                .ok_or("Invalid module data type")?;
            data_ref.clone()
        };

        Self::new_with_data(object_id, Arc::new(data))
    }

    pub fn new_with_data(
        object_id: ObjectID,
        data: Arc<SpawnBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if data.spawn_template_name_data.is_empty() {
            return Err("SpawnBehavior requires at least one spawn template".into());
        }

        // Find first template
        let first_template_name = &data.spawn_template_name_data[0];
        let spawn_template = TheObjectFactory::find_template(first_template_name).ok_or(
            format!("Could not find spawn template: {}", first_template_name),
        )?;

        let one_shot_countdown = if data.is_one_shot_data {
            data.spawn_number_data
        } else {
            -1
        };

        Ok(Self {
            object_id,
            module_data: data.clone(),
            spawn_template: Some(spawn_template),
            template_name_iterator: 0,
            one_shot_countdown,
            frames_to_wait: 0,
            first_batch_count: 0,
            initial_burst_countdown: data.initial_burst as UnsignedInt,
            initial_burst_times_inited: false,
            replacement_times: VecDeque::new(),
            spawn_ids: Vec::new(),
            active: true,
            aggregate_health: data.aggregate_health,
            spawn_count: NONE_SPAWNED_YET,
            self_tasking_spawn_count: 0,
        })
    }

    pub fn set_object(&mut self, object_id: ObjectID) {
        self.object_id = object_id;
    }

    pub fn set_object_id(&mut self, object_id: ObjectID) {
        self.object_id = object_id;
    }

    fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    fn with_object<R>(
        &self,
        f: impl FnOnce(&Object) -> R,
    ) -> Result<R, Box<dyn std::error::Error + Send + Sync>> {
        let id = self.get_object_id();
        if id == INVALID_ID {
            return Err("Object not set".into());
        }
        crate::object::registry::OBJECT_REGISTRY
            .with_object(id, f)
            .ok_or_else(|| "Object not found".into())
    }

    fn with_object_mut<R>(
        &self,
        f: impl FnOnce(&mut Object) -> R,
    ) -> Result<R, Box<dyn std::error::Error + Send + Sync>> {
        let id = self.get_object_id();
        if id == INVALID_ID {
            return Err("Object not set".into());
        }
        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(id, f)
            .ok_or_else(|| "Object not found".into())
    }

    fn get_object(&self) -> Result<Arc<RwLock<Object>>, Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return Err("dual-world object registry unavailable".into());
        }

        let id = self.get_object_id();
        if id == INVALID_ID {
            return Err("Object not set".into());
        }
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
            .ok_or_else(|| "Object not found".into())
    }

    fn notify_slaved_update(
        &self,
        spawned_id: ObjectID,
        master_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let spawned = crate::helpers::TheGameLogic::find_object_by_id(spawned_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(spawned_id))
            .ok_or("spawned object unavailable")?;
        let spawn_guard = spawned.read().map_err(|_| "Failed to read spawn")?;
        if let Some(result) =
            spawn_guard.with_slaved_update_interface(|slaved| slaved.on_enslave(master_id))
        {
            return result;
        }

        for behavior in spawn_guard.get_behavior_modules() {
            let mut behavior_guard = behavior
                .lock()
                .map_err(|_| "Failed to lock behavior module")?;
            if let Some(slaved) = behavior_guard.get_slaved_update_interface() {
                slaved.on_enslave(master_id)?;
                break;
            }
        }
        Ok(())
    }

    fn should_try_to_spawn(&mut self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let data = Arc::clone(&self.module_data);

        if !self.active {
            return Ok(false);
        }

        let (hole_rebuild_one_shot, blocked, neutral) = self.with_object(|obj_guard| {
            let hole_rebuild_one_shot = hole_rebuild_one_shot_should_latch(
                obj_guard
                    .get_status_bits()
                    .test(OBJECT_STATUS_RECONSTRUCTING),
                data.is_one_shot_data,
            );
            let blocked = obj_guard.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION)
                || obj_guard.test_status(OBJECT_STATUS_SOLD);
            let neutral = obj_guard.is_neutral_controlled();
            (hole_rebuild_one_shot, blocked, neutral)
        })?;

        // C++ SpawnBehavior::shouldTryToSpawn (SpawnBehavior.cpp:809-813):
        // Hole rebuild + OneShot latches off forever via stopSpawning().
        if hole_rebuild_one_shot {
            self.stop_spawning();
            return Ok(false);
        }
        if blocked || neutral {
            return Ok(false);
        }
        Ok(true)
    }

    fn create_spawn(&mut self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let data = Arc::clone(&self.module_data);

        // Get exit interface
        let exit_interface = self
            .with_object(|obj_guard| obj_guard.get_object_exit_interface())
            .map_err(|_| "Failed to read object")?
            .ok_or("Object must have ExitInterface to use SpawnBehavior")?;

        let exit_door = {
            let mut exit_guard = exit_interface
                .lock()
                .map_err(|_| "Failed to lock exit interface")?;
            exit_guard.reserve_door_for_exit(None, None)
        };

        if exit_door == DOOR_NONE_AVAILABLE {
            return Ok(false);
        }

        let mut new_spawn = None;
        let mut reclaimed_orphan = false;

        // Try to reclaim orphaned objects if possible
        if should_attempt_orphan_reclaim(data.can_reclaim_orphans, data.is_one_shot_data) {
            new_spawn = self.reclaim_orphan_spawn()?;
            if new_spawn.is_some() {
                reclaimed_orphan = true;
            }
        }

        // Create new spawn if no orphan was reclaimed
        if new_spawn.is_none() {
            let template = self
                .spawn_template
                .as_ref()
                .ok_or("No spawn template available")?;

            let parent_team = self
                .with_object(|obj_guard| obj_guard.get_team())
                .map_err(|_| "Failed to read object")?;

            let spawn_obj = TheObjectFactory::new_object(Arc::clone(template), parent_team)?;

            // Count this unit towards our score
            let controlling_player = self
                .with_object(|obj_guard| obj_guard.get_controlling_player())
                .map_err(|_| "Failed to read object")?;

            if let Some(player) = controlling_player {
                let mut player_guard = player.write().map_err(|_| "Failed to write player")?;
                {
                    let producer_id = self.get_object_id();
                    let unit_id = spawn_obj
                        .read()
                        .ok()
                        .map(|g| g.get_id())
                        .unwrap_or(crate::common::INVALID_ID);
                    player_guard.on_unit_created_id(producer_id, unit_id);
                }
                drop(player_guard);
            }

            // Advance template iterator
            self.template_name_iterator += 1;
            if self.template_name_iterator >= data.spawn_template_name_data.len() {
                self.template_name_iterator = 0;
            }

            // Update spawn template for next time
            let next_template_name = &data.spawn_template_name_data[self.template_name_iterator];
            self.spawn_template = TheObjectFactory::find_template(next_template_name);

            new_spawn = Some(spawn_obj);
        }

        let new_spawn = new_spawn.unwrap();

        // Set producer relationship
        {
            let mut spawn_guard = new_spawn.write().map_err(|_| "Failed to write spawn")?;
            let _ = self.with_object(|parent_obj| {
                spawn_guard.set_producer(Some(parent_obj));
            })?;
            drop(spawn_guard);
        }

        // If spawned object has a SlavedUpdate, tell them who their master is
        {
            let spawn_id = new_spawn
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(INVALID_ID);
            let master_id = self.get_object_id();
            self.notify_slaved_update(spawn_id, master_id)?;
        }

        // Add to spawn tracking
        let spawn_id = {
            let spawn_guard = new_spawn.read().map_err(|_| "Failed to read spawn")?;
            spawn_guard.get_id()
        };
        self.spawn_ids.push(spawn_id);

        // Handle exit behavior
        if !reclaimed_orphan {
            let mut exit_guard = exit_interface
                .lock()
                .map_err(|_| "Failed to lock exit interface")?;

            if data.exit_by_budding {
                let mut barracks_exit_success = false;

                if self.initial_burst_countdown > 0 {
                    // Try to exit from parent's producer (barracks)
                    let producer_id = self
                        .with_object(|obj_guard| obj_guard.get_producer_id())
                        .map_err(|_| "Failed to read object")?;

                    if producer_id != INVALID_ID {
                        if let Some(barracks) = TheGameLogic::find_object_by_id(producer_id) {
                            let barracks_guard =
                                barracks.read().map_err(|_| "Failed to read barracks")?;
                            let is_structure = barracks_guard.is_kind_of(KindOf::Structure);
                            drop(barracks_guard);

                            if is_structure {
                                if let Some(barracks_exit) =
                                    barracks.read().unwrap().get_object_exit_interface()
                                {
                                    let mut barracks_exit_guard = barracks_exit
                                        .lock()
                                        .map_err(|_| "Failed to lock barracks exit")?;
                                    let barracks_door =
                                        barracks_exit_guard.reserve_door_for_exit(None, None);
                                    if barracks_door != DOOR_NONE_AVAILABLE {
                                        barracks_exit_guard.exit_object_via_door(
                                            new_spawn.read().map(|g| g.get_id()).unwrap_or(0),
                                            barracks_door,
                                        )?;
                                        drop(barracks_exit_guard);

                                        // Set producer back to parent
                                        let mut spawn_guard = new_spawn
                                            .write()
                                            .map_err(|_| "Failed to write spawn")?;
                                        let _ = self.with_object(|parent_obj| {
                                            spawn_guard.set_producer(Some(parent_obj));
                                        })?;
                                        drop(spawn_guard);

                                        self.initial_burst_countdown -= 1;
                                        barracks_exit_success = true;
                                    }
                                }
                            }
                        }
                    }
                }

                if !barracks_exit_success {
                    // Find closest spawn to bud from
                    let mut bud_host = None;
                    let mut closest_distance = BIG_DISTANCE;

                    for &spawn_id in &self.spawn_ids {
                        if spawn_id == new_spawn.read().unwrap().get_id() {
                            continue; // Skip the new spawn itself
                        }

                        if let Some(cur_spawn) = TheGameLogic::find_object_by_id(spawn_id) {
                            let distance = {
                                let cur_spawn_guard =
                                    cur_spawn.read().map_err(|_| "Failed to read spawn")?;
                                self.with_object(|parent_guard| {
                                    ThePartitionManager::get_distance_squared(
                                        &cur_spawn_guard,
                                        parent_guard,
                                        FROM_CENTER_2D,
                                    )
                                })
                                .map_err(|_| "Failed to read parent")?
                            };
                            if distance < closest_distance {
                                closest_distance = distance;
                                bud_host = Some(cur_spawn);
                            }
                        }
                    }

                    exit_guard.exit_object_by_budding(
                        new_spawn.read().map(|g| g.get_id()).unwrap_or(0),
                        bud_host
                            .as_ref()
                            .and_then(|h| h.read().ok().map(|g| g.get_id())),
                    )?;
                }
            } else {
                exit_guard.exit_object_via_door(
                    new_spawn.read().map(|g| g.get_id()).unwrap_or(0),
                    exit_door,
                )?;
            }
            drop(exit_guard);
        } else {
            // Unreserve the door since we used a reclaimed orphan
            let mut exit_guard = exit_interface
                .lock()
                .map_err(|_| "Failed to lock exit interface")?;
            exit_guard.unreserve_door_for_exit(exit_door);
            drop(exit_guard);
        }

        // Update counters
        if data.is_one_shot_data {
            self.one_shot_countdown -= 1;
        }

        if self.spawn_count == NONE_SPAWNED_YET {
            self.spawn_count = 1;
        } else {
            self.spawn_count += 1;
        }

        Ok(true)
    }

    fn reclaim_orphan_spawn(
        &self,
    ) -> Result<Option<Arc<RwLock<Object>>>, Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        let data = self.module_data.clone();

        let (player, object_pos) = self.with_object(|obj_guard| {
            let player = obj_guard.get_controlling_player();
            let pos = obj_guard.get_position().clone();
            (player, pos)
        })?;
        let player = player.ok_or("No controlling player")?;

        // Find closest orphan matching our templates
        let mut closest_orphan = None;
        let mut closest_distance = BIG_DISTANCE;

        // Check each template type
        let mut prev_template_name = String::new();
        for template_name in &data.spawn_template_name_data {
            if orphan_template_is_redundant(&prev_template_name, template_name.as_str()) {
                continue;
            }
            prev_template_name = template_name.as_str().to_string();

            if let Some(template) = TheObjectFactory::find_template(template_name) {
                let player_object_ids = {
                    let player_guard = player.read().map_err(|_| "Failed to read player")?;
                    player_guard.get_object_ids()
                };

                for obj_id in player_object_ids {
                    let Some(player_obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let obj_guard = player_obj
                        .read()
                        .map_err(|_| "Failed to read player object")?;

                    if obj_guard.get_template_name() != template.get_name().as_str() {
                        continue;
                    }

                    if obj_guard.get_producer_id() != INVALID_ID {
                        continue;
                    }

                    let distance = ThePartitionManager::get_distance_squared_to_pos(
                        &obj_guard,
                        &object_pos,
                        FROM_CENTER_2D,
                    );

                    if distance < closest_distance {
                        closest_distance = distance;
                        closest_orphan = Some(player_obj.clone());
                    }
                }
            }
        }

        Ok(closest_orphan)
    }

    fn compute_aggregate_states(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !self.aggregate_health {
            return Ok(()); // Not using aggregate health
        }

        if self.get_object_id() == INVALID_ID {
            return Err("SpawnBehavior missing owning object".into());
        }
        let data = self.module_data.clone();

        let mut spawn_count = 0;
        let spawn_count_max = data.spawn_number_data;
        let mut avg_spawn_pos = Coord3D::new(0.0, 0.0, 0.0);
        let mut acr_health = 0.0;
        let mut avg_health_max = 0.0;

        let mut somebody_is_selected = false;
        let mut somebody_is_not_selected = false;
        self.self_tasking_spawn_count = 0;

        // Process each spawn
        for &spawn_id in &self.spawn_ids {
            if let Some(current_spawn) = TheGameLogic::find_object_by_id(spawn_id) {
                let spawn_guard = current_spawn.read().map_err(|_| "Failed to read spawn")?;

                // Count self-tasking spawns
                for behavior in spawn_guard.get_behavior_modules() {
                    let mut behavior_guard = behavior
                        .lock()
                        .map_err(|_| "Failed to lock behavior module")?;
                    if let Some(slaved) = behavior_guard.get_slaved_update_interface() {
                        if slaved.is_self_tasking() {
                            self.self_tasking_spawn_count += 1;
                        }
                        break;
                    }
                }

                // Handle veterancy synchronization
                let spawn_vet_level = spawn_guard.get_veterancy_level();
                let obj_vet_level = self
                    .with_object(|obj_guard| obj_guard.get_veterancy_level())
                    .map_err(|_| "Failed to read object")?;

                if spawn_vet_level > obj_vet_level {
                    let _ = self.with_object_mut(|obj_guard| {
                        if let Some(exp_tracker) = obj_guard.get_experience_tracker() {
                            if let Ok(mut tracker_guard) = exp_tracker.lock() {
                                tracker_guard.set_veterancy_level_with_requirements(
                                    spawn_vet_level,
                                    &ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED,
                                );
                            }
                        }
                    });
                } else if spawn_vet_level < obj_vet_level {
                    if let Some(spawn_exp_tracker) = spawn_guard.get_experience_tracker() {
                        let mut spawn_tracker_guard = spawn_exp_tracker
                            .lock()
                            .map_err(|_| "Failed to lock spawn experience tracker")?;
                        spawn_tracker_guard.set_veterancy_level_with_requirements(
                            obj_vet_level,
                            &ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED,
                        );
                        drop(spawn_tracker_guard);
                    }
                }

                // Aggregate position and health
                avg_spawn_pos += *spawn_guard.get_position();

                if let Some(body) = spawn_guard.get_body_module() {
                    let body_guard = body.lock().map_err(|_| "Failed to lock spawn body")?;
                    acr_health += body_guard.get_health();
                    avg_health_max += body_guard.get_max_health();
                    drop(body_guard);
                }

                // Check selection status
                if let Some(drawable) = spawn_guard.get_drawable() {
                    let drawable_guard = drawable
                        .read()
                        .map_err(|_| "Failed to read spawn drawable")?;
                    if drawable_guard.is_selected() {
                        somebody_is_selected = true;
                    } else {
                        somebody_is_not_selected = true;
                    }
                }

                spawn_count += 1;
                drop(spawn_guard);
            }
        }

        if somebody_is_selected {
            let obj_selected = self
                .with_object(|obj_guard| {
                    obj_guard
                        .get_drawable()
                        .and_then(|drawable| drawable.read().ok().map(|d| d.is_selected()))
                })
                .map_err(|_| "Failed to read object")?
                .unwrap_or(false);

            if !obj_selected || somebody_is_not_selected {
                // Create selection group message
                let mut team_msg = TheMessageStream::append_message(MSG_CREATE_SELECTED_GROUP);
                team_msg.append_boolean_argument(false); // Not creating new team

                // Select all unselected spawns
                if somebody_is_not_selected {
                    for &spawn_id in &self.spawn_ids {
                        if let Some(current_spawn) = TheGameLogic::find_object_by_id(spawn_id) {
                            let spawn_guard =
                                current_spawn.read().map_err(|_| "Failed to read spawn")?;
                            if let Some(drawable) = spawn_guard.get_drawable() {
                                let drawable_guard = drawable
                                    .read()
                                    .map_err(|_| "Failed to read spawn drawable")?;
                                if !drawable_guard.is_selected() {
                                    TheInGameUI::select_drawable(&drawable);
                                    TheInGameUI::set_displayed_max_warning(false);
                                    team_msg.append_boolean_argument(false);
                                    team_msg.append_object_id_argument(spawn_id);
                                }
                            }
                        }
                    }
                }

                // Select parent object if not selected
                if !obj_selected {
                    let _ = self.with_object(|obj_guard| {
                        if let Some(drawable) = obj_guard.get_drawable() {
                            TheInGameUI::select_drawable(&drawable);
                            TheInGameUI::set_displayed_max_warning(false);
                            team_msg.append_boolean_argument(false);
                            team_msg.append_object_id_argument(obj_guard.get_id());
                        }
                    });
                }
            }
        }

        // Update health box position (average of spawn positions)
        if spawn_count > 0 {
            avg_spawn_pos /= spawn_count as Real;
            let obj_pos = self
                .with_object(|obj_guard| *obj_guard.get_position())
                .map_err(|_| "Failed to read object")?;
            avg_spawn_pos -= obj_pos;

            let _ = self.with_object_mut(|obj_guard| {
                obj_guard.set_health_box_offset(avg_spawn_pos);
            });
        }

        // Update aggregate health
        if spawn_count > 0 {
            let percent = aggregate_initial_health_percent(
                acr_health,
                avg_health_max,
                spawn_count,
                spawn_count_max,
            );

            self.with_object_mut(|obj_guard| {
                if let Some(body) = obj_guard.get_body_module() {
                    let mut body_guard = body.lock().map_err(|_| "Failed to lock object body")?;
                    body_guard
                        .set_initial_health(percent)
                        .map_err(|e| format!("Failed to set spawn initial health: {e}"))?;
                }
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            })??;
        } else {
            self.with_object_mut(|obj_guard| {
                if let Some(body) = obj_guard.get_body_module() {
                    let mut body_guard = body.lock().map_err(|_| "Failed to lock object body")?;
                    body_guard
                        .set_initial_health(0)
                        .map_err(|e| format!("Failed to set spawn initial zero health: {e}"))?;
                }
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            })??;
        }

        // Make sure no enemies are shooting at the nexus, since it doesn't 'exist'
        let _ = self.with_object_mut(|obj_guard| {
            obj_guard.set_status(MAKE_OBJECT_STATUS_MASK!(OBJECT_STATUS_MASKED), true);
        });

        Ok(())
    }

    pub fn stop_spawning(&mut self) {
        self.active = false;
    }

    pub fn start_spawning(&mut self) {
        self.active = true;
    }
}

impl UpdateModuleInterface for SpawnBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        // Handle aggregate health every frame
        if self.aggregate_health {
            self.compute_aggregate_states()?;
        }

        let data = self.module_data.clone();

        // Initialize burst times on first update
        if !self.initial_burst_times_inited {
            self.initial_burst_times_inited = true;

            let runtime_produced =
                self.with_object(|obj_guard| obj_guard.get_producer_id() != INVALID_ID)?;

            let now = TheGameLogic::get_frame();
            for birth in initial_burst_replacement_times(
                now,
                data.spawn_number_data,
                data.initial_burst,
                runtime_produced,
            ) {
                self.replacement_times.push_back(birth);
            }
        }

        // Sparse update - only process every SPAWN_UPDATE_RATE frames
        self.frames_to_wait -= 1;
        if self.frames_to_wait > 0 {
            return Ok(UPDATE_SLEEP_NONE);
        }
        self.frames_to_wait = SPAWN_UPDATE_RATE;

        // Process replacement times
        if self.should_try_to_spawn()? {
            let current_time = TheGameLogic::get_frame() as Int;
            // C++ SpawnBehavior::update: erase a due slot only after createSpawn
            // succeeds. A busy door leaves the replacement time on the queue so
            // the hive retries next update instead of permanently losing the slot.
            let mut index = 0;
            while index < self.replacement_times.len() {
                if current_time > self.replacement_times[index] {
                    if self.create_spawn()? {
                        self.replacement_times.remove(index);
                    } else {
                        index += 1;
                    }
                } else {
                    index += 1;
                }
            }

            // Check if one-shot spawning is complete
            if data.is_one_shot_data && self.one_shot_countdown <= 0 {
                self.stop_spawning();
            }
        }

        Ok(UPDATE_SLEEP_NONE)
    }
}

impl DieModuleInterface for SpawnBehavior {
    fn on_die(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let data = &self.module_data;
        if !self.with_object(|obj| data.die_mux_data.is_die_applicable(obj, damage_info))? {
            return Ok(());
        }

        // Notify all spawns that their master has died
        for &spawn_id in &self.spawn_ids {
            if let Some(current_spawn) = TheGameLogic::find_object_by_id(spawn_id) {
                let mut handled = false;
                {
                    let spawn_guard = current_spawn.read().map_err(|_| "Failed to read spawn")?;
                    if let Some(result) = spawn_guard.with_slaved_update_interface(|slaved| {
                        slaved.on_slaver_die(Some(damage_info))
                    }) {
                        result?;
                        handled = true;
                    }
                }

                if !handled {
                    let spawn_behaviors = {
                        let spawn_guard =
                            current_spawn.read().map_err(|_| "Failed to read spawn")?;
                        spawn_guard.get_behavior_modules()
                    };

                    for behavior in spawn_behaviors {
                        let mut behavior_guard = behavior
                            .lock()
                            .map_err(|_| "Failed to lock behavior module")?;
                        if let Some(slaved) = behavior_guard.get_slaved_update_interface() {
                            slaved.on_slaver_die(Some(damage_info))?;
                            break;
                        }
                    }
                }

                let mut spawn_guard = current_spawn.write().map_err(|_| "Failed to write spawn")?;
                spawn_guard.set_producer(None);
            }
        }

        // Kill spawns that require the spawner
        if data.spawned_require_spawner {
            for &spawn_id in &self.spawn_ids {
                if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                    let spawn_guard = spawn_obj.read().map_err(|_| "Failed to read spawn")?;
                    let is_dead = spawn_guard.is_effectively_dead();
                    drop(spawn_guard);

                    if !is_dead {
                        let mut spawn_guard =
                            spawn_obj.write().map_err(|_| "Failed to write spawn")?;
                        spawn_guard.kill(None, None);
                        drop(spawn_guard);
                    }
                }
            }
        }

        Ok(())
    }
}

impl DamageModuleInterface for SpawnBehavior {
    fn receive_damage(&mut self, _object_id: ObjectID, _damage: &DamageInfo) -> Real {
        0.0
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        // Notify all spawns that their master was damaged
        for &spawn_id in &self.spawn_ids {
            if let Some(current_spawn) = TheGameLogic::find_object_by_id(spawn_id) {
                let mut handled = false;
                {
                    let spawn_guard = current_spawn.read().map_err(|_| "Failed to read spawn")?;
                    if let Some(result) = spawn_guard
                        .with_slaved_update_interface(|slaved| slaved.on_slaver_damage(damage_info))
                    {
                        result?;
                        handled = true;
                    }
                }

                if !handled {
                    let spawn_behaviors = {
                        let spawn_guard =
                            current_spawn.read().map_err(|_| "Failed to read spawn")?;
                        spawn_guard.get_behavior_modules()
                    };

                    for behavior in spawn_behaviors {
                        let mut behavior_guard = behavior
                            .lock()
                            .map_err(|_| "Failed to lock behavior module")?;
                        if let Some(slaved) = behavior_guard.get_slaved_update_interface() {
                            slaved.on_slaver_damage(damage_info)?;
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn on_healing(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(()) // No special healing handling
    }

    fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        _old_state: BodyDamageType,
        _new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(()) // No special damage state handling
    }
}

impl SpawnBehaviorInterface for SpawnBehavior {
    fn may_spawn_self_task_ai(&self, max_self_taskers_ratio: Real) -> bool {
        let parent_last_command_source = self
            .with_object(|object| {
                object
                    .get_ai_update_interface()
                    .and_then(|ai| ai.lock().ok().map(|g| g.get_last_command_source()))
            })
            .ok()
            .flatten();
        may_spawn_self_task_ai_decision(
            self.spawn_count,
            max_self_taskers_ratio,
            self.self_tasking_spawn_count,
            parent_last_command_source,
        )
    }

    fn on_spawn_death(
        &mut self,
        dead_spawn: ObjectID,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        // Find and remove the dead spawn from our list
        if let Some(pos) = self.spawn_ids.iter().position(|&id| id == dead_spawn) {
            self.spawn_ids.remove(pos);

            let data = &self.module_data;
            let replacement_time = data.spawn_replace_delay_data + TheGameLogic::get_frame() as Int;
            self.replacement_times.push_back(replacement_time);

            self.spawn_count = self.spawn_count.saturating_sub(1);

            // If aggregate health and no spawns left, destroy parent
            if self.spawn_count == 0 && self.aggregate_health {
                if let Some(killer) = TheGameLogic::find_object_by_id(damage_info.input.source_id) {
                    let mut killer_guard = killer.write().map_err(|_| "Failed to write killer")?;
                    let _ = self.with_object(|obj_guard| {
                        killer_guard.score_the_kill(obj_guard);
                    });
                }

                TheGameLogic::destroy_object_by_id(self.get_object_id())?;
            }
        }

        Ok(())
    }

    fn get_closest_slave(&self, pos: &Coord3D) -> Option<Arc<RwLock<Object>>> {
        // Wave 346: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let mut closest: Option<Arc<RwLock<Object>>> = None;
        let mut closest_distance = Real::INFINITY;

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                let distance = if let Ok(spawn_guard) = spawn_obj.read() {
                    ThePartitionManager::get_distance_squared_to_pos(
                        &spawn_guard,
                        pos,
                        FROM_CENTER_2D,
                    )
                } else {
                    continue;
                };

                if closest.is_none() || closest_distance > distance {
                    closest = Some(spawn_obj);
                    closest_distance = distance;
                }
            }
        }

        closest
    }

    fn order_slaves_to_attack_target(
        &mut self,
        target: &Object,
        max_shots_to_fire: Int,
        cmd_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let target_id = target.get_id();
        let target_handle = TheGameLogic::find_object_by_id(target_id);
        if let Some(target_handle) = target_handle {
            for &spawn_id in &self.spawn_ids {
                if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                    if let Ok(spawn_guard) = spawn_obj.read() {
                        if let Some(ai) = spawn_guard.get_ai_update_interface() {
                            // C++ SpawnBehavior::orderSlavesToAttackTarget
                            // (SpawnBehavior.cpp:314): aiForceAttackObject.
                            ai.ai_force_attack_object(
                                target_handle.read().ok().map(|g| g.get_id()).unwrap_or(0),
                                max_shots_to_fire,
                                cmd_source,
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn order_slaves_to_attack_position(
        &mut self,
        pos: &Coord3D,
        max_shots_to_fire: Int,
        cmd_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                if let Ok(spawn_guard) = spawn_obj.read() {
                    if let Some(ai) = spawn_guard.get_ai_update_interface() {
                        ai.ai_attack_position(pos, max_shots_to_fire, cmd_source);
                    }
                }
            }
        }
        Ok(())
    }

    fn get_can_any_slaves_attack_specific_target(
        &self,
        attack_type: AbleToAttackType,
        target: &Object,
        cmd_source: CommandSourceType,
    ) -> CanAttackResult {
        // Wave 346: empty dual-world → not possible.
        if dual_world_registry_unavailable() {
            return ATTACKRESULT_NOT_POSSIBLE;
        }

        let mut invalid_shot = false;

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                let spawn_guard = spawn_obj.read().unwrap();
                let result =
                    spawn_guard.get_able_to_attack_specific_object(attack_type, target, cmd_source);
                drop(spawn_guard);

                match result {
                    ATTACKRESULT_POSSIBLE | ATTACKRESULT_POSSIBLE_AFTER_MOVING => return result,
                    ATTACKRESULT_NOT_POSSIBLE => {}
                    ATTACKRESULT_INVALID_SHOT => invalid_shot = true,
                }
            }
        }

        if invalid_shot {
            ATTACKRESULT_INVALID_SHOT
        } else {
            ATTACKRESULT_NOT_POSSIBLE
        }
    }

    fn get_can_any_slaves_use_weapon_against_target(
        &self,
        attack_type: AbleToAttackType,
        victim: &Object,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
    ) -> CanAttackResult {
        // Wave 346: empty dual-world → not possible.
        if dual_world_registry_unavailable() {
            return ATTACKRESULT_NOT_POSSIBLE;
        }

        let mut invalid_shot = false;

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                let spawn_guard = spawn_obj.read().unwrap();
                let result = spawn_guard.get_able_to_use_weapon_against_target(
                    attack_type,
                    victim,
                    pos,
                    cmd_source,
                );
                drop(spawn_guard);

                match result {
                    ATTACKRESULT_POSSIBLE | ATTACKRESULT_POSSIBLE_AFTER_MOVING => return result,
                    ATTACKRESULT_NOT_POSSIBLE => {}
                    ATTACKRESULT_INVALID_SHOT => invalid_shot = true,
                }
            }
        }

        if invalid_shot {
            ATTACKRESULT_INVALID_SHOT
        } else {
            ATTACKRESULT_NOT_POSSIBLE
        }
    }

    fn can_any_slaves_attack(&self) -> bool {
        // Wave 346: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                let spawn_guard = spawn_obj.read().unwrap();
                let can_attack = spawn_guard.is_able_to_attack();
                drop(spawn_guard);

                if can_attack {
                    return true;
                }
            }
        }
        false
    }

    fn order_slaves_to_go_idle(
        &mut self,
        cmd_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                if let Ok(spawn_guard) = spawn_obj.read() {
                    if let Some(ai) = spawn_guard.get_ai_update_interface() {
                        ai.ai_idle(cmd_source);
                    }
                }
            }
        }
        Ok(())
    }

    fn order_slaves_disabled_until(
        &mut self,
        disabled_type: DisabledType,
        frame: UnsignedInt,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                let mut spawn_guard = spawn_obj.write().map_err(|_| "Failed to write spawn")?;
                // C++ SpawnBehavior::orderSlavesDisabledUntil (SpawnBehavior.cpp:362-367):
                // idle slave AI first, then setDisabledUntil.
                if let Some(ai) = spawn_guard.get_ai_update_interface() {
                    ai.ai_idle(CMD_FROM_AI);
                }
                spawn_guard.set_disabled_until(disabled_type, frame);
                drop(spawn_guard);
            }
        }
        Ok(())
    }

    fn order_slaves_to_clear_disabled(
        &mut self,
        disabled_type: DisabledType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                let mut spawn_guard = spawn_obj.write().map_err(|_| "Failed to write spawn")?;
                spawn_guard.clear_disabled(disabled_type);
                drop(spawn_guard);
            }
        }
        Ok(())
    }

    fn give_slaves_stealth_upgrade(
        &mut self,
        grant_stealth: Bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                let mut spawn_guard = spawn_obj.write().map_err(|_| "Failed to write spawn")?;
                spawn_guard.set_status(
                    MAKE_OBJECT_STATUS_MASK!(OBJECT_STATUS_CAN_STEALTH),
                    grant_stealth,
                );
                drop(spawn_guard);
            }
        }
        Ok(())
    }

    fn are_all_slaves_stealthed(&self) -> bool {
        // Wave 346: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                let spawn_guard = spawn_obj.read().unwrap();
                if let Some(stealth) = spawn_guard.get_stealth() {
                    let stealth_guard = stealth.lock().unwrap();
                    let allowed = stealth_guard.allowed_to_stealth(&*spawn_guard);
                    drop(stealth_guard);
                    drop(spawn_guard);

                    if !allowed {
                        return false;
                    }
                } else {
                    drop(spawn_guard);
                    return false;
                }
            }
        }
        true
    }

    fn reveal_slaves(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 346: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                let spawn_guard = spawn_obj.read().unwrap();
                if let Some(stealth) = spawn_guard.get_stealth() {
                    let mut stealth_guard = stealth.lock().map_err(|_| "Failed to lock stealth")?;
                    stealth_guard.mark_as_detected();
                    drop(stealth_guard);
                }
                drop(spawn_guard);
            }
        }
        Ok(())
    }

    fn do_slaves_have_freedom(&self) -> bool {
        self.module_data.slaves_have_free_will
    }
}

impl BehaviorModuleInterface for SpawnBehavior {
    fn get_interface_mask() -> u32 {
        MODULEINTERFACE_UPDATE | MODULEINTERFACE_DIE | MODULEINTERFACE_DAMAGE
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }

    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(self)
    }

    fn get_spawn_behavior_interface(&mut self) -> Option<&mut dyn ModuleSpawnBehaviorInterface> {
        Some(self)
    }

    fn get_spawn_behavior_full_interface(
        &mut self,
    ) -> Option<&mut dyn crate::object::behavior::spawn_behavior::SpawnBehaviorInterface> {
        Some(self)
    }
}

impl ModuleSpawnBehaviorInterface for SpawnBehavior {
    fn get_spawn_count(&self) -> u32 {
        self.spawn_ids.len() as u32
    }

    fn get_spawn_object(&self, index: u32) -> Option<ObjectID> {
        self.spawn_ids.get(index as usize).copied()
    }

    fn order_slaves_disabled_until(
        &mut self,
        disabled_type: DisabledType,
        frame: UnsignedInt,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        SpawnBehaviorInterface::order_slaves_disabled_until(self, disabled_type, frame)
    }
}

// Handle cleanup on deletion
impl Drop for SpawnBehavior {
    fn drop(&mut self) {
        // Wave 346: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let data = &self.module_data;

        // Destroy spawns that require the spawner
        if data.spawned_require_spawner {
            for &spawn_id in &self.spawn_ids {
                if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                    if let Ok(spawn_guard) = spawn_obj.read() {
                        if !spawn_guard.is_effectively_dead() {
                            let _ = TheGameLogic::destroy_object(&*spawn_guard);
                        }
                    }
                }
            }
        }
    }
}

impl Snapshotable for SpawnBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ SpawnBehavior::crc (SpawnBehavior.cpp:1043-1048):
        // BehaviorModule::crc only — no spawn-module payload.
        xfer_behavior_module_base_versions(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|err| format!("SpawnBehavior::xfer version failed: {err}"))?;

        xfer_behavior_module_base_versions(xfer)?;

        if version >= 2 {
            xfer.xfer_bool(&mut self.initial_burst_times_inited)
                .map_err(|err| {
                    format!("SpawnBehavior::xfer initial_burst_times_inited failed: {err}")
                })?;
        }

        let mut template_name = self
            .spawn_template
            .as_ref()
            .map(|template| template.get_name().to_string())
            .unwrap_or_default();
        xfer.xfer_ascii_string(&mut template_name)
            .map_err(|err| format!("SpawnBehavior::xfer spawn_template failed: {err}"))?;
        if xfer.is_loading() {
            self.spawn_template = if template_name.is_empty() {
                None
            } else {
                let name = AsciiString::from(template_name.as_str());
                Some(TheObjectFactory::find_template(&name).ok_or_else(|| {
                    format!("SpawnBehavior::xfer unable to find template '{template_name}'")
                })?)
            };
        }

        xfer.xfer_int(&mut self.one_shot_countdown)
            .map_err(|err| format!("SpawnBehavior::xfer one_shot_countdown failed: {err}"))?;
        xfer.xfer_int(&mut self.frames_to_wait)
            .map_err(|err| format!("SpawnBehavior::xfer frames_to_wait failed: {err}"))?;
        xfer.xfer_int(&mut self.first_batch_count)
            .map_err(|err| format!("SpawnBehavior::xfer first_batch_count failed: {err}"))?;

        let mut replacement_times: Vec<Int> = self.replacement_times.iter().copied().collect();
        if xfer.is_loading() {
            replacement_times.clear();
        }
        xfer.xfer_stl_int_list(&mut replacement_times)
            .map_err(|err| format!("SpawnBehavior::xfer replacement_times failed: {err}"))?;
        if xfer.is_loading() {
            self.replacement_times = replacement_times.into();
        }

        if xfer.is_loading() {
            self.spawn_ids.clear();
        }
        xfer.xfer_stl_object_id_list(&mut self.spawn_ids)
            .map_err(|err| format!("SpawnBehavior::xfer spawn_ids failed: {err}"))?;

        xfer.xfer_bool(&mut self.active)
            .map_err(|err| format!("SpawnBehavior::xfer active failed: {err}"))?;
        xfer.xfer_bool(&mut self.aggregate_health)
            .map_err(|err| format!("SpawnBehavior::xfer aggregate_health failed: {err}"))?;
        xfer.xfer_unsigned_int(&mut self.spawn_count)
            .map_err(|err| format!("SpawnBehavior::xfer spawn_count failed: {err}"))?;
        xfer.xfer_unsigned_int(&mut self.self_tasking_spawn_count)
            .map_err(|err| format!("SpawnBehavior::xfer self_tasking_spawn_count failed: {err}"))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes SpawnBehavior through the common Module trait.
pub struct SpawnBehaviorModule {
    behavior: SpawnBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<SpawnBehaviorModuleData>,
}

impl SpawnBehaviorModule {
    pub fn new(
        behavior: SpawnBehavior,
        module_name: &AsciiString,
        module_data: Arc<SpawnBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut SpawnBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for SpawnBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for SpawnBehaviorModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }

    fn get_spawn_control_interface(&mut self) -> Option<&mut dyn SpawnControlInterface> {
        Some(self)
    }
}

impl SpawnControlInterface for SpawnBehaviorModule {
    fn closest_slave_id_for_position(&self, pos: [f32; 3]) -> Option<ObjectID> {
        self.behavior
            .get_closest_slave(&Coord3D {
                x: pos[0],
                y: pos[1],
                z: pos[2],
            })
            .and_then(|slave| slave.read().ok().map(|guard| guard.get_id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_data_creation() {
        let data = SpawnBehaviorModuleData::new();
        assert_eq!(data.spawn_number_data, 0);
        assert_eq!(data.spawn_replace_delay_data, 0);
        assert!(!data.is_one_shot_data);
        assert!(!data.can_reclaim_orphans);
        assert!(!data.aggregate_health);
        assert!(data.spawn_template_name_data.is_empty());
    }

    #[test]
    fn test_constants() {
        assert_eq!(SPAWN_DELAY_MIN_FRAMES, 16);
        assert_eq!(NONE_SPAWNED_YET, 0xFFFFFFFF);
        assert!(BIG_DISTANCE > 1000000.0);
    }

    #[test]
    fn parse_duration_frames_accepts_duration_suffixes() {
        assert_eq!(parse_duration_frames(&["1500ms"]).expect("duration"), 45);
        assert_eq!(parse_duration_frames(&["1.5s"]).expect("duration"), 45);
    }

    // C++ SpawnBehavior::update (SpawnBehavior.cpp:196-204):
    // runtime-produced InitialBurst slots stagger by SPAWN_DELAY_MIN_FRAMES.
    #[test]
    fn burst_replacement_times_stagger_runtime_produced() {
        let times = initial_burst_replacement_times(100, 4, 3, true);
        assert_eq!(times, vec![100, 116, 132, 100]);
    }

    #[test]
    fn burst_replacement_times_no_stagger_without_producer() {
        let times = initial_burst_replacement_times(100, 4, 3, false);
        assert_eq!(times, vec![100, 100, 100, 100]);
    }

    #[test]
    fn burst_replacement_times_without_initial_burst_uses_index() {
        let times = initial_burst_replacement_times(100, 3, 0, true);
        assert_eq!(times, vec![0, 1, 2]);
    }

    // C++ SpawnBehavior::createSpawn (SpawnBehavior.cpp:600).
    #[test]
    fn orphan_reclaim_only_when_authored_and_not_one_shot() {
        assert!(should_attempt_orphan_reclaim(true, false));
        assert!(!should_attempt_orphan_reclaim(false, false));
        assert!(!should_attempt_orphan_reclaim(true, true));
        assert!(!should_attempt_orphan_reclaim(false, true));
    }

    #[test]
    fn orphan_template_skips_consecutive_redundancy_not_different_names() {
        assert!(!orphan_template_is_redundant("", "A"));
        assert!(orphan_template_is_redundant("A", "A"));
        assert!(!orphan_template_is_redundant("A", "B"));
    }

    // C++ computeAggregateStates (SpawnBehavior.cpp:985): Int cast, no clamp/round.
    #[test]
    fn aggregate_health_percent_truncates_like_cpp() {
        assert_eq!(aggregate_initial_health_percent(300.0, 300.0, 3, 4), 75);
        // 73.7 would round to 74; C++ Int(73.7f) == 73.
        assert_eq!(aggregate_initial_health_percent(73.7, 100.0, 1, 1), 73);
        // Over-population must not clamp to 100 (5/4 → 125).
        assert_eq!(aggregate_initial_health_percent(500.0, 500.0, 5, 4), 125);
        assert_eq!(aggregate_initial_health_percent(0.0, 0.0, 0, 4), 0);
    }

    // C++ maySpawnSelfTaskAI: parent with no AI still evaluates the ratio.
    #[test]
    fn may_spawn_self_task_ai_allows_when_parent_has_no_ai() {
        assert!(may_spawn_self_task_ai_decision(4, 0.5, 0, None));
        assert!(!may_spawn_self_task_ai_decision(
            4,
            0.5,
            0,
            Some(CommandSourceType::FromPlayer)
        ));
        assert!(may_spawn_self_task_ai_decision(
            4,
            0.5,
            0,
            Some(CommandSourceType::FromAi)
        ));
        assert!(!may_spawn_self_task_ai_decision(4, 0.5, 3, None));
        assert!(!may_spawn_self_task_ai_decision(0, 0.5, 0, None));
    }

    // C++ shouldTryToSpawn hole-rebuild latch (SpawnBehavior.cpp:809-813).
    #[test]
    fn hole_rebuild_one_shot_latches() {
        assert!(hole_rebuild_one_shot_should_latch(true, true));
        assert!(!hole_rebuild_one_shot_should_latch(true, false));
        assert!(!hole_rebuild_one_shot_should_latch(false, true));
    }

    // C++ SpawnBehavior::crc is BehaviorModule::crc only (SpawnBehavior.cpp:1043-1048).
    #[test]
    fn crc_does_not_write_spawn_payload() {
        let src = include_str!("spawn_behavior.rs");
        let crc_fn = src
            .split("impl Snapshotable for SpawnBehavior")
            .nth(1)
            .and_then(|rest| rest.split("fn xfer(").next())
            .expect("crc impl");
        assert!(crc_fn.contains("xfer_behavior_module_base_versions"));
        assert!(!crc_fn.contains("xfer_ascii_string"));
        assert!(!crc_fn.contains("xfer_stl_int_list"));
        assert!(!crc_fn.contains("one_shot_countdown"));
    }
}
