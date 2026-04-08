//! Compatibility module for C++ Common/AudioAffect.h
//! Re-exports AudioAffect from the canonical location in common::audio.
//!
//! PARITY_NOTE: The C++ AudioAffect.h defines an unsigned 32-bit flag enum:
//!   Music=0x01, Sound=0x02, Sound3D=0x04, Speech=0x08, SystemSetting=0x10,
//!   All=0x0F (Music|Sound|Sound3D|Speech).
//! The Rust AudioAffect in common::audio::game_audio also includes Ambient=0x20
//! and uses SoundEffects=0x06 (Sound|Sound3D).  These extra variants are
//! Rust-specific extensions; the core C++ values are preserved exactly.

pub use crate::common::audio::game_audio::AudioAffect;
