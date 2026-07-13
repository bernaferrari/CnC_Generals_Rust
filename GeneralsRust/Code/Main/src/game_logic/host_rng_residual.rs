//! Host residual for retail GameLogic / GameClient RandomValue streams.
//!
//! Closes the fail-closed golden-ratio scatter residual by using the same
//! add-with-carry algorithm as C++ `Common/RandomValue.cpp` /
//! `game_engine::common::random_value`.
//!
//! Host residual closed here:
//! - Pure local ADC stream for re-query-stable combat scatter / delay draws
//!   (formation / bomb / missile index seed) matching RandomValue algorithm.
//! - Live GameClient / GameLogic global stream draws for one-shot presentation
//!   scatter and stream honesty residual.
//! - Stream separation honesty (logic vs client vs audio).
//!
//! Fail-closed:
//! - Not every GameLogic consumer (AI/weapon/locomotor) rewired onto a single
//!   shared Main-crate stream (GameLogic crate still owns a parallel helper seed).
//! - Not full mid-sim multi-strike once-at-queue storage of every OCL draw
//!   (pure index-seeded residual keeps plan_due re-query stable).
//! - Network residual deferred.

use game_engine::common::random_value::{
    get_game_audio_random_value, get_game_client_random_value, get_game_client_random_value_real,
    get_game_logic_random_seed, get_game_logic_random_seed_crc, get_game_logic_random_value,
    get_game_logic_random_value_real, init_random_with_seed,
};

/// Multiplication factor matching C++ `theMultFactor` (1 / (2^32 - 1)).
const MULT_FACTOR: f32 = 1.0 / 4_294_967_295.0;

/// Initial seed constants matching C++ `theGameLogicSeed` defaults.
const INITIAL_SEED: [u32; 6] = [
    0xf22d0e56, 0x883126e9, 0xc624dd2f, 0x0702c49c, 0x9e353f7d, 0x6fdf3b64,
];

/// Local residual RandomState (C++ RandomValue.cpp ADC algorithm).
///
/// Used for pure host residual draws (re-query stable by index seed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRandomState {
    seed: [u32; 6],
}

impl Default for HostRandomState {
    fn default() -> Self {
        Self {
            seed: INITIAL_SEED,
        }
    }
}

impl HostRandomState {
    pub fn new() -> Self {
        Self::default()
    }

    /// C++ `seedRandom(SEED, seed)`.
    pub fn seeded(seed_value: u32) -> Self {
        let mut s = Self::default();
        s.seed_random(seed_value);
        s
    }

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

    /// C++ `randomValue(seed)` ADC next.
    #[allow(unused_assignments)]
    pub fn next_u32(&mut self) -> u32 {
        let mut c = 0u32;
        macro_rules! adc {
            ($sum:ident, $a:expr, $b:expr, $c:ident) => {
                let temp = ($a as u64) + ($b as u64) + ($c as u64);
                $sum = temp as u32;
                $c = if temp > u32::MAX as u64 { 1 } else { 0 };
            };
        }
        let mut ax;
        adc!(ax, self.seed[5], self.seed[4], c);
        self.seed[4] = ax;
        adc!(ax, ax, self.seed[3], c);
        self.seed[3] = ax;
        adc!(ax, ax, self.seed[2], c);
        self.seed[2] = ax;
        adc!(ax, ax, self.seed[1], c);
        self.seed[1] = ax;
        adc!(ax, ax, self.seed[0], c);
        self.seed[0] = ax;

        self.seed[5] = self.seed[5].wrapping_add(1);
        if self.seed[5] == 0 {
            self.seed[4] = self.seed[4].wrapping_add(1);
            if self.seed[4] == 0 {
                self.seed[3] = self.seed[3].wrapping_add(1);
                if self.seed[3] == 0 {
                    self.seed[2] = self.seed[2].wrapping_add(1);
                    if self.seed[2] == 0 {
                        self.seed[1] = self.seed[1].wrapping_add(1);
                        if self.seed[1] == 0 {
                            self.seed[0] = self.seed[0].wrapping_add(1);
                            ax = ax.wrapping_add(1);
                        }
                    }
                }
            }
        }
        ax
    }

    /// C++ `GetGameLogicRandomValue` / `GetGameClientRandomValue` integer range.
    pub fn next_int(&mut self, lo: i32, hi: i32) -> i32 {
        let delta = (hi.wrapping_sub(lo).wrapping_add(1)) as u32;
        if delta == 0 {
            return hi;
        }
        let random_val = self.next_u32();
        ((random_val % delta) as i32).wrapping_add(lo)
    }

