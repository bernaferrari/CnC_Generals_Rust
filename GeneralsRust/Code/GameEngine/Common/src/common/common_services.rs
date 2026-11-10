//! Owned Common engine-context services (C++ TheXxx singletons on TheGameEngine).
//!
//! Dense Common modules hang mutable TheXxx state off [`CommonServices`]. Each game
//! instance owns an `Arc<CommonServices>` (on `WorldServices` / the host) and publishes
//! it for free-function bridges via [`with_common_services`] or
//! [`install_common_services`]. Publication is thread-local so two instances on two
//! threads never share RNG / ThingFactory / CRC flags.
//!
//! There is no process-wide bag and no per-thread fallback Arc: unscoped
//! [`common_services`] panics. Ownership lives on the instance; TLS only binds the
//! currently published owner for legacy free functions.

use crate::common::random_value::{RandomState, SessionRandom};
use crate::common::thing::thing_factory::ThingFactory;
use crate::common::thing::module_factory::ModuleFactory;
use crate::common::name_key_generator::NameKeyNamespace;
use crate::common::rts::player::PlayerObjectWorld;
use crate::common::rts::academy_stats::AcademyTemplateContextProvider;
use crate::common::rts::action_manager::ObjectDataProvider;
use crate::common::rts::team::TeamCommandSink;
use crate::common::system::build_assistant::BuildAssistantBackend;
use crate::common::rts::energy::{EnergyObjectLookup, EnergyOwnerCallbacks};
use crate::common::message_stream::message_stream::CommandListAppendFn;
use crate::common::damage_fx::DamageFXStore;
use crate::common::resource_manager::ResourceManager;
use crate::common::ini::ini_online_chat_colors::OnlineChatColors;
use crate::common::ini::ini_webpage_url::WebBrowser;
use crate::common::ini::ini_credits::CreditsManager;
use crate::common::ini_stores::IniStores;
use crate::common::system_stores::SystemStores;

use crate::common::system::geometry::Coord3D;
use crate::common::time::SimulationClock;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use parking_lot::Mutex as ParkingMutex;
use parking_lot::RwLock as ParkingRwLock;

/// GameLogic random stream owned by one game instance. Client/audio streams
/// live on their receiving presentation/audio owners.
#[derive(Debug)]
pub struct RandomServices {
    pub logic: Mutex<RandomState>,
    pub logic_base_seed: AtomicU32,
}

impl Default for RandomServices {
    fn default() -> Self {
        Self {
            logic: Mutex::new(RandomState::default()),
            logic_base_seed: AtomicU32::new(0),
        }
    }
}

impl RandomServices {
    pub fn init_with_seed(&self, seed: u32) {
        match self.logic.lock() {
            Ok(mut g) => g.seed_random(seed),
            Err(p) => p.into_inner().seed_random(seed),
        }
        self.logic_base_seed.store(seed, Ordering::SeqCst);
    }

    pub fn with_logic_mut<R>(&self, f: impl FnOnce(&mut RandomState) -> R) -> R {
        match self.logic.lock() {
            Ok(mut g) => f(&mut g),
            Err(p) => f(&mut p.into_inner()),
        }
    }
}

/// Runtime CRC interval / sync-ignore flags (C++ debug CRC globals).
#[derive(Debug)]
pub struct CrcRuntimeFlags {
    pub debug_ignore_sync_errors: AtomicBool,
    pub net_crc_interval: AtomicI32,
    pub replay_crc_interval: AtomicI32,
}

impl Default for CrcRuntimeFlags {
    fn default() -> Self {
        Self {
            debug_ignore_sync_errors: AtomicBool::new(false),
            net_crc_interval: AtomicI32::new(1),
            replay_crc_interval: AtomicI32::new(100),
        }
    }
}

