//! EngineStores — GameLogic-owned engine singleton stores.
//!
//! Groups the C++-inherited process-lifetime engine globals that gameplay
//! reads through global accessors into one struct whose lifetime is owned by
//! the GameLogic world lifecycle:
//!
//! - `upgrade_center` — C++ `TheUpgradeCenter` (UpgradeCenter, Upgrade.h).
//! - `ai` — C++ `TheAI` (AI, AI.h), including its `AiData` (`TheAI->getAiData()`).
//! - `ini_upgrade_center` — the Common-crate INI-side `Upgrade.ini` store
//!   (`game_engine::common::ini::ini_upgrade`, C++ TheUpgradeCenter parse half).
//! - `ai_data` — the Common-crate `AIData.ini` parse-side store
//!   (`game_engine::common::ini::ini_ai_data`).
//! - `shroud` — the shroud/fog-of-war manager (C++ PartitionManager shroud).
//!
//! The two Common-crate stores stay *types* of the Common crate (the INI
//! parser there writes them during `AIData`/`Upgrade` block parsing, and
//! `game_engine` cannot depend on `gamelogic`); only their *instances* moved
//! here. Common keeps an active-slot with an engine-lifetime fallback
//! (`install_ai_data_store` / `install_upgrade_center`); the install/pop
//! boundaries below keep those slots pointing at the head bundle's instances.
//!
//! C++ engine init order (GameEngine.cpp `GameEngine::init`): TheUpgradeCenter
//! subsystem (:468) is created and `Upgrade.ini`-loaded before TheAI (:480),
//! and both precede TheGameLogic (:481). `EngineStores::engine_defaults`
//! preserves that order. Per game, C++ `GameLogic::clearGameData` resets
//! TheAI (GameLogic.cpp:436) while TheUpgradeCenter content persists across
//! matches (upgrades are INI-level state); the world bundle below mirrors
//! that split: fresh `AI` per world, upgrade-center content cloned from the
//! engine-lifetime store so INI-loaded definitions survive world turnover.
//!
//! Resolution model: accessors resolve through the innermost
//! [`with_active_stores`] scope on the calling thread, else the head of the
//! active-bundle stack, else the engine-lifetime bundle while no world is
//! active (C++ has exactly one process-lifetime world). World bundles are
//! created pure ([`new_for_world`], no active-slot write) and installed at
//! explicit Main boundaries — world start (`GameLogic::new` outside a staged
//! restore; `reset`/`start_new_game` for staged candidates) and staged-world
//! commit. `install_active` *pushes* onto the stack and world drop *pops*
//! (reinstating the bundle the dropping world displaced), so a staged
//! candidate that fails before commit restores the live world's resolution
//! instead of unselecting it. Per-world store mutations and lock poisoning
//! die with the world instead of leaking across tests or matches.
//!
//! C++ accessor-name mapping is preserved at the existing call sites:
//! `ctx.upgrade_center()` ~ `TheUpgradeCenter`, `ctx.ai()` ~ `TheAI`.

use std::cell::RefCell;
use std::sync::{Arc, LazyLock, Mutex, RwLock};

use game_engine::common::ini::ini_ai_data::{self, AIDataStore};
use game_engine::common::ini::ini_upgrade::{self, UpgradeCenter as IniUpgradeCenter};

use crate::ai::AI;
use crate::system::shroud_manager::ShroudManager;
use crate::upgrade::center::UpgradeCenter;

