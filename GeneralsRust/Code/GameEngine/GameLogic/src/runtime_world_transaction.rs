//! Raw singleton transaction boundary for staged host-world replacement.
//!
//! C++ has one active `TheTerrainLogic`/`ThePlayerList`/`TheTeamFactory` world.
//! Rust's host restore builds a candidate `GameLogic` before committing it, so
//! map loading must temporarily give that candidate its own contents without
//! replacing any singleton wrapper (`Arc`, `RwLock`, or `OnceLock`).  Replacing
//! wrappers would strand existing aliases in the old world; clearing live
//! contents would corrupt a still-playable match when staging fails.
//!
//! This module is deliberately a small, raw boundary rather than save schema.
//! It does not serialize these values and does not change snapshot/Xfer v4.

use crate::ai::integration::{
    AiIntegrationManager, replace_ai_integration_for_world_boundary,
    take_ai_integration_for_world_boundary,
};
use crate::ai::{AI, replace_global_ai_for_world_boundary, take_global_ai_for_world_boundary};
use crate::player::{PlayerList, player_list};
use crate::scripting::engine::{
    ScriptEngine, get_area_tracker, get_named_object_tracker, get_script_engine,
};
use crate::scripting::events::{AreaTrackerState, NamedObjectTrackerState};
use crate::sides_list::{SidesList, get_sides_list};
use crate::system::shroud_manager::{ShroudManager, get_shroud_manager};
use crate::team::{
    TeamFactory, TeamFactoryDeferredEffects, TeamScriptEventQueue, get_team_factory,
    replace_pending_team_script_events_for_world_boundary,
    take_pending_team_script_events_for_world_boundary,
};
use crate::terrain::{TerrainLogic, get_terrain_logic};
use std::cell::{Cell, RefCell};
use std::sync::{Mutex, MutexGuard, OnceLock};

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
/// host/shadow world.  It is independent from [`RuntimeWorldStage`] so Main can
/// also guard its thread-local presentation queues around the same operation.
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

/// Owned contents of the process-global systems that map loading mutates.
///
/// The fields are intentionally private.  The only supported operations are
/// taking all current singleton contents, restoring them, or installing a
/// completed staged bundle through the host commit API below.
struct RuntimeWorldGlobals {
    ai: AI,
    ai_integration: Option<AiIntegrationManager>,
    terrain: TerrainLogic,
    players: PlayerList,
    teams: TeamFactory,
    sides: SidesList,
    script_engine: Option<ScriptEngine>,
    shroud: ShroudManager,
    named_objects: NamedObjectTrackerState,
    areas: AreaTrackerState,
    pending_team_script_events: TeamScriptEventQueue,
}

impl RuntimeWorldGlobals {
    /// Move singleton contents into an owned bundle and leave fresh defaults
    /// behind for map/bootstrap work.  Lock poisoning is recovered here rather
    /// than surfacing a fallible half-take: failure handling must always be
    /// able to restore a coherent active world.
    fn take_from_singletons() -> Self {
        // Keep this order stable.  We never hold two locks at once, but a
        // deterministic order makes future extensions auditable.
        let ai = take_global_ai_for_world_boundary();
        let ai_integration = take_ai_integration_for_world_boundary();
        let terrain = {
            let mut guard = get_terrain_logic()
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, TerrainLogic::new())
        };
        let players = {
            let mut guard = player_list()
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, PlayerList::new())
        };
        let teams = get_team_factory().replace_for_world_boundary(TeamFactory::new());
        let sides = {
            let sides_list = get_sides_list();
            let mut guard = sides_list
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, SidesList::new())
        };
        let script_engine = {
            let handle = get_script_engine();
            let mut guard = handle
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::take(&mut *guard)
        };
        let shroud = {
            let shroud_manager = get_shroud_manager();
            let mut guard = shroud_manager
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, ShroudManager::new())
        };
        let named_objects = get_named_object_tracker().take_state_for_world_boundary();
        let areas = get_area_tracker().take_state_for_world_boundary();
        let pending_team_script_events = take_pending_team_script_events_for_world_boundary();

        Self {
            ai,
            ai_integration,
            terrain,
            players,
            teams,
            sides,
            script_engine,
            shroud,
            named_objects,
            areas,
            pending_team_script_events,
        }
    }

    /// Install this bundle into the stable singleton wrappers and return the
    /// bundle it replaced.  No normal TeamFactory guard is created here, so no
    /// create-action callback can run while values are only half installed.
    fn install_into_singletons(self) -> Self {
        let Self {
            ai,
            ai_integration,
            terrain,
            players,
            teams,
            sides,
            script_engine,
            shroud,
            named_objects,
            areas,
            pending_team_script_events,
        } = self;

        let old_ai = replace_global_ai_for_world_boundary(ai);
        let old_ai_integration = replace_ai_integration_for_world_boundary(ai_integration);
        let old_terrain = {
            let mut guard = get_terrain_logic()
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, terrain)
        };
        let old_players = {
            let mut guard = player_list()
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, players)
        };
        let old_teams = get_team_factory().replace_for_world_boundary(teams);
        let old_sides = {
            let sides_list = get_sides_list();
            let mut guard = sides_list
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, sides)
        };
        let old_script_engine = {
            let handle = get_script_engine();
            let mut guard = handle
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, script_engine)
        };
        let old_shroud = {
            let shroud_manager = get_shroud_manager();
            let mut guard = shroud_manager
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, shroud)
        };
        let old_named_objects =
            get_named_object_tracker().replace_state_for_world_boundary(named_objects);
        let old_areas = get_area_tracker().replace_state_for_world_boundary(areas);
        let old_pending_team_script_events =
            replace_pending_team_script_events_for_world_boundary(pending_team_script_events);

        Self {
            ai: old_ai,
            ai_integration: old_ai_integration,
            terrain: old_terrain,
            players: old_players,
            teams: old_teams,
            sides: old_sides,
            script_engine: old_script_engine,
            shroud: old_shroud,
            named_objects: old_named_objects,
            areas: old_areas,
            pending_team_script_events: old_pending_team_script_events,
        }
    }
}

