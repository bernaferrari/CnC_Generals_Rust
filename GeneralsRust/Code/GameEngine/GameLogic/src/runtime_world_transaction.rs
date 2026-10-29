//! Raw singleton transaction boundary for staged host-world replacement.
//!
//! C++ has one active `TheTerrainLogic`/`ThePlayerList`/`TheTeamFactory` world.
//! Rust's host restore builds a candidate `GameLogic` before committing it, so
//! map loading must temporarily give that candidate its own contents without
//! replacing any singleton wrapper (`Arc`, `RwLock`, or `OnceLock`).  Replacing
//! wrappers would strand existing aliases in the old world; clearing live
//! contents would corrupt a still-playable match when staging fails.
//!
//! Main's native save admission uses [`HostRuntimeWorldStage`], which owns the
//! remaining shared metadata swap. Per-world services stay on their explicit
//! `WorldServices` owner throughout staging.
//!
//! This module is deliberately a small, raw boundary rather than save schema.
//! It does not serialize these values and does not change snapshot/Xfer v4.

use crate::player::{PlayerList, player_list};
use crate::scripting::engine::{ScriptEngine, get_script_engine};
use crate::team::{
    TeamFactory, TeamFactoryDeferredEffects, TeamScriptEventQueue, get_team_factory,
    replace_pending_team_script_events_for_world_boundary,
    take_pending_team_script_events_for_world_boundary,
};
use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// Singleton contents are process-global, so staging cannot run concurrently
/// on two threads even though the side-effect marker itself is thread-local.
/// The host normally owns this lock implicitly through its game thread; the
/// explicit mutex also makes test/worker misuse serialize rather than swap two
/// candidate worlds through the same wrappers.
static WORLD_RUNTIME_TRANSACTION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn world_runtime_transaction_lock() -> &'static Mutex<()> {
    WORLD_RUNTIME_TRANSACTION_LOCK.get_or_init(|| Mutex::new(()))
}

thread_local! {
    /// Nested staging guards share one deferred callback queue.  Only the
    /// outermost guard may take or discard it, so a helper which enters a
    /// nested effect scope cannot accidentally execute stage callbacks early.
    static WORLD_RUNTIME_STAGE_DEPTH: Cell<u32> = const { Cell::new(0) };
    static DEFERRED_TEAM_FACTORY_EFFECTS: RefCell<Vec<TeamFactoryDeferredEffects>> =
        RefCell::new(Vec::new());
}

/// True while the current thread is constructing a candidate whole world.
///
/// This is intentionally thread-local: the host owns world construction on a
/// single game thread and unrelated worker threads must not inherit a staging
/// world accidentally.
pub fn world_runtime_staging_active() -> bool {
    WORLD_RUNTIME_STAGE_DEPTH.with(|depth| depth.get() != 0)
}

/// Hold a normal `TeamFactoryGuard` post-unlock callback until a candidate
/// world has committed.
///
/// The payload order is the exact guard-drop order.  Rollback drops this queue;
/// successful staging hands it to Main, which executes it only after both the
/// host `GameLogic` and this module's singleton bundle are installed.
/// Returns the original payload unchanged when no stage is active, allowing
/// the ordinary guard-drop path to execute it without cloning or loss.
pub(crate) fn defer_team_factory_effects_if_staging(
    effects: TeamFactoryDeferredEffects,
) -> Result<(), TeamFactoryDeferredEffects> {
    if !world_runtime_staging_active() {
        return Err(effects);
    }
    DEFERRED_TEAM_FACTORY_EFFECTS.with(|pending| pending.borrow_mut().push(effects));
    Ok(())
}

/// Scope for effects which must never escape a candidate world into the active
/// host/shadow world. Main uses it around its receiving Host stage and can also
/// guard thread-local presentation queues around the same operation.
pub struct WorldRuntimeStageScope {
    outermost: bool,
    active: bool,
}

impl WorldRuntimeStageScope {
    pub fn enter() -> Self {
        let outermost = WORLD_RUNTIME_STAGE_DEPTH.with(|depth| {
            let prior = depth.get();
            depth.set(
                prior
                    .checked_add(1)
                    .expect("world runtime staging depth overflow"),
            );
            prior == 0
        });

        if outermost {
            // A prior scope must have either committed or rolled back its
            // effects.  Do not silently discard a bug-produced payload here.
            DEFERRED_TEAM_FACTORY_EFFECTS.with(|pending| {
                debug_assert!(pending.borrow().is_empty());
            });
        }

        Self {
            outermost,
            active: true,
        }
    }

