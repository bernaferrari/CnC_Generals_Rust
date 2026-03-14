//! Advanced Scripting System
//!
//! This module provides a comprehensive scripting system for game logic,
//! including script actions, conditions, victory conditions, and dynamic scripting.
#![allow(ambiguous_glob_reexports)]

use crate::common::Coord3D;
use crate::{GameLogicError, GameLogicResult};

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use game_engine::common::system::{Xfer, XferStatus};
use rhai::{
    Array as RhaiArray, Dynamic, Engine, FuncRegistration, Map as RhaiMap, Module, Scope, AST,
};
use serde::{Deserialize, Serialize};

pub mod actions;
pub mod conditions;
pub mod core;
pub mod engine;
pub mod evaluator;
pub mod events;
pub mod executor;
pub mod ini_parser;
pub mod map_scripts;
pub mod rhai_bridge;
pub mod script_actions;
pub mod script_conditions;
pub mod script_engine;
pub mod scripts;
pub mod triggers;
pub mod variables;
pub mod victory;

pub use actions::*;
pub use conditions::*;
pub use core::*;
pub use engine::*;
pub use events::*;
pub use executor::*;
pub use map_scripts::{MapMetadata, MapScriptLoader};
pub use rhai_bridge::RhaiScriptExecutor;
pub use script_actions::*;
pub use script_conditions::*;
pub use script_engine::*;
pub use scripts::*;
pub use triggers::{GameDifficulty, Trigger, TriggerMode, TriggerState, TriggerSystem};
pub use variables::{VariableScope, VariableScopeManager};
pub use victory::*;

/// Snapshot serialization trait for scripting data.
pub trait XferSnapshot {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus>;
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus>;
    fn load_post_process(&mut self) -> Result<(), XferStatus>;
}

/// Script execution priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScriptPriority {
    /// Background/ambient scripts
    Low = 0,
    /// Normal event scripts
    Normal = 1,
    /// Important gameplay scripts
    High = 2,
    /// Critical system scripts
    Critical = 3,
    /// Emergency/debug scripts
    Emergency = 4,
}

/// Script execution context
#[derive(Debug, Clone)]
pub struct ScriptContext {
    /// Current game time
    pub game_time: Duration,
    /// Active player
    pub active_player: Option<u32>,
    /// Script-specific variables
    pub variables: HashMap<String, ScriptValue>,
    /// Global game state
    pub game_state: GameStateContext,
}

/// Game state context for scripts
#[derive(Debug, Clone)]
pub struct GameStateContext {
    /// Current map name
    pub map_name: String,
    /// Game mode
    pub game_mode: String,
    /// Player information
    pub players: Vec<PlayerInfo>,
    /// Current objectives
    pub objectives: Vec<Objective>,
}

/// Player information for scripts
#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub id: u32,
    pub name: String,
    pub team: u32,
    pub color: String,
    pub is_human: bool,
    pub is_alive: bool,
    pub score: i32,
}

/// Game objective
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    pub id: String,
    pub name: String,
    pub description: String,
    pub completed: bool,
    pub hidden: bool,
    pub priority: i32,
}

/// Script value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScriptValue {
    /// Null/None value
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value
    Int(i64),
    /// Float value  
    Float(f64),
    /// String value
    String(String),
    /// 3D coordinate
    Coord3D([f32; 3]),
    /// Object ID reference
    ObjectId(u32),
    /// Player ID reference
    PlayerId(u32),
    /// Team name
    Team(String),
    /// Array of values
    Array(Vec<ScriptValue>),
    /// Object/map of values
    Object(HashMap<String, ScriptValue>),
}

/// Script execution result
#[derive(Debug, Clone)]
pub enum ScriptResult {
    /// Script executed successfully
    Success(Option<ScriptValue>),
    /// Script failed with error
    Failed(String),
    /// Script was skipped due to conditions
    Skipped,
    /// Script is waiting for condition
    Waiting,
    /// Script was cancelled
    Cancelled,
}

