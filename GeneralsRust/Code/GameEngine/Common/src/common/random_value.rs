////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// RandomValue.rs
// Pseudo-random number generators
// Author: Michael S. Booth, January 1998

use crate::common::crc::Crc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Multiplication factor for converting to floating point
const MULT_FACTOR: f32 = 1.0 / (4294967295.0); // 2^32 - 1

/// Versioned state capsule inside the existing six-word snapshot field.
const STATE_MAGIC: u32 = u32::from_le_bytes(*b"FRNG");
const STATE_VERSION: u32 = 1;

/// Instance-owned random stream backed by the existing Rust RNG dependency.
///
/// The user explicitly approved replacing the original C++ ADC sequence.
/// Stream ownership, call ordering and range semantics remain unchanged;
/// original C++ replays and old saves no longer have identical continuation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomState {
    rng: fastrand::Rng,
}

impl Default for RandomState {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomState {
    const fn new() -> Self {
        Self {
            rng: fastrand::Rng::with_seed(0),
        }
    }

    fn next_value(&mut self) -> u32 {
        self.next_u32()
    }

    /// Draw directly from an explicitly borrowed stream owner.
    pub fn next_u32(&mut self) -> u32 {
        self.rng.u32(..)
    }

    /// Inclusive integer draw from this borrowed stream.
    pub fn next_int(&mut self, lo: i32, hi: i32) -> i32 {
        logic_integer_with_draw(lo, hi, || self.next_u32())
    }

    /// The same range arithmetic as the existing real-value adapter, using
    /// this explicitly borrowed stream rather than selecting an active owner.
    pub fn next_real(&mut self, lo: f32, hi: f32) -> f32 {
        let delta = hi - lo;
        if delta <= 0.0 {
            return hi;
        }
        (self.next_u32() as f32 * MULT_FACTOR) * delta + lo
    }

    /// Reset this stream without publishing or changing another instance.
    pub fn seed_random(&mut self, seed_value: u32) {
        self.rng.seed(u64::from(seed_value));
    }

    /// Restore a Rust capsule or deterministically import old C++ seed words.
    ///
    /// Legacy import uses a byte-defined FNV-1a fold, not the retired generator.
    /// It accepts the old payload shape but intentionally changes its future
    /// random sequence. Newly saved capsules preserve exact Rust continuation.
    pub fn set_seed_words(&mut self, words: [u32; 6]) {
        let seed = if words[0] == STATE_MAGIC
            && words[1] == STATE_VERSION
            && words[4] == !STATE_MAGIC
            && words[5] == !STATE_VERSION
        {
            u64::from(words[2]) | (u64::from(words[3]) << 32)
        } else {
            words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
                })
        };
        self.rng.seed(seed);
    }

    /// Current backend/version and 64-bit state in the unchanged wire shape.
    pub fn seed_words(&self) -> [u32; 6] {
        let seed = self.rng.get_seed();
        [
            STATE_MAGIC,
            STATE_VERSION,
            seed as u32,
            (seed >> 32) as u32,
            !STATE_MAGIC,
            !STATE_VERSION,
        ]
    }
}

/// Process random streams live on [`crate::common::common_services::CommonServices::random`].

/// A session's sole logic stream, used by synchronous callbacks.
/// The atomic handle supports the actual prepared-world startup worker handoff;
/// it does not schedule concurrent simulation. Primitive draws linearize here.
/// Cloning this handle shares the stream; `snapshot()` makes an independent
/// state value. No references or placeholder state are published to callbacks.
#[derive(Debug, Clone)]
pub struct SessionRandom {
    state: Arc<AtomicU64>,
}
impl Default for SessionRandom {
    fn default() -> Self {
        Self::from_state(RandomState::default())
    }
}
impl SessionRandom {
    pub fn from_state(state: RandomState) -> Self {
        Self {
            state: Arc::new(AtomicU64::new(state.rng.get_seed())),
        }
    }
    pub fn snapshot(&self) -> RandomState {
        RandomState {
            rng: fastrand::Rng::with_seed(self.state.load(Ordering::Relaxed)),
        }
    }
    pub fn seed_random(&self, seed: u32) {
        self.state.store(u64::from(seed), Ordering::Relaxed);
    }
    pub fn set_seed_words(&self, words: [u32; 6]) {
        let mut state = RandomState::default();
        state.set_seed_words(words);
        self.state.store(state.rng.get_seed(), Ordering::Relaxed);
    }
    pub fn seed_words(&self) -> [u32; 6] {
        self.snapshot().seed_words()
    }
    /// Original inclusive integer range, consuming only this driving stream.
    pub fn next_int(&self, lo: i32, hi: i32) -> i32 {
        logic_integer_with_draw(lo, hi, || self.next_u32())
    }

