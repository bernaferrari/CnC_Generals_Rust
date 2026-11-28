// Script-engine types, RefCell inner state, and re-entrancy accessors.
//
// Included into `engine/mod.rs` so private fields and the current
// RefCell / TLS re-entrancy pattern stay in the parent module.

pub use generals_sim::scripting::world::{
    ScriptAiPlayerRequest,
    ScriptAudioRequest,
    ScriptBridgeStatus,
    ScriptCameraRequest,
    ScriptContainmentStatus,
    ScriptDisplayRequest,
    ScriptMapVisibilityOperation,
    ScriptNamedCommand,
    ScriptNamedLifecycle,
    ScriptNamedRuntimeRequest,
    ScriptNearestObjectRequest,
    ScriptNearestRecipient,
    ScriptObjectCreation,
    ScriptObjectStatus,
    ScriptOwnerQuery,
    ScriptPlayerAreaCensus,
    ScriptPlayerAreaQuery,
    ScriptPlayerCensusScope,
    ScriptPlayerTeamRelationship,
    ScriptSelectionStatus,
    ScriptSkirmishCensus,
    ScriptSpecialPowerReadiness,
    ScriptSupplyQuery,
    ScriptTeamAreaMembership,
    ScriptTeamAreaTransition,
    ScriptTeamPresence,
    ScriptTeamRelationship,
    ScriptTeamStatus,
    ScriptWaterRequest,
    ScriptWorldRequest,
    script_team_is_contained,
};

pub use generals_sim::scripting::driver::ScriptExecutionDriver;

pub(crate) struct CanonicalScriptExecutionDriver;
impl ScriptExecutionDriver for CanonicalScriptExecutionDriver {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
}

struct ScriptExecution<'a> {
    frame: u32,
    context: &'a std::cell::RefCell<ScriptContext>,
    driver: &'a mut dyn ScriptExecutionDriver,
}

impl<'a> ScriptExecution<'a> {
    fn new(
        context: &'a std::cell::RefCell<ScriptContext>,
        driver: &'a mut dyn ScriptExecutionDriver,
    ) -> Self {
        Self {
            frame: context.borrow().current_frame,
            context,
            driver,
        }
    }
}

/// C++ Scripts.h skirmish trigger names.
pub(crate) use generals_sim::scripting::{
    ENEMY_INNER_PERIMETER, ENEMY_OUTER_PERIMETER, INNER_PERIMETER, MY_INNER_PERIMETER,
    MY_OUTER_PERIMETER, OUTER_PERIMETER,
};

/// C++ ScriptEngine::getQualifiedTriggerAreaByName rewrite (ScriptEngine.cpp:5888-5916).
pub(crate) fn qualify_trigger_area_name(
    area_name: &str,
    current_player_name: Option<&str>,
) -> Option<String> {
    let Some(player_name) = current_player_name else {
        return generals_sim::scripting::qualify_trigger_area_without_player(area_name)
            .map(str::to_owned);
    };

    if area_name == MY_INNER_PERIMETER || area_name == MY_OUTER_PERIMETER {
        let __player_list_arc = crate::player::player_list();
            let ndx = __player_list_arc.read()
            .ok()
            .and_then(|list| list.find_player_by_name(player_name))
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|guard| guard.get_mp_start_index() + 1)
            })?;
        if area_name == MY_INNER_PERIMETER {
            return Some(format!("{INNER_PERIMETER}{ndx}"));
        }
        return Some(format!("{OUTER_PERIMETER}{ndx}"));
    }

    if area_name == ENEMY_INNER_PERIMETER || area_name == ENEMY_OUTER_PERIMETER {
        let mut mp_ndx = -1;
        let player_list = crate::player::player_list();
        if let Ok(list) = player_list.read() {
            if let Some(player) = list.find_player_by_name(player_name) {
                if let Ok(guard) = player.read() {
                    if let Some(enemy_index) = guard.get_current_enemy_player_index() {
                        if let Some(enemy) = list.get_player(enemy_index) {
                            if let Ok(enemy_guard) = enemy.read() {
                                mp_ndx = enemy_guard.get_mp_start_index() + 1;
                            }
                        }
                    }
                }
            }
        }
        if area_name == ENEMY_INNER_PERIMETER {
            return Some(format!("{INNER_PERIMETER}{mp_ndx}"));
        }
        return Some(format!("{OUTER_PERIMETER}{mp_ndx}"));
    }

    Some(area_name.to_string())
}

