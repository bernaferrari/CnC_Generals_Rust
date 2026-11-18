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
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

/// Multiplication factor for converting to floating point
const MULT_FACTOR: f32 = 1.0 / (4294967295.0); // 2^32 - 1

/// C++ `theGameLogicSeed` initial words (RandomValue.cpp, also the table
/// `seedRandom` offsets from).
const INITIAL_SEED: [u32; 6] = [
    0xf22d0e56, 0x883126e9, 0xc624dd2f, 0x0702c49c, 0x9e353f7d, 0x6fdf3b64,
];

/// C++ `#define ADC(SUM,A,B,C) SUM=(A)+(B)+(C); C=((SUM<(A))||(SUM<(B)))`.
///
/// Unsigned wrap plus the C++ carry predicate, not mathematical carry-out:
/// when `A = B = 0xFFFFFFFF` and the incoming carry is 1, SUM wraps to
/// `0xFFFFFFFF` and the carry is **cleared**.
fn adc(a: u32, b: u32, c: u32) -> (u32, u32) {
    let sum = a.wrapping_add(b).wrapping_add(c);
    let carry = u32::from(sum < a || sum < b);
    (sum, carry)
}

/// One C++ RandomValue.cpp stream: the six-word add-with-carry generator
/// (`theGameLogicSeed` / `theGameClientSeed` / `theGameAudioSeed`).
///
/// Instance-owned: the driving simulation, presentation and audio owners each
/// hold their own state and draw only through explicit borrows or a scoped
/// [`SessionRandom`]. The six words are the exact C++ state, so snapshots,
/// CRCs and replays are C++-compatible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomState {
    seed: [u32; 6],
}

impl Default for RandomState {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomState {
    const fn new() -> Self {
        Self { seed: INITIAL_SEED }
    }

    /// C++ `randomValue(UnsignedInt *seed)`: step the stream, return `ax`.
    pub fn next_u32(&mut self) -> u32 {
        let seed = &mut self.seed;
        let (mut ax, mut c) = adc(seed[5], seed[4], 0);
        seed[4] = ax;
        (ax, c) = adc(ax, seed[3], c);
        seed[3] = ax;
        (ax, c) = adc(ax, seed[2], c);
        seed[2] = ax;
        (ax, c) = adc(ax, seed[1], c);
        seed[1] = ax;
        (ax, _) = adc(ax, seed[0], c);
        seed[0] = ax;

        // Increment the seed array, bubbling up the carries.
        seed[5] = seed[5].wrapping_add(1);
        if seed[5] == 0 {
            seed[4] = seed[4].wrapping_add(1);
            if seed[4] == 0 {
                seed[3] = seed[3].wrapping_add(1);
                if seed[3] == 0 {
                    seed[2] = seed[2].wrapping_add(1);
                    if seed[2] == 0 {
                        seed[1] = seed[1].wrapping_add(1);
                        if seed[1] == 0 {
                            seed[0] = seed[0].wrapping_add(1);
                            ax = ax.wrapping_add(1);
                        }
                    }
                }
            }
        }
        ax
    }

    /// Inclusive integer draw from this borrowed stream
    /// (`GetGame{Logic,Client,Audio}RandomValue`).
    pub fn next_int(&mut self, lo: i32, hi: i32) -> i32 {
        logic_integer_with_draw(lo, hi, || self.next_u32())
    }

    /// `GetGame{Logic,Client,Audio}RandomValueReal` on this borrowed stream.
    pub fn next_real(&mut self, lo: f32, hi: f32) -> f32 {
        let delta = hi - lo;
        if delta <= 0.0 {
            return hi;
        }
        (self.next_u32() as f32 * MULT_FACTOR) * delta + lo
    }

    /// C++ `seedRandom(SEED, seed[])` (RandomValue.cpp:150-174): the initial
    /// table offset by `SEED`, expressed as the same running additions.
    pub fn seed_random(&mut self, seed_value: u32) {
        let mut ax = seed_value;
        ax = ax.wrapping_add(0xf22d0e56);
        self.seed[0] = ax;
        ax = ax.wrapping_add(0x883126e9u32.wrapping_sub(0xf22d0e56));
        self.seed[1] = ax;
        ax = ax.wrapping_add(0xc624dd2fu32.wrapping_sub(0x883126e9));
        self.seed[2] = ax;
        ax = ax.wrapping_add(0x0702c49cu32.wrapping_sub(0xc624dd2f));
        self.seed[3] = ax;
        ax = ax.wrapping_add(0x9e353f7du32.wrapping_sub(0x0702c49c));
        self.seed[4] = ax;
        ax = ax.wrapping_add(0x6fdf3b64u32.wrapping_sub(0x9e353f7d));
        self.seed[5] = ax;
    }

