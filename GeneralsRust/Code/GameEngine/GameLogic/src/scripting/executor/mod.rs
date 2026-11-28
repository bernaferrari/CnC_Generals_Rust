//! Script Action and Condition Executor
//!
//! This module implements the script execution engine matching C++ ScriptActions and ScriptConditions.
//! It provides the complete action and condition system for mission scripting.
//!
//! C++ Reference: ScriptActions.cpp, ScriptConditions.cpp
//! Functions: executeAction(), evaluateCondition()

//! Split into focused submodules by action/condition family.

pub use generals_sim::scripting::world::{
    HostScriptMoneyRequest, HostScriptObjectSoundRequest,
};

use crate::object::types::LegacyObjectTypes;
use super::core::*;
use std::sync::atomic::Ordering;
use super::engine::{
    CanonicalScriptExecutionDriver, ScriptExecutionDriver, TFade, get_area_tracker,
    get_named_object_tracker,
};
use crate::ai::integration::{IntegratedAiPlayer, with_ai_integration_mut};
use crate::ai::{
    AiCommandInterface, AiCommandParams, AiCommandType, AiGroup, AttitudeType, GuardMode,
};
use crate::commands::{
    Command, CommandPriority, CommandType, QueuedCommand, get_command_queue_manager,
};
use crate::common::{
    AsciiString, Color, CommandSourceType, Coord3D, INVALID_ID, LOGICFRAMES_PER_SECOND, ObjectID,
    Relationship, WaypointID,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{TheAudio, TheGameLogic, ThePartitionManager, TheVictoryConditions};
use crate::modules::AIAttitudeType;
use crate::object::behavior::auto_heal_behavior::parse_kind_of;
use crate::object::object_types::ObjectTypes;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::get_special_power_store;
use crate::object::update::special_power_update::SpecialPowerCommandOption;
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use crate::object_manager::get_object_manager;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::player::{PlayerType, player_list};
use crate::system::game_logic::TheObjectFactory;
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::upgrade::center::get_upgrade_center;
use generals_sim::scripting::presentation::ScriptActionHandler;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::{SCIENCE_INVALID, ScienceType, get_science_store};
#[cfg(test)]
use game_engine::common::system::radar::RadarEventType;
use once_cell::sync::Lazy;
use std::cell::RefCell;
#[cfg(test)]
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// Wave 284: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

pub use generals_sim::scripting::{ScriptActionResult, ScriptError};

/// Script condition evaluation result
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptConditionResult {
    /// Condition is true
    True,
    /// Condition is false
    False,
    /// Condition evaluation error
    Error(String),
}

/// Script execution context
///
/// C++ Reference: ScriptActions class member variables
/// This provides access to all game systems needed for script execution
#[derive(Clone)]
pub struct ScriptContext {
    // Game system references (reserved for tighter integration points)
    pub game_logic_id: u32,
    pub object_manager_id: u32,
    pub player_manager_id: u32,
    pub event_system_id: u32,
    pub camera_system_id: u32,
    pub audio_system_id: u32,
    pub partition_manager_id: u32,
    pub special_powers_id: u32,

    // Runtime state
    pub current_frame: u32,
    pub suppress_new_windows: bool,
    /// Trigger edge state for this world context. Main supplies the owning
    /// GameLogic instance's handle; standalone executor contexts own a fresh
    /// state instead of consulting ambient TLS.
    pub host_trigger_world: Arc<Mutex<crate::scripting::HostTriggerWorld>>,
}

impl ScriptContext {
    pub fn with_session_world(
        mut self,
        world: Arc<Mutex<crate::scripting::HostTriggerWorld>>,
    ) -> Self {
        self.host_trigger_world = world;
        self
    }

    /// An installed owner is authoritative, including an installed empty
    /// map. Only standalone, uninitialized contexts retain ambient fallback.
    fn owned_trigger_area(
        &self,
        area_name: &str,
    ) -> Option<Option<crate::polygon_trigger::PolygonTrigger>> {
        let world = self
            .host_trigger_world
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        world
            .has_authored_trigger_geometry()
            .then(|| world.trigger_area_by_name(area_name))
    }

    pub fn new() -> Self {
        Self::at_frame(TheGameLogic::get_frame())
    }

    /// Construct operation state at the driving world's observed logic frame.
    /// The caller supplies the clock; construction never acquires GameLogic.
    pub fn at_frame(current_frame: u32) -> Self {
        Self::for_session_at_frame(current_frame, Arc::new(Mutex::new(Default::default())))
    }

    /// Build a context using the trigger state already owned by the session.
    pub fn for_session_at_frame(
        current_frame: u32,
        host_trigger_world: Arc<Mutex<crate::scripting::HostTriggerWorld>>,
    ) -> Self {
        Self {
            game_logic_id: 0,
            object_manager_id: 0,
            player_manager_id: 0,
            event_system_id: 0,
            camera_system_id: 0,
            audio_system_id: 0,
            partition_manager_id: 0,
            special_powers_id: 0,
            current_frame,
            suppress_new_windows: false,
            host_trigger_world,
        }
    }
}

/// Script action dispatcher
///
/// C++ Reference: ScriptActions::executeAction()
/// This is the main entry point for executing script actions
pub struct ScriptActionDispatcher<'engine> {
    context: ExecutionContext<'engine>,
    /// Read-only name projection refreshed at each driving-owner action.
    this_team_name: crate::scripting::engine::ScriptOwnerQuery<String>,
}