    pub fn next_real(&self, lo: f32, hi: f32) -> f32 {
        let delta = hi - lo;
        if delta <= 0. {
            return hi;
        }
        (self.next_u32() as f32 * MULT_FACTOR) * delta + lo
    }

    /// Linearize one library draw; retries have no gameplay side effects.
    pub fn next_u32(&self) -> u32 {
        let mut before = self.state.load(Ordering::Relaxed);
        loop {
            let mut rng = fastrand::Rng::with_seed(before);
            let value = rng.u32(..);
            match self.state.compare_exchange_weak(
                before,
                rng.get_seed(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return value,
                Err(current) => before = current,
            }
        }
    }
}

/// Publish a safe owning handle for one synchronous operation via CommonServices.
pub fn with_session_random<R>(owner: &SessionRandom, f: impl FnOnce() -> R) -> R {
    crate::common::common_services::common_services()
        .with_logic_rng_owner(owner.clone(), f)
}
/// Compatibility for explicitly borrowed, independently cloned RandomState.
/// Rust's exclusive borrow prevents safe callbacks from accessing `owner`
/// while its state is moved into the scoped handle. The private guard writes
/// continuation back even during unwind; Main uses `with_session_random` and
/// never moves/replaces its live field.
pub fn with_logic_rng_owner<R>(owner: &mut RandomState, f: impl FnOnce() -> R) -> R {
    struct Restore<'a> {
        owner: &'a mut RandomState,
        stream: SessionRandom,
    }
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            *self.owner = self.stream.snapshot();
        }
    }
    let stream = SessionRandom::from_state(std::mem::take(owner));
    let _restore = Restore {
        owner,
        stream: stream.clone(),
    };
    with_session_random(&stream, f)
}
fn scoped_logic_rng() -> Option<SessionRandom> {
    crate::common::common_services::common_services().scoped_logic_rng()
}
fn with_global_logic_rng<R>(f: impl FnOnce(&mut RandomState) -> R) -> R {
    crate::common::common_services::common_services()
        .random
        .with_logic_mut(f)
}

/// Initialize all random number generators
pub fn init_random() {
    #[cfg(feature = "deterministic")]
    {
        init_random_with_seed(0);
    }
    #[cfg(not(feature = "deterministic"))]
    {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        init_random_with_seed(seed);
    }
}

/// Reseed the global logic fallback (poison-recovering). Helper for the
/// standalone initialization below.
fn seed_global_logic_random(seed: u32) {
    with_global_logic_rng(|global| global.seed_random(seed));
}

/// Initialize random number generators with specific seed
pub fn init_random_with_seed(seed: u32) {
    init_client_audio_random_with_seed(seed);
    seed_global_logic_random(seed);
    crate::common::common_services::common_services().random.logic_base_seed.store(seed, Ordering::SeqCst);
}

/// Reset application presentation streams without changing any match stream.
/// Main uses this for the C++ mission-restart/initial-map presentation reset.
pub fn init_client_audio_random_with_seed(seed: u32) {
    let services = crate::common::common_services::common_services();
    services.random.with_client_mut(|client| client.seed_random(seed));
    services.random.with_audio_mut(|audio| audio.seed_random(seed));
}

/// Initialize the standalone/process logic fallback. This never reseeds a
/// published match: match admission must explicitly address its SessionRandom.
pub fn init_game_logic_random(seed: u32) {
    #[cfg(feature = "deterministic")]
    let seed = {
        let _ = seed;
        0
    };
    seed_global_logic_random(seed);
    crate::common::common_services::common_services().random.logic_base_seed.store(seed, Ordering::SeqCst);
}

/// Get the game logic random seed
pub fn get_game_logic_random_seed() -> u32 {
    crate::common::common_services::common_services().random.logic_base_seed.load(Ordering::SeqCst)
}

/// Restore the six-word GameLogic RNG capsule (or import legacy C++ state).
///
/// Used by GameLogic helpers bridge so crate-local RNG draws share the Common stream.
/// Routes through the scoped-owner resolver: a snapshot restore issued inside
/// a tick writes the driving instance, else the global fallback.
pub fn set_game_logic_random_seed_state(words: [u32; 6]) {
    if let Some(owner) = scoped_logic_rng() {
        owner.set_seed_words(words);
    } else {
        with_global_logic_rng(|logic| logic.set_seed_words(words));
    }
}

/// Read the six-word GameLogic RNG capsule.
pub fn get_game_logic_random_seed_state() -> [u32; 6] {
    if let Some(owner) = scoped_logic_rng() {
        owner.seed_words()
    } else {
        with_global_logic_rng(|logic| logic.seed_words())
    }
}

