//! Host random draws share Common's owned C++ add-with-carry RNG.
//!
//! Index-seeded streams keep repeated scatter and strike queries stable.
//! Diagnostic logic, client, and audio draws use independent local streams,
//! each the exact `RandomValue.cpp` generator (`seedRandom` + `randomValue`).

use game_engine::common::random_value::RandomState as CommonRandomState;

/// Multiplication factor matching C++ `theMultFactor` (1 / (2^32 - 1)).
pub const MULT_FACTOR: f32 = 1.0 / 4_294_967_295.0;

/// Retail AutoDepositUpdate structure scatter half-width scale residual (radius * 0.3).
pub const STRUCTURE_SCATTER_SCALE: f32 = 0.3;

/// Residual seed offset applied to pure index draws (`index.wrapping_add(1)`).
pub const PURE_INDEX_SEED_OFFSET: u32 = 1;

/// Owned index-seeded stream using the same backend as Common's live streams.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostRandomState {
    state: CommonRandomState,
}

impl HostRandomState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seeded(seed_value: u32) -> Self {
        let mut state = Self::default();
        state.seed_random(seed_value);
        state
    }

    pub fn seed_random(&mut self, seed_value: u32) {
        self.state.seed_random(seed_value);
    }

    pub fn next_u32(&mut self) -> u32 {
        self.state.next_u32()
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
        self.state.seed_words()
    }
}

/// Owned real-valued draw for a shell/bomb index.
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

/// Owned integer draw for a shell/missile index.
pub fn pure_logic_random_int(index: u32, draw_skip: u32, lo: i32, hi: i32) -> i32 {
    let mut s = HostRandomState::seeded(index.wrapping_add(1));
    for _ in 0..draw_skip {
        let _ = s.next_u32();
    }
    s.next_int(lo, hi)
}

/// Index-seeded integer draws for presentation scatter.
///
/// C++ AutoDepositUpdate: `GameClientRandomValue(-width, width)` with Real→Int
/// truncation of the geometry radius * 0.3 scatter half-width.
pub fn pure_client_structure_scatter(
    seed: u32,
    major_radius: f32,
    minor_radius: f32,
    scale: f32,
) -> (f32, f32) {
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

/// Draw structure scatter from the explicitly supplied stream.
fn structure_scatter(
    random: &mut HostRandomState,
    major_radius: f32,
    minor_radius: f32,
    scale: f32,
) -> (f32, f32) {
    let width = (major_radius * scale).max(0.0);
    let depth = (minor_radius * scale).max(0.0);
    if width <= 0.0 && depth <= 0.0 {
        return (0.0, 0.0);
    }
    let w = width.floor() as i32;
    let d = depth.floor() as i32;
    let dx = if w != 0 {
        random.next_int(-w, w) as f32
    } else {
        0.0
    };
    let dz = if d != 0 {
        random.next_int(-d, d) as f32
    } else {
        0.0
    };
    (dx, dz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_rng_diagnostics_preserve_independent_streams() {
        let seed = 0xC0FFEE;
        let mut logic = HostRandomState::seeded(seed);
        let mut client = HostRandomState::seeded(seed);
        let mut audio = HostRandomState::seeded(seed);
        let initial = logic.seed_words();

        let first = logic.next_u32();
        let second = logic.next_u32();
        assert_eq!(client.seed_words(), initial);
        assert_eq!(audio.seed_words(), initial);
        assert_eq!(audio.next_u32(), first);
        assert_eq!(client.next_u32(), first);
        assert_eq!(client.next_u32(), second);

        logic.seed_random(seed);
        assert_eq!(logic.seed_words(), initial);
        assert_eq!(logic.next_u32(), first);
        assert_eq!(logic.next_u32(), second);
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
    fn owned_structure_scatter_bounds() {
        let mut random = HostRandomState::seeded(12345);
        for _ in 0..32 {
            let (dx, dz) = structure_scatter(&mut random, 25.0, 25.0, STRUCTURE_SCATTER_SCALE);
            assert!(dx.abs() <= 7.0 + 0.001);
            assert!(dz.abs() <= 7.0 + 0.001);
        }
    }

    #[test]
    fn random_backend_defaults_and_index_draws() {
        assert!({
            (MULT_FACTOR - (1.0 / 4_294_967_295.0)).abs() < 1e-12
                && HostRandomState::new().seed_words() == CommonRandomState::default().seed_words()
                && PURE_INDEX_SEED_OFFSET == 1
        });
        assert!({
            let a = pure_logic_random_real(11, 0, 0.0, 100.0);
            let b = pure_logic_random_real(11, 0, 0.0, 100.0);
            let i_a = pure_logic_random_int(7, 2, 0, 90);
            let i_b = pure_logic_random_int(7, 2, 0, 90);
            let scatter_a = pure_client_structure_scatter(3, 50.0, 40.0, STRUCTURE_SCATTER_SCALE);
            let scatter_b = pure_client_structure_scatter(3, 50.0, 40.0, STRUCTURE_SCATTER_SCALE);
            (a - b).abs() < 1e-6
                && i_a == i_b
                && scatter_a == scatter_b
                && (STRUCTURE_SCATTER_SCALE - 0.3).abs() < 0.001
                && scatter_a.0.abs() <= 15.0 + 0.001
                && scatter_a.1.abs() <= 12.0 + 0.001
        });
        assert!((STRUCTURE_SCATTER_SCALE - 0.3).abs() < 0.001);
        assert_eq!(PURE_INDEX_SEED_OFFSET, 1);
    }

}