impl<'engine> ScriptActionDispatcher<'engine> {
    /// C++ `TheAudio` of the engine running this script, never the one
    /// published on the thread.
    fn the_audio(&self) -> Option<crate::helpers::TheAudio> {
        self.context
            .borrowed_engine()
            .owning_world_services()
            .map(|world| crate::helpers::TheAudio::of(&world))
    }

    /// C++ the host facts of the engine running this script (its own
    /// world's snapshot), never the world published on the thread.
    fn host_query(&self) -> crate::scripting::HostScriptQuery {
        crate::scripting::HostScriptQuery::from_world(
            self.context.borrowed_engine().owning_world_services(),
        )
    }

    /// C++ TheControlBar of the engine dispatching this script (its world's
    /// resolved catalog), never the world published on the thread.
    fn owning_control_bar_bridge(&self) -> Option<Arc<crate::control_bar::ControlBarBridge>> {
        self.context.with_engine_ref(|engine| engine.control_bar_bridge()).flatten()
    }

    /// Script command-bar override on the dispatching engine's own world (its
    /// ControlBar slot and that world's GameLogic override table), followed by
    /// C++ `TheControlBar->markUIDirty()` on the same world.
    pub(crate) fn set_owning_command_set_slot_override(
        &self,
        command_set_name: &str,
        slot: usize,
        button_name: Option<&str>,
    ) -> Result<(), String> {
        let (bridge_slot, ui_hooks) = self
            .context
            .with_engine_ref(|engine| (engine.control_bar_bridge_slot(), engine.control_bar_ui_hooks()))
            .ok_or_else(|| "script command-bar override has no dispatching engine".to_string())?;
        let result = crate::control_bar::set_command_set_slot_override_in(
            &bridge_slot,
            command_set_name,
            slot,
            button_name,
        );
        crate::control_bar::mark_ui_dirty_in(&ui_hooks);
        result
    }

    fn current_script_player_name(&self) -> String {
        self.context
            .with_engine_ref(|engine| engine.get_current_player_name())
            .flatten()
            .unwrap_or_default()
    }

    /// Return an owned handler before crossing callbacks; no engine/state guard escapes.
    fn current_script_action_handler(
        &self,
    ) -> Option<Arc<dyn ScriptActionHandler>> {
        self.context
            .with_engine_ref(|engine| engine.action_handler())
            .flatten()
    }

    /// Dispatch synchronously on this engine using caller-owned operation state.
    /// Construction does not publish the engine or select an ambient world.
    pub fn new(
        engine: &'engine crate::scripting::engine::ScriptEngine,
        state: &'engine std::cell::RefCell<ScriptContext>,
    ) -> Self {
        Self {
            context: ExecutionContext::borrowed(engine, state),
            this_team_name: crate::scripting::engine::ScriptOwnerQuery::Unavailable,
        }
    }
}

