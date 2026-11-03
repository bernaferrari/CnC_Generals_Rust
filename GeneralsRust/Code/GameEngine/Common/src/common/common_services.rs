//! Owned Common engine-context services (C++ TheXxx singletons on TheGameEngine).
//!
//! Dense Common modules hang their process slots off [`CommonServices`] instead of
//! file-level Mutex/TLS. A scoped install publishes the active Arc for legacy free
//! functions during the migration; GameEngine (and tests) own the Arc.

use crate::common::random_value::{RandomState, SessionRandom};
use crate::common::thing::thing_factory::ThingFactory;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

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
    /// ThingFactory process slot (was `static THING_FACTORY`).
    pub thing_factory: Arc<Mutex<Option<ThingFactory>>>,
    /// Session logic RNG publish slot (was TLS; Mutex is fine on the game thread).
    logic_rng_owner: Mutex<Option<SessionRandom>>,
}

impl Default for CommonServices {
    fn default() -> Self {
        Self {
            random: RandomServices::default(),
            crc_flags: CrcRuntimeFlags::default(),
            thing_factory: Arc::new(Mutex::new(None)),
            logic_rng_owner: Mutex::new(None),
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

struct ProcessCommon(LazyLock<Arc<CommonServices>>);
static PROCESS_COMMON: ProcessCommon =
    ProcessCommon(LazyLock::new(|| Arc::new(CommonServices::default())));

struct ScopedCommonStack {
    stack: Mutex<Vec<Arc<CommonServices>>>,
}
static SCOPED_COMMON: LazyLock<ScopedCommonStack> = LazyLock::new(|| ScopedCommonStack {
    stack: Mutex::new(Vec::new()),
});

/// Active CommonServices: scoped install, else process fallback.
pub fn common_services() -> Arc<CommonServices> {
    SCOPED_COMMON
        .stack
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .last()
        .cloned()
        .unwrap_or_else(|| Arc::clone(&PROCESS_COMMON.0))
}

/// Publish `services` for the duration of `f` (nested worlds restore prior).
pub fn with_common_services<R>(services: &Arc<CommonServices>, f: impl FnOnce() -> R) -> R {
    SCOPED_COMMON
        .stack
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(Arc::clone(services));
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            SCOPED_COMMON
                .stack
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .pop();
        }
    }
    let _pop = Pop;
    f()
}

/// Cold process Arc (tests that need a fresh bag should prefer [`with_common_services`]).
pub fn process_common_services() -> Arc<CommonServices> {
    Arc::clone(&PROCESS_COMMON.0)
}