/// The C++-inherited engine stores owned by a GameLogic world.
pub struct EngineStores {
    /// C++ `TheUpgradeCenter`. World bundles hold a snapshot clone of the
    /// engine-lifetime content (INI definitions persist across worlds, C++
    /// GameEngine.cpp:468) under a fresh lock so per-world poisoning and
    /// scripted leftover registrations cannot leak into other worlds.
    upgrade_center: Arc<RwLock<UpgradeCenter>>,
    /// C++ `TheAI` (C++ AI.cpp:280). Fresh per world, mirroring the
    /// contents swap the whole-world restore transaction already performs at
    /// map-load boundaries and C++ `TheAI->reset()` at clearGameData.
    ai: Arc<RwLock<AI>>,
    /// Common-crate INI-side TheUpgradeCenter store (`Upgrade.ini` parse
    /// state). Engine-lifetime bundles share the Common process store; world
    /// bundles hold a snapshot clone under a fresh lock.
    ini_upgrade_center: Arc<RwLock<IniUpgradeCenter>>,
    /// Common-crate `AIData.ini` parse-side store. Same split as
    /// `ini_upgrade_center`: shared engine-lifetime store, snapshot per world.
    ai_data: Arc<RwLock<AIDataStore>>,
    /// Shroud/fog-of-war manager (C++ PartitionManager shroud state). World
    /// bundles snapshot-clone the engine-lifetime content under a fresh lock
    /// so per-world mutations die with the world.
    shroud: Arc<Mutex<ShroudManager>>,
}

impl EngineStores {
    /// Engine-lifetime defaults in C++ engine-init order: TheUpgradeCenter
    /// (with its built-in `init()` veterancy templates) before TheAI.
    fn engine_defaults() -> Self {
        let mut center = UpgradeCenter::new();
        // C++ UpgradeCenter::init runs before Upgrade.ini is parsed.
        center.init();
        Self {
            upgrade_center: Arc::new(RwLock::new(center)),
            ai: Arc::new(RwLock::new(AI::new())),
            // Engine-lifetime bundles share the Common stores themselves so
            // INI loads outside any world land in the store gameplay reads.
            ini_upgrade_center: ini_upgrade::process_lifetime_upgrade_center(),
            ai_data: ini_ai_data::process_lifetime_ai_data_store(),
            shroud: Arc::new(Mutex::new(ShroudManager::new())),
        }
    }

    /// Create the stores for a new GameLogic world: a fresh `AI` and a
    /// snapshot of the current engine-lifetime upgrade-center content under
    /// a fresh lock. Pure — installing the bundle as active is the caller's
    /// explicit boundary ([`install_active`]).
    pub fn new_for_world() -> Self {
        let upgrade_center = engine_upgrade_center_snapshot();
        Self {
            upgrade_center: Arc::new(RwLock::new(upgrade_center)),
            ai: Arc::new(RwLock::new(AI::new())),
            ini_upgrade_center: ini_upgrade_center_snapshot(),
            ai_data: ai_data_snapshot(),
            shroud: Arc::new(Mutex::new(engine_shroud_snapshot())),
        }
    }

    /// C++ `TheUpgradeCenter`.
    pub fn upgrade_center(&self) -> &Arc<RwLock<UpgradeCenter>> {
        &self.upgrade_center
    }

    /// C++ `TheAI`.
    pub fn ai(&self) -> &Arc<RwLock<AI>> {
        &self.ai
    }

    /// Common INI-side UpgradeCenter store.
    pub fn ini_upgrade_center(&self) -> &Arc<RwLock<IniUpgradeCenter>> {
        &self.ini_upgrade_center
    }

    /// Common `AIData.ini` store.
    pub fn ai_data(&self) -> &Arc<RwLock<AIDataStore>> {
        &self.ai_data
    }

    /// Shroud/fog-of-war manager.
    pub fn shroud(&self) -> &Arc<Mutex<ShroudManager>> {
        &self.shroud
    }
}

/// Engine-lifetime bundle. C++ keeps one process-lifetime engine; this is
/// the fallback store for work outside any GameLogic world (engine boot INI
/// loads, headless snippets, tests that never construct a world).
static PROCESS_LIFETIME: LazyLock<Arc<EngineStores>> =
    LazyLock::new(|| Arc::new(EngineStores::engine_defaults()));

