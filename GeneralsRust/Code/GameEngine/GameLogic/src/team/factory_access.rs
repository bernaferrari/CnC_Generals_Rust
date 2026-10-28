// TeamFactoryGuard, singleton, and TeamArcExt
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

/// RAII guard for TeamFactory that flushes pending create-actions after unlock.
pub struct TeamFactoryGuard<'a> {
    inner: Option<MutexGuard<'a, TeamFactory>>,
}

impl<'a> TeamFactoryGuard<'a> {
    fn new(inner: MutexGuard<'a, TeamFactory>) -> Self {
        Self { inner: Some(inner) }
    }
}

impl Deref for TeamFactoryGuard<'_> {
    type Target = TeamFactory;

    fn deref(&self) -> &Self::Target {
        self.inner
            .as_deref()
            .expect("TeamFactoryGuard missing inner")
    }
}

impl DerefMut for TeamFactoryGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
            .as_deref_mut()
            .expect("TeamFactoryGuard missing inner")
    }
}

impl Drop for TeamFactoryGuard<'_> {
    fn drop(&mut self) {
        let Some(mut guard) = self.inner.take() else {
            return;
        };
        let pending = guard.drain_pending_create_action_scripts();
        let pending_generic = guard.drain_pending_generic_script_evals();
        drop(guard);

        // A staged Main-world restore temporarily installs an isolated set of
        // C++-style singleton contents.  Running these callbacks while that
        // scope is active would let a map bootstrap action escape into the
        // active host/shadow world through the legacy script bridge.  The
        // transaction boundary deliberately defers them instead: they execute
        // only after Main installs the matching staged host world, never
        // through whichever world happens to be live at guard drop.
        let effects = TeamFactoryDeferredEffects::new(pending, pending_generic);
        match crate::runtime_world_transaction::defer_team_factory_effects_if_staging(effects) {
            Ok(()) => return,
            Err(effects) => effects.execute_after_world_commit(),
        }
    }
}

/// Mutex wrapper that returns TeamFactoryGuard with post-unlock flush semantics.
#[derive(Debug)]
pub struct TeamFactoryMutex {
    inner: Mutex<TeamFactory>,
}

impl TeamFactoryMutex {
    fn new() -> Self {
        Self {
            inner: Mutex::new(TeamFactory::new()),
        }
    }

    pub fn lock(&self) -> LockResult<TeamFactoryGuard<'_>> {
        match self.inner.lock() {
            Ok(guard) => Ok(TeamFactoryGuard::new(guard)),
            Err(poisoned) => Err(PoisonError::new(TeamFactoryGuard::new(
                poisoned.into_inner(),
            ))),
        }
    }

    /// Non-blocking lock for map-load fail-open. An abandoned startup worker
    /// can still hold this mutex while `start_game_from_ui` syncs the live map.
    pub fn try_lock(&self) -> TryLockResult<TeamFactoryGuard<'_>> {
        match self.inner.try_lock() {
            Ok(guard) => Ok(TeamFactoryGuard::new(guard)),
            Err(TryLockError::Poisoned(poisoned)) => Err(TryLockError::Poisoned(PoisonError::new(
                TeamFactoryGuard::new(poisoned.into_inner()),
            ))),
            Err(TryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
        }
    }

    /// Replace the factory contents at a whole-world transaction boundary.
    ///
    /// This intentionally bypasses [`TeamFactoryGuard`].  The normal guard
    /// executes queued `ExecuteActionsOnCreate` and generic-script callbacks
    /// when it is dropped, which is correct for an ordinary simulation update
    /// but unsafe while a save restore is only staging a replacement world.
    /// Call only from the single host game thread while no regular team update
    /// is in progress.
    pub(crate) fn replace_for_world_boundary(&self, next: TeamFactory) -> TeamFactory {
        let mut guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        std::mem::replace(&mut *guard, next)
    }
}

/// Cloneable access to one match's team factory. The handle is owned by the
/// driving game instance; cloning it gives a subsystem an explicit dependency
/// without selecting a process-global active factory.
#[derive(Clone, Debug)]
pub struct TeamFactoryHandle {
    inner: Arc<TeamFactoryMutex>,
}

