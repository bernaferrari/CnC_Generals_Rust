//! Compatibility module for C++ Common/AudioRandomValue.h
//! Re-exports audio random value functions from common::random_value.
//!
//! PARITY_NOTE: The C++ AudioRandomValue.h provides two inline functions:
//!   - get_game_audio_random_value(lo, hi) -> int  (with __FILE__, __LINE__ params)
//!   - get_game_audio_random_value_real(lo, hi) -> Real
//! These wrap the game's audio random seed for deterministic audio behavior.
//! The Rust port in common::random_value preserves both signatures without
//! the file/line debug params (Rust's backtrace serves the same purpose).

pub use crate::common::random_value::{
    get_game_audio_random_value, get_game_audio_random_value_real,
};
