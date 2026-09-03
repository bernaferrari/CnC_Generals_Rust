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
//! (`install_ai_data_store` / `install_upgrade_center`); world install/uninstall
//! below keeps those slots pointing at this bundle's instances.
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
    /// a fresh lock. Installing the returned bundle as active makes every
    /// subsequent store access resolve to this world.
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
    ini_ai_data::install_ai_data_store(Arc::clone(&world.ai_data));
    ini_upgrade::install_upgrade_center(Arc::clone(&world.ini_upgrade_center));
    let mut active = ACTIVE
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    active.replace(world)
}

/// Uninstall the active bundle only if it is still `world` (a stale world
/// dropping after a newer world must not deactivate the newer world).
/// Returns `true` when the active slot was cleared.
pub fn uninstall_active_if_current(world: &Arc<EngineStores>) -> bool {
    // Clear the EngineStores slot first and release its lock before touching
    // the Common-side slots: world install takes them in the opposite order.
    let was_current = {
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
    };
    if was_current {
        ini_ai_data::uninstall_ai_data_store_if_current(&world.ai_data);
        ini_upgrade::uninstall_upgrade_center_if_current(&world.ini_upgrade_center);
    }
    was_current
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
