//! Mission script engine host integration, split by original C++ scripting
//! domains (ScriptEngine.cpp, Scripts.cpp, ScriptActions.cpp).
//!
//! The fragments are textual members of this module, so item visibility,
//! action order, lookup semantics, frame timing, side effects, and the
//! request interface are preserved. Script execution and active state live in
//! the canonical GameLogic ScriptEngine; hooks do not own an interpreter.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::localization;
use gamelogic::GameLogicResult;
use gamelogic::scripting::engine::{ScriptActionHandler, ScriptCameraRequest};
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

fn camera_coord3d_to_world(x: f32, y: f32, z: f32) -> Vec3 {
    // Generals Coord3D: (x,y) on the map plane, z = height.
    // Main renderer world: x/z on the map plane, y = height.
    Vec3::new(x, z, y)
}

include!("script_requests.rs");
include!("script_hooks.rs");
include!("script_camera_actions.rs");
include!("script_actions.rs");

#[cfg(test)]
include!("tests.rs");

#[cfg(test)]
mod speech_completion_tests;