/// One game-instance owned Common service bag (C++ TheGameEngine singleton fields).
pub struct CommonServices {
    pub random: RandomServices,
    pub crc_flags: CrcRuntimeFlags,
    /// ThingFactory slot for this instance (C++ `TheThingFactory`).
    pub thing_factory: Arc<ParkingMutex<Option<ThingFactory>>>,
    /// ModuleFactory slot for this instance (C++ `TheModuleFactory`).
    pub module_factory: Arc<ParkingMutex<Option<ModuleFactory>>>,
    /// NameKeyGenerator namespace (was process NAME_KEY_STATE bag).
    pub name_key_namespace: Mutex<NameKeyNamespace>,
    /// Session logic RNG publish slot for this instance.
    logic_rng_owner: Mutex<Option<SessionRandom>>,
    /// Simulation clock for this instance (was process SimClockBag).
    pub sim_clock: Mutex<SimulationClock>,
    /// Players with pending C++ recordIncome marks (was PendingIncomeBag).
    pub pending_income: Mutex<HashSet<i32>>,
    /// Optional PlayerObjectWorld bridge (was process PLAYER_OBJECT_WORLD bag).
    pub player_object_world: Mutex<Option<Arc<dyn PlayerObjectWorld>>>,
    /// Academy template context provider (was process ACADEMY_TEMPLATE_CONTEXT bag).
    pub academy_template_context: Mutex<Option<AcademyTemplateContextProvider>>,
    /// ActionManager object data bridge (was process OBJECT_DATA_PROVIDER bag).
    pub object_data_provider: Mutex<Option<Arc<dyn ObjectDataProvider + Send + Sync>>>,
    /// Team home waypoint resolver (was process TEAM_HOME_WAYPOINT bag).
    pub team_home_waypoint_resolver: Mutex<Option<fn(&str) -> Option<Coord3D>>>,
    /// Team command sink (was process TEAM_COMMAND_SINK_SLOT).
    pub team_command_sink: Mutex<Option<Arc<dyn TeamCommandSink>>>,
    /// BuildAssistant backend bridge (was process BUILD_ASSISTANT_BACKEND).
    pub build_assistant_backend: Mutex<Option<Arc<dyn BuildAssistantBackend>>>,
    /// Energy template lookup (was process ENERGY_OBJECT_LOOKUP).
    pub energy_object_lookup: Mutex<Option<Arc<dyn EnergyObjectLookup>>>,
    /// Energy brownout callbacks (was process ENERGY_OWNER_CALLBACKS).
    pub energy_owner_callbacks: Mutex<Option<Arc<dyn EnergyOwnerCallbacks>>>,
    /// MessageStream → CommandList append sink (was process COMMAND_LIST_APPEND_FN).
    pub command_list_append: Mutex<Option<Box<CommandListAppendFn>>>,
    /// AudioEventRTS localization language (was process AUDIO_EVENT_LOCALIZATION_LANGUAGE).
    pub audio_event_localization_language: Mutex<String>,
    /// Async playback handle allocator (was process ASYNC_PLAYBACK_HANDLE_POOL).
    pub async_playback_handle_pool: AtomicU32,
    /// Damage FX definition store (was process DAMAGE_FX_STORE).
    pub damage_fx_store: Mutex<DamageFXStore>,
    /// Resource manager (was process RESOURCE_MANAGER).
    pub resource_manager: Arc<Mutex<ResourceManager>>,
    /// Online chat colors (was process ONLINE_CHAT_COLORS).
    pub online_chat_colors: Arc<ParkingRwLock<OnlineChatColors>>,
    /// Web browser URL table (was process WEB_BROWSER).
    pub web_browser: Arc<ParkingRwLock<WebBrowser>>,
    /// Credits manager (was process CREDITS_MANAGER).
    pub credits_manager: Arc<ParkingRwLock<CreditsManager>>,
    /// INI definition registries (was process OnceCell stores).
    pub ini: IniStores,
    /// System managers (AI, Upgrade, FS, team, FX, radar, …).
    pub system: SystemStores,
}

impl Default for CommonServices {
    fn default() -> Self {
        Self {
            random: RandomServices::default(),
            crc_flags: CrcRuntimeFlags::default(),
            thing_factory: Arc::new(ParkingMutex::new(None)),
            module_factory: Arc::new(ParkingMutex::new(None)),
            name_key_namespace: Mutex::new(NameKeyNamespace::new()),
            logic_rng_owner: Mutex::new(None),
            sim_clock: Mutex::new(SimulationClock::default()),
            pending_income: Mutex::new(HashSet::new()),
            player_object_world: Mutex::new(None),
            academy_template_context: Mutex::new(None),
            object_data_provider: Mutex::new(None),
            team_home_waypoint_resolver: Mutex::new(None),
            team_command_sink: Mutex::new(None),
            build_assistant_backend: Mutex::new(None),
            energy_object_lookup: Mutex::new(None),
            energy_owner_callbacks: Mutex::new(None),
            command_list_append: Mutex::new(None),
            audio_event_localization_language: Mutex::new(String::from("English")),
            async_playback_handle_pool: AtomicU32::new(10_000_000),
            damage_fx_store: Mutex::new(DamageFXStore::new()),
            resource_manager: Arc::new(Mutex::new(ResourceManager::new())),
            online_chat_colors: Arc::new(ParkingRwLock::new(OnlineChatColors::new())),
            web_browser: Arc::new(ParkingRwLock::new(WebBrowser::new())),
            credits_manager: Arc::new(ParkingRwLock::new(CreditsManager::new())),
            ini: IniStores::new(),
            system: SystemStores::new(),
        }
    }
}