/// Script execution statistics
#[derive(Debug, Clone, Default)]
pub struct ScriptStats {
    pub num_frames: f64,
    pub total_update_time: f64,
    pub max_update_time: f64,
    pub cur_update_time: f64,
}

/// Legacy gameplay and Xfer adapter for the receiving scripting runtime.
///
/// Adapter state lives in `RefCell<ScriptEngineInner>` so nested
/// `CALL_SUBROUTINE` (C++ runs the callee immediately) can re-enter through
/// `&ScriptEngine` + `lock_inner_mut` without minting a second `&mut ScriptEngine`.
/// `RefCell` enforces exclusive/shared borrows; `with_inner` cannot alias a
/// live `lock_inner_mut` (that used to be a comment-only rule over `UnsafeCell`).
pub struct ScriptEngine {
    /// Roster of the world that constructed this engine, for Core rank actions.
    players: Arc<RwLock<crate::player::PlayerList>>,
    team_factory: crate::team::TeamFactoryHandle,
    common: Arc<game_engine::common::common_services::CommonServices>,
    /// Exact world owners for Core cave-index actions and Xfer fixups.
    cave_system: Arc<Mutex<crate::system::cave_system::CaveSystem>>,
    object_registry: Arc<crate::object::registry::ObjectRegistry>,
    /// Waypoints for standalone script actions belong to this engine's terrain.
    terrain: Arc<RwLock<crate::terrain::TerrainLogic>>,
    /// Core's named object table, also read during legacy Xfer.
    named_objects: Arc<NamedObjectTracker>,
    /// C++ GarrisonContain named evac dispositions of this engine's world.
    named_evac: Arc<Mutex<std::collections::HashMap<String, u32>>>,
    /// C++ TheEva of the world that built this engine (Eva.cpp setEnabled).
    eva: Arc<Mutex<crate::helpers::EvaState>>,
    /// The world that built this engine (C++ its GameEngine's TheInGameUI).
    owner_world: std::sync::Weak<crate::system::engine_stores::WorldServices>,
    /// The resolved ControlBar catalog slot of the world that built this engine.
    control_bar_bridge: Arc<RwLock<Option<Arc<crate::control_bar::ControlBarBridge>>>>,
    control_bar_ui_hooks: Arc<Mutex<Option<Arc<dyn crate::control_bar::ControlBarUiHooks>>>>,
    runtime: ScriptInterpreter,
    inner: std::cell::RefCell<ScriptEngineInner>,
}

/// State still interpreted by the standalone and presentation adapters.
pub struct ScriptEngineInner {
    templates: ScriptTemplateCatalog,
    #[cfg(feature = "script_profiling")]
    stats: ScriptStats,

}

/// RAII exclusive borrow of `ScriptEngineInner` from a shared `&ScriptEngine`.
///
/// Dropping the `RefMut` ends the exclusive borrow so nested `lock_inner_mut`
/// can run after the outer section finishes.
struct InnerMutGuard<'a> {
    inner: std::cell::RefMut<'a, ScriptEngineInner>,
}

impl Deref for InnerMutGuard<'_> {
    type Target = ScriptEngineInner;
    fn deref(&self) -> &ScriptEngineInner {
        &self.inner
    }
}

impl DerefMut for InnerMutGuard<'_> {
    fn deref_mut(&mut self) -> &mut ScriptEngineInner {
        &mut self.inner
    }
}

impl ScriptEngine {
    /// Match active read's failure policy without selecting an ambient engine.
    pub(crate) fn with_execution_read<R>(&self, f: impl FnOnce(&Self) -> R) -> Option<R> {
        if self.inner.try_borrow().is_err() || !self.runtime.can_read_state() {
            None
        } else {
            Some(f(self))
        }
    }

