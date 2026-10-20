//! Main save staging isolates only the remaining shared metadata systems.
//! Main prepared SidesList definitions already belong to its GameLogic.
//! Terrain, shroud, visuals and AI already belong to their driving world and
//! are never taken or swapped here. Existing global metadata still requires
//! the process transaction mutex and the original deferred callback order.
use super::*;
use crate::system::engine_stores::WorldServices;
struct HostRuntimeGlobals {
    players: PlayerList,
    teams: TeamFactory,
    script_engine: Option<ScriptEngine>,
    named_objects: NamedObjectTrackerState,
    areas: AreaTrackerState,
    pending_team_script_events: TeamScriptEventQueue,
}

impl HostRuntimeGlobals {
    /// Move singleton contents into an owned bundle and leave fresh defaults
    /// behind for map/bootstrap work.  Lock poisoning is recovered here rather
    /// than surfacing a fallible half-take: failure handling must always be
    /// able to restore a coherent active world.
    ///
    /// The metadata below is still process-global. The explicit service scope
    /// pins only transitive terrain/shroud/visual readers; it does not provide
    /// a separate PlayerList, TeamFactory, or script/tracker store.
    fn take_from_singletons(bundle: &Arc<WorldServices>) -> Self {
        // Keep this order stable.  We never hold two locks at once, but a
        // deterministic order makes future extensions auditable.
        engine_stores::with_world_services(bundle, || {
            let players = {
                let mut guard = player_list()
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                std::mem::replace(&mut *guard, PlayerList::new())
            };
            let teams = get_team_factory().replace_for_world_boundary(TeamFactory::new());
            let script_engine = {
                let handle = get_script_engine();
                let mut guard = handle
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                std::mem::take(&mut *guard)
            };
            let named_objects = get_named_object_tracker().take_state_for_world_boundary();
            let areas = get_area_tracker().take_state_for_world_boundary();
            let pending_team_script_events = take_pending_team_script_events_for_world_boundary();

            Self {
                players,
                teams,
                script_engine,
                named_objects,
                areas,
                pending_team_script_events,
            }
        })
    }

    /// Install this bundle into the stable singleton wrappers and return the
    /// bundle it replaced.  No normal TeamFactory guard is created here, so no
    /// create-action callback can run while values are only half installed.
    /// The service scope pins transitive readers while these remaining global
    /// metadata values are installed. It does not select their ownership.
    fn install_into_singletons(self, bundle: &Arc<WorldServices>) -> Self {
        engine_stores::with_world_services(bundle, move || {
            let Self {
                players,
                teams,
                script_engine,
                named_objects,
                areas,
                pending_team_script_events,
            } = self;

            let old_players = {
                let mut guard = player_list()
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                std::mem::replace(&mut *guard, players)
            };
            let old_teams = get_team_factory().replace_for_world_boundary(teams);
            let old_script_engine = {
                let handle = get_script_engine();
                let mut guard = handle
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                std::mem::replace(&mut *guard, script_engine)
            };
            let old_named_objects =
                get_named_object_tracker().replace_state_for_world_boundary(named_objects);
            let old_areas = get_area_tracker().replace_state_for_world_boundary(areas);
            let old_pending_team_script_events =
                replace_pending_team_script_events_for_world_boundary(pending_team_script_events);

            Self {
                players: old_players,
                teams: old_teams,
                script_engine: old_script_engine,
                named_objects: old_named_objects,
                areas: old_areas,
                pending_team_script_events: old_pending_team_script_events,
            }
        })
    }
}

pub struct HostRuntimeWorldStage {
    live: Option<HostRuntimeGlobals>,
    /// Explicit driving services; their terrain/shroud/visual values are never swapped.
    live_bundle: Arc<WorldServices>,
    effect_scope: Option<WorldRuntimeStageScope>,
    transaction_lock: Option<MutexGuard<'static, ()>>,
}

