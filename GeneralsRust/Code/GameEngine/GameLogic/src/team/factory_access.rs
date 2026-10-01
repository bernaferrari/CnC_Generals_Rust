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
        let mut guard = self.lock_raw();
        std::mem::replace(&mut *guard, next)
    }

    /// Lock without [`TeamFactoryGuard`] flush. Checkout uses this so dropping
    /// the mutex does not run create-actions while a team is out of the map.
    pub(super) fn lock_raw(&self) -> MutexGuard<'_, TeamFactory> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
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

    pub(crate) fn execute_after_world_commit(self) {
        execute_pending_team_create_action_scripts(self.create_action_scripts);
        execute_pending_team_generic_script_evals(self.generic_script_evals);
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

/// Check `id` out of the global team factory, drop the factory mutex, then call `f`.
///
/// Same-id re-entry inside `f` returns `None`. Use the borrowed team.
pub fn with_team<R>(id: TeamID, f: impl FnOnce(&Team) -> R) -> Option<R> {
    let checkout = checkout_team(id)?;
    Some(f(checkout.team.as_ref()?))
}

/// Mutable checkout. See [`with_team`].
pub fn with_team_mut<R>(id: TeamID, f: impl FnOnce(&mut Team) -> R) -> Option<R> {
    let mut checkout = checkout_team(id)?;
    Some(f(checkout.team.as_mut()?))
}

struct TeamCheckout {
    id: TeamID,
    team: Option<Team>,
}

impl Drop for TeamCheckout {
    fn drop(&mut self) {
        if let Some(team) = self.team.take() {
            get_team_factory().lock_raw().teams.insert(self.id, team);
        }
    }
}

fn checkout_team(id: TeamID) -> Option<TeamCheckout> {
    let mut factory = get_team_factory().lock_raw();
    let team = factory.teams.remove(&id)?;
    drop(factory);
    Some(TeamCheckout {
        id,
        team: Some(team),
    })
}