    /// The running simulation interpreter, shared with receiving world callbacks.
    pub fn interpreter(&self) -> &ScriptInterpreter {
        &self.runtime
    }

    pub(crate) fn standalone_named_object_id(&self, name: &str) -> Option<ObjectID> {
        self.named_objects.get_object_id(name).ok().flatten()
    }

    /// Scoped shared read of inner state. The `&ScriptEngineInner` cannot escape `f`.
    ///
    /// Panics if an `InnerMutGuard` is live (`RefCell` already borrowed).
    pub fn with_inner<R>(&self, f: impl FnOnce(&ScriptEngineInner) -> R) -> R {
        f(&self.inner.borrow())
    }

    /// Exclusive inner access from `&mut ScriptEngine`. Safe: no other alias exists.
    pub fn with_inner_mut<R>(&mut self, f: impl FnOnce(&mut ScriptEngineInner) -> R) -> R {
        f(self.inner.get_mut())
    }

    /// Exclusive inner borrow from `&ScriptEngine` (nested CALL_SUBROUTINE path).
    ///
    /// Panics if another `InnerMutGuard` or `with_inner` borrow is live.
    /// Callers must drop the guard before `dispatcher.execute_action` / nested
    /// `with_script_engine_mut`, matching C++ immediate nested execution.
    fn lock_inner_mut(&self) -> InnerMutGuard<'_> {
        InnerMutGuard {
            inner: self.inner.borrow_mut(),
        }
    }
}

/// Exclusive handle to the process-global script engine.
///
/// `read`/`write` are both exclusive (`Mutex`) so `ScriptEngine` does not
/// need to be `Sync`. Call sites keep `.read()` / `.write()`.
#[derive(Clone)]
pub struct ScriptEngineHandle {
    inner: Arc<Mutex<Option<ScriptEngine>>>,
}

impl ScriptEngineHandle {
    pub fn from_engine(engine: ScriptEngine) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(engine))),
        }
    }

    pub fn write(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, Option<ScriptEngine>>> {
        self.inner.lock()
    }

    pub fn read(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, Option<ScriptEngine>>> {
        self.inner.lock()
    }

    pub fn try_write(
        &self,
    ) -> std::sync::TryLockResult<std::sync::MutexGuard<'_, Option<ScriptEngine>>> {
        self.inner.try_lock()
    }

    pub fn try_read(
        &self,
    ) -> std::sync::TryLockResult<std::sync::MutexGuard<'_, Option<ScriptEngine>>> {
        self.inner.try_lock()
    }
}

/// Non-owning reference to a world's script engine slot.
#[derive(Clone)]
pub struct WeakScriptEngineHandle {
    inner: std::sync::Weak<Mutex<Option<ScriptEngine>>>,
}

impl WeakScriptEngineHandle {
    pub fn upgrade(&self) -> Option<ScriptEngineHandle> {
        self.inner.upgrade().map(|inner| ScriptEngineHandle { inner })
    }
}

impl ScriptEngineHandle {
    pub fn downgrade(&self) -> WeakScriptEngineHandle {
        WeakScriptEngineHandle { inner: Arc::downgrade(&self.inner) }
    }

    /// An empty slot: C++ `TheScriptEngine` before `init()`.
    pub fn empty() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }
}

/// Per-world C++ `TheScriptEngine` slot plus the script side tables that C++
/// keeps on the engine (named objects, trigger areas, event fan-out).
///
/// Owned by [`crate::system::engine_stores::WorldServices`]; each world has
/// its own, so two worlds never observe each other's scripts or names.
pub struct WorldScriptState {
    engine: ScriptEngineHandle,
    events: Arc<EventManager>,
    named_objects: Arc<NamedObjectTracker>,
    areas: Arc<AreaTracker>,
}

impl Default for WorldScriptState {
    fn default() -> Self {
        Self {
            engine: ScriptEngineHandle::empty(),
            events: Arc::new(EventManager::new()),
            named_objects: Arc::new(NamedObjectTracker::new()),
            areas: Arc::new(AreaTracker::new()),
        }
    }
}

