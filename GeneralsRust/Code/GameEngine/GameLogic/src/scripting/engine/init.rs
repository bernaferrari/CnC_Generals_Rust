/// Restores only callback fields: conditionTeam has a separate lifetime.
struct CallingContextRestore<'a> {
    engine: &'a ScriptEngine,
    calling_team: Option<TeamID>,
    current_player: Option<String>,
}
impl Drop for CallingContextRestore<'_> {
    fn drop(&mut self) {
        let mut inner = self.engine.lock_inner_mut();
        inner.calling_team = self.calling_team;
        inner.current_player = self.current_player.clone();
    }
}

// ScriptEngine construction, templates, script lists, and reset
//
// Split from `scripting/engine.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEngine {
    /// CPP ScriptEngine5975: THIS_OBJECT prefers the calling object, then the
    /// condition object. Return owned identity before crossing owner callbacks.
    pub(crate) fn script_object_id(&self) -> Option<ObjectID> {
        self.with_inner(|inner| inner.calling_object.or(inner.condition_object))
    }

    const MAX_PLAYER_COUNT: usize = crate::common::MAX_PLAYER_COUNT;
    /// Roster associated with this interpreter at construction.
    pub fn players(&self) -> &Arc<RwLock<crate::player::PlayerList>> {
        &self.players
    }

    /// C++ TheControlBar of the world that built this engine (its current
    /// resolved catalog; refreshed in that world's slot).
    pub(crate) fn control_bar_bridge(&self) -> Option<Arc<crate::control_bar::ControlBarBridge>> {
        self.control_bar_bridge.read().ok().and_then(|guard| guard.clone())
    }

    /// This world's ControlBar slot itself (script button overrides write it).
    pub(crate) fn control_bar_bridge_slot(
        &self,
    ) -> Arc<RwLock<Option<Arc<crate::control_bar::ControlBarBridge>>>> {
        Arc::clone(&self.control_bar_bridge)
    }

    /// This world's ControlBar UI hooks (C++ TheControlBar->markUIDirty).
    pub(crate) fn control_bar_ui_hooks(
        &self,
    ) -> Arc<Mutex<Option<Arc<dyn crate::control_bar::ControlBarUiHooks>>>> {
        Arc::clone(&self.control_bar_ui_hooks)
    }

    /// The world that built this engine, while it lives.
    pub fn owning_world_services(
        &self,
    ) -> Option<Arc<crate::system::engine_stores::WorldServices>> {
        self.owner_world.upgrade()
    }

    /// C++ this engine's host facts: the snapshot of the world that built it.
    pub(crate) fn host_query(&self) -> crate::scripting::HostScriptQuery {
        crate::scripting::HostScriptQuery::from_world(self.owning_world_services())
    }

    /// The Eva of the world that built this engine.
    pub(crate) fn eva_owner(&self) -> &Mutex<crate::helpers::EvaState> {
        &self.eva
    }

    /// The named evac disposition map of the world that built this engine.
    pub(crate) fn named_evac_owner(&self) -> &Mutex<std::collections::HashMap<String, u32>> {
        &self.named_evac
    }

    pub(crate) fn cave_action_owners(
        &self,
    ) -> (&NamedObjectTracker, &crate::object::registry::ObjectRegistry, &Mutex<crate::system::cave_system::CaveSystem>) {
        (&self.named_objects, &self.object_registry, &self.cave_system)
    }

    /// A script engine on a fresh private world. It never adopts the world
    /// published on the calling thread; callers that belong to a world use
    /// [`Self::new_with_services`] with that world.
    pub fn new() -> GameLogicResult<Self> {
        Self::new_with_services(&crate::system::engine_stores::new_world_services())
    }

    /// Fixture/probe convenience: build on the world published on this
    /// thread, or a private world keyed by the published Common names when
    /// none is. Gameplay code names its world with [`Self::new_with_services`].
    pub fn new_in_published_world() -> GameLogicResult<Self> {
        if let Some(services) = crate::system::engine_stores::try_active_services() {
            return Self::new_with_services(&services);
        }
        let services = crate::system::engine_stores::new_world_services();
        match game_engine::common::common_services::try_common_services() {
            Some(common) => {
                let mut names = common.name_key_namespace.lock().unwrap_or_else(|p| p.into_inner());
                Self::new_with_names(&services, &mut names)
            }
            None => Self::new_with_services(&services),
        }
    }

    /// Build on the specified world and its name namespace.
    pub fn new_with_services(
        services: &std::sync::Arc<crate::system::engine_stores::WorldServices>,
    ) -> GameLogicResult<Self> {
        let mut names = services.common().name_key_namespace.lock().unwrap_or_else(|p| p.into_inner());
        Self::new_with_names(services, &mut names)
    }

    fn new_with_names(
        services: &std::sync::Arc<crate::system::engine_stores::WorldServices>,
        names: &mut game_engine::common::name_key_generator::NameKeyNamespace,
    ) -> GameLogicResult<Self> {
        let mut engine = Self {
            players: Arc::clone(services.players()),
            team_factory: services.team_factory().clone(),
            common: Arc::clone(services.common()),
            cave_system: Arc::clone(&services.logic_managers().cave),
            object_registry: Arc::clone(services.object_registry()),
            named_objects: Arc::clone(services.scripts().named_objects()),
            named_evac: Arc::clone(services.named_evac_disposition()),
            eva: Arc::clone(services.eva()),
            owner_world: Arc::downgrade(services),
            control_bar_bridge: Arc::clone(services.control_bar_bridge()),
            control_bar_ui_hooks: Arc::clone(services.control_bar_ui_hooks()),
            side_scripts: SideScripts::new(Self::MAX_PLAYER_COUNT),
            sequential_scripts: SequentialScripts::default(),
            script_events: ScriptEvents::new(Self::MAX_PLAYER_COUNT),
            inner: std::cell::RefCell::new(ScriptEngineInner {
                templates: ScriptTemplateCatalog::new(names),

                frame: ScriptFrame::default(),
                attack_priorities: AttackPriorityTable::new(),
                receiving_transport_statuses: HashMap::new(),

                calling_team: None,
                calling_object: None,
                condition_team: None,
                condition_object: None,
                current_player: None,
                skirmish_human_player: None,
                current_track_name: String::new(),

                frame_object_count_changed: 0,
                object_counts: HashMap::new(),
                object_types: HashMap::new(),

                topple_directions: Vec::new(),
                named_reveals: NamedReveals::default(),
                breeze_info: BreezeInfo::new(),
                game_difficulty: crate::player::GameDifficulty::Normal,

                freeze_by_script: false,
                objects_should_receive_difficulty_bonus: true,
                choose_victim_always_uses_normal: false,
                shown_mp_local_defeat_window: false,
                suppress_new_windows: false,
                win_lose_window_layout: None,

                campaign_victorious: false,


                #[cfg(feature = "script_profiling")]
                stats: ScriptStats::default(),

                action_handler: None,
            }),
        };

        {
            let inner = engine.inner.get_mut();
            inner.frame.variables.reserve_empty_slots();
            inner.attack_priorities.ensure_default();
        }

        Ok(engine)
    }

    pub fn get_frame_object_count_changed(&self) -> u32 {
        self.with_inner(|inner| inner.frame_object_count_changed)
    }

    /// Owned snapshot — never a borrow into `UnsafeCell`.
    pub fn get_current_player_name(&self) -> Option<String> {
        self.with_inner(|inner| inner.current_player.clone())
    }

    pub fn get_calling_team_id(&self) -> Option<TeamID> {
        self.with_inner(|inner| inner.calling_team)
    }

    pub fn get_condition_team_id(&self) -> Option<TeamID> {
        self.with_inner(|inner| inner.condition_team)
    }

    /// Name projection for standalone Core token adapters. Production drivers
    /// receive IDs directly; names never become mutable context authority.
    pub fn get_calling_team_name(&self) -> Option<String> {
        Self::standalone_team_name(self.get_calling_team_id())
    }

    pub fn get_condition_team_name(&self) -> Option<String> {
        Self::standalone_team_name(self.get_condition_team_id())
    }

    fn standalone_team_name(id: Option<TeamID>) -> Option<String> {
        let published_team_factory = get_team_factory();
        let team = published_team_factory.lock().ok()?.find_team_by_id(id?)?;
        team.read().ok().map(|team| team.get_name().to_string())
    }

    pub(crate) fn standalone_context_team(
        &self,
        raw: &str,
    ) -> Option<Arc<RwLock<crate::team::Team>>> {
        let (calling, condition) =
            self.with_inner(|inner| (inner.calling_team, inner.condition_team));
        let published_team_factory = get_team_factory();
        let factory = published_team_factory.lock().ok()?;
        if raw == crate::scripting::core::THIS_TEAM {
            return factory.find_team_by_id(calling.or(condition)?);
        }
        for id in [calling, condition].into_iter().flatten() {
            if let Some(team) = factory.find_team_by_id(id) {
                let matches = team.read().ok().is_some_and(|team| team.get_name() == raw);
                if matches {
                    return Some(team);
                }
            }
        }
        let team = factory.find_team_instances(raw).into_iter().next()?;
        if factory
            .find_team_prototype(raw)
            .is_some_and(|proto| proto.is_singleton())
            && !team.read().ok().is_some_and(|team| team.is_active())
        {
            return None;
        }
        Some(team)
    }

    fn standalone_team_player(&self, id: TeamID) -> Option<String> {
        let team = self.team_factory.lock().ok()?.find_team_by_id(id)?;
        let player_id = team.read().ok()?.get_controlling_player_id()?;
        let player = self.players.read().ok()?.get_player(player_id as i32).cloned()?;
        let key = player.read().ok()?.get_player_name_key();
        self.common.name_key_namespace.lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .key_to_name(key)
    }

    /// C++ friend_executeAction scopes the actual team and its controller,
    /// preserving conditionTeam. No borrow survives synchronous callbacks.
    pub fn friend_execute_action(&self, action: &ScriptAction, team_id: Option<TeamID>) {
        let mut driver = CanonicalScriptExecutionDriver;
        self.friend_execute_action_with_driver(
            action,
            team_id,
            crate::scripting::executor::ScriptContext::new(),
            &mut driver,
        );
    }

    pub fn friend_execute_action_with_driver(
        &self,
        action: &ScriptAction,
        team_id: Option<TeamID>,
        context: crate::scripting::executor::ScriptContext,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) {
        // Standalone callers keep their established reporting policy. Native
        // constructor callbacks use the checked entry and unwind on failure.
        if let Err(err) =
            self.friend_execute_action_with_driver_checked(action, team_id, context, driver)
        {
            log::warn!("friend_execute_action: {}", err);
        }
    }

    pub fn friend_execute_action_with_driver_checked(
        &self,
        action: &ScriptAction,
        team_id: Option<TeamID>,
        context: crate::scripting::executor::ScriptContext,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<()> {
        self.observe_object_count_changed_frame(driver);
        let current_player = team_id.and_then(|id| match driver.team_current_player(id) {
            ScriptOwnerQuery::Present(player) => player,
            ScriptOwnerQuery::Missing => None,
            ScriptOwnerQuery::Unavailable => self.standalone_team_player(id),
        });
        let restore = self.with_inner(|inner| CallingContextRestore {
            engine: self,
            calling_team: inner.calling_team,
            current_player: inner.current_player.clone(),
        });
        {
            let mut inner = self.lock_inner_mut();
            inner.calling_team = team_id;
            inner.current_player = current_player;
        }
        let context = std::cell::RefCell::new(context);
        let mut execution = ScriptExecution::new(&context, driver);
        let result = self.with_active(|| {
            let mut program = ScriptProgramWorld::new(self, &mut execution);
            program_execution::execute_action_chain(Some(action), &mut program).map(|_| ())
        });
        drop(restore);
        result
    }

    /// Set temporary runtime context used by external script evaluation helpers.
    ///
    /// Returns the previous `(current_player, condition_team)` tuple so callers can restore it.
    pub fn set_external_eval_context(
        &mut self,
        current_player: Option<String>,
        condition_team: Option<TeamID>,
    ) -> (Option<String>, Option<TeamID>) {
        let inner = self.inner.get_mut();
        let saved = (inner.current_player.clone(), inner.condition_team.clone());
        inner.current_player = current_player;
        inner.condition_team = condition_team;
        saved
    }

    /// Restore runtime context previously returned by `set_external_eval_context`.
    pub fn restore_external_eval_context(&mut self, saved: (Option<String>, Option<TeamID>)) {
        let inner = self.inner.get_mut();
        inner.current_player = saved.0;
        inner.condition_team = saved.1;
    }

    pub fn set_frame_object_count_changed(&self, frame: u32) {
        let mut inner = self.lock_inner_mut();
        inner.frame_object_count_changed = frame;
    }

    /// Set the script list for a player index (side).
    ///
    /// C++ Reference: `SideInfo::getScriptList()` (ScriptEngine::update loops sides and executes
    /// the side's ScriptList + ScriptGroups).
    pub fn set_script_list_for_player(
        &self,
        player_index: usize,
        script_list: Option<Box<ScriptList>>,
    ) -> GameLogicResult<()> {
        self.install_script_list(
            player_index,
            script_list,
            crate::helpers::get_game_logic_random_value,
        )
    }

    /// Prepare authored scripts with the RNG of the accepting simulation.
    pub fn set_script_list_for_player_with_random(
        &self,
        player_index: usize,
        script_list: Option<Box<ScriptList>>,
        random: &game_engine::common::random_value::SessionRandom,
    ) -> GameLogicResult<()> {
        self.install_script_list(player_index, script_list, |lo, hi| random.next_int(lo, hi))
    }

    fn install_script_list(
        &self,
        player_index: usize,
        script_list: Option<Box<ScriptList>>,
        draw: impl FnMut(i32, i32) -> i32,
    ) -> GameLogicResult<()> {
        self.side_scripts.install(player_index, script_list, |list| {
            generals_sim::scripting::program_install::initialize_script_list(list, draw, |name| {
                let prototype = self.team_factory.lock().ok()?.find_team_prototype(name)?;
                Some(prototype.is_singleton() || prototype.get_max_instances() < 2)
            });
        })
    }

    pub fn clear_script_lists(&mut self) {
        self.side_scripts.clear();
    }

    /// Enable or disable matching script and group declarations in original order.
    pub fn set_script_active_by_name(&self, script_name: &str, active: bool) -> bool {
        let updated = self.side_scripts.set_active(script_name, active);
        if let Some(handler) = self.with_inner(|inner| inner.action_handler.clone()) {
            let _ = handler.enable_script(script_name, active);
        }
        updated
    }

    /// Find a subroutine script by name and return a clone for immediate execution.
    pub fn get_subroutine_clone_by_name(&self, name: &str) -> Option<Script> {
        self.side_scripts.find_script(name, true, Script::clone)
    }

    /// Execute a subroutine script or subroutine group by name.
    ///
    /// Matches C++ `ScriptEngine::callSubroutine`: group lookup by name takes precedence over
    /// direct script lookup by name.
    ///
    /// Installs this lexical active engine so nested CALL_SUBROUTINE / flag /
    /// timer mutations can re-enter without taking the global mutex again.
    pub fn execute_subroutine_by_name(&self, name: &str) -> GameLogicResult<bool> {
        let mut driver = CanonicalScriptExecutionDriver;
        self.execute_subroutine_by_name_with_driver(
            name,
            crate::scripting::executor::ScriptContext::new(),
            &mut driver,
        )
    }

    pub fn execute_subroutine_by_name_with_driver(
        &self,
        name: &str,
        context: crate::scripting::executor::ScriptContext,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> GameLogicResult<bool> {
        let context = std::cell::RefCell::new(context);
        let mut execution = ScriptExecution::new(&context, driver);
        self.execute_subroutine_with_execution(name, &mut execution)
    }

    fn execute_subroutine_with_execution(
        &self,
        name: &str,
        execution: &mut ScriptExecution<'_>,
    ) -> GameLogicResult<bool> {
        self.with_active(|| {
            let mut program = ScriptProgramWorld::new(self, execution);
            self.side_scripts.call_subroutine(name, |script| program.run_script(script))
        })
    }

    /// Find a script by name and return a clone (non-subroutine allowed).
    pub fn find_script_clone_by_name(&self, name: &str) -> Option<Script> {
        self.side_scripts.find_script(name, false, Script::clone)
    }

    /// Initial TeamPrototype cache admission uses CPP Script::duplicate.
    pub(crate) fn find_script_duplicate_by_name(&self, name: &str) -> Option<Script> {
        self.side_scripts.find_script(name, false, Script::duplicate_definition)
    }

    pub fn capture_side_script_continuation(&self) -> GameLogicResult<SideScriptContinuation> {
        self.side_scripts.capture()
    }

    pub fn validate_side_script_continuation(
        &self,
        saved: &SideScriptContinuation,
    ) -> GameLogicResult<()> {
        self.side_scripts.validate(saved)
    }

    pub fn restore_side_script_continuation(
        &self,
        saved: &SideScriptContinuation,
    ) -> GameLogicResult<()> {
        self.side_scripts.restore(saved)
    }

    pub fn get_object_count(&self, player_index: i32, type_name: &str) -> i32 {
        self.with_inner(|inner| {
            inner
                .object_counts
                .get(&(player_index, type_name.to_string()))
                .copied()
                .unwrap_or(0)
        })
    }

    pub fn set_object_count(&self, player_index: i32, type_name: &str, count: i32) {
        let mut inner = self.lock_inner_mut();
        inner
            .object_counts
            .insert((player_index, type_name.to_string()), count);
    }

    /// Get a named ObjectTypes list (matches C++ ScriptEngine::getObjectTypes).
    pub fn get_object_types(&self, name: &str) -> Option<ObjectTypes> {
        self.with_inner(|inner| inner.object_types.get(name).cloned())
    }

    /// Register or replace a named ObjectTypes list.
    pub fn set_object_types(&self, name: String, types: ObjectTypes) {
        let mut inner = self.lock_inner_mut();
        inner.object_types.insert(name, types);
    }

    /// Mutate one live row without retaining the engine borrow across dispatch.
    fn with_attack_info_mut<R>(
        &self,
        name: &str,
        add_if_missing: bool,
        f: impl FnOnce(&mut AttackPriorityInfo) -> R,
    ) -> Option<R> {
        self.lock_inner_mut()
            .attack_priorities
            .with_mut(name, add_if_missing, f)
    }

    pub fn get_attack_info(&self, name: &str) -> Option<AttackPriorityInfo> {
        self.with_inner(|inner| inner.attack_priorities.get(name).map(|row| (**row).clone()))
    }

    /// Immutable row used across the combat and interpreter update boundaries.
    pub fn shared_attack_info(&self, name: &str) -> Option<Arc<AttackPriorityInfo>> {
        self.with_inner(|inner| inner.attack_priorities.get(name).cloned())
    }

    pub fn set_object_attack_priority_set(&self, object_id: ObjectID, set_name: &str) {
        self.lock_inner_mut().attack_priorities.assign(object_id, set_name);
    }

    pub fn clear_object_attack_priority_set(&self, object_id: ObjectID) {
        self.lock_inner_mut().attack_priorities.remove_assignment(object_id);
    }

    pub fn get_object_attack_priority_set(&self, object_id: ObjectID) -> Option<String> {
        self.with_inner(|inner| inner.attack_priorities.assigned_set(object_id).map(str::to_owned))
    }

    fn template_matches_kind(template: &EngineThingTemplate, kind: KindOf) -> bool {
        for idx in kind_of_indices(kind) {
            if template.is_kind_of((*idx) as u64) {
                return true;
            }
        }
        false
    }

    pub fn set_priority_thing(&self, set_name: &str, type_or_list: &str, priority: i32) -> bool {
        self.set_priority_thing_with_lookup(set_name, type_or_list, priority, |name| {
            TheThingFactory::find_template(name).map(|template| template.get_name().to_string())
        })
    }

    /// CPP SetPriorityThing uses the driving session's effective content definitions.
    /// Named lists preserve their prefix writes when a later template is absent.
    pub(crate) fn set_priority_thing_with_lookup(
        &self,
        set_name: &str,
        type_or_list: &str,
        priority: i32,
        mut template_name: impl FnMut(&str) -> Option<String>,
    ) -> bool {
        if let Some(list) = self.get_object_types(type_or_list) {
            for type_name in list.iter() {
                let Some(resolved_name) = template_name(type_name.as_str()) else {
                    return false;
                };
                let template_name = resolved_name;
                if self
                    .with_attack_info_mut(set_name, true, |info| {
                        info.set_priority(&template_name, priority);
                    })
                    .is_none()
                {
                    return false;
                }
            }
            return true;
        }

        let Some(resolved_name) = template_name(type_or_list) else {
            return false;
        };
        let template_name = resolved_name;
        self.with_attack_info_mut(set_name, true, |info| {
            info.set_priority(&template_name, priority);
        })
        .is_some()
    }

    pub fn set_priority_kind(&self, set_name: &str, kind: KindOf, priority: i32) -> bool {
        // C++ allocates the set before iterating all templates, including when
        // no template ultimately matches the requested KindOf mask.
        if self.with_attack_info_mut(set_name, true, |_| ()).is_none() {
            return false;
        }

        let Ok(factory_guard) = get_thing_factory() else {
            return false;
        };
        let Some(factory) = factory_guard.as_ref() else {
            return false;
        };

        let mut matching_template_names = Vec::new();
        let mut current = factory.first_template().cloned();
        while let Some(template) = current {
            if Self::template_matches_kind(&template, kind) {
                matching_template_names.push(template.get_name().to_string());
            }
            current = template.get_next_template().as_ref().cloned();
        }

        drop(factory_guard);
        for template_name in matching_template_names {
            if self
                .with_attack_info_mut(set_name, true, |info| {
                    info.set_priority(&template_name, priority);
                })
                .is_none()
            {
                return false;
            }
        }

        true
    }

    pub fn set_priority_default(&self, set_name: &str, priority: i32) -> bool {
        self.with_attack_info_mut(set_name, true, |info| {
            info.default_priority = priority;
        })
        .is_some()
    }

    /// Reset the script engine
    pub fn reset(&mut self) {
        self.script_events.clear();
        {
            let inner = self.inner.get_mut();
            // Clear runtime state
            inner.frame.reset();
            inner.attack_priorities.clear();
            inner.receiving_transport_statuses.clear();

            inner.calling_team = None;
            inner.calling_object = None;
            inner.condition_team = None;
            inner.condition_object = None;
            inner.current_player = None;
            inner.skirmish_human_player = None;
            inner.current_track_name.clear();


            inner.topple_directions.clear();
            inner.named_reveals.clear();
            inner.object_types.clear();
            inner.breeze_info = BreezeInfo::new();
            inner.game_difficulty = crate::player::GameDifficulty::Normal;

            inner.freeze_by_script = false;
            inner.objects_should_receive_difficulty_bonus = true;
            inner.choose_victim_always_uses_normal = false;
            inner.shown_mp_local_defeat_window = false;
            inner.suppress_new_windows = false;
            inner.win_lose_window_layout = None;

            inner.campaign_victorious = false;

        }
        self.sequential_scripts.clear();
        self.clear_script_lists();
        let inner = self.inner.get_mut();

        #[cfg(feature = "script_profiling")]
        {
            inner.stats = ScriptStats::default();
        }

        inner.attack_priorities.ensure_default();
    }

    /// C++ `ScriptEngine::newMap` (`ScriptEngine.cpp:5409-5473`).
    /// Resets transient counters/flags then starts the default 33-frame
    /// `FADE_MULTIPLY` fade-in from black. Does **not** clear script lists.
    pub fn new_map(&mut self) {
        self.inner.get_mut().frame.new_map();
        self.script_events.clear();
    }
}

#[cfg(test)]
mod script_object_context_tests {
    use super::*;

    /// C++ each GameEngine constructs its own TheScriptEngine. A bare
    /// `ScriptEngine::new()` must not adopt the world published on the thread:
    /// two worlds taking turns get distinct rosters and caves, and an engine
    /// built while one is published stays off it.
    #[test]
    fn bare_script_engine_never_adopts_the_published_world() {
        use crate::system::engine_stores::{with_active_stores, EngineStores};
        let _lock = crate::test_sync::lock();
        let world_a = std::sync::Arc::new(EngineStores::new_for_world());
        let world_b = std::sync::Arc::new(EngineStores::new_for_world());
        for (published, other) in [(&world_a, &world_b), (&world_b, &world_a)] {
            with_active_stores(published, || {
                let bare = ScriptEngine::new().expect("bare engine");
                for world in [published, other] {
                    assert!(!Arc::ptr_eq(bare.players(), world.services().players()));
                    assert!(!Arc::ptr_eq(&bare.cave_system, &world.services().logic_managers().cave));
                }
                let owned = ScriptEngine::new_with_services(other.services()).expect("owned engine");
                assert!(Arc::ptr_eq(owned.players(), other.services().players()));
                assert!(!Arc::ptr_eq(owned.players(), published.services().players()));
            });
        }
    }

    #[test]
    fn named_command_snapshots_this_object_from_borrowed_engine() {
        use crate::scripting::core::{
            Parameter, ParameterType, ScriptAction, ScriptActionType, THIS_OBJECT,
        };
        use crate::scripting::executor::{
            ScriptActionDispatcher, ScriptActionResult, ScriptContext,
        };

        struct NamedOwner(Vec<(String, Option<ObjectID>)>);
        impl ScriptExecutionDriver<ScriptEngine> for NamedOwner {
            fn after_action(&mut self) -> GameLogicResult<()> {
                Ok(())
            }
            fn named_command(
                &mut self,
                request: ScriptNamedCommand<'_>,
                this_object: Option<ObjectID>,
            ) -> Option<GameLogicResult<()>> {
                let unit = match request {
                    ScriptNamedCommand::ForceAttack { unit, .. }
                    | ScriptNamedCommand::Hunt { unit }
                    | ScriptNamedCommand::Guard { unit } => unit,
                    ScriptNamedCommand::Stop { .. }
                    | ScriptNamedCommand::FaceObject { .. }
                    | ScriptNamedCommand::Enter { .. }
                    | ScriptNamedCommand::Evacuate { .. } => {
                        panic!("unexpected named command: {request:?}");
                    }
                };
                self.0.push((unit.into(), this_object));
                Some(Ok(()))
            }
        }

        let mut own = ScriptEngine::new_in_published_world().unwrap();
        own.with_inner_mut(|inner| {
            inner.calling_object = Some(73);
            inner.condition_object = Some(41);
        });
        let mut foreign = ScriptEngine::new_in_published_world().unwrap();
        foreign.with_inner_mut(|inner| inner.calling_object = Some(13));
        let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
        let mut dispatcher = ScriptActionDispatcher::new(&own, &context);
        let mut action = ScriptAction::new(ScriptActionType::NamedAttackNamed);
        for name in [THIS_OBJECT, "Victim"] {
            action
                .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
                .unwrap();
        }
        let mut owner = NamedOwner(Vec::new());
        foreign.with_active(|| {
            assert_eq!(
                dispatcher
                    .execute_action_with_driver(&action, &mut owner)
                    .unwrap(),
                ScriptActionResult::Success
            );
            let mut hunt = ScriptAction::new(ScriptActionType::NamedHunt);
            hunt.add_parameter(Parameter::with_string(
                ParameterType::Unit,
                THIS_OBJECT.into(),
            ))
            .unwrap();
            assert_eq!(
                dispatcher
                    .execute_action_with_driver(&hunt, &mut owner)
                    .unwrap(),
                ScriptActionResult::Success
            );
            let mut guard = ScriptAction::new(ScriptActionType::NamedGuard);
            guard
                .add_parameter(Parameter::with_string(
                    ParameterType::Unit,
                    THIS_OBJECT.into(),
                ))
                .unwrap();
            assert_eq!(
                dispatcher
                    .execute_action_with_driver(&guard, &mut owner)
                    .unwrap(),
                ScriptActionResult::Success
            );
        });
        assert_eq!(
            owner.0,
            vec![
                (THIS_OBJECT.into(), Some(73)),
                (THIS_OBJECT.into(), Some(73)),
                (THIS_OBJECT.into(), Some(73))
            ]
        );
        assert_eq!(foreign.script_object_id(), Some(13));
    }

    #[test]
    fn script_object_id_prefers_calling_object_then_condition_object() {
        let mut engine = ScriptEngine::new_in_published_world().unwrap();
        assert_eq!(engine.script_object_id(), None);
        engine.with_inner_mut(|inner| inner.condition_object = Some(41));
        assert_eq!(engine.script_object_id(), Some(41));
        engine.with_inner_mut(|inner| inner.calling_object = Some(73));
        assert_eq!(engine.script_object_id(), Some(73));
        engine.with_inner_mut(|inner| inner.calling_object = None);
        assert_eq!(engine.script_object_id(), Some(41));
        engine.with_inner_mut(|inner| inner.condition_object = None);
        assert_eq!(engine.script_object_id(), None);
    }
}