/// The installed world bundles, newest first. `install_active` pushes and
/// world drop pops, so the stack itself is the restoration record: a dropping
/// world reinstates the bundle it displaced instead of unselecting the live
/// world (the single-slot predecessor cleared the slot, which made the
/// staged-restore rollback resolve the engine-lifetime fallback). C++ has
/// exactly one live world; the stack only ever holds nested install
/// boundaries — a candidate staged while a live world plays.
static ACTIVE: RwLock<Vec<Arc<EngineStores>>> = RwLock::new(Vec::new());

thread_local! {
    /// Innermost-first scoped publication stack for [`with_active_stores`].
    /// Thread-local like Main's `with_gameworld_authority`: a scope pins
    /// resolution for the operation that opened it on this thread without
    /// re-authoring resolution for unrelated worker threads.
    static SCOPED_ACTIVE: RefCell<Vec<Arc<EngineStores>>> =
        const { RefCell::new(Vec::new()) };
}

fn active_locked() -> Arc<EngineStores> {
    let active = ACTIVE
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    active
        .last()
        .cloned()
        .unwrap_or_else(|| Arc::clone(&PROCESS_LIFETIME))
}

/// The active engine-store bundle (C++: the one live world's singletons):
/// the innermost [`with_active_stores`] scope on this thread, else the head
/// of the install stack, else the engine-lifetime fallback.
pub fn active() -> Arc<EngineStores> {
    if let Some(scoped) = SCOPED_ACTIVE.with(|stack| stack.borrow().last().cloned()) {
        return scoped;
    }
    active_locked()
}

/// True while `bundle` heads the install stack (a [`with_active_stores`]
/// scope is deliberately ignored). Guard for idempotent installs: a world
/// starting a game while already the active one must not push a duplicate.
pub fn is_active(bundle: &Arc<EngineStores>) -> bool {
    ACTIVE
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .last()
        .is_some_and(|head| Arc::ptr_eq(head, bundle))
}

/// Point the Common INI-side slots at `world`'s stores. Install order:
/// Common slots first, the ACTIVE stack second.
fn install_common_slots(world: &Arc<EngineStores>) {
    ini_ai_data::install_ai_data_store(Arc::clone(&world.ai_data));
    ini_upgrade::install_upgrade_center(Arc::clone(&world.ini_upgrade_center));
}

/// Install a world bundle as the new head of the active stack and return the
/// bundle it displaced (the previous head, if any). The displaced bundle is
/// reinstated automatically when this one is later removed by
/// [`uninstall_active_if_current`] or [`restore_active`] — the stack itself
/// is the restoration record callers used to have to keep (and could drop).
pub fn install_active(world: Arc<EngineStores>) -> Option<Arc<EngineStores>> {
    install_common_slots(&world);
    let mut active = ACTIVE
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = active.last().cloned();
    active.push(world);
    previous
}

/// Remove `world` from the active stack. While it is still the head, the
/// bundle it displaced on install becomes the head again (and the Common
/// INI-side slots are re-pointed at it), so a dropping world — including a
/// staged candidate dropping before commit — restores the live world's
/// resolution instead of unselecting it. While a newer bundle is the head,
/// that newer world keeps resolution and only `world`'s buried stack entry is
/// dropped, so no later pop can reinstate a dead world. Returns `true` when
/// the head was changed.
pub fn uninstall_active_if_current(world: &Arc<EngineStores>) -> bool {
    // Clear the EngineStores slot first and release its lock before touching
    // the Common-side slots: world install takes them in the opposite order.
    let (head_was_cleared, restored) = {
        let mut active = ACTIVE
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let head_is_world = active
            .last()
            .is_some_and(|head| Arc::ptr_eq(head, world));
        if head_is_world {
            active.pop();
            (true, active.last().cloned())
        } else {
            // A stale world dropping after a newer world must not deactivate
            // the newer world; only its buried entry is forgotten. The head
            // and the Common slots (owned by the head) are unaffected.
            active.retain(|entry| !Arc::ptr_eq(entry, world));
            (false, None)
        }
    };
    if head_was_cleared {
        ini_ai_data::uninstall_ai_data_store_if_current(&world.ai_data);
        ini_upgrade::uninstall_upgrade_center_if_current(&world.ini_upgrade_center);
        if let Some(restored) = restored {
            // The reinstated head must own the Common INI funnels too.
            install_common_slots(&restored);
        }
    }
    head_was_cleared
}