impl CommonServices {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run NameKey-dependent store init after this bag is published on TLS.
    pub fn finish_bootstrap_after_publish(&self) {
        let mut upgrade = self
            .system
            .upgrade_center_lifetime
            .write()
            .unwrap_or_else(|e| e.into_inner());
        if upgrade.get_upgrade_names().is_empty() {
            upgrade.init();
        }
    }

    pub fn with_logic_rng_owner<R>(&self, owner: SessionRandom, f: impl FnOnce() -> R) -> R {
        let prev = {
            let mut slot = self.logic_rng_owner.lock().unwrap_or_else(|p| p.into_inner());
            std::mem::replace(&mut *slot, Some(owner))
        };
        struct Restore<'a> {
            slot: &'a Mutex<Option<SessionRandom>>,
            prev: Option<SessionRandom>,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                *self.slot.lock().unwrap_or_else(|p| p.into_inner()) = self.prev.take();
            }
        }
        let _restore = Restore {
            slot: &self.logic_rng_owner,
            prev,
        };
        f()
    }

    pub fn scoped_logic_rng(&self) -> Option<SessionRandom> {
        self.logic_rng_owner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

thread_local! {
    /// Nested `with_common_services` / `install_common_services` binds for this thread.
    static ACTIVE_STACK: RefCell<Vec<Arc<CommonServices>>> = const { RefCell::new(Vec::new()) };
}

/// Active CommonServices for this thread. Panics if nothing is published.
pub fn common_services() -> Arc<CommonServices> {
    try_common_services().expect(
        "CommonServices requires with_common_services / install_common_services (no thread fallback)",
    )
}

/// Active CommonServices for this thread, if published.
pub fn try_common_services() -> Option<Arc<CommonServices>> {
    let head = ACTIVE_STACK.with(|stack| stack.borrow().last().cloned());
    #[cfg(feature = "test-ambient-services")]
    if head.is_none() {
        return Some(test_ambient_services());
    }
    head
}

/// Unit-test convenience (feature `test-ambient-services`, enabled only through
/// dev-dependencies): a test thread that never published a world gets its own
/// owned `CommonServices`, published once for that thread. Never shared across
/// threads and never compiled into production builds.
#[cfg(feature = "test-ambient-services")]
fn test_ambient_services() -> Arc<CommonServices> {
    let services = Arc::new(CommonServices::new());
    install_common_services(Arc::clone(&services));
    services.finish_bootstrap_after_publish();
    services
}

/// Push `services` as the active bind; returns the previous head if any.
pub fn install_common_services(services: Arc<CommonServices>) -> Option<Arc<CommonServices>> {
    let prior = ACTIVE_STACK.with(|stack| {
        let mut active = stack.borrow_mut();
        let prior = active.last().cloned();
        active.push(Arc::clone(&services));
        prior
    });
    // Veterancy NameKey interning needs this bind; safe / idempotent on re-install.
    services
        .system
        .init_lifetime_upgrades_after_common_published();
    prior
}

/// Pop only if `services` is the current head.
pub fn uninstall_common_services_if_current(services: &Arc<CommonServices>) -> bool {
    ACTIVE_STACK.with(|stack| {
        let mut active = stack.borrow_mut();
        if active.last().is_some_and(|head| Arc::ptr_eq(head, services)) {
            active.pop();
            true
        } else {
            false
        }
    })
}


/// RAII publish of a fresh [`CommonServices`] for unit tests / local scopes.
pub struct CommonServicesScope {
    services: Arc<CommonServices>,
}

impl CommonServicesScope {
    pub fn new() -> Self {
        let services = Arc::new(CommonServices::new());
        install_common_services(Arc::clone(&services));
        services.finish_bootstrap_after_publish();
        Self { services }
    }

    pub fn from_services(services: Arc<CommonServices>) -> Self {
        install_common_services(Arc::clone(&services));
        services.finish_bootstrap_after_publish();
        Self { services }
    }

    pub fn services(&self) -> &Arc<CommonServices> {
        &self.services
    }
}

impl Default for CommonServicesScope {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CommonServicesScope {
    fn drop(&mut self) {
        uninstall_common_services_if_current(&self.services);
    }
}

/// Publish `services` for the duration of `f` on **this thread** (nested restore).
pub fn with_common_services<R>(services: &Arc<CommonServices>, f: impl FnOnce() -> R) -> R {
    let _prior = install_common_services(Arc::clone(services));
    struct Pop {
        services: Arc<CommonServices>,
        prior: Option<Arc<CommonServices>>,
    }
    impl Drop for Pop {
        fn drop(&mut self) {
            if uninstall_common_services_if_current(&self.services) {
                if let Some(prior) = self.prior.take() {
                    install_common_services(prior);
                }
            }
        }
    }
    let _pop = Pop {
        services: Arc::clone(services),
        prior: _prior,
    };
    f()
}

#[cfg(test)]
mod independence_tests {
    use super::*;
    use std::sync::{Barrier, Arc as StdArc};
    use std::thread;

    #[test]
    fn two_common_services_on_two_threads_are_independent() {
        let a = Arc::new(CommonServices::new());
        let b = Arc::new(CommonServices::new());
        a.random.init_with_seed(111);
        b.random.init_with_seed(222);
        assert_eq!(a.random.logic_base_seed.load(Ordering::SeqCst), 111);
        assert_eq!(b.random.logic_base_seed.load(Ordering::SeqCst), 222);
        assert!(!Arc::ptr_eq(&a, &b));

        let barrier = StdArc::new(Barrier::new(2));
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);
        let bar1 = StdArc::clone(&barrier);
        let bar2 = StdArc::clone(&barrier);

        let t1 = thread::spawn(move || {
            with_common_services(&a2, || {
                common_services()
                    .random
                    .logic_base_seed
                    .store(1110, Ordering::SeqCst);
                bar1.wait();
                // Peer has now written 2220 into b; our a must stay 1110.
                assert_eq!(
                    common_services().random.logic_base_seed.load(Ordering::SeqCst),
                    1110
                );
            });
        });
        let t2 = thread::spawn(move || {
            with_common_services(&b2, || {
                common_services()
                    .random
                    .logic_base_seed
                    .store(2220, Ordering::SeqCst);
                bar2.wait();
                assert_eq!(
                    common_services().random.logic_base_seed.load(Ordering::SeqCst),
                    2220
                );
            });
        });
        t1.join().unwrap();
        t2.join().unwrap();

        assert_eq!(a.random.logic_base_seed.load(Ordering::SeqCst), 1110);
        assert_eq!(b.random.logic_base_seed.load(Ordering::SeqCst), 2220);
    }

    #[test]
    fn two_ini_stores_on_two_threads_are_independent() {
        use std::sync::Barrier;
        let a = Arc::new(CommonServices::new());
        let b = Arc::new(CommonServices::new());
        // Mutate weapon catalogs independently.
        a.ini.weapon.write().clear();
        b.ini.weapon.write().clear();
        // Use name_key-free marker: clear leaves empty; insert via raw field count.
        // Science store length proxy: is_empty.
        assert!(a.ini.science.read().is_empty());
        assert!(b.ini.science.read().is_empty());

        let barrier = StdArc::new(Barrier::new(2));
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);
        let bar1 = StdArc::clone(&barrier);
        let bar2 = StdArc::clone(&barrier);

        let t1 = thread::spawn(move || {
            with_common_services(&a2, || {
                // Mark a via async handle pool (already instance-owned) AND damage_fx init.
                common_services()
                    .async_playback_handle_pool
                    .store(42, Ordering::SeqCst);
                *common_services().ini.damage_fx.write() =
                    crate::common::ini::ini_damage_fx::DamageFXStore::new();
                common_services()
                    .ini
                    .damage_fx
                    .write()
                    .init();
                bar1.wait();
                assert_eq!(
                    common_services()
                        .async_playback_handle_pool
                        .load(Ordering::SeqCst),
                    42
                );
            });
        });
        let t2 = thread::spawn(move || {
            with_common_services(&b2, || {
                common_services()
                    .async_playback_handle_pool
                    .store(99, Ordering::SeqCst);
                bar2.wait();
                assert_eq!(
                    common_services()
                        .async_playback_handle_pool
                        .load(Ordering::SeqCst),
                    99
                );
            });
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert_eq!(a.async_playback_handle_pool.load(Ordering::SeqCst), 42);
        assert_eq!(b.async_playback_handle_pool.load(Ordering::SeqCst), 99);
        assert!(!Arc::ptr_eq(&a.ini.weapon, &b.ini.weapon));
        assert!(!Arc::ptr_eq(&a.ini.damage_fx, &b.ini.damage_fx));
        assert!(!Arc::ptr_eq(&a.credits_manager, &b.credits_manager));
        assert!(!Arc::ptr_eq(&a.resource_manager, &b.resource_manager));
        assert!(!Arc::ptr_eq(&a.online_chat_colors, &b.online_chat_colors));
        assert!(!Arc::ptr_eq(&a.web_browser, &b.web_browser));
    }

    #[test]
    fn unscoped_common_services_panics_without_fallback_bag() {
        let exploded = std::panic::catch_unwind(|| {
            let _ = common_services();
        })
        .is_err();
        assert!(exploded, "unscoped common_services must not mint a TLS bag");
    }
}