/// Script condition evaluator
///
/// C++ Reference: ScriptConditions::evaluateCondition()
/// This evaluates script conditions to determine script flow
#[allow(dead_code)]
pub struct ScriptConditionEvaluator<'engine> {
    context: ExecutionContext<'engine>,
    /// Read-only projection; the engine's canonical references remain IDs.
    this_team_name: crate::scripting::engine::ScriptOwnerQuery<String>,
}

impl<'engine> ScriptConditionEvaluator<'engine> {
    /// C++ the host facts of the engine running this script (its own
    /// world's snapshot), never the world published on the thread.
    fn host_query(&self) -> crate::scripting::HostScriptQuery {
        crate::scripting::HostScriptQuery::from_world(
            self.context.borrowed_engine().owning_world_services(),
        )
    }

    /// C++ TheControlBar of the engine dispatching this script (its world's
    /// resolved catalog), never the world published on the thread.
    fn owning_control_bar_bridge(&self) -> Option<Arc<crate::control_bar::ControlBarBridge>> {
        self.context.with_engine_ref(|engine| engine.control_bar_bridge()).flatten()
    }

    /// Dispatch synchronously on this engine using caller-owned operation state.
    /// Construction does not publish the engine or select an ambient world.
    pub fn new(
        engine: &'engine crate::scripting::engine::ScriptEngine,
        state: &'engine std::cell::RefCell<ScriptContext>,
    ) -> Self {
        Self {
            context: ExecutionContext::borrowed(engine, state),
            this_team_name: crate::scripting::engine::ScriptOwnerQuery::Unavailable,
        }
    }

    pub(crate) fn with_host_trigger_world<R>(
        &self,
        f: impl FnOnce(&crate::scripting::HostTriggerWorld, u32) -> R,
    ) -> R {
        self.context.with_state(|context| {
            let world = context
                .host_trigger_world
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            f(&world, context.current_frame)
        })
    }
}

mod execution_context;
use execution_context::ExecutionContext;

mod actions_attack_priority;
mod actions_garrison;
mod actions_input_ui;
mod actions_named;
mod actions_player;
mod actions_player_control;
mod actions_skirmish;
mod actions_team_build;
mod actions_team_command;
mod actions_team_relations;
mod actions_victory_team;
mod actions_world;
mod camera_driver;
mod map_driver;
mod object_actions;
mod media_driver;
mod borrowed_presentation;
mod dispatch;
pub(crate) use borrowed_presentation::{apply_script_drawable_flash, script_indicator_color};
mod eval_basic;
mod eval_named;
#[cfg(test)]
mod eval_owned_body_census;
#[cfg(test)]
mod eval_owned_player_area;
#[cfg(test)]
mod eval_owned_player_census;
#[cfg(test)]
mod eval_owned_skirmish;
mod eval_player_events;
#[cfg(test)]
mod eval_receiving_named_facts;
mod eval_skirmish;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod host_query_owner_tests {
    use super::*;
    use crate::system::engine_stores::{new_world_services, with_world_services};

    #[test]
    fn evaluator_host_query_is_its_engines_world_not_the_published_one() {
        let a = new_world_services();
        let b = new_world_services();
        let mut snap = crate::scripting::HostScriptQuerySnapshot::default();
        snap.named.insert("Hero".to_string(), 7);
        crate::scripting::HostScriptQuery::of(&a).set_host_script_query_snapshot(snap);
        let engine = crate::scripting::engine::ScriptEngine::new_with_services(&a).unwrap();
        let state = std::cell::RefCell::new(ScriptContext::at_frame(0));
        with_world_services(&b, || {
            let evaluator = ScriptConditionEvaluator::new(&engine, &state);
            assert_eq!(evaluator.host_query().host_script_named_unit_id("Hero"), Some(7));
            assert!(!crate::scripting::HostScriptQuery::of(&b).host_script_query_has_any());
        });
    }
}
