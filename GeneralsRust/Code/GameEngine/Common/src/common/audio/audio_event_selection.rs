//! Per-submission sound variation. Shared event logic never selects a world.
use crate::common::random_value::{
    RandomState, get_game_audio_random_value, get_game_audio_random_value_real,
    get_game_logic_random_value,
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

/// Compatibility adapter for callers not yet supplying their presentation owner.
pub(crate) struct AmbientAudioSelection;
impl AudioEventSelection for AmbientAudioSelection {
    fn index(&mut self, logical: bool, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        let maximum = count as i32 - 1;
        if logical {
            get_game_logic_random_value(0, maximum) as usize
        } else {
            get_game_audio_random_value(0, maximum) as usize
        }
    }
    fn real(&mut self, low: f32, high: f32) -> f32 {
        get_game_audio_random_value_real(low, high)
    }
}