    fn leave(&mut self, keep_effects: bool) -> Vec<TeamFactoryDeferredEffects> {
        if !self.active {
            return Vec::new();
        }
        self.active = false;
        let became_inactive = WORLD_RUNTIME_STAGE_DEPTH.with(|depth| {
            let prior = depth.get();
            debug_assert!(prior != 0, "unbalanced world runtime stage scope");
            let next = prior.saturating_sub(1);
            depth.set(next);
            next == 0
        });

        if !self.outermost || !became_inactive {
            return Vec::new();
        }

        DEFERRED_TEAM_FACTORY_EFFECTS.with(|pending| {
            let mut pending = pending.borrow_mut();
            if keep_effects {
                std::mem::take(&mut *pending)
            } else {
                pending.clear();
                Vec::new()
            }
        })
    }

    /// Finish a successful stage and return its callbacks to the commit
    /// boundary.  This does not execute anything.
    pub(crate) fn finish(mut self) -> Vec<TeamFactoryDeferredEffects> {
        self.leave(true)
    }
}

impl Drop for WorldRuntimeStageScope {
    fn drop(&mut self) {
        // Errors/panics in map loading must discard staged callbacks.  They
        // cannot target live hooks after the singleton values have rolled back.
        let _ = self.leave(false);
    }
}

/// Native map admission already executes its actions against its owned
/// interpreter. A deferred Core payload here has no justified native phase.
fn validate_native_team_effects(effects: &[TeamFactoryDeferredEffects]) -> Result<(), String> {
    let (create, generic) = effects
        .iter()
        .fold((0usize, 0usize), |(create, generic), effect| {
            let (next_create, next_generic) = effect.pending_callback_counts();
            (
                create.saturating_add(next_create),
                generic.saturating_add(next_generic),
            )
        });
    if create != 0 || generic != 0 {
        return Err(format!(
            "native staged world has unexpected Core team callbacks: {create} create, {generic} generic"
        ));
    }
    Ok(())
}

/// Deferred post-unlock TeamFactory callbacks belonging to a committed world.
///
/// This token has no rollback path: it is created only after `install_globals`
/// has succeeded.  It must execute after Main assigns the staged `GameLogic`,
/// so the legacy script engine's action handler points at the same world.
#[must_use = "call execute_after_logic_commit after installing the staged host GameLogic"]
pub struct CommittedRuntimeWorldEffects {
    team_factory_effects: Option<Vec<TeamFactoryDeferredEffects>>,
    transaction_lock: Option<MutexGuard<'static, ()>>,
}

impl CommittedRuntimeWorldEffects {
    pub fn execute_after_logic_commit(mut self) {
        let effects = self.team_factory_effects.take().unwrap_or_default();
        for effect in effects {
            effect.execute_after_world_commit();
        }
        // No other candidate stage may observe the briefly in-progress commit
        // between singleton install and these C++ post-unlock callbacks.
        drop(self.transaction_lock.take());
    }
}

impl Drop for CommittedRuntimeWorldEffects {
    fn drop(&mut self) {
        if self
            .team_factory_effects
            .as_ref()
            .is_some_and(|effects| !effects.is_empty())
        {
            log::error!(
                "committed staged-world TeamFactory effects were dropped without post-commit execution"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_effect() -> TeamFactoryDeferredEffects {
        TeamFactoryDeferredEffects::empty_for_world_transaction_test()
    }

    #[test]
    fn rollback_discards_staged_team_effects() {
        let _serial = crate::test_sync::lock();
        let scope = WorldRuntimeStageScope::enter();
        assert!(defer_team_factory_effects_if_staging(empty_effect()).is_ok());
        drop(scope);

        let next = WorldRuntimeStageScope::enter();
        assert!(next.finish().is_empty());
    }

    #[test]
    fn successful_scope_defers_effects_for_commit_in_order() {
        let _serial = crate::test_sync::lock();
        let scope = WorldRuntimeStageScope::enter();
        assert!(defer_team_factory_effects_if_staging(empty_effect()).is_ok());
        assert!(defer_team_factory_effects_if_staging(empty_effect()).is_ok());
        assert_eq!(scope.finish().len(), 2);
    }
}

mod host;
pub use host::{
    CheckedNativeRuntimeWorld, HostRuntimeWorldStage, NativeRuntimeWorldCommit,
    StagedHostRuntimeWorld,
};

#[cfg(test)]
#[path = "runtime_world_transaction/native_team_preflight_tests.rs"]
mod native_team_preflight_tests;