impl TeamFactoryHandle {
    /// Create an independent factory for a new match.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TeamFactoryMutex::new()),
        }
    }

    pub fn lock(&self) -> LockResult<TeamFactoryGuard<'_>> {
        self.inner.lock()
    }

    pub fn try_lock(&self) -> TryLockResult<TeamFactoryGuard<'_>> {
        self.inner.try_lock()
    }

    /// Inspect this factory without draining its deferred callbacks. Ordinary
    /// `lock` owns a callback boundary; an observation must not create one.
    pub fn with_read_without_flushing<R>(
        &self,
        read: impl FnOnce(&TeamFactory) -> R,
    ) -> LockResult<R> {
        match self.inner.inner.lock() {
            Ok(factory) => Ok(read(&factory)),
            Err(poisoned) => Err(PoisonError::new(read(&poisoned.into_inner()))),
        }
    }

    /// Run one controller's Team generic-script phase with the receiving
    /// interpreter and driver. This deliberately bypasses TeamFactoryGuard:
    /// generic callbacks execute synchronously in caller order, after the
    /// factory lock is released, and never drain unrelated factory callbacks.
    pub fn update_generic_scripts_for_controller_with_driver(
        &self,
        controller: u32,
        current_player_name: Option<&str>,
        script_engine: &crate::scripting::engine::ScriptEngine,
        context: crate::scripting::executor::ScriptContext,
        driver: &mut dyn crate::scripting::engine::ScriptExecutionDriver,
    ) -> crate::GameLogicResult<()> {
        let prototypes = self
            .with_read_without_flushing(|factory| {
                factory.owned_team_prototypes_for_controller(controller)
            })
            .unwrap_or_else(|error| error.into_inner());

        for prototype in prototypes {
            // C++ opens each prototype's TeamInstanceList only when the
            // Player::update cursor reaches it. A prior hook can create an
            // instance for a later prototype during this same player pass.
            if !self.owns_generic_prototype(controller, &prototype) {
                continue;
            }
            let entries = self
                .with_read_without_flushing(|factory| {
                    factory.generic_script_entries_for_prototype(&prototype)
                })
                .unwrap_or_else(|error| error.into_inner());
            for (_, team) in entries {
                let team_id = match team.read() {
                    Ok(team) => team.get_id(),
                    Err(error) => error.into_inner().get_id(),
                };
                for script_index in 0..MAX_GENERIC_SCRIPTS {
                    // Once Team::updateGenericScripts starts, it continues
                    // the current instance's remaining slots synchronously.
                    if !self.owns_generic_instance(&prototype, &team) {
                        break;
                    }
                    let should_attempt = match team.read() {
                        Ok(team) => team.should_attempt_generic_script(script_index),
                        Err(error) => error
                            .into_inner()
                            .should_attempt_generic_script(script_index),
                    };
                    if !should_attempt {
                        continue;
                    }

                    let script_name = prototype
                        .get_generic_script(script_index)
                        .map(|name| name.to_string())
                        .unwrap_or_default();
                    if script_name.is_empty() {
                        match team.write() {
                            Ok(mut team) => team.disable_generic_script_attempt(script_index),
                            Err(error) => error
                                .into_inner()
                                .disable_generic_script_attempt(script_index),
                        }
                        continue;
                    }

                    let mut script = match prototype
                        .take_or_load_generic_script_runtime_owned(script_index, script_engine)
                    {
                        Ok(Some(script)) => script,
                        Err(crate::team::GenericScriptRuntimeBusy::InFlight) => {
                            return Err(crate::GameLogicError::Configuration(format!(
                                "Team generic script '{}' was reentered while its runtime was active",
                                script_name
                            )));
                        }
                        Ok(None) => {
                            match team.write() {
                                Ok(mut team) => team.disable_generic_script_attempt(script_index),
                                Err(error) => error
                                    .into_inner()
                                    .disable_generic_script_attempt(script_index),
                            }
                            continue;
                        }
                    };

                    // Team generic hooks call evaluateConditions directly. They
                    // bypass executeScript's root active/difficulty/periodic gates.
                    let condition_true = match script_engine
                        .evaluate_conditions_with_driver_checked(
                            &mut script,
                            Some(team_id),
                            current_player_name,
                            context.clone(),
                            driver,
                        ) {
                        Ok(condition_true) => condition_true,
                        Err(error) => {
                            log::warn!(
                                "Team generic script '{}' evaluation failed for team '{}': {}",
                                script_name,
                                prototype.get_name(),
                                error
                            );
                            continue;
                        }
                    };

                    let still_eligible = match team.read() {
                        Ok(team) => team.should_attempt_generic_script(script_index),
                        Err(error) => error
                            .into_inner()
                            .should_attempt_generic_script(script_index),
                    };
                    if condition_true
                        && still_eligible
                        && self.owns_generic_instance(&prototype, &team)
                    {
                        if script.is_one_shot() {
                            // C++ consumes a one-shot before the synchronous action.
                            match team.write() {
                                Ok(mut team) => team.disable_generic_script_attempt(script_index),
                                Err(error) => error
                                    .into_inner()
                                    .disable_generic_script_attempt(script_index),
                            }
                        }
                        if let Some(action) = script.get_action().cloned() {
                            script_engine.friend_execute_action_with_driver_checked(
                                &action,
                                Some(team_id),
                                context.clone(),
                                driver,
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Whether an admitted eligible slot has a non-empty definition and thus
    /// requires the receiving session's interpreter for its scheduled phase.
    pub fn generic_scripts_require_interpreter_for_controller(&self, controller: u32) -> bool {
        self.with_read_without_flushing(|factory| {
            factory
                .owned_team_generic_script_entries(controller)
                .into_iter()
                .any(|(prototype, team)| {
                    let team = match team.read() {
                        Ok(team) => team,
                        Err(error) => error.into_inner(),
                    };
                    (0..MAX_GENERIC_SCRIPTS).any(|index| {
                        team.should_attempt_generic_script(index)
                            && prototype
                                .get_generic_script(index)
                                .is_some_and(|name| !name.is_empty())
                    })
                })
        })
        .unwrap_or_else(|error| error.into_inner())
    }

    fn owns_generic_prototype(&self, controller: u32, prototype: &Arc<TeamPrototype>) -> bool {
        self.with_read_without_flushing(|factory| {
            factory
                .owned_team_prototypes_for_controller(controller)
                .iter()
                .any(|current| Arc::ptr_eq(current, prototype))
        })
        .unwrap_or_else(|error| error.into_inner())
    }

    fn owns_generic_instance(
        &self,
        prototype: &Arc<TeamPrototype>,
        team: &Arc<RwLock<Team>>,
    ) -> bool {
        self.with_read_without_flushing(|factory| {
            factory
                .generic_script_entries_for_prototype(prototype)
                .iter()
                .any(|(_, current_team)| Arc::ptr_eq(current_team, team))
        })
        .unwrap_or_else(|error| error.into_inner())
    }

    /// Resolve a team without constructing `TeamFactoryGuard`: its Drop
    /// intentionally drains queued factory callbacks after unlocking.
    fn resolve_team_without_flushing(&self, team_id: TeamID) -> Option<Arc<RwLock<Team>>> {
        self.with_read_without_flushing(|factory| factory.find_team_by_id(team_id))
            .unwrap_or_else(|error| error.into_inner())
    }

    /// Mark a team's trigger-area edge flag without flushing factory callbacks.
    /// Returns false when the team ID is not present.
    pub fn mark_team_entered_exited(&self, team_id: TeamID) -> bool {
        let Some(team) = self.resolve_team_without_flushing(team_id) else {
            return false;
        };
        let mut team = match team.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        team.set_entered_exited();
        true
    }

    /// Read a team's trigger-area edge flag without flushing factory callbacks.
    pub fn team_entered_or_exited(&self, team_id: TeamID) -> Option<bool> {
        let team = self.resolve_team_without_flushing(team_id)?;
        let team = match team.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        Some(team.did_enter_or_exit())
    }

    /// Copy only real available definitions for a detached logical restore.
    /// This is a read, so it must not drain callbacks like TeamFactoryGuard.
    pub fn detached_definition_catalog(
        &self,
        names: Option<&[String]>,
    ) -> Result<TeamFactory, String> {
        let guard = self
            .inner
            .inner
            .lock()
            .map_err(|_| "team definition catalog poisoned")?;
        guard.detached_definition_catalog(names)
    }

    /// Replace contents only at an explicit whole-world boundary.
    pub fn replace_for_world_boundary(&self, next: TeamFactory) -> TeamFactory {
        self.inner.replace_for_world_boundary(next)
    }
}

impl Default for TeamFactoryHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Post-unlock team callbacks captured by a whole-world staging transaction.
///
/// `TeamFactoryGuard` ordinarily executes these synchronously after releasing
/// the factory mutex.  During save staging they must instead remain ordered
/// with the staged world and run only once Main has installed both that world
/// and its singleton contents.  This is intentionally opaque outside the
/// transaction module: callers cannot execute one callback against a mixture
/// of old and staged globals.
pub(crate) struct TeamFactoryDeferredEffects {
    create_action_scripts: Vec<String>,
    generic_script_evals: Vec<PendingTeamGenericScriptEval>,
}

impl TeamFactoryDeferredEffects {
    fn new(
        create_action_scripts: Vec<String>,
        generic_script_evals: Vec<PendingTeamGenericScriptEval>,
    ) -> Self {
        Self {
            create_action_scripts,
            generic_script_evals,
        }
    }

    /// Empty guard-drop receipts carry no script work. The outer staging
    /// vector may contain many such receipts, so admission checks payloads.
    pub(crate) fn pending_callback_counts(&self) -> (usize, usize) {
        (
            self.create_action_scripts.len(),
            self.generic_script_evals.len(),
        )
    }

    pub(crate) fn execute_after_world_commit(self) {
        execute_pending_team_create_action_scripts(self.create_action_scripts);
        execute_pending_team_generic_script_evals(self.generic_script_evals);
    }

    #[cfg(test)]
    pub(crate) fn pending_for_world_transaction_test(create: bool, generic: bool) -> Self {
        let prototype = Arc::new(TeamPrototype::new("ForeignPrototype".into()));
        let team = Arc::new(RwLock::new(Team::new("ForeignTeam".into(), 7)));
        Self::new(
            if create {
                vec!["ForeignCreate".to_owned()]
            } else {
                Vec::new()
            },
            if generic {
                vec![PendingTeamGenericScriptEval {
                    team,
                    prototype,
                    team_name: "ForeignTeam".to_owned(),
                    script_name: "ForeignGeneric".to_owned(),
                    script_index: 0,
                    current_player_name: None,
                }]
            } else {
                Vec::new()
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn empty_for_world_transaction_test() -> Self {
        Self::new(Vec::new(), Vec::new())
    }
}

/// Global team factory instance (matching C++ TheTeamFactory)
static THE_TEAM_FACTORY: OnceLock<TeamFactoryHandle> = OnceLock::new();

fn global_team_factory_handle() -> &'static TeamFactoryHandle {
    THE_TEAM_FACTORY.get_or_init(|| TeamFactoryHandle::new())
}

/// Get the compatibility singleton used by legacy engine-only callers.
pub fn get_team_factory() -> &'static TeamFactoryMutex {
    global_team_factory_handle().inner.as_ref()
}

/// Convenience alias for C++ compatibility
pub use get_team_factory as TheTeamFactory;

/// Extension trait for Arc<RwLock<Team>> to provide helper methods
pub trait TeamArcExt {
    fn get_relationship(&self, that_team: &Team) -> Relationship;
}

impl TeamArcExt for Arc<RwLock<Team>> {
    /// Get relationship between this team and another team
    fn get_relationship(&self, that_team: &Team) -> Relationship {
        if let Ok(guard) = self.read() {
            guard.get_relationship(that_team)
        } else {
            Relationship::Neutral
        }
    }
}

/// Run `f` with exclusive access to the team addressed by `team_id`.
///
/// The team is resolved through the factory singleton; returns `None` when the
/// id is unknown or a lock is poisoned. The factory lock is released before
/// `f` runs so object creation under a team cannot re-enter it.
pub fn with_team<R>(team_id: TeamID, f: impl FnOnce(&mut Team) -> R) -> Option<R> {
    let team = get_team_factory().lock().ok()?.find_team_by_id(team_id)?;
    let mut guard = team.write().ok()?;
    Some(f(&mut guard))
}

/// Run `f` with exclusive access to an already-resolved team handle.
pub fn with_team_mut<R>(team: Arc<RwLock<Team>>, f: impl FnOnce(&mut Team) -> R) -> Option<R> {
    let mut guard = team.write().ok()?;
    Some(f(&mut guard))
}

#[cfg(test)]
mod detached_definition_tests {
    use super::*;
    #[test]
    fn definition_catalog_keeps_ids_order_controllers_without_runtime_aliases_or_callbacks() {
        let handle = TeamFactoryHandle::new();
        {
            let mut factory = handle.inner.inner.lock().unwrap();
            let mut definition = TeamPrototype::new("OwnedDefinition".into());
            definition.set_id(37);
            definition.owner_name = "actual_owner".into();
            definition
                .production_condition_runtime
                .lock()
                .unwrap()
                .always_false = true;
            factory
                .prototypes
                .insert("OwnedDefinition".into(), Arc::new(definition));
            factory.prototype_order.push("OwnedDefinition".into());
            factory
                .prototype_controllers
                .insert("OwnedDefinition".into(), Some(4));
            factory
                .pending_create_action_scripts
                .push("do_not_flush".into());
            factory.unique_team_prototype_id = 38;
        }
        let names = vec!["OwnedDefinition".to_string()];
        let detached = handle.detached_definition_catalog(Some(&names)).unwrap();
        let original = handle.inner.inner.lock().unwrap();
        assert_eq!(original.pending_create_action_scripts, ["do_not_flush"]);
        assert!(detached.pending_create_action_scripts.is_empty());
        assert_eq!(detached.prototype_order, original.prototype_order);
        assert_eq!(
            detached.prototype_controllers,
            original.prototype_controllers
        );
        assert_eq!(detached.unique_team_prototype_id, 38);
        assert!(detached.teams.is_empty());
        let before = original.prototypes.get("OwnedDefinition").unwrap();
        let after = detached.prototypes.get("OwnedDefinition").unwrap();
        assert_eq!(after.get_id(), before.get_id());
        assert_eq!(after.get_owner_name(), before.get_owner_name());
        assert!(!Arc::ptr_eq(
            &before.production_condition_runtime,
            &after.production_condition_runtime
        ));
        assert!(!Arc::ptr_eq(
            &before.generic_script_runtime,
            &after.generic_script_runtime
        ));
        after
            .production_condition_runtime
            .lock()
            .unwrap()
            .always_false = false;
        after
            .generic_script_runtime
            .lock()
            .unwrap()
            .push(GenericScriptRuntimeSlot::Empty);
        assert!(before.production_condition_always_false());
        assert_eq!(
            before.generic_script_runtime.lock().unwrap().len(),
            MAX_GENERIC_SCRIPTS
        );
        assert!(!after.production_condition_always_false());
    }

    #[test]
    fn edge_flag_access_does_not_flush_pending_factory_callbacks() {
        let handle = TeamFactoryHandle::new();
        let team_id = 17;
        {
            let mut factory = handle.inner.inner.lock().unwrap();
            let team = Arc::new(RwLock::new(Team::new("OwnedTeam".into(), team_id)));
            factory.teams.insert(team_id, Arc::clone(&team));
            factory.instance_order.push(team_id);
            factory
                .pending_create_action_scripts
                .push("pending create callback".into());
            factory
                .pending_generic_script_evals
                .push(PendingTeamGenericScriptEval {
                    team,
                    prototype: Arc::new(TeamPrototype::new("OwnedPrototype".into())),
                    team_name: "OwnedTeam".into(),
                    script_name: "PendingGeneric".into(),
                    script_index: 0,
                    current_player_name: None,
                });
        }

        assert_eq!(handle.team_entered_or_exited(team_id), Some(false));
        assert!(handle.mark_team_entered_exited(team_id));
        assert_eq!(handle.team_entered_or_exited(team_id), Some(true));
        assert_eq!(handle.team_entered_or_exited(team_id + 1), None);
        assert!(!handle.mark_team_entered_exited(team_id + 1));

        let factory = handle.inner.inner.lock().unwrap();
        assert_eq!(
            factory.pending_create_action_scripts,
            ["pending create callback"]
        );
        assert_eq!(factory.pending_generic_script_evals.len(), 1);
        assert_eq!(
            factory.pending_generic_script_evals[0].script_name,
            "PendingGeneric"
        );
    }
}