impl WorldScriptState {
    pub fn engine(&self) -> &ScriptEngineHandle {
        &self.engine
    }
    pub fn events(&self) -> &Arc<EventManager> {
        &self.events
    }
    pub fn named_objects(&self) -> &Arc<NamedObjectTracker> {
        &self.named_objects
    }
    pub fn areas(&self) -> &Arc<AreaTracker> {
        &self.areas
    }

    /// C++ `TheScriptEngine->init()` for this world. Idempotent: a live
    /// engine is never replaced (repeat init must not drop script state).
    pub(crate) fn initialize_engine(
        &self,
        services: &Arc<crate::system::engine_stores::WorldServices>,
    ) -> GameLogicResult<()> {
        let mut slot = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire script engine lock: {}", e))
        })?;
        if slot.is_none() {
            *slot = Some(ScriptEngine::new_with_services(services)?);
        }
        Ok(())
    }
}

/// Script state of the published world, if any.
fn published_world_scripts() -> Option<Arc<crate::system::engine_stores::WorldServices>> {
    crate::system::engine_stores::try_active_services()
}

fn required_world_scripts(what: &str) -> Arc<crate::system::engine_stores::WorldServices> {
    published_world_scripts().unwrap_or_else(|| {
        panic!("{what} requires with_world_services / install_services (no process bag)")
    })
}

// Re-entrancy for nested CALL_SUBROUTINE / timer / flag mutations.
// std::sync::RwLock is not re-entrant: holding write while nested code
// calls get_script_engine().write()/read() deadlocks the host thread.
// Campaign maps (MD_USA01) hang on frame-0 "SUB-Generate Random Number"
// which CALL_SUBROUTINEs while the outer execute still holds the lock.
//
// Scoped TLS carries a checked, lexical `&ScriptEngine` through nested script
// execution.  This is intentionally not a raw pointer: `scoped_tls::set`
// restores the prior value during normal return and unwinding, while nested
// sets restore the outer engine.  C++ `callSubroutine` → `executeScript` runs
// immediately; we retain that order rather than queueing work.
scoped_tls::scoped_thread_local!(static ACTIVE_SCRIPT_ENGINE: ScriptEngine);

impl ScriptEngine {
    /// Run `f` with this engine installed as the current lexical nested target.
    /// `scoped_tls` restores an outer active engine even if `f` panics.
    pub(crate) fn with_active<R>(&self, f: impl FnOnce() -> R) -> R {
        ACTIVE_SCRIPT_ENGINE.set(self, f)
    }

    #[cfg(test)]
    pub(crate) fn with_active_for_test<R>(&self, f: impl FnOnce() -> R) -> R {
        self.with_active(f)
    }

}

/// Run `f` against the currently executing ScriptEngine (nested path), if any.
///
/// Hands out `&ScriptEngine` only. Nested mutations use `lock_inner_mut`
/// (no aliased `&mut ScriptEngine`). C++ `CALL_SUBROUTINE` runs immediately.
pub fn with_active_script_engine_mut<R>(f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
    ACTIVE_SCRIPT_ENGINE
        .is_set()
        .then(|| ACTIVE_SCRIPT_ENGINE.with(f))
}

/// Run `f` against the currently executing ScriptEngine (shared view), if any.
///
/// Skip if an exclusive adapter or runtime state borrow is active.
pub fn with_active_script_engine_ref<R>(f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
    if !ACTIVE_SCRIPT_ENGINE.is_set() {
        return None;
    }
    ACTIVE_SCRIPT_ENGINE.with(|engine| engine.with_execution_read(f))
}

/// Whether the current thread is evaluating a live `ScriptEngine` scope.
///
/// Callers that carry an alternate `ScriptEngineHandle` must distinguish an
/// absent active scope from an active scope whose interior state cannot be
/// borrowed at that instant.  The former may safely use their own handle; the
/// latter must fail closed rather than re-locking the active global engine.
pub fn is_script_engine_active() -> bool {
    ACTIVE_SCRIPT_ENGINE.is_set()
}

