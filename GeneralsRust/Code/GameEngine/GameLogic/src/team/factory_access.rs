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
        execute_pending_team_create_action_scripts(pending);
        execute_pending_team_generic_script_evals(pending_generic);
    }
}

/// Mutex wrapper that returns TeamFactoryGuard with post-unlock flush semantics.
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
}

/// Global team factory instance (matching C++ TheTeamFactory)
static THE_TEAM_FACTORY: OnceLock<TeamFactoryMutex> = OnceLock::new();

/// Get global team factory instance
pub fn get_team_factory() -> &'static TeamFactoryMutex {
    THE_TEAM_FACTORY.get_or_init(TeamFactoryMutex::new)
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
