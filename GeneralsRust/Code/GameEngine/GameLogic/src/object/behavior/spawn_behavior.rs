//! SpawnBehavior - Rust conversion of C++ SpawnBehavior
//!
//! Behavior will create and monitor a group of spawned units and replace as needed
//! Original Authors: Graham Smallwood, January 2002; Colin Day, October 2002
//! Rust conversion: 2025

use crate::common::{
    AsciiString, Bool, Byte, Coord3D, DisabledType, Int, KindOf, ModuleData, ObjectID,
    PlayerMaskType, Real, TheObjectFactory, UnsignedInt, VeterancyLevel, INVALID_ID,
    LOGICFRAMES_PER_SECOND,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

// Forward declarations
use crate::attack::{
    AbleToAttackType, CanAttackResult, ATTACKRESULT_INVALID_SHOT, ATTACKRESULT_NOT_POSSIBLE,
    ATTACKRESULT_POSSIBLE, ATTACKRESULT_POSSIBLE_AFTER_MOVING,
};
use crate::common::CommandSourceType;
use crate::common::DamageTypeFlags;
use crate::common::{
    TheGameLogic, TheInGameUI, TheMessageStream, ThePartitionManager, FROM_CENTER_2D,
    OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_MASKED, OBJECT_STATUS_RECONSTRUCTING,
    OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::damage::{BodyDamageType, DamageInfo, DamageType};
use crate::experience::ExperienceTracker;
use crate::messages::{GameMessage, MSG_CREATE_SELECTED_GROUP};
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BehaviorModule, BehaviorModuleInterface,
    BodyModuleInterface, DamageModuleInterface, DieModuleInterface, ExitDoorType, ExitInterface,
    ModuleInterface, SlavedUpdateInterface, SpawnBehaviorInterface as ModuleSpawnBehaviorInterface,
    UpdateModule, UpdateModuleInterface, UpdateSleepTime, DOOR_NONE_AVAILABLE,
    MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE, MODULEINTERFACE_UPDATE, UPDATE_SLEEP,
    UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::object::drawable::DrawableExt;
use crate::object::{Object, ObjectStatusTypes};
use crate::player::{Player, CMD_FROM_AI};
use crate::team::Team;
use crate::template::ObjectTemplate;
use crate::MAKE_OBJECT_STATUS_MASK;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
pub type DieMuxData = crate::object::die::DieMuxData;
use crate::object::die::{
    parse_death_type_flags_tokens, parse_object_status_mask_tokens,
    parse_veterancy_level_flags_tokens,
};
use game_engine::common::ini::{FieldParse, INIError, INI};
use std::str::FromStr;

// Constants
const SPAWN_UPDATE_RATE: Int = (LOGICFRAMES_PER_SECOND / 2) as Int; // Low priority update rate
const SPAWN_DELAY_MIN_FRAMES: Int = 16; // Minimum delay between successive exits
const NONE_SPAWNED_YET: UnsignedInt = 0xFFFFFFFF;
const BIG_DISTANCE: Real = 99999999.9;

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
    object: Option<Arc<RwLock<Object>>>,
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
        _thing: Arc<RwLock<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = {
            let data_ref = module_data
                .as_any()
                .downcast_ref::<SpawnBehaviorModuleData>()
                .ok_or("Invalid module data type")?;
            data_ref.clone()
        };

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
            object: None, // Will be set after creation
            module_data: Arc::new(data.clone()),
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

    pub fn set_object(&mut self, object: Arc<RwLock<Object>>) {
        self.object = Some(object);
    }

    fn get_object(&self) -> Result<Arc<RwLock<Object>>, Box<dyn std::error::Error + Send + Sync>> {
        self.object.clone().ok_or("Object not set".into())
    }

    fn notify_slaved_update(
        &self,
        spawned: &Arc<RwLock<Object>>,
        master: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let module_handle = {
            let spawn_guard = spawned.read().map_err(|_| "Failed to read spawn")?;
            spawn_guard.find_update_module("SlavedUpdate")
        };

        if let Some(module) = module_handle {
            let handled = module.with_module_downcast::<crate::object::update::slaved_update::SlavedUpdateModule, _, _>(|module| {
                let _ = module.behavior_mut().on_enslave(master);
            });
            if handled.is_some() {
                return Ok(());
            }
        }

        let spawn_guard = spawned.read().map_err(|_| "Failed to read spawn")?;
        for behavior in spawn_guard.get_behavior_modules() {
            let mut behavior_guard = behavior
                .lock()
                .map_err(|_| "Failed to lock behavior module")?;
            if let Some(slaved) = behavior_guard.get_slaved_update_interface() {
                slaved.on_enslave(master)?;
                break;
            }
        }
        Ok(())
    }

    fn should_try_to_spawn(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let data = Arc::clone(&self.module_data);

        // Not if we are turned off
        if !self.active {
            return Ok(false);
        }

        let object = self.get_object()?;
        let obj_guard = object.read().map_err(|_| "Failed to read object")?;

        // Check for reconstruction and one-shot spawning
        if obj_guard
            .get_status_bits()
            .test(OBJECT_STATUS_RECONSTRUCTING)
            && data.is_one_shot_data
        {
            drop(obj_guard);
            // If we are a Hole rebuild, not only should we not, but we should never ask again.
            return Ok(false);
        }

        // Not if we are under construction or being sold
        if obj_guard.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION)
            || obj_guard.test_status(OBJECT_STATUS_SOLD)
        {
            return Ok(false);
        }

        // Not if we are civilian controlled
        if obj_guard.is_neutral_controlled() {
            return Ok(false);
        }

        Ok(true)
    }

    fn create_spawn(&mut self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let object = self.get_object()?;
        let data = Arc::clone(&self.module_data);

        // Get exit interface
        let exit_interface = {
            let obj_guard = object.read().map_err(|_| "Failed to read object")?;
            obj_guard
                .get_object_exit_interface()
                .ok_or("Object must have ExitInterface to use SpawnBehavior")?
        };

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
        if data.can_reclaim_orphans && !data.is_one_shot_data {
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

            let parent_team = {
                let obj_guard = object.read().map_err(|_| "Failed to read object")?;
                obj_guard.get_team()
            };

            let spawn_obj = TheObjectFactory::new_object(Arc::clone(template), parent_team)?;

            // Count this unit towards our score
            let controlling_player = {
                let obj_guard = object.read().map_err(|_| "Failed to read object")?;
                obj_guard.get_controlling_player()
            };

            if let Some(player) = controlling_player {
                let mut player_guard = player.write().map_err(|_| "Failed to write player")?;
                player_guard.on_unit_created(&object, &spawn_obj);
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
            let parent_obj = object.read().map_err(|_| "Failed to read parent")?;
            spawn_guard.set_producer(Some(&*parent_obj));
            drop(parent_obj);
            drop(spawn_guard);
        }

        // If spawned object has a SlavedUpdate, tell them who their master is
        self.notify_slaved_update(&new_spawn, &object)?;

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
                    let producer_id = {
                        let obj_guard = object.read().map_err(|_| "Failed to read object")?;
                        obj_guard.get_producer_id()
                    };

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
                                        barracks_exit_guard
                                            .exit_object_via_door(&new_spawn, barracks_door)?;
                                        drop(barracks_exit_guard);

                                        // Set producer back to parent
                                        let mut spawn_guard = new_spawn
                                            .write()
                                            .map_err(|_| "Failed to write spawn")?;
                                        let parent_obj =
                                            object.read().map_err(|_| "Failed to read parent")?;
                                        spawn_guard.set_producer(Some(&*parent_obj));
                                        drop(parent_obj);
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
                                let parent_guard =
                                    object.read().map_err(|_| "Failed to read parent")?;
                                ThePartitionManager::get_distance_squared(
                                    &cur_spawn_guard,
                                    &parent_guard,
                                    FROM_CENTER_2D,
                                )
                            };
                            if distance < closest_distance {
                                closest_distance = distance;
                                bud_host = Some(cur_spawn);
                            }
                        }
                    }

                    exit_guard.exit_object_by_budding(&new_spawn, bud_host.as_ref())?;
                }
            } else {
                exit_guard.exit_object_via_door(&new_spawn, exit_door)?;
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
        let object = self.get_object()?;
        let data = self.module_data.clone();

        let controlling_player = {
            let obj_guard = object.read().map_err(|_| "Failed to read object")?;
            obj_guard.get_controlling_player()
        };

        let player = controlling_player.ok_or("No controlling player")?;

        // Find closest orphan matching our templates
        let mut closest_orphan = None;
        let mut closest_distance = BIG_DISTANCE;
        let object_pos = {
            let obj_guard = object.read().map_err(|_| "Failed to read object")?;
            obj_guard.get_position().clone()
        };

        // Check each template type
        let mut checked_templates = std::collections::HashSet::new();
        for template_name in &data.spawn_template_name_data {
            if checked_templates.contains(template_name) {
                continue; // Skip duplicates
            }
            checked_templates.insert(template_name.clone());

            if let Some(template) = TheObjectFactory::find_template(template_name) {
                let player_objects = {
                    let player_guard = player.read().map_err(|_| "Failed to read player")?;
                    player_guard.get_objects()
                };

                for player_obj in player_objects {
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
        if !self.aggregate_health {
            return Ok(()); // Not using aggregate health
        }

        let object = self.get_object()?;
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
                let obj_guard = object.read().map_err(|_| "Failed to read object")?;
                let obj_vet_level = obj_guard.get_veterancy_level();

                if spawn_vet_level > obj_vet_level {
                    drop(obj_guard);
                    let obj_guard = object.write().map_err(|_| "Failed to write object")?;
                    if let Some(exp_tracker) = obj_guard.get_experience_tracker() {
                        let mut tracker_guard = exp_tracker
                            .lock()
                            .map_err(|_| "Failed to lock experience tracker")?;
                        tracker_guard.set_veterancy_level_with_requirements(
                            spawn_vet_level,
                            &ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED,
                        );
                        drop(tracker_guard);
                    }
                    drop(obj_guard);
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
            let obj_selected = {
                let obj_guard = object.read().map_err(|_| "Failed to read object")?;
                if let Some(drawable) = obj_guard.get_drawable() {
                    drawable
                        .read()
                        .map_err(|_| "Failed to read object drawable")?
                        .is_selected()
                } else {
                    false
                }
            };

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
                    let obj_guard = object.read().map_err(|_| "Failed to read object")?;
                    if let Some(drawable) = obj_guard.get_drawable() {
                        TheInGameUI::select_drawable(&drawable);
                        TheInGameUI::set_displayed_max_warning(false);
                        team_msg.append_boolean_argument(false);
                        team_msg.append_object_id_argument(obj_guard.get_id());
                    }
                }
            }
        }

        // Update health box position (average of spawn positions)
        if spawn_count > 0 {
            avg_spawn_pos /= spawn_count as Real;
            let obj_pos = {
                let obj_guard = object.read().map_err(|_| "Failed to read object")?;
                obj_guard.get_position().clone()
            };
            avg_spawn_pos -= obj_pos;

            let mut obj_guard = object.write().map_err(|_| "Failed to write object")?;
            obj_guard.set_health_box_offset(avg_spawn_pos);
            drop(obj_guard);
        }

        // Update aggregate health
        if spawn_count > 0 {
            avg_health_max /= spawn_count as Real;
            let perfect_total_health = avg_health_max * spawn_count_max as Real;
            let actual_health = acr_health / perfect_total_health;

            let obj_guard = object.write().map_err(|_| "Failed to write object")?;
            if let Some(body) = obj_guard.get_body_module() {
                let mut body_guard = body.lock().map_err(|_| "Failed to lock object body")?;
                let percent = (100.0 * actual_health).clamp(0.0, 100.0).round() as i32;
                body_guard
                    .set_initial_health(percent)
                    .map_err(|e| format!("Failed to set spawn initial health: {e}"))?;
                drop(body_guard);
            }
            drop(obj_guard);
        } else {
            let obj_guard = object.write().map_err(|_| "Failed to write object")?;
            if let Some(body) = obj_guard.get_body_module() {
                let mut body_guard = body.lock().map_err(|_| "Failed to lock object body")?;
                body_guard
                    .set_initial_health(0)
                    .map_err(|e| format!("Failed to set spawn initial zero health: {e}"))?;
                drop(body_guard);
            }
            drop(obj_guard);
        }

        // Make sure no enemies are shooting at the nexus, since it doesn't 'exist'
        let mut obj_guard = object.write().map_err(|_| "Failed to write object")?;
        obj_guard.set_status(MAKE_OBJECT_STATUS_MASK!(OBJECT_STATUS_MASKED), true);
        drop(obj_guard);

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

            let object = self.get_object()?;
            let runtime_produced = {
                let obj_guard = object.read().map_err(|_| "Failed to read object")?;
                obj_guard.get_producer_id() != INVALID_ID
            };

            let now = TheGameLogic::get_frame();
            let mut burst_init_count = self.initial_burst_countdown;

            for list_index in 0..data.spawn_number_data {
                if data.initial_burst > 0 {
                    let mut birth_frame = now;
                    if runtime_produced && burst_init_count > 0 {
                        burst_init_count -= 1;
                        birth_frame += (list_index * SPAWN_DELAY_MIN_FRAMES) as UnsignedInt;
                    }
                    self.replacement_times.push_back(birth_frame as Int);
                } else {
                    self.replacement_times.push_back(list_index);
                }
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

            while let Some(replacement_time) = self.replacement_times.front().cloned() {
                if current_time <= replacement_time {
                    break;
                }
                self.replacement_times.pop_front();
                if self.create_spawn()? {
                    // Successfully created spawn
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
        let data = &self.module_data;
        let object = self.get_object()?;

        if !data
            .die_mux_data
            .is_die_applicable(&object.read().unwrap(), damage_info)
        {
            return Ok(());
        }

        // Notify all spawns that their master has died
        for &spawn_id in &self.spawn_ids {
            if let Some(current_spawn) = TheGameLogic::find_object_by_id(spawn_id) {
                let module_handle = {
                    let spawn_guard = current_spawn.read().map_err(|_| "Failed to read spawn")?;
                    spawn_guard.find_update_module("SlavedUpdate")
                };

                let mut handled = false;
                if let Some(module) = module_handle {
                    handled = module
                        .with_module_downcast::<crate::object::update::slaved_update::SlavedUpdateModule, _, _>(|module| {
                            let _ = module.behavior_mut().on_slaver_die(Some(damage_info));
                        })
                        .is_some();
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
        // Notify all spawns that their master was damaged
        for &spawn_id in &self.spawn_ids {
            if let Some(current_spawn) = TheGameLogic::find_object_by_id(spawn_id) {
                let module_handle = {
                    let spawn_guard = current_spawn.read().map_err(|_| "Failed to read spawn")?;
                    spawn_guard.find_update_module("SlavedUpdate")
                };

                let mut handled = false;
                if let Some(module) = module_handle {
                    handled = module
                        .with_module_downcast::<crate::object::update::slaved_update::SlavedUpdateModule, _, _>(|module| {
                            let _ = module.behavior_mut().on_slaver_damage(damage_info);
                        })
                        .is_some();
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
        if self.spawn_count == 0 || max_self_taskers_ratio == 0.0 {
            return false;
        }

        // Check if last attack command was from player/script
        if let Ok(object) = self.get_object() {
            if let Some(ai) = object.read().unwrap().get_ai_update_interface() {
                let ai_guard = ai.lock().unwrap();
                let last_command_source = ai_guard.get_last_command_source();
                drop(ai_guard);

                if last_command_source != CMD_FROM_AI {
                    return false;
                }
            }
        }

        let cur_self_taskers_ratio =
            self.self_tasking_spawn_count as Real / self.spawn_count as Real;
        cur_self_taskers_ratio < max_self_taskers_ratio
    }

    fn on_spawn_death(
        &mut self,
        dead_spawn: ObjectID,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                    let target_object = self.get_object()?;
                    let mut killer_guard = killer.write().map_err(|_| "Failed to write killer")?;
                    let obj_guard = target_object.read().map_err(|_| "Failed to read object")?;
                    killer_guard.score_the_kill(&*obj_guard);
                }

                let object = self.get_object()?;
                let obj_guard = object.read().map_err(|_| "Failed to read object")?;
                TheGameLogic::destroy_object(&*obj_guard)?;
            }
        }

        Ok(())
    }

    fn get_closest_slave(&self, pos: &Coord3D) -> Option<Arc<RwLock<Object>>> {
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
        let target_id = target.get_id();
        let target_handle = TheGameLogic::find_object_by_id(target_id);
        if let Some(target_handle) = target_handle {
            for &spawn_id in &self.spawn_ids {
                if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                    if let Ok(spawn_guard) = spawn_obj.read() {
                        if let Some(ai) = spawn_guard.get_ai_update_interface() {
                            ai.ai_attack_object(&target_handle, max_shots_to_fire, cmd_source);
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
        for &spawn_id in &self.spawn_ids {
            if let Some(spawn_obj) = TheGameLogic::find_object_by_id(spawn_id) {
                let mut spawn_guard = spawn_obj.write().map_err(|_| "Failed to write spawn")?;
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
}

// Handle cleanup on deletion
impl Drop for SpawnBehavior {
    fn drop(&mut self) {
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
        self.module_data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Arc::make_mut(&mut self.module_data).xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Arc::make_mut(&mut self.module_data).load_post_process()
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
}

// Thread safety
unsafe impl Send for SpawnBehavior {}
unsafe impl Sync for SpawnBehavior {}

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
}