/// Mutate the global ScriptEngine with re-entrant nesting support.
///
/// Prefers the TLS active engine (no lock). Otherwise acquires the global
/// write lock (thread exclusion; mutations are interior via `lock_inner_mut`),
/// installs TLS for the duration of `f`, and runs `f` with `&ScriptEngine`.
///
/// Nested `with_script_engine_mut` from inside `f` runs **immediately**
/// (C++ `ScriptEngine::callSubroutine` → `executeScript` order).
pub fn with_script_engine_mut<R>(f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
    // Manual branch so we do not consume `f` when the active path is empty.
    if ACTIVE_SCRIPT_ENGINE.is_set() {
        return with_active_script_engine_mut(f);
    }
    let arc = get_script_engine();
    let guard = arc.write().ok()?;
    let engine = guard.as_ref()?;
    Some(engine.with_active(|| f(engine)))
}

/// Read the global ScriptEngine with re-entrant nesting support.
pub fn with_script_engine_ref<R>(f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
    if ACTIVE_SCRIPT_ENGINE.is_set() {
        return with_active_script_engine_ref(f);
    }
    let arc = get_script_engine();
    let guard = arc.read().ok()?;
    let engine = guard.as_ref()?;
    Some(f(engine))
}

/// Initialize the published world's script engine (C++ `TheScriptEngine->init`).
pub fn initialize_script_engine() -> GameLogicResult<()> {
    let services = published_world_scripts().ok_or_else(|| {
        GameLogicError::Threading("initialize_script_engine: no world published".to_string())
    })?;
    services.scripts().initialize_engine(&services)
}

/// The published world's script engine slot.
///
/// Transitional ambient path: prefer `WorldServices::scripts().engine()` from
/// an explicit owner. With no world published this is an empty slot, which
/// reads exactly like C++ `TheScriptEngine == NULL`.
pub fn get_script_engine() -> ScriptEngineHandle {
    published_world_scripts()
        .map(|services| services.scripts().engine().clone())
        .unwrap_or_else(ScriptEngineHandle::empty)
}

/// The published world's script event manager.
pub fn get_event_manager() -> Arc<EventManager> {
    Arc::clone(required_world_scripts("EventManager").scripts().events())
}

/// The published world's named object tracker.
pub fn get_named_object_tracker() -> Arc<NamedObjectTracker> {
    Arc::clone(
        required_world_scripts("NamedObjectTracker")
            .scripts()
            .named_objects(),
    )
}

/// Transfer a script-visible object name to another object (C++ ScriptEngine::transferObjectName).
pub fn transfer_object_name(
    from_name: &AsciiString,
    to_object_id: ObjectID,
) -> GameLogicResult<()> {
    // Wave 348: empty dual-world → Ok(()).
    if dual_world_registry_unavailable() {
        return Ok(());
    }

    OBJECT_REGISTRY
        .with_object_mut(to_object_id, |object| {
            transfer_object_name_to_object(from_name, object)
        })
        .ok_or(GameLogicError::InvalidObject(to_object_id))?
}

/// C++ transfers the name through its existing destination pointer. Callers
/// already holding that object must not resolve and lock it again by ID.
pub(crate) fn transfer_object_name_to_object(
    from_name: &AsciiString,
    destination: &mut crate::object::Object,
) -> GameLogicResult<()> {
    if from_name.is_empty() {
        return Ok(());
    }
    let tracker = get_named_object_tracker();
    if let Ok(Some(old_id)) = tracker.get_object_id(from_name.as_str()) {
        let _ = tracker.unregister_object(old_id);
    }
    destination.set_name(from_name.clone());
    tracker.register_named_object(from_name.to_string(), destination.get_id())?;
    Ok(())
}

/// The published world's trigger-area tracker.
pub fn get_area_tracker() -> Arc<AreaTracker> {
    Arc::clone(required_world_scripts("AreaTracker").scripts().areas())
}