/// Undo one [`install_active`] of `expected_removed` by reinstating
/// `previous` — the bundle that install returned. The pop happens only while
/// `expected_removed` is still the head (the same `Arc::ptr_eq` discipline as
/// [`uninstall_active_if_current`]): a newer bundle installed in between must
/// not be silently deactivated. Returns `true` when the head was changed.
///
/// The stack keeps the pairing itself — the entry beneath `expected_removed`
/// is `previous` by construction — so a disciplined call reinstates through
/// the pop alone; a hand-assembled pair that does not match still ends with
/// `previous` at the head.
pub(crate) fn restore_active(
    expected_removed: &Arc<EngineStores>,
    previous: Option<Arc<EngineStores>>,
) -> bool {
    // Same lock ordering as uninstall: ACTIVE first, Common slots after.
    let (head_was_cleared, reinstated) = {
        let mut active = ACTIVE
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !active
            .last()
            .is_some_and(|head| Arc::ptr_eq(head, expected_removed))
        {
            return false;
        }
        active.pop();
        let beneath_is_previous = match (active.last(), previous.as_ref()) {
            (Some(beneath), Some(previous)) => Arc::ptr_eq(beneath, previous),
            (None, None) => true,
            _ => false,
        };
        if !beneath_is_previous {
            if let Some(previous) = previous {
                // Head := `previous` exactly; any deeper chain stays beneath.
                active.push(previous);
            }
        }
        (true, active.last().cloned())
    };
    if head_was_cleared {
        ini_ai_data::uninstall_ai_data_store_if_current(&expected_removed.ai_data);
        ini_upgrade::uninstall_upgrade_center_if_current(&expected_removed.ini_upgrade_center);
        if let Some(reinstated) = reinstated {
            install_common_slots(&reinstated);
        }
    }
    head_was_cleared
}

/// Publish `bundle` as this thread's active-store resolution for the
/// duration of `f`, restoring the previous resolution afterwards (normal
/// return or unwind) — the store-side twin of Main's
/// `with_gameworld_authority` scope. Inside `f`, every legacy accessor
/// ([`active`], [`the_ai`], [`shroud_manager`], [`upgrade_center`]) and the
/// Common INI parse slots resolve `bundle`, so raw boundary swaps and tick
/// scopes pin their target without last-writer ambiguity against whatever
/// the global install stack happens to hold.
///
/// The EngineStores override is thread-local (worker threads keep resolving
/// the install-stack head); the Common INI slots are process-global, so a
/// scope must not outlive the thread that opened it — the single host thread
/// that owns world turnover does. Whole-world turnover (install/uninstall)
/// must not happen inside `f`: a bundle installed within the scope that
/// outlives it legitimately owns the Common slots afterwards.
pub fn with_active_stores<R>(bundle: &Arc<EngineStores>, f: impl FnOnce() -> R) -> R {
    /// Pops this scope's publication and restores the Common INI slots it
    /// displaced; runs exactly once, including on unwind.
    struct RestoreScope {
        bundle: Arc<EngineStores>,
        prev_ai_data: Option<Arc<RwLock<AIDataStore>>>,
        prev_ini_upgrade: Option<Arc<RwLock<IniUpgradeCenter>>>,
    }

    impl Drop for RestoreScope {
        fn drop(&mut self) {
            SCOPED_ACTIVE.with(|stack| {
                let mut stack = stack.borrow_mut();
                if stack
                    .last()
                    .is_some_and(|head| Arc::ptr_eq(head, &self.bundle))
                {
                    stack.pop();
                }
            });
            // Restore the Common slots only while they still resolve this
            // scope's stores: a bundle installed inside `f` that outlives the
            // scope owns them now and must not be clobbered.
            if Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), &self.bundle.ai_data) {
                match self.prev_ai_data.take() {
                    Some(previous) => {
                        ini_ai_data::install_ai_data_store(previous);
                    }
                    None => {
                        ini_ai_data::uninstall_ai_data_store_if_current(&self.bundle.ai_data);
                    }
                }
            }
            if Arc::ptr_eq(
                &ini_upgrade::get_upgrade_center(),
                &self.bundle.ini_upgrade_center,
            ) {
                match self.prev_ini_upgrade.take() {
                    Some(previous) => {
                        ini_upgrade::install_upgrade_center(previous);
                    }
                    None => {
                        ini_upgrade::uninstall_upgrade_center_if_current(
                            &self.bundle.ini_upgrade_center,
                        );
                    }
                }
            }
        }
    }

    let restore = RestoreScope {
        prev_ai_data: ini_ai_data::install_ai_data_store(Arc::clone(&bundle.ai_data)),
        prev_ini_upgrade: ini_upgrade::install_upgrade_center(Arc::clone(
            &bundle.ini_upgrade_center,
        )),
        bundle: Arc::clone(bundle),
    };
    SCOPED_ACTIVE.with(|stack| stack.borrow_mut().push(Arc::clone(bundle)));
    let _restore = restore;
    f()
}