/// Get CRC of the game logic random seed
pub fn get_game_logic_random_seed_crc() -> u32 {
    // Scoped-owner resolver: CRC the driving instance's state during a tick
    // (this is the network sync check — it must reflect the state the draws
    // actually consume), else the global fallback.
    let mut crc = Crc::new();
    for seed_part in get_game_logic_random_seed_state() {
        crc.compute_single(&seed_part);
    }
    crc.get()
}

fn next_logic_value() -> u32 {
    if let Some(owner) = scoped_logic_rng() {
        owner.next_u32()
    } else {
        with_global_logic_rng(RandomState::next_u32)
    }
}

/// Get game logic random integer value
pub fn get_game_logic_random_value(lo: i32, hi: i32) -> i32 {
    logic_integer_with_draw(lo, hi, next_logic_value)
}

fn logic_integer_with_draw(lo: i32, hi: i32, draw: impl FnOnce() -> u32) -> i32 {
    // C++ RandomValue.cpp:189 `UnsignedInt delta = hi - lo + 1;` — MSVC x86
    // evaluates the Int expression with two's-complement wrap and stores the
    // resulting bits as UnsignedInt. Reproduce exactly: wrap on i32, bit-cast.
    // (0, i32::MAX) -> 0x80000000; (i32::MIN, i32::MAX) -> 0.
    let delta = hi.wrapping_sub(lo).wrapping_add(1) as u32;
    // C++ RandomValue.cpp:193-194: delta == 0 returns hi BEFORE any draw.
    if delta == 0 {
        return hi;
    }

    let random_val = draw();
    // C++ RandomValue.cpp:196 `((Int)(randomValue(...) % delta)) + lo` —
    // unsigned mod, quotient bits reinterpreted as Int, then a signed add
    // that wraps on MSVC x86 (final addition can overflow only when delta
    // itself wrapped, e.g. lo = i32::MAX - 1, hi = i32::MIN).
    ((random_val % delta) as i32).wrapping_add(lo)
}

/// Get game client random integer value
pub fn get_game_client_random_value(lo: i32, hi: i32) -> i32 {
    // Same C++ shape as the logic stream (RandomValue.cpp:217/220/223).
    let delta = hi.wrapping_sub(lo).wrapping_add(1) as u32;
    if delta == 0 {
        return hi;
    }

    let random_val = crate::common::common_services::common_services()
        .random
        .with_client_mut(|client| client.next_value());
    ((random_val % delta) as i32).wrapping_add(lo)
}

/// Get game audio random integer value
pub fn get_game_audio_random_value(lo: i32, hi: i32) -> i32 {
    // Same C++ shape as the logic stream (RandomValue.cpp:240/243/246).
    let delta = hi.wrapping_sub(lo).wrapping_add(1) as u32;
    if delta == 0 {
        return hi;
    }

    let random_val = crate::common::common_services::common_services()
        .random
        .with_audio_mut(|audio| audio.next_value());
    ((random_val % delta) as i32).wrapping_add(lo)
}

/// Get game logic random real value
pub fn get_game_logic_random_value_real(lo: f32, hi: f32) -> f32 {
    let delta = hi - lo;
    if delta <= 0.0 {
        return hi;
    }

    // Scoped-owner resolver: same routing as the integer logic draw above.
    let random_val = next_logic_value();
    (random_val as f32 * MULT_FACTOR) * delta + lo
}

/// Get game client random real value
pub fn get_game_client_random_value_real(lo: f32, hi: f32) -> f32 {
    let delta = hi - lo;
    if delta <= 0.0 {
        return hi;
    }

    let random_val = crate::common::common_services::common_services()
        .random
        .with_client_mut(|client| client.next_value());
    (random_val as f32 * MULT_FACTOR) * delta + lo
}

/// Run a fallible presentation-state decode with a private copy of the current
/// client stream. Only successful admission advances the application stream.
/// The operation must use its borrowed stream and never reenter client helpers.
/// This uses the existing presentation synchronization boundary, not a second
/// generator or a different random algorithm.
pub fn with_game_client_random_transaction<T, E>(
    operation: impl FnOnce(&mut RandomState) -> Result<T, E>,
) -> Result<T, E> {
    crate::common::common_services::common_services()
        .random
        .with_client_mut(|stream| {
            let mut candidate = stream.clone();
            let result = operation(&mut candidate)?;
            *stream = candidate;
            Ok(result)
        })
}

/// Get game audio random real value
pub fn get_game_audio_random_value_real(lo: f32, hi: f32) -> f32 {
    let delta = hi - lo;
    if delta <= 0.0 {
        return hi;
    }

    let random_val = crate::common::common_services::common_services()
        .random
        .with_audio_mut(|audio| audio.next_value());
    (random_val as f32 * MULT_FACTOR) * delta + lo
}