/// Script trigger types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScriptTrigger {
    /// Time-based trigger
    Timer(Duration),
    /// Event-based trigger
    Event(String),
    /// Condition becomes true
    Condition(String),
    /// Player action
    PlayerAction { player: u32, action: String },
    /// Object event
    ObjectEvent { object: u32, event: String },
    /// Area trigger
    AreaTrigger { area: String, event: String },
    /// Unit count threshold
    UnitCount {
        player: u32,
        unit_type: String,
        count: i32,
    },
    /// Resource threshold
    ResourceCount {
        player: u32,
        resource: String,
        amount: i32,
    },
}

/// Script definition
#[derive(Debug, Clone)]
pub struct Script {
    /// Unique script ID
    pub id: String,
    /// Script name for display
    pub name: String,
    /// Script source code
    pub source: String,
    /// Compiled AST (cached)
    pub ast: Option<AST>,
    /// Script priority
    pub priority: ScriptPriority,
    /// Execution triggers
    pub triggers: Vec<ScriptTrigger>,
    /// Prerequisites (other scripts that must run first)
    pub prerequisites: Vec<String>,
    /// Maximum execution count (0 = unlimited)
    pub max_executions: u32,
    /// Current execution count
    pub execution_count: u32,
    /// Whether script is enabled
    pub enabled: bool,
    /// Execution timeout
    pub timeout: Option<Duration>,
}

/// Script execution state
#[derive(Debug)]
struct ScriptExecution {
    /// Script being executed
    script: Script,
    /// Execution context
    context: ScriptContext,
    /// Start time
    start_time: Instant,
    /// Rhai execution scope
    scope: Scope<'static>,
}

/// Advanced scripting engine
pub struct ScriptingEngine {
    /// Rhai engine for script execution
    rhai_engine: Engine,
    /// Registered scripts
    scripts: Arc<RwLock<HashMap<String, Script>>>,
    /// Active script executions
    active_executions: Arc<RwLock<HashMap<String, ScriptExecution>>>,
    /// Event queue for triggers
    event_queue: Arc<Mutex<VecDeque<ScriptEvent>>>,
    /// Global script variables
    global_variables: Arc<RwLock<HashMap<String, ScriptValue>>>,
    /// Action registry
    action_registry: Arc<RwLock<ActionRegistry>>,
    /// Condition registry
    condition_registry: Arc<RwLock<ConditionRegistry>>,
    /// Victory condition manager
    victory_manager: Arc<RwLock<VictoryManager>>,
    /// Performance metrics
    metrics: Arc<RwLock<ScriptMetrics>>,
    /// External action handler (mission hooks)
    action_handler: Option<Arc<dyn ScriptActionHandler>>,
    /// Host-provided game state context for scripts (map name, players, objectives, etc.).
    host_game_state: Arc<RwLock<GameStateContext>>,
}

/// Script execution metrics
#[derive(Debug, Clone, Default)]
pub struct ScriptMetrics {
    /// Total scripts executed
    pub total_executions: u64,
    /// Total execution time
    pub total_execution_time: Duration,
    /// Average execution time
    pub avg_execution_time: Duration,
    /// Failed executions
    pub failed_executions: u64,
    /// Currently active scripts
    pub active_scripts: u32,
}

/// Script event for trigger system
#[derive(Debug, Clone)]
pub struct ScriptEvent {
    /// Event type
    pub event_type: String,
    /// Event parameters
    pub parameters: HashMap<String, ScriptValue>,
    /// Event timestamp
    pub timestamp: Instant,
    /// Event priority
    pub priority: ScriptPriority,
}

impl ScriptingEngine {
    /// Create a new scripting engine
    pub fn new() -> GameLogicResult<Self> {
        let mut rhai_engine = Engine::new();

        // Configure Rhai engine
        rhai_engine.set_max_operations(100_000); // Prevent infinite loops
        rhai_engine.set_max_string_size(10_000);
        rhai_engine.set_max_array_size(1_000);
        rhai_engine.set_max_map_size(1_000);

        let mut engine = Self {
            rhai_engine,
            scripts: Arc::new(RwLock::new(HashMap::new())),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
            global_variables: Arc::new(RwLock::new(HashMap::new())),
            action_registry: Arc::new(RwLock::new(ActionRegistry::new())),
            condition_registry: Arc::new(RwLock::new(ConditionRegistry::new())),
            victory_manager: Arc::new(RwLock::new(VictoryManager::new())),
            metrics: Arc::new(RwLock::new(ScriptMetrics::default())),
            action_handler: None,
            host_game_state: Arc::new(RwLock::new(GameStateContext {
                map_name: String::new(),
                game_mode: String::new(),
                players: Vec::new(),
                objectives: Vec::new(),
            })),
        };

        engine.initialize_engine()?;
        Ok(engine)
    }

