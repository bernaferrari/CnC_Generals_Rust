//! Existing client sampling explicitly borrowed from the already staged native-load transaction.
use game_engine::common::random_value::RandomState;

pub(in crate::effects) trait ParticleRandomSource {
    fn real(&mut self, lo: f32, hi: f32) -> f32;
    fn integer(&mut self, lo: i32, hi: i32) -> i32;
}

impl ParticleRandomSource for RandomState {
    fn real(&mut self, lo: f32, hi: f32) -> f32 {
        self.next_real(lo, hi)
    }
    fn integer(&mut self, lo: i32, hi: i32) -> i32 {
        // Identical inclusive-range arithmetic to the existing ambient adapter.
        let delta = hi.wrapping_sub(lo).wrapping_add(1) as u32;
        if delta == 0 {
            return hi;
        }
        ((self.next_u32() % delta) as i32).wrapping_add(lo)
    }
}