/// Distribution types for random variables
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionType {
    Constant,
    Uniform,
    Gaussian,
    Triangular,
    LowBias,
    HighBias,
}

impl DistributionType {
    pub const NAMES: &'static [&'static str] = &[
        "CONSTANT",
        "UNIFORM",
        "GAUSSIAN",
        "TRIANGULAR",
        "LOW_BIAS",
        "HIGH_BIAS",
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "CONSTANT" => Some(Self::Constant),
            "UNIFORM" => Some(Self::Uniform),
            "GAUSSIAN" => Some(Self::Gaussian),
            "TRIANGULAR" => Some(Self::Triangular),
            "LOW_BIAS" => Some(Self::LowBias),
            "HIGH_BIAS" => Some(Self::HighBias),
            _ => None,
        }
    }
}

/// Game client random variable
#[derive(Debug, Clone)]
pub struct GameClientRandomVariable {
    low: f32,
    high: f32,
    distribution_type: DistributionType,
}

impl Default for GameClientRandomVariable {
    fn default() -> Self {
        Self {
            low: 0.0,
            high: 0.0,
            distribution_type: DistributionType::Constant,
        }
    }
}

impl GameClientRandomVariable {
    /// Create a new random variable
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the range and distribution type
    pub fn set_range(&mut self, low: f32, high: f32, distribution_type: DistributionType) {
        self.low = low;
        self.high = high;
        self.distribution_type = distribution_type;
    }

    /// C++ `setRange(min, max)` — type defaults to `UNIFORM`.
    pub fn set_range_uniform(&mut self, low: f32, high: f32) {
        self.set_range(low, high, DistributionType::Uniform);
    }

    /// C++ `getMinimumValue`.
    pub fn get_minimum_value(&self) -> f32 {
        self.low
    }

    /// C++ `getMaximumValue`.
    pub fn get_maximum_value(&self) -> f32 {
        self.high
    }

    /// C++ `getDistributionType`.
    pub fn get_distribution_type(&self) -> DistributionType {
        self.distribution_type
    }

    /// Get a value from the random distribution
    pub fn get_value(&self) -> f32 {
        match self.distribution_type {
            DistributionType::Constant => {
                if self.low == self.high {
                    self.low
                } else {
                    get_game_client_random_value_real(self.low, self.high)
                }
            }
            DistributionType::Uniform => get_game_client_random_value_real(self.low, self.high),
            _ => {
                // C++ GameClientRandomVariable::getValue (RandomValue.cpp:366-369):
                // DEBUG_CRASH then return 0.0f — release builds just return 0.0.
                0.0
            }
        }
    }

    /// Get the distribution type names
    pub fn get_distribution_type_names() -> &'static [&'static str] {
        DistributionType::NAMES
    }
}

/// Game logic random variable
#[derive(Debug, Clone)]
pub struct GameLogicRandomVariable {
    low: f32,
    high: f32,
    distribution_type: DistributionType,
}

impl Default for GameLogicRandomVariable {
    fn default() -> Self {
        Self {
            low: 0.0,
            high: 0.0,
            distribution_type: DistributionType::Constant,
        }
    }
}

impl GameLogicRandomVariable {
    /// Create a new random variable
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the range and distribution type
    pub fn set_range(&mut self, low: f32, high: f32, distribution_type: DistributionType) {
        self.low = low;
        self.high = high;
        self.distribution_type = distribution_type;
    }

    /// C++ `setRange(min, max)` — type defaults to `UNIFORM`.
    pub fn set_range_uniform(&mut self, low: f32, high: f32) {
        self.set_range(low, high, DistributionType::Uniform);
    }

    /// C++ `getMinimumValue`.
    pub fn get_minimum_value(&self) -> f32 {
        self.low
    }

    /// C++ `getMaximumValue`.
    pub fn get_maximum_value(&self) -> f32 {
        self.high
    }

    /// C++ `getDistributionType`.
    pub fn get_distribution_type(&self) -> DistributionType {
        self.distribution_type
    }

    /// Get a value from the random distribution
    pub fn get_value(&self) -> f32 {
        match self.distribution_type {
            DistributionType::Constant => {
                if self.low == self.high {
                    self.low
                } else {
                    get_game_logic_random_value_real(self.low, self.high)
                }
            }
            DistributionType::Uniform => get_game_logic_random_value_real(self.low, self.high),
            _ => {
                // C++ GameLogicRandomVariable::getValue (RandomValue.cpp:410-413):
                // DEBUG_CRASH then return 0.0f — release builds just return 0.0.
                0.0
            }
        }
    }

    /// Get the distribution type names
    pub fn get_distribution_type_names() -> &'static [&'static str] {
        DistributionType::NAMES
    }
}

#[cfg(test)]
#[path = "random_value_tests.rs"]
mod tests;