    /// Initialize the scripting engine
    fn initialize_engine(&mut self) -> GameLogicResult<()> {
        // Register custom types
        self.register_custom_types()?;

        // Register built-in functions
        self.register_builtin_functions()?;

        // Register all game-specific functions (shared with `RhaiScriptExecutor`)
        crate::scripting::rhai_bridge::RhaiScriptExecutor::register_game_functions(
            &mut self.rhai_engine,
        )?;

        Ok(())
    }

    pub fn set_action_handler(&mut self, handler: Option<Arc<dyn ScriptActionHandler>>) {
        self.action_handler = handler;
    }

    /// Provide the current game state snapshot for scripts (host runtime integration).
    pub fn set_game_state_context(&self, context: GameStateContext) -> GameLogicResult<()> {
        let mut guard = self.host_game_state.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to write host game state: {}", e))
        })?;
        *guard = context;
        Ok(())
    }

    /// Register custom types with Rhai
    fn register_custom_types(&mut self) -> GameLogicResult<()> {
        self.rhai_engine
            .register_type_with_name::<Coord3D>("Coord3D");
        self.rhai_engine
            .register_fn("coord3d", |x: f64, y: f64, z: f64| -> Coord3D {
                Coord3D::new(x as f32, y as f32, z as f32)
            });
        self.rhai_engine
            .register_get("x", |v: &mut Coord3D| -> f64 { v.x as f64 });
        self.rhai_engine
            .register_get("y", |v: &mut Coord3D| -> f64 { v.y as f64 });
        self.rhai_engine
            .register_get("z", |v: &mut Coord3D| -> f64 { v.z as f64 });
        self.rhai_engine
            .register_set("x", |v: &mut Coord3D, x: f64| v.x = x as f32);
        self.rhai_engine
            .register_set("y", |v: &mut Coord3D, y: f64| v.y = y as f32);
        self.rhai_engine
            .register_set("z", |v: &mut Coord3D, z: f64| v.z = z as f32);

        Ok(())
    }

    /// Register built-in script functions
    fn register_builtin_functions(&mut self) -> GameLogicResult<()> {
        // Game state functions
        self.rhai_engine.register_fn("get_player_count", || -> i64 {
            crate::player::player_list()
                .read()
                .ok()
                .map(|players| players.get_player_count() as i64)
                .unwrap_or(0)
        });
        self.rhai_engine.register_fn("get_game_time", || -> f64 {
            let frame = crate::helpers::TheGameLogic::get_frame() as f64;
            frame / crate::common::LOGICFRAMES_PER_SECOND as f64
        });

        // Math functions
        self.rhai_engine
            .register_fn("distance_2d", |x1: f64, y1: f64, x2: f64, y2: f64| -> f64 {
                ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt()
            });

        // Utility functions
        self.rhai_engine.register_fn("print_debug", |msg: &str| {
            log::debug!("Script Debug: {}", msg);
        });

        Ok(())
    }

    /// Load script from source code
    pub async fn load_script(&self, script: Script) -> GameLogicResult<()> {
        // Compile the script
        let mut compiled_script = script;
        compiled_script.ast = Some(self.rhai_engine.compile(&compiled_script.source).map_err(
            |e| GameLogicError::Configuration(format!("Script compilation error: {}", e)),
        )?);

        // Store the script
        let mut scripts = self.scripts.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire scripts lock: {}", e))
        })?;
        scripts.insert(compiled_script.id.clone(), compiled_script);

        Ok(())
    }

    /// Execute script by ID
    pub async fn execute_script(
        &self,
        script_id: &str,
        context: ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Get the script
        let script = {
            let scripts = self.scripts.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire scripts lock: {}", e))
            })?;

            scripts
                .get(script_id)
                .ok_or_else(|| {
                    GameLogicError::Configuration(format!("Script not found: {}", script_id))
                })?
                .clone()
        };

        // Check if script is enabled and hasn't exceeded max executions
        if !script.enabled {
            return Ok(ScriptResult::Skipped);
        }

        if script.max_executions > 0 && script.execution_count >= script.max_executions {
            return Ok(ScriptResult::Skipped);
        }

        // Execute the script
        self.execute_script_impl(script, context).await
    }

    /// Internal script execution implementation
    async fn execute_script_impl(
        &self,
        mut script: Script,
        context: ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let start_time = Instant::now();

        // Prepare execution scope
        let mut scope = Scope::new();
        self.setup_script_scope(&mut scope, &context)?;

        // Execute the script
        let result = if let Some(ast) = &script.ast {
            match self
                .rhai_engine
                .eval_ast_with_scope::<Dynamic>(&mut scope, ast)
            {
                Ok(result) => {
                    script.execution_count += 1;
                    ScriptResult::Success(Some(self.dynamic_to_script_value(result)))
                }
                Err(e) => {
                    log::error!("Script execution error in '{}': {}", script.id, e);
                    ScriptResult::Failed(format!("Execution error: {}", e))
                }
            }
        } else {
            ScriptResult::Failed("Script not compiled".to_string())
        };

        // Update metrics
        let execution_time = start_time.elapsed();
        self.update_metrics(&result, execution_time).await?;

        // Update script execution count
        {
            let mut scripts = self.scripts.write().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire scripts lock: {}", e))
            })?;
            if let Some(stored_script) = scripts.get_mut(&script.id) {
                stored_script.execution_count = script.execution_count;
            }
        }

        Ok(result)
    }

    /// Setup script execution scope with context variables
    fn setup_script_scope(
        &self,
        scope: &mut Scope<'_>,
        context: &ScriptContext,
    ) -> GameLogicResult<()> {
        // Add context variables to scope
        scope.push("game_time", context.game_time.as_secs_f64());

        if let Some(player) = context.active_player {
            scope.push("active_player", player as i64);
        }

        self.push_game_state_to_scope(scope, &context.game_state)?;

        // Add script variables
        for (name, value) in &context.variables {
            scope.push_dynamic(name, self.script_value_to_dynamic(value));
        }

        // Add global variables
        let globals = self
            .global_variables
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read globals: {}", e)))?;
        for (name, value) in globals.iter() {
            scope.push_dynamic(name, self.script_value_to_dynamic(value));
        }

        Ok(())
    }

    fn push_game_state_to_scope(
        &self,
        scope: &mut Scope<'_>,
        game_state: &GameStateContext,
    ) -> GameLogicResult<()> {
        let mut state = RhaiMap::new();
        state.insert(
            "map_name".into(),
            Dynamic::from(game_state.map_name.clone()),
        );
        state.insert(
            "game_mode".into(),
            Dynamic::from(game_state.game_mode.clone()),
        );

        let mut players = RhaiArray::new();
        for p in &game_state.players {
            let mut player = RhaiMap::new();
            player.insert("id".into(), Dynamic::from(p.id as i64));
            player.insert("name".into(), Dynamic::from(p.name.clone()));
            player.insert("team".into(), Dynamic::from(p.team as i64));
            player.insert("color".into(), Dynamic::from(p.color.clone()));
            player.insert("is_human".into(), Dynamic::from(p.is_human));
            player.insert("is_alive".into(), Dynamic::from(p.is_alive));
            player.insert("score".into(), Dynamic::from(p.score));
            players.push(Dynamic::from(player));
        }
        state.insert("players".into(), Dynamic::from(players));

        let mut objectives = RhaiArray::new();
        for o in &game_state.objectives {
            let mut objective = RhaiMap::new();
            objective.insert("id".into(), Dynamic::from(o.id.clone()));
            objective.insert("name".into(), Dynamic::from(o.name.clone()));
            objective.insert("description".into(), Dynamic::from(o.description.clone()));
            objective.insert("completed".into(), Dynamic::from(o.completed));
            objective.insert("hidden".into(), Dynamic::from(o.hidden));
            objective.insert("priority".into(), Dynamic::from(o.priority));
            objectives.push(Dynamic::from(objective));
        }
        state.insert("objectives".into(), Dynamic::from(objectives));

        scope.push_dynamic("game_state", Dynamic::from(state));
        Ok(())
    }

    /// Process script events and triggers
    pub async fn process_events(&self) -> GameLogicResult<()> {
        // Process event queue
        let events = {
            let mut queue = self.event_queue.lock().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire event queue: {}", e))
            })?;

            let events: Vec<_> = queue.drain(..).collect();
            events
        };

        for event in events {
            self.process_event(event).await?;
        }

        Ok(())
    }

    /// Process a single script event
    async fn process_event(&self, event: ScriptEvent) -> GameLogicResult<()> {
        // Find scripts triggered by this event
        let triggered_scripts = self.find_triggered_scripts(&event).await?;

        // Execute triggered scripts
        for script_id in triggered_scripts {
            let game_time_seconds = crate::helpers::TheGameLogic::get_frame() as f64
                / crate::common::LOGICFRAMES_PER_SECOND as f64;
            let game_time = Duration::from_secs_f64(game_time_seconds.max(0.0));

            let game_state = self
                .host_game_state
                .read()
                .map_err(|e| {
                    GameLogicError::Threading(format!("Failed to read host game state: {}", e))
                })?
                .clone();

            let context = ScriptContext {
                game_time,
                active_player: None,
                variables: event.parameters.clone(),
                game_state,
            };

            if let Err(e) = self.execute_script(&script_id, context).await {
                log::error!("Failed to execute triggered script '{}': {}", script_id, e);
            }
        }

        Ok(())
    }

    /// Find scripts triggered by an event
    async fn find_triggered_scripts(&self, event: &ScriptEvent) -> GameLogicResult<Vec<String>> {
        let mut triggered = Vec::new();

        let scripts = self.scripts.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire scripts lock: {}", e))
        })?;

        for (script_id, script) in scripts.iter() {
            if self.script_triggered_by_event(script, event)? {
                triggered.push(script_id.clone());
            }
        }

        Ok(triggered)
    }

    /// Check if script is triggered by event
    fn script_triggered_by_event(
        &self,
        script: &Script,
        event: &ScriptEvent,
    ) -> GameLogicResult<bool> {
        for trigger in &script.triggers {
            match trigger {
                ScriptTrigger::Event(event_name) => {
                    if *event_name == event.event_type {
                        return Ok(true);
                    }
                }
                ScriptTrigger::PlayerAction { action, .. } => {
                    if event.event_type == "player_action" {
                        if let Some(ScriptValue::String(event_action)) =
                            event.parameters.get("action")
                        {
                            if action == event_action {
                                return Ok(true);
                            }
                        }
                    }
                }
                _ => {} // Handle other trigger types
            }
        }
        Ok(false)
    }

    /// Fire a script event
    pub async fn fire_event(&self, event: ScriptEvent) -> GameLogicResult<()> {
        self.fire_event_sync(event)
    }

    /// Fire a script event synchronously.
    pub fn fire_event_sync(&self, event: ScriptEvent) -> GameLogicResult<()> {
        let mut queue = self.event_queue.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire event queue: {}", e))
        })?;
        queue.push_back(event);
        let pending = queue.len();
        if pending >= 256 && pending % 128 == 0 {
            log::warn!(
                "Script event queue backlog: {} pending events before processing",
                pending
            );
        }
        Ok(())
    }

    /// Number of currently queued script events awaiting processing.
    pub fn pending_event_count(&self) -> usize {
        self.event_queue
            .lock()
            .map(|queue| queue.len())
            .unwrap_or_default()
    }

    /// Set global variable
    pub async fn set_global_variable(
        &self,
        name: String,
        value: ScriptValue,
    ) -> GameLogicResult<()> {
        let mut globals = self.global_variables.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire globals lock: {}", e))
        })?;
        globals.insert(name, value);
        Ok(())
    }

    /// Get global variable
    pub async fn get_global_variable(&self, name: &str) -> GameLogicResult<Option<ScriptValue>> {
        let globals = self.global_variables.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire globals lock: {}", e))
        })?;
        Ok(globals.get(name).cloned())
    }

    /// Update performance metrics
    async fn update_metrics(
        &self,
        result: &ScriptResult,
        execution_time: Duration,
    ) -> GameLogicResult<()> {
        let mut metrics = self.metrics.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire metrics lock: {}", e))
        })?;

        metrics.total_executions += 1;
        metrics.total_execution_time += execution_time;
        metrics.avg_execution_time = metrics.total_execution_time / metrics.total_executions as u32;

        if matches!(result, ScriptResult::Failed(_)) {
            metrics.failed_executions += 1;
        }

        Ok(())
    }

    /// Convert Rhai Dynamic to ScriptValue
    fn dynamic_to_script_value(&self, value: Dynamic) -> ScriptValue {
        if value.is_unit() {
            ScriptValue::Null
        } else if value.is::<bool>() {
            ScriptValue::Bool(value.cast::<bool>())
        } else if value.is::<i64>() {
            ScriptValue::Int(value.cast::<i64>())
        } else if value.is::<f64>() {
            ScriptValue::Float(value.cast::<f64>())
        } else if value.is::<String>() {
            ScriptValue::String(value.cast::<String>())
        } else if value.is::<Coord3D>() {
            let v = value.cast::<Coord3D>();
            ScriptValue::Coord3D([v.x, v.y, v.z])
        } else if value.is::<RhaiArray>() {
            let arr = value.cast::<RhaiArray>();
            ScriptValue::Array(
                arr.into_iter()
                    .map(|v| self.dynamic_to_script_value(v))
                    .collect(),
            )
        } else if value.is::<RhaiMap>() {
            let map = value.cast::<RhaiMap>();
            let mut out = HashMap::with_capacity(map.len());
            for (k, v) in map {
                out.insert(k.to_string(), self.dynamic_to_script_value(v));
            }
            ScriptValue::Object(out)
        } else {
            ScriptValue::Null
        }
    }

    fn script_value_to_dynamic(&self, value: &ScriptValue) -> Dynamic {
        match value {
            ScriptValue::Null => Dynamic::UNIT,
            ScriptValue::Bool(b) => Dynamic::from(*b),
            ScriptValue::Int(i) => Dynamic::from(*i),
            ScriptValue::Float(f) => Dynamic::from(*f),
            ScriptValue::String(s) => Dynamic::from(s.clone()),
            ScriptValue::Coord3D([x, y, z]) => Dynamic::from(Coord3D::new(*x, *y, *z)),
            ScriptValue::ObjectId(id) => Dynamic::from(*id as i64),
            ScriptValue::PlayerId(id) => Dynamic::from(*id as i64),
            ScriptValue::Team(team) => Dynamic::from(team.clone()),
            ScriptValue::Array(values) => Dynamic::from(
                values
                    .iter()
                    .map(|v| self.script_value_to_dynamic(v))
                    .collect::<RhaiArray>(),
            ),
            ScriptValue::Object(values) => {
                let mut map = RhaiMap::new();
                for (k, v) in values {
                    map.insert(k.into(), self.script_value_to_dynamic(v));
                }
                Dynamic::from(map)
            }
        }
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> GameLogicResult<ScriptMetrics> {
        let metrics = self.metrics.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire metrics lock: {}", e))
        })?;
        Ok(metrics.clone())
    }

    /// List all registered scripts
    pub async fn list_scripts(&self) -> GameLogicResult<Vec<String>> {
        let scripts = self.scripts.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire scripts lock: {}", e))
        })?;
        Ok(scripts.keys().cloned().collect())
    }

    /// Enable/disable script
    pub async fn set_script_enabled(&self, script_id: &str, enabled: bool) -> GameLogicResult<()> {
        let mut scripts = self.scripts.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire scripts lock: {}", e))
        })?;

        if let Some(script) = scripts.get_mut(script_id) {
            script.enabled = enabled;
            Ok(())
        } else {
            Err(GameLogicError::Configuration(format!(
                "Script not found: {}",
                script_id
            )))
        }
    }

    /// Clear all scripts
    pub async fn clear_scripts(&self) -> GameLogicResult<()> {
        let mut scripts = self.scripts.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire scripts lock: {}", e))
        })?;
        scripts.clear();
        Ok(())
    }
}

