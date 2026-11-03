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
use crate::common::rts::player::PlayerObjectWorld;
use crate::common::rts::academy_stats::AcademyTemplateContextProvider;
use crate::common::time::SimulationClock;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// GameClient / GameAudio / GameLogic random streams owned by one game instance.
#[derive(Debug)]
pub struct RandomServices {
    pub client: Mutex<RandomState>,
    pub audio: Mutex<RandomState>,
    pub logic: Mutex<RandomState>,
    pub logic_base_seed: AtomicU32,
}

impl Default for RandomServices {
    fn default() -> Self {
        Self {
            client: Mutex::new(RandomState::default()),
            audio: Mutex::new(RandomState::default()),
            logic: Mutex::new(RandomState::default()),
            logic_base_seed: AtomicU32::new(0),
        }
    }
}

impl RandomServices {
    pub fn init_with_seed(&self, seed: u32) {
        for slot in [&self.client, &self.audio, &self.logic] {
            match slot.lock() {
                Ok(mut g) => g.seed_random(seed),
                Err(p) => p.into_inner().seed_random(seed),
            }
        }
        self.logic_base_seed.store(seed, Ordering::SeqCst);
    }

    pub fn with_logic_mut<R>(&self, f: impl FnOnce(&mut RandomState) -> R) -> R {
        match self.logic.lock() {
            Ok(mut g) => f(&mut g),
            Err(p) => f(&mut p.into_inner()),
        }
    }

    pub fn with_client_mut<R>(&self, f: impl FnOnce(&mut RandomState) -> R) -> R {
        match self.client.lock() {
            Ok(mut g) => f(&mut g),
            Err(p) => f(&mut p.into_inner()),
        }
    }

    pub fn with_audio_mut<R>(&self, f: impl FnOnce(&mut RandomState) -> R) -> R {
        match self.audio.lock() {
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
    pub thing_factory: Arc<Mutex<Option<ThingFactory>>>,
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
}

impl Default for CommonServices {
    fn default() -> Self {
        Self {
            random: RandomServices::default(),
            crc_flags: CrcRuntimeFlags::default(),
            thing_factory: Arc::new(Mutex::new(None)),
            logic_rng_owner: Mutex::new(None),
            sim_clock: Mutex::new(SimulationClock::default()),
            pending_income: Mutex::new(HashSet::new()),
            player_object_world: Mutex::new(None),
            academy_template_context: Mutex::new(None),
        }
    }
}

impl CommonServices {
    pub fn new() -> Self {
        Self::default()
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
    ACTIVE_STACK.with(|stack| {
        stack
            .borrow()
            .last()
            .cloned()
            .expect("CommonServices requires with_common_services / install_common_services (no thread fallback)")
    })
}

/// Push `services` as the active bind; returns the previous head if any.
pub fn install_common_services(services: Arc<CommonServices>) -> Option<Arc<CommonServices>> {
    ACTIVE_STACK.with(|stack| {
        let mut active = stack.borrow_mut();
        let prior = active.last().cloned();
        active.push(services);
        prior
    })
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
    fn unscoped_common_services_panics_without_fallback_bag() {
        let exploded = std::panic::catch_unwind(|| {
            let _ = common_services();
        })
        .is_err();
        assert!(exploded, "unscoped common_services must not mint a TLS bag");
    }
}