/// Owns the old live singleton bundle while a map/save candidate is built.
/// Dropping it without `finish_and_restore_live` rolls the candidate back.
pub struct RuntimeWorldStage {
    live: Option<RuntimeWorldGlobals>,
    effect_scope: Option<WorldRuntimeStageScope>,
    transaction_lock: Option<MutexGuard<'static, ()>>,
}

impl RuntimeWorldStage {
    /// Begin a whole-world staging transaction.  Nested *effect* guards are
    /// supported, but nested singleton transactions are a programmer error:
    /// there is only one set of stable singleton wrappers to isolate.
    pub fn begin() -> Self {
        assert!(
            !world_runtime_staging_active(),
            "nested RuntimeWorldStage would overwrite the outer candidate world"
        );
        let transaction_lock = match world_runtime_transaction_lock().lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        };
        let effect_scope = WorldRuntimeStageScope::enter();
        let live = RuntimeWorldGlobals::take_from_singletons();
        Self {
            live: Some(live),
            effect_scope: Some(effect_scope),
            transaction_lock: Some(transaction_lock),
        }
    }

    /// Extract the completed candidate bundle and restore the pre-stage live
    /// singleton contents.  The caller can now validate/prepare the host
    /// commit while the active match remains completely intact.
    pub fn finish_and_restore_live(mut self) -> StagedRuntimeWorld {
        let staged = RuntimeWorldGlobals::take_from_singletons();
        let live = self
            .live
            .take()
            .expect("RuntimeWorldStage missing pre-stage globals");
        let replaced = live.install_into_singletons();
        drop(replaced);
        let team_factory_effects = self
            .effect_scope
            .take()
            .expect("RuntimeWorldStage missing effect scope")
            .finish();
        StagedRuntimeWorld {
            globals: staged,
            team_factory_effects,
            transaction_lock: self
                .transaction_lock
                .take()
                .expect("RuntimeWorldStage missing transaction lock"),
        }
    }
}

impl Drop for RuntimeWorldStage {
    fn drop(&mut self) {
        let Some(live) = self.live.take() else {
            return;
        };

        // Discard whatever the candidate map created, then put back precisely
        // the bundle that was active before `begin`.  The effect scope remains
        // active during the raw swap and drops its deferred callbacks after it.
        let staged = RuntimeWorldGlobals::take_from_singletons();
        drop(staged);
        let replaced = live.install_into_singletons();
        drop(replaced);
    }
}

/// Candidate singleton state returned after the live world has been restored.
/// It is deliberately opaque to Main; only `install_globals` can consume it.
pub struct StagedRuntimeWorld {
    globals: RuntimeWorldGlobals,
    team_factory_effects: Vec<TeamFactoryDeferredEffects>,
    // Keep all other staging out until Main either commits this candidate or
    // drops it.  The live singleton bundle was restored before this token was
    // built, but a second stage still must not interleave its global writes
    // with the pending combined commit.
    transaction_lock: MutexGuard<'static, ()>,
}

impl StagedRuntimeWorld {
    /// Replace currently-live singleton contents with this candidate's bundle.
    ///
    /// This is the no-fail raw half of the host commit.  It intentionally does
    /// *not* run the deferred TeamFactory effects.  Main must first install its
    /// matching `GameLogic`, then call
    /// [`CommittedRuntimeWorldEffects::execute_after_logic_commit`].
    #[must_use = "deferred TeamFactory effects must run after the host GameLogic commit"]
    pub fn install_globals(self) -> CommittedRuntimeWorldEffects {
        let Self {
            globals,
            team_factory_effects,
            transaction_lock,
        } = self;
        let replaced = globals.install_into_singletons();
        drop(replaced);
        CommittedRuntimeWorldEffects {
            team_factory_effects: Some(team_factory_effects),
            transaction_lock: Some(transaction_lock),
        }
    }
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