impl fmt::Display for ScriptValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptValue::Null => write!(f, "null"),
            ScriptValue::Bool(b) => write!(f, "{}", b),
            ScriptValue::Int(i) => write!(f, "{}", i),
            ScriptValue::Float(fl) => write!(f, "{}", fl),
            ScriptValue::String(s) => write!(f, "\"{}\"", s),
            ScriptValue::Coord3D(coord) => write!(f, "({}, {}, {})", coord[0], coord[1], coord[2]),
            ScriptValue::ObjectId(id) => write!(f, "Object({})", id),
            ScriptValue::PlayerId(id) => write!(f, "Player({})", id),
            ScriptValue::Team(team) => write!(f, "Team({})", team),
            ScriptValue::Array(arr) => {
                write!(f, "[")?;
                for (i, val) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", val)?;
                }
                write!(f, "]")
            }
            ScriptValue::Object(obj) => {
                write!(f, "{{")?;
                for (i, (key, val)) in obj.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", key, val)?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl Default for ScriptingEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create default scripting engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scripting_engine_creation() {
        let engine = ScriptingEngine::new().unwrap();
        let metrics = engine.get_metrics().await.unwrap();
        assert_eq!(metrics.total_executions, 0);
    }

    #[tokio::test]
    async fn test_script_loading() {
        let engine = ScriptingEngine::new().unwrap();

        let script = Script {
            id: "test_script".to_string(),
            name: "Test Script".to_string(),
            source: "print_debug(\"Hello from script!\"); true".to_string(),
            ast: None,
            priority: ScriptPriority::Normal,
            triggers: vec![],
            prerequisites: vec![],
            max_executions: 0,
            execution_count: 0,
            enabled: true,
            timeout: None,
        };

        engine.load_script(script).await.unwrap();

        let scripts = engine.list_scripts().await.unwrap();
        assert!(scripts.contains(&"test_script".to_string()));
    }

    #[tokio::test]
    async fn test_script_execution() {
        let engine = ScriptingEngine::new().unwrap();

        let script = Script {
            id: "test_exec".to_string(),
            name: "Test Execution".to_string(),
            source: "let result = 2 + 3; result".to_string(),
            ast: None,
            priority: ScriptPriority::Normal,
            triggers: vec![],
            prerequisites: vec![],
            max_executions: 0,
            execution_count: 0,
            enabled: true,
            timeout: None,
        };

        engine.load_script(script).await.unwrap();

        let context = ScriptContext {
            game_time: Duration::from_secs(100),
            active_player: Some(1),
            variables: HashMap::new(),
            game_state: GameStateContext {
                map_name: "TestMap".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        let result = engine.execute_script("test_exec", context).await.unwrap();
        assert!(matches!(result, ScriptResult::Success(_)));
    }

    #[tokio::test]
    async fn test_global_variables() {
        let engine = ScriptingEngine::new().unwrap();

        engine
            .set_global_variable("test_var".to_string(), ScriptValue::Int(42))
            .await
            .unwrap();

        let value = engine.get_global_variable("test_var").await.unwrap();
        assert_eq!(value, Some(ScriptValue::Int(42)));
    }

    #[test]
    fn test_script_value_display() {
        let val = ScriptValue::Array(vec![
            ScriptValue::Int(1),
            ScriptValue::String("test".to_string()),
            ScriptValue::Bool(true),
        ]);

        let display = format!("{}", val);
        assert!(display.contains("1"));
        assert!(display.contains("test"));
        assert!(display.contains("true"));
    }
}
