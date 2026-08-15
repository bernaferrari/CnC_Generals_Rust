//! Client RNG seam for bounce / overlap vibration.
//!
//! C++ uses `GameClientRandomValue` / `GameClientRandomValueReal`
//! (`ClientRandomValue.h:18-19`). Production callers use the live client
//! stream; tests inject a scripted sequence.

/// Integer + real draws matching the C++ client RNG macros.
pub trait ClientVisualRng {
    fn random_int(&mut self, lo: i32, hi: i32) -> i32;
    fn random_real(&mut self, lo: f32, hi: f32) -> f32;
}

/// Live `GameClientRandomValue` stream.
#[derive(Debug, Default, Clone, Copy)]
pub struct LiveClientRng;

impl ClientVisualRng for LiveClientRng {
    fn random_int(&mut self, lo: i32, hi: i32) -> i32 {
        crate::client_random_value::get_game_client_random_value(lo, hi, file!(), line!())
    }

    fn random_real(&mut self, lo: f32, hi: f32) -> f32 {
        crate::client_random_value::get_game_client_random_value_real(lo, hi, file!(), line!())
    }
}

/// Deterministic RNG for focused calc tests.
#[derive(Debug, Clone)]
pub struct ScriptedClientRng {
    ints: Vec<i32>,
    reals: Vec<f32>,
    int_at: usize,
    real_at: usize,
}

impl ScriptedClientRng {
    #[must_use]
    pub fn new(ints: Vec<i32>, reals: Vec<f32>) -> Self {
        Self {
            ints,
            reals,
            int_at: 0,
            real_at: 0,
        }
    }

    #[must_use]
    pub fn ints(ints: Vec<i32>) -> Self {
        Self::new(ints, Vec::new())
    }

    #[must_use]
    pub fn reals(reals: Vec<f32>) -> Self {
        Self::new(Vec::new(), reals)
    }
}

impl ClientVisualRng for ScriptedClientRng {
    fn random_int(&mut self, lo: i32, hi: i32) -> i32 {
        if let Some(value) = self.ints.get(self.int_at).copied() {
            self.int_at += 1;
            value.clamp(lo, hi)
        } else {
            lo
        }
    }

    fn random_real(&mut self, lo: f32, hi: f32) -> f32 {
        if let Some(value) = self.reals.get(self.real_at).copied() {
            self.real_at += 1;
            value.clamp(lo.min(hi), lo.max(hi))
        } else {
            lo
        }
    }
}
