// FILE: audio/simpleplayer.rs
// Ported from: GeneralsMD/Code/GameEngine/Include/Common/SimplePlayer.h + Source/Common/Audio/SimplePlayer.cpp
//
// PARITY_NOTE: The C++ CSimplePlayer uses Windows Media Format SDK (WMF)
// and waveOut for audio playback.  The Rust port uses a cross-platform
// audio engine (rodio-based) via the common::audio::simple_player module.
// All public API methods (play, close, add_ref, release, etc.) are preserved.

pub use crate::common::audio::simple_player::*;