impl HostRuntimeWorldStage {
    /// Begin a whole-world staging transaction.  Nested *effect* guards are
    /// supported, but nested singleton transactions are a programmer error:
    /// there is only one set of stable singleton wrappers to isolate.
    pub fn begin(live_bundle: &Arc<WorldServices>) -> Self {
        assert!(
            !world_runtime_staging_active(),
            "nested HostRuntimeWorldStage would overwrite the outer candidate world"
        );
        let transaction_lock = match world_runtime_transaction_lock().lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        };
        let effect_scope = WorldRuntimeStageScope::enter();
        // The caller supplies the driving services; no active-world selection.
        let live_bundle = Arc::clone(live_bundle);
        let live = HostRuntimeGlobals::take_from_singletons(&live_bundle);
        Self {
            live: Some(live),
            live_bundle,
            effect_scope: Some(effect_scope),
            transaction_lock: Some(transaction_lock),
        }
    }

    /// Extract the completed candidate bundle and restore the pre-stage live
    /// singleton contents.  The caller can now validate/prepare the host
    /// commit while the active match remains completely intact.
    pub fn finish_and_restore_live(
        mut self,
        candidate_bundle: &Arc<WorldServices>,
    ) -> StagedHostRuntimeWorld {
        // Capture only shared metadata, using the explicit candidate services.
        let candidate_bundle = Arc::clone(candidate_bundle);
        let staged = HostRuntimeGlobals::take_from_singletons(&candidate_bundle);
        let live = self
            .live
            .take()
            .expect("HostRuntimeWorldStage missing pre-stage globals");
        let replaced = live.install_into_singletons(&self.live_bundle);
        drop(replaced);
        // Return ambient resolution to the live world before handing the
        // opaque candidate token to Main: between this call and the host
        // commit the still-playable match must keep resolving its own bundle.
        // (Skipped when the candidate never installed — staging then ran
        // against the live bundle itself and the head is already correct.)
        if !Arc::ptr_eq(&candidate_bundle, &self.live_bundle) {
            engine_stores::uninstall_services(&candidate_bundle);
        }
        let team_factory_effects = self
            .effect_scope
            .take()
            .expect("HostRuntimeWorldStage missing effect scope")
            .finish();
        StagedHostRuntimeWorld {
            globals: staged,
            candidate_bundle,
            team_factory_effects,
            transaction_lock: self
                .transaction_lock
                .take()
                .expect("HostRuntimeWorldStage missing transaction lock"),
        }
    }
}

impl Drop for HostRuntimeWorldStage {
    fn drop(&mut self) {
        let Some(live) = self.live.take() else {
            return;
        };

        // Discard whatever the candidate map created, then put back precisely
        // the bundle that was active before `begin` — through the captured
        // live-bundle handle, never ambient resolution (and so never the
        // engine-lifetime fallback).  The effect scope remains active during
        // the raw swap and drops its deferred callbacks after it.
        let staged = HostRuntimeGlobals::take_from_singletons(&self.live_bundle);
        drop(staged);
        let replaced = live.install_into_singletons(&self.live_bundle);
        drop(replaced);
    }
}

/// Candidate singleton state returned after the live world has been restored.
/// It is deliberately opaque to Main; native preflight must succeed before
/// the resulting checked token can install it.
pub struct StagedHostRuntimeWorld {
    globals: HostRuntimeGlobals,
    /// The candidate world's engine-store bundle: the commit makes it the
    /// ambient resolution target and lands the candidate contents in it.
    candidate_bundle: Arc<WorldServices>,
    team_factory_effects: Vec<TeamFactoryDeferredEffects>,
    // Keep all other staging out until Main either commits this candidate or
    // drops it.  The live singleton bundle was restored before this token was
    // built, but a second stage still must not interleave its global writes
    // with the pending combined commit.
    transaction_lock: MutexGuard<'static, ()>,
}

impl StagedHostRuntimeWorld {
    /// Validate while the old Main world and UI are still intact. On failure
    /// this detached candidate is dropped; no callback or singleton install runs.
    pub fn preflight_native(self) -> Result<CheckedNativeRuntimeWorld, String> {
        validate_native_team_effects(&self.team_factory_effects)?;
        Ok(CheckedNativeRuntimeWorld(self))
    }

    /// Private raw half of the checked native commit. Only
    /// `CheckedNativeRuntimeWorld` may reach this operation after preflight.
    /// It installs metadata but never executes a Core callback.
    fn install_globals_unchecked(self) -> CommittedRuntimeWorldEffects {
        let Self {
            globals,
            candidate_bundle,
            team_factory_effects,
            transaction_lock,
        } = self;
        // Make the candidate bundle the ambient target for the commit swap;
        // the host installs the matching GameLogic immediately after, and the
        // old world's drop then only removes that old bundle's stack entry.
        if !engine_stores::is_services_active(&candidate_bundle) {
            engine_stores::install_services(Arc::clone(&candidate_bundle));
        }
        let replaced = globals.install_into_singletons(&candidate_bundle);
        drop(replaced);
        CommittedRuntimeWorldEffects {
            team_factory_effects: Some(team_factory_effects),
            transaction_lock: Some(transaction_lock),
        }
    }
}

/// A native candidate whose Core callback payloads have been proven empty.
/// Only this token exposes the callback-free native commit path.
pub struct CheckedNativeRuntimeWorld(StagedHostRuntimeWorld);

impl CheckedNativeRuntimeWorld {
    pub fn install_globals(self) -> NativeRuntimeWorldCommit {
        NativeRuntimeWorldCommit(self.0.install_globals_unchecked())
    }
}

/// Keeps staging excluded until Main has installed the matching owned world.
#[must_use = "finish the checked native commit after installing Main GameLogic"]
pub struct NativeRuntimeWorldCommit(CommittedRuntimeWorldEffects);

impl NativeRuntimeWorldCommit {
    /// Infallible: preflight proved that every receipt contains no script work.
    /// Do not discover or execute a process-global interpreter at this boundary.
    pub fn finish_after_logic_commit(mut self) {
        drop(self.0.team_factory_effects.take());
        drop(self.0.transaction_lock.take());
    }
}
