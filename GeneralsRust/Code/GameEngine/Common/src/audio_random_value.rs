//! Compatibility module for C++ Common/AudioRandomValue.h.
//!
//! C++ `GameAudioRandomValue[Real]` draw from the audio stream. In the port the
//! stream is a [`RandomState`] owned by the receiving audio manager; draw with
//! `RandomState::next_int` / `RandomState::next_real` on the borrowed stream.

pub use crate::common::random_value::RandomState;