/// Create the stores for a new GameLogic world (C++ engine-init order:
/// stores precede the world that owns them). Pure — no active-slot write:
/// installing the returned bundle is the caller's explicit boundary
/// ([`install_active`]); Main installs at world start/commit and uninstalls
/// on world drop.
pub fn new_for_world() -> Arc<EngineStores> {
    Arc::new(EngineStores::new_for_world())
}

/// C++ `TheUpgradeCenter` accessor: the active bundle's center.
pub fn upgrade_center() -> Arc<RwLock<UpgradeCenter>> {
    Arc::clone(active().upgrade_center())
}

/// Snapshot clone of the engine-lifetime upgrade-center content.
fn engine_upgrade_center_snapshot() -> UpgradeCenter {
    let center = PROCESS_LIFETIME
        .upgrade_center()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    center.clone()
}

/// Snapshot clone of the engine-lifetime shroud content under a fresh lock,
/// so a new world inherits seeded/INI-level shroud state while per-world
/// mutations cannot leak back.
fn engine_shroud_snapshot() -> ShroudManager {
    PROCESS_LIFETIME
        .shroud()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// Snapshot clone of the engine-lifetime Common UpgradeCenter content under
/// a fresh lock, mirroring [`engine_upgrade_center_snapshot`].
fn ini_upgrade_center_snapshot() -> Arc<RwLock<IniUpgradeCenter>> {
    let store = ini_upgrade::process_lifetime_upgrade_center();
    let snapshot = store
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    Arc::new(RwLock::new(snapshot))
}

/// Snapshot clone of the engine-lifetime Common AIData store under a fresh
/// lock so per-world mutations die with the world.
fn ai_data_snapshot() -> Arc<RwLock<AIDataStore>> {
    let store = ini_ai_data::process_lifetime_ai_data_store();
    let snapshot = store
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    Arc::new(RwLock::new(snapshot))
}

/// Shroud/fog-of-war manager accessor: the active bundle's manager.
pub fn shroud_manager() -> Arc<Mutex<ShroudManager>> {
    Arc::clone(active().shroud())
}

/// C++ `TheAI` accessor: the active bundle's AI.
pub fn the_ai() -> Arc<RwLock<AI>> {
    Arc::clone(active().ai())
}

/// Move `bundle`'s AI contents out for a whole-world restore transaction
/// while preserving the lock identity aliases hold (contents swap, C++
/// AI.cpp:280 wrapper semantics). The runtime world transaction owns the
/// only raw use of this boundary API and passes the explicit bundle it
/// captured at `begin`, so the swap never depends on ambient resolution.
pub(crate) fn take_ai_for_world_boundary(bundle: &Arc<EngineStores>) -> AI {
    let mut ai = bundle
        .ai()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::replace(&mut *ai, AI::new())
}

/// Install AI contents into `bundle` at a whole-world restore boundary and
/// return the contents they replaced. See [`take_ai_for_world_boundary`].
pub(crate) fn replace_ai_for_world_boundary(bundle: &Arc<EngineStores>, next: AI) -> AI {
    let mut ai = bundle
        .ai()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::replace(&mut *ai, next)
}

/// Test-only depth of the install stack (asserting full cleanup).
#[cfg(test)]
fn active_stack_depth() -> usize {
    ACTIVE
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_newer_world_restores_previous_as_active() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        // Construct A, construct B (both at their start boundaries), drop B:
        // the audit's headline ordering. Resolution must fail closed to A,
        // never to the engine-lifetime fallback.
        let a = new_for_world();
        install_active(Arc::clone(&a));
        let b = new_for_world();
        let displaced_by_b = install_active(Arc::clone(&b));
        assert!(displaced_by_b.as_ref().is_some_and(|p| Arc::ptr_eq(p, &a)));
        assert!(Arc::ptr_eq(&active(), &b));

        uninstall_active_if_current(&b); // GameLogic::drop for B
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(Arc::ptr_eq(&the_ai(), a.ai()));
        assert!(Arc::ptr_eq(&upgrade_center(), a.upgrade_center()));
        assert!(Arc::ptr_eq(&shroud_manager(), a.shroud()));
        // The Common INI funnels mirror the reinstated head.
        assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), a.ai_data()));
        assert!(Arc::ptr_eq(
            &ini_upgrade::get_upgrade_center(),
            a.ini_upgrade_center()
        ));

        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn staged_restore_rollback_never_touches_process_lifetime() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        // Live world A with content worth protecting.
        let a = new_for_world();
        install_active(Arc::clone(&a));
        let shroud_before = a
            .shroud()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state();
        let process_shroud_before = PROCESS_LIFETIME
            .shroud()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state();

        // RuntimeWorldStage::begin: capture the live bundle explicitly and
        // take the live AI/shroud contents out (fresh defaults stay behind
        // as the staging scratch the candidate map writes into).
        let live_bundle = active();
        assert!(Arc::ptr_eq(&live_bundle, &a));
        let live_ai = take_ai_for_world_boundary(&live_bundle);
        let live_shroud = with_active_stores(&live_bundle, || {
            let manager = shroud_manager();
            let mut guard = manager
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, ShroudManager::new())
        });

        // The candidate installs at its start_new_game boundary.
        let b = new_for_world();
        install_active(Arc::clone(&b));

        // Early Err: locals drop newest-declared-first, so the staged
        // GameLogic B drops *before* the RuntimeWorldStage. B's drop pops it
        // and reinstates A as head...
        uninstall_active_if_current(&b);
        assert!(Arc::ptr_eq(&active(), &a));

        // ...then the stage's Drop reinstalls the live contents through the
        // CAPTURED bundle handle (pinned via with_active_stores), never
        // through ambient/engine-lifetime resolution. This is exactly the
        // ordering that used to install A's AI/shroud into the
        // engine-lifetime bundle.
        let pinned_bundle = Arc::clone(&live_bundle);
        with_active_stores(&live_bundle, move || {
            let scratch_ai = take_ai_for_world_boundary(&pinned_bundle);
            drop(scratch_ai);
            let replaced = replace_ai_for_world_boundary(&pinned_bundle, live_ai);
            drop(replaced);
            // Shroud rides the same pinned ambient funnel.
            let manager = shroud_manager();
            let mut guard = manager
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let scratch_shroud = std::mem::replace(&mut *guard, live_shroud);
            drop(scratch_shroud);
        });

        // A serves its own bundle and contents again...
        assert!(Arc::ptr_eq(&the_ai(), a.ai()));
        assert!(Arc::ptr_eq(&shroud_manager(), a.shroud()));
        let shroud_after = a
            .shroud()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state();
        assert_eq!(shroud_before, shroud_after);
        // ...and the engine-lifetime bundle is untouched (the pollution the
        // audit found landed there because ambient resolution fell back to
        // it after B's drop cleared the slot).
        let process_shroud_after = PROCESS_LIFETIME
            .shroud()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state();
        assert_eq!(process_shroud_before, process_shroud_after);

        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn world_construction_does_not_touch_the_active_slot() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        let before = active();

        let _bundled = new_for_world();
        let _plain = EngineStores::new_for_world();

        assert_eq!(active_stack_depth(), 0);
        assert!(Arc::ptr_eq(&active(), &before));
    }

    #[test]
    fn with_active_stores_pins_and_restores_resolution() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        let a = new_for_world();
        install_active(Arc::clone(&a));
        let b = new_for_world();

        let scoped_saw_bundle = with_active_stores(&b, || {
            assert!(Arc::ptr_eq(&active(), &b));
            assert!(Arc::ptr_eq(&the_ai(), b.ai()));
            assert!(Arc::ptr_eq(&shroud_manager(), b.shroud()));
            assert!(Arc::ptr_eq(
                &ini_ai_data::get_ai_data_store(),
                b.ai_data()
            ));
            // Nested scopes: innermost wins, outer restored after.
            let nested = with_active_stores(&a, || Arc::ptr_eq(&active(), &a));
            assert!(nested);
            assert!(Arc::ptr_eq(&active(), &b));
            true
        });
        assert!(scoped_saw_bundle);
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), a.ai_data()));
        assert!(Arc::ptr_eq(
            &ini_upgrade::get_upgrade_center(),
            a.ini_upgrade_center()
        ));

        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn with_active_stores_restores_on_unwind() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        let b = new_for_world();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_active_stores(&b, || panic!("scoped active-stores unwind probe"));
        }))
        .is_err();
        assert!(panicked);

        // Scoped publication and the Common INI slots are restored even on
        // unwind: resolution falls back to the engine-lifetime store.
        assert!(!Arc::ptr_eq(&active(), &b));
        assert!(Arc::ptr_eq(&active(), &PROCESS_LIFETIME));
        assert!(Arc::ptr_eq(
            &ini_ai_data::get_ai_data_store(),
            &ini_ai_data::process_lifetime_ai_data_store()
        ));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn restore_active_reinstates_previous_only_while_head() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        let a = new_for_world();
        let b = new_for_world();
        let c = new_for_world();
        install_active(Arc::clone(&a));

        let displaced_by_b = install_active(Arc::clone(&b));
        assert!(displaced_by_b.as_ref().is_some_and(|p| Arc::ptr_eq(p, &a)));
        assert!(restore_active(&b, displaced_by_b));
        assert!(Arc::ptr_eq(&active(), &a));

        // A newer install between install and restore must not be clobbered.
        let displaced_by_c = install_active(Arc::clone(&c));
        assert!(displaced_by_c.as_ref().is_some_and(|p| Arc::ptr_eq(p, &a)));
        assert!(!restore_active(&b, Some(Arc::clone(&a))));
        assert!(Arc::ptr_eq(&active(), &c));

        assert!(uninstall_active_if_current(&c));
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn dropping_buried_world_cannot_be_reinstated_by_later_pop() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        // The staged-commit ordering: A live, B staged on top, A dies while
        // B is the newer world (host drops the old world after install).
        let a = new_for_world();
        let b = new_for_world();
        install_active(Arc::clone(&a));
        install_active(Arc::clone(&b));

        assert!(!uninstall_active_if_current(&a)); // buried: head unchanged
        assert!(Arc::ptr_eq(&active(), &b));
        assert_eq!(active_stack_depth(), 1);

        // The newer world dying later must fall back to the engine-lifetime
        // store, never reinstate the dead world A.
        assert!(uninstall_active_if_current(&b));
        assert_eq!(active_stack_depth(), 0);
        assert!(Arc::ptr_eq(&active(), &PROCESS_LIFETIME));
    }
}