    /// Restore the six raw C++ seed words (xfer/snapshot load).
    ///
    /// Saves written between 2026-10-09 and the ADC restoration carry a tagged
    /// "FRNG" capsule of a retired generator's 64-bit seed instead of ADC
    /// words. Those cannot continue their old sequence; they re-derive a
    /// C++ `seedRandom` stream from the capsule's seed (lossy, deterministic).
    pub fn set_seed_words(&mut self, words: [u32; 6]) {
        const RETIRED_MAGIC: u32 = u32::from_le_bytes(*b"FRNG");
        if words[0] == RETIRED_MAGIC
            && words[1] == 1
            && words[4] == !RETIRED_MAGIC
            && words[5] == !1u32
        {
            self.seed_random(words[2] ^ words[3]);
        } else {
            self.seed = words;
        }
    }

    /// The six raw C++ seed words (xfer/snapshot save, CRC input).
    pub fn seed_words(&self) -> [u32; 6] {
        self.seed
    }
}

/// Process random streams live on [`crate::common::common_services::CommonServices::random`].

/// A session's sole logic stream, used by synchronous callbacks.
/// The shared handle supports the actual prepared-world startup worker handoff;
/// it does not schedule concurrent simulation. Primitive draws linearize on
/// the one lock. Cloning this handle shares the stream; `snapshot()` makes an
/// independent state value. No references or placeholder state are published
/// to callbacks.
#[derive(Debug, Clone)]
pub struct SessionRandom {
    state: Arc<Mutex<RandomState>>,
}
impl Default for SessionRandom {
    fn default() -> Self {
        Self::from_state(RandomState::default())
    }
}
impl SessionRandom {
    pub fn from_state(state: RandomState) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }
    pub fn snapshot(&self) -> RandomState {
        self.state.lock().clone()
    }
    pub fn seed_random(&self, seed: u32) {
        self.state.lock().seed_random(seed);
    }
    pub fn set_seed_words(&self, words: [u32; 6]) {
        self.state.lock().set_seed_words(words);
    }
    pub fn seed_words(&self) -> [u32; 6] {
        self.state.lock().seed_words()
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

    /// One `randomValue` step on the shared stream.
    pub fn next_u32(&self) -> u32 {
        self.state.lock().next_u32()
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

/// C++ `InitRandom(seed)`, logic half. The client and audio halves are
/// seeded by their receiving presentation/audio owners (a fresh owner equals
/// `InitRandom(0)`), never through process-visible services.
pub fn init_random_with_seed(seed: u32) {
    seed_global_logic_random(seed);
    crate::common::common_services::common_services().random.logic_base_seed.store(seed, Ordering::SeqCst);
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


/// Run a fallible presentation-state decode with a private copy of the current
/// client stream. Only successful admission advances the application stream.
/// The operation must use its borrowed stream and never reenter client helpers.
/// This uses the existing presentation synchronization boundary, not a second
/// generator or a different random algorithm.
pub fn with_client_random_transaction<T, E>(
    stream: &mut RandomState,
    operation: impl FnOnce(&mut RandomState) -> Result<T, E>,
) -> Result<T, E> {
    let mut candidate = stream.clone();
    let result = operation(&mut candidate)?;
    *stream = candidate;
    Ok(result)
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

    /// C++ `GameClientRandomVariable::getValue` drawing from the receiving
    /// presentation owner's client stream (RandomValue.cpp:355-370).
    pub fn get_value_with(&self, rng: &mut RandomState) -> f32 {
        match self.distribution_type {
            DistributionType::Constant => {
                if self.low == self.high {
                    self.low
                } else {
                    rng.next_real(self.low, self.high)
                }
            }
            DistributionType::Uniform => rng.next_real(self.low, self.high),
            _ => 0.0,
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
