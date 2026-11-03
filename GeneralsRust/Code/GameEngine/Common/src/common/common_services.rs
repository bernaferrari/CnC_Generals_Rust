//! Owned Common engine-context services (C++ TheXxx singletons on TheGameEngine).
//!
//! Dense Common modules hang mutable TheXxx state off [`CommonServices`]. GameEngine
//! (and tests) own an `Arc<CommonServices>` and publish it for free-function bridges
//! via [`with_common_services`]. The active pointer is **thread-local** so two game
//! instances on two threads never share RNG / ThingFactory / CRC flags.
//!
//! There is no process-wide shared `CommonServices`. An unscoped thread gets a
//! private per-thread fallback Arc (legacy single-threaded call sites) that is
//! never visible to other threads.

use crate::common::random_value::{RandomState, SessionRandom};
use crate::common::thing::thing_factory::ThingFactory;
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
    /// Nested `with_common_services` installs for this thread only.
    static ACTIVE_STACK: RefCell<Vec<Arc<CommonServices>>> = const { RefCell::new(Vec::new()) };
    /// Private per-thread fallback — never shared across threads.
    static THREAD_FALLBACK: RefCell<Option<Arc<CommonServices>>> = const { RefCell::new(None) };
}

/// Active CommonServices for this thread: scoped install, else private fallback.
pub fn common_services() -> Arc<CommonServices> {
    ACTIVE_STACK.with(|stack| {
        if let Some(top) = stack.borrow().last() {
            return Arc::clone(top);
        }
        THREAD_FALLBACK.with(|cell| {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                *slot = Some(Arc::new(CommonServices::default()));
            }
            Arc::clone(slot.as_ref().unwrap())
        })
    })
}

/// Publish `services` for the duration of `f` on **this thread** (nested restore).
pub fn with_common_services<R>(services: &Arc<CommonServices>, f: impl FnOnce() -> R) -> R {
    ACTIVE_STACK.with(|stack| {
        stack.borrow_mut().push(Arc::clone(services));
    });
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            ACTIVE_STACK.with(|stack| {
                stack.borrow_mut().pop();
            });
        }
    }
    let _pop = Pop;
    f()
}

/// Reset this thread's private fallback (tests). Does not touch other threads.
pub fn reset_thread_common_services_fallback() {
    THREAD_FALLBACK.with(|cell| {
        *cell.borrow_mut() = None;
    });
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
    fn unscoped_threads_get_private_fallbacks_not_a_shared_process_bag() {
        let barrier = StdArc::new(Barrier::new(2));
        let bar1 = StdArc::clone(&barrier);
        let bar2 = StdArc::clone(&barrier);
        let t1 = thread::spawn(move || {
            reset_thread_common_services_fallback();
            common_services().random.init_with_seed(41);
            bar1.wait();
            assert_eq!(
                common_services().random.logic_base_seed.load(Ordering::SeqCst),
                41,
                "peer thread must not overwrite this thread's fallback"
            );
        });
        let t2 = thread::spawn(move || {
            reset_thread_common_services_fallback();
            common_services().random.init_with_seed(42);
            bar2.wait();
            assert_eq!(
                common_services().random.logic_base_seed.load(Ordering::SeqCst),
                42
            );
        });
        t1.join().unwrap();
        t2.join().unwrap();
    }
}