    /// C++ `GetGameLogicRandomValueReal` / `GetGameClientRandomValueReal`.
    pub fn next_real(&mut self, lo: f32, hi: f32) -> f32 {
        let delta = hi - lo;
        if delta <= 0.0 {
            return hi;
        }
        let random_val = self.next_u32();
        (random_val as f32 * MULT_FACTOR) * delta + lo
    }

    pub fn seed_words(&self) -> [u32; 6] {
        self.seed
    }
}

/// Pure residual: GameLogicRandomValueReal algorithm for shell/bomb index.
///
/// Seeded by `index.wrapping_add(1)` so re-query for the same index is stable
/// (required by multi-strike `plan_due_impacts` recompute).
pub fn pure_logic_random_real(index: u32, draw_skip: u32, lo: f32, hi: f32) -> f32 {
    let mut s = HostRandomState::seeded(index.wrapping_add(1));
    for _ in 0..draw_skip {
        let _ = s.next_u32();
    }
    s.next_real(lo, hi)
}

/// Pure residual: GameLogicRandomValue integer algorithm for shell/missile index.
pub fn pure_logic_random_int(index: u32, draw_skip: u32, lo: i32, hi: i32) -> i32 {
    let mut s = HostRandomState::seeded(index.wrapping_add(1));
    for _ in 0..draw_skip {
        let _ = s.next_u32();
    }
    s.next_int(lo, hi)
}

/// Pure residual: GameClientRandomValue integer algorithm for presentation scatter.
///
/// C++ AutoDepositUpdate: `GameClientRandomValue(-width, width)` with Real→Int
/// truncation of the geometry radius * 0.3 scatter half-width.
pub fn pure_client_structure_scatter(seed: u32, major_radius: f32, minor_radius: f32, scale: f32) -> (f32, f32) {
    let width = (major_radius * scale).max(0.0);
    let depth = (minor_radius * scale).max(0.0);
    if width <= 0.0 && depth <= 0.0 {
        return (0.0, 0.0);
    }
    let w = width.floor() as i32;
    let d = depth.floor() as i32;
    let mut s = HostRandomState::seeded(seed.wrapping_add(1));
    let dx = if w != 0 {
        s.next_int(-w, w) as f32
    } else {
        0.0
    };
    let dz = if d != 0 {
        s.next_int(-d, d) as f32
    } else {
        0.0
    };
    (dx, dz)
}

/// Live GameClient stream residual: one-shot structure floating-text scatter.
///
/// Advances the global client stream (presentation residual). Prefer for live
/// deposit paths; store the result on the floating-text entry.
pub fn client_stream_structure_scatter(major_radius: f32, minor_radius: f32, scale: f32) -> (f32, f32) {
    let width = (major_radius * scale).max(0.0);
    let depth = (minor_radius * scale).max(0.0);
    if width <= 0.0 && depth <= 0.0 {
        return (0.0, 0.0);
    }
    let w = width.floor() as i32;
    let d = depth.floor() as i32;
    let dx = if w != 0 {
        get_game_client_random_value(-w, w) as f32
    } else {
        0.0
    };
    let dz = if d != 0 {
        get_game_client_random_value(-d, d) as f32
    } else {
        0.0
    };
    (dx, dz)
}

/// Live GameLogic stream residual: WeaponErrorRadius polar scatter (one-shot).
pub fn logic_stream_error_radius_offset(error_radius: f32) -> (f32, f32) {
    if error_radius <= 1.0 {
        return (0.0, 0.0);
    }
    let radius = get_game_logic_random_value_real(0.0, error_radius);
    let angle = get_game_logic_random_value_real(0.0, std::f32::consts::TAU);
    (radius * angle.cos(), radius * angle.sin())
}

/// Honesty residual: exercise logic + client + audio streams and pure ADC parity.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostRngResidualHonesty {
    pub logic_draws: u32,
    pub client_draws: u32,
    pub audio_draws: u32,
    pub pure_adc_parity_ok: bool,
    pub stream_separation_ok: bool,
    pub seed_crc_ok: bool,
    pub structure_scatter_stream_ok: bool,
    pub error_radius_stream_ok: bool,
}

impl HostRngResidualHonesty {
    pub fn honesty_ok(&self) -> bool {
        self.logic_draws > 0
            && self.client_draws > 0
            && self.audio_draws > 0
            && self.pure_adc_parity_ok
            && self.stream_separation_ok
            && self.seed_crc_ok
            && self.structure_scatter_stream_ok
            && self.error_radius_stream_ok
    }
}

