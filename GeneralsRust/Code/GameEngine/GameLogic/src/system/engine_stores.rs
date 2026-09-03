//! EngineStores — GameLogic-owned engine singleton stores.
//!
//! Groups the C++-inherited process-lifetime engine globals that gameplay
//! reads through global accessors into one struct whose lifetime is owned by
//! the GameLogic world lifecycle:
//!
//! - `upgrade_center` — C++ `TheUpgradeCenter` (UpgradeCenter, Upgrade.h).
//! - `ai` — C++ `TheAI` (AI, AI.h), including its `AiData` (`TheAI->getAiData()`).
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
//! Resolution model: accessors resolve through the *active* bundle
//! ([`active()`]). The engine-lifetime bundle is the fallback while no world
//! is active (C++ has exactly one process-lifetime world); constructing a
//! world ([`EngineStores::new_for_world`] + [`install_active`]) installs a
//! fresh bundle, so per-world store mutations and lock poisoning die with
//! the world instead of leaking across tests or matches.
//!
//! C++ accessor-name mapping is preserved at the existing call sites:
//! `ctx.upgrade_center()` ~ `TheUpgradeCenter`, `ctx.ai()` ~ `TheAI`.

use std::sync::{Arc, LazyLock, RwLock};

use crate::ai::AI;
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
        }
    }

    /// Create the stores for a new GameLogic world: a fresh `AI` and a
    /// snapshot of the current engine-lifetime upgrade-center content under
    /// a fresh lock. Installing the returned bundle as active makes every
    /// subsequent store access resolve to this world.
    pub fn new_for_world() -> Self {
        let upgrade_center = engine_upgrade_center_snapshot();
        Self {
            upgrade_center: Arc::new(RwLock::new(upgrade_center)),
            ai: Arc::new(RwLock::new(AI::new())),
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
}

/// Engine-lifetime bundle. C++ keeps one process-lifetime engine; this is
/// the fallback store for work outside any GameLogic world (engine boot INI
/// loads, headless snippets, tests that never construct a world).
static PROCESS_LIFETIME: LazyLock<Arc<EngineStores>> =
    LazyLock::new(|| Arc::new(EngineStores::engine_defaults()));

/// The active world bundle, if a GameLogic world installed one.
static ACTIVE: RwLock<Option<Arc<EngineStores>>> = RwLock::new(None);

fn active_locked() -> Arc<EngineStores> {
    let active = ACTIVE
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match active.as_ref() {
        Some(bundle) => Arc::clone(bundle),
        None => Arc::clone(&PROCESS_LIFETIME),
    }
}

/// The active engine-store bundle (C++: the one live world's singletons).
pub fn active() -> Arc<EngineStores> {
    active_locked()
}

/// Install a world bundle as active and return the bundle it replaced.
pub fn install_active(world: Arc<EngineStores>) -> Option<Arc<EngineStores>> {
    let mut active = ACTIVE
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    active.replace(world)
}

/// Uninstall the active bundle only if it is still `world` (a stale world
/// dropping after a newer world must not deactivate the newer world).
/// Returns `true` when the active slot was cleared.
pub fn uninstall_active_if_current(world: &Arc<EngineStores>) -> bool {
    let mut active = ACTIVE
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match active.as_ref() {
        Some(current) if Arc::ptr_eq(current, world) => {
            *active = None;
            true
        }
        _ => false,
    }
}

/// Create the stores for a new GameLogic world and install them as the
/// active bundle in one step (C++ engine-init order: stores precede the
/// world that owns them). Returns the bundle Arc for later
/// [`uninstall_active_if_current`] on world drop.
pub fn new_for_world_installed() -> Arc<EngineStores> {
    let world = Arc::new(EngineStores::new_for_world());
    install_active(Arc::clone(&world));
    world
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

/// C++ `TheAI` accessor: the active bundle's AI.
pub fn the_ai() -> Arc<RwLock<AI>> {
    Arc::clone(active().ai())
}

/// Move the active AI contents out for a whole-world restore transaction
/// while preserving the lock identity aliases hold (contents swap, C++
/// AI.cpp:280 wrapper semantics). The runtime world transaction owns the
/// only raw use of this boundary API.
pub(crate) fn take_ai_for_world_boundary() -> AI {
    let ai_store = the_ai();let mut ai = ai_store
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::replace(&mut *ai, AI::new())
}

/// Install AI contents at a whole-world restore boundary and return the
/// contents they replaced. See [`take_ai_for_world_boundary`].
pub(crate) fn replace_ai_for_world_boundary(next: AI) -> AI {
    let ai_store = the_ai();let mut ai = ai_store
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::replace(&mut *ai, next)
}
