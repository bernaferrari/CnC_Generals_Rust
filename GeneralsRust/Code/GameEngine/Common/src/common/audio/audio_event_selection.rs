//! Per-submission sound variation. Shared event logic never selects a world.
use crate::common::random_value::{
    RandomState, get_game_logic_random_value,
};

pub trait AudioEventSelection {
    fn index(&mut self, logical: bool, count: usize) -> usize;
    fn real(&mut self, low: f32, high: f32) -> f32;
}

/// Receiving match's logic stream and audio presentation's separate stream.
pub struct AudioSelectionStreams<'a> {
    pub logic: &'a mut RandomState,
    pub audio: &'a mut RandomState,
}
impl AudioEventSelection for AudioSelectionStreams<'_> {
    fn index(&mut self, logical: bool, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        let stream = if logical {
            &mut *self.logic
        } else {
            &mut *self.audio
        };
        stream.next_u32() as usize % count
    }
    fn real(&mut self, low: f32, high: f32) -> f32 {
        self.audio.next_real(low, high)
    }
}

/// Logic draws stay on the thread's logic stream; audio draws come from the
/// borrowed audio stream of the receiving audio owner.
pub(crate) struct AmbientAudioSelection<'a>(pub(crate) &'a mut RandomState);

impl AudioEventSelection for AmbientAudioSelection<'_> {
    fn index(&mut self, logical: bool, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        let maximum = count as i32 - 1;
        if logical {
            get_game_logic_random_value(0, maximum) as usize
        } else {
            self.0.next_int(0, maximum) as usize
        }
    }
    fn real(&mut self, low: f32, high: f32) -> f32 {
        self.0.next_real(low, high)
    }
}