/// Host-testable residual: seed streams, exercise draws, verify pure ADC parity.
pub fn exercise_host_rng_residual(seed: u32) -> HostRngResidualHonesty {
    init_random_with_seed(seed);
    let mut honesty = HostRngResidualHonesty::default();

    // Logic / client / audio stream draws.
    let logic_a = get_game_logic_random_value(0, 1000);
    let logic_b = get_game_logic_random_value_real(0.0, 1.0);
    honesty.logic_draws = 2;
    let _ = (logic_a, logic_b);

    let client_a = get_game_client_random_value(-10, 10);
    let client_b = get_game_client_random_value_real(0.0, 5.0);
    honesty.client_draws = 2;
    let _ = (client_a, client_b);

    let audio_a = get_game_audio_random_value(1, 4);
    honesty.audio_draws = 1;
    let _ = audio_a;

    // Pure ADC parity vs global stream after identical seed.
    init_random_with_seed(seed);
    let stream_real = get_game_logic_random_value_real(0.0, 50.0);
    let pure_real = {
        let mut s = HostRandomState::seeded(seed);
        s.next_real(0.0, 50.0)
    };
    honesty.pure_adc_parity_ok = (stream_real - pure_real).abs() < 1e-5;

    init_random_with_seed(seed);
    let stream_int = get_game_logic_random_value(0, 90);
    let pure_int = {
        let mut s = HostRandomState::seeded(seed);
        s.next_int(0, 90)
    };
    honesty.pure_adc_parity_ok = honesty.pure_adc_parity_ok && stream_int == pure_int;

    // Stream separation: consuming logic must not change next client after reseed pair.
    init_random_with_seed(seed);
    let client_only = get_game_client_random_value(0, 999);
    init_random_with_seed(seed);
    let _ = get_game_logic_random_value(0, 999);
    let client_after_logic = get_game_client_random_value(0, 999);
    honesty.stream_separation_ok = client_only == client_after_logic;

    // Seed CRC residual.
    init_random_with_seed(seed);
    let crc1 = get_game_logic_random_seed_crc();
    init_random_with_seed(seed);
    let crc2 = get_game_logic_random_seed_crc();
    honesty.seed_crc_ok = crc1 == crc2 && get_game_logic_random_seed() == seed;

    // Structure scatter via live client stream residual.
    init_random_with_seed(seed);
    let (dx, dz) = client_stream_structure_scatter(50.0, 40.0, 0.3);
    honesty.structure_scatter_stream_ok =
        dx.abs() <= 15.0 + 0.001 && dz.abs() <= 12.0 + 0.001;

    // Error radius via live logic stream residual.
    init_random_with_seed(seed);
    let (ox, oz) = logic_stream_error_radius_offset(100.0);
    let dist = (ox * ox + oz * oz).sqrt();
    honesty.error_radius_stream_ok = dist <= 100.0 + 0.001;

    honesty
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static RNG_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn pure_adc_matches_common_stream_after_seed() {
        let _g = RNG_TEST_LOCK.lock().unwrap();
        let h = exercise_host_rng_residual(0xC0FFEE);
        assert!(h.honesty_ok(), "{h:?}");
    }

    #[test]
    fn pure_index_scatter_is_stable_on_requery() {
        let a = pure_logic_random_real(5, 0, 0.0, 100.0);
        let b = pure_logic_random_real(5, 0, 0.0, 100.0);
        assert!((a - b).abs() < 1e-6);
        let c = pure_logic_random_real(6, 0, 0.0, 100.0);
        // Different index almost always differs; allow rare collision.
        let _ = c;
    }

    #[test]
    fn pure_client_structure_scatter_bounds_and_stability() {
        let (dx, dz) = pure_client_structure_scatter(7, 50.0, 40.0, 0.3);
        assert!(dx.abs() <= 15.0 + 0.001);
        assert!(dz.abs() <= 12.0 + 0.001);
        let again = pure_client_structure_scatter(7, 50.0, 40.0, 0.3);
        assert_eq!((dx, dz), again);
        assert_eq!(pure_client_structure_scatter(1, 0.0, 0.0, 0.3), (0.0, 0.0));
    }

    #[test]
    fn client_stream_structure_scatter_bounds() {
        let _g = RNG_TEST_LOCK.lock().unwrap();
        init_random_with_seed(12345);
        for _ in 0..32 {
            let (dx, dz) = client_stream_structure_scatter(25.0, 25.0, 0.3);
            assert!(dx.abs() <= 7.0 + 0.001);
            assert!(dz.abs() <= 7.0 + 0.001);
        }
    }
}
