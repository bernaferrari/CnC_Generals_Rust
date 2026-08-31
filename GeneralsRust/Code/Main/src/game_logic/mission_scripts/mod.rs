//! Mission script engine host integration, split by original C++ scripting
//! domains (ScriptEngine.cpp, Scripts.cpp, ScriptActions.cpp).
//!
//! The fragments are textual members of this module, so item visibility,
//! action order, lookup semantics, frame timing, side effects, and the
//! public API are unchanged.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::localization;
use gamelogic::scripting::core::{Script, ScriptAction, ScriptActionType, ScriptList};
use gamelogic::scripting::engine::{
    ScriptActionHandler, get_script_engine, initialize_script_engine,
};
use gamelogic::scripting::evaluator::ScriptEvaluator;
use gamelogic::{GameLogicError, GameLogicResult};
use glam::Vec3;

const SPEECH_SUBTITLE_DURATION_MS: i32 = 8000;

/// Live-only identity allocator for C++'s one active InGamePopupMessage WND.
///
/// This deliberately outlives individual `MissionScriptHooks` / `GameLogic`
/// instances.  Map loads and whole-world replacement create new hook objects;
/// keeping the counter there would let a delayed acknowledgement for old popup
/// #1 accidentally match a new world's popup #1.  It is neither gameplay nor
/// presentation/save/Xfer data.
static NEXT_LIVE_POPUP_GENERATION: AtomicUsize = AtomicUsize::new(1);

fn next_live_popup_generation() -> usize {
    // Zero is the explicit fail-closed "no active host popup" value.  Skip it
    // if an effectively-unreachable usize wrap occurs rather than publishing
    // a token Main intentionally refuses to acknowledge.
    loop {
        let generation = NEXT_LIVE_POPUP_GENERATION.fetch_add(1, Ordering::Relaxed);
        if generation != 0 {
            return generation;
        }
    }
}

fn speech_subtitle_label(name: &str) -> String {
    format!("DIALOGEVENT:{}Subtitle", name)
}

fn speech_subtitle_label_if_displayable<F>(name: &str, lookup: F) -> Option<String>
where
    F: FnOnce(&str) -> Option<String>,
{
    let label = speech_subtitle_label(name);
    let subtitle = lookup(&label)?;
    if subtitle.is_empty() || subtitle.starts_with('*') {
        return None;
    }
    Some(label)
}

/// C++ `ScriptEngine::isSpeechComplete` completion frame
/// (`ScriptEngine.cpp:7278-7284`, leftover `named_trackers.rs`).
/// `REAL_TO_UNSIGNEDINT(TheAudio->getAudioLengthMS / MSEC_PER_LOGICFRAME_REAL)`.
fn speech_frames_from_length_ms(audio_length_ms: f32) -> u64 {
    ((audio_length_ms.max(0.0) / 1000.0)
        * game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32) as u64
}

fn speech_completion_frame(now: u64, name: &str) -> u64 {
    let audio_length_ms = gamelogic::helpers::TheAudio::get()
        .map(|audio| {
            let event = gamelogic::common::audio::AudioEventRts::new(name);
            audio.get_audio_length_ms(&event)
        })
        .unwrap_or(0.0);
    now.saturating_add(speech_frames_from_length_ms(audio_length_ms))
}

fn camera_coord3d_to_world(x: f32, y: f32, z: f32) -> Vec3 {
    // Generals Coord3D: (x,y) on the map plane, z = height.
    // Main renderer world: x/z on the map plane, y = height.
    Vec3::new(x, z, y)
}

fn delay_frames(seconds: i32) -> u64 {
    if seconds <= 0 {
        1
    } else {
        (seconds as u64 * 30).max(1)
    }
}

include!("script_requests.rs");
include!("script_engine.rs");
include!("script_hooks.rs");
include!("script_actions.rs");

#[cfg(test)]
include!("tests.rs");
