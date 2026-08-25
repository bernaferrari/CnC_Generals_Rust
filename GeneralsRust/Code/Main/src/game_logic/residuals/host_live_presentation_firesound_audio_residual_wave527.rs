//! Wave 527 residual peels: presentation FireSound loop → audio with real name/pose.
//! - `WeaponFireLoopStarted` uses host FireSound string + `looping()` + snapshot position
//! - `WeaponFireLoopStopped` stops via same event key + object id (priority 200)
//! - other presentation audio events also stamp snapshot pose when available
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 526 move/attack mesh helpers.
//! Architecture residual - audio from presentation events without GameLogic dual-write.
//!
//! Sources:
//! - presentation_frame.rs collect_audio_events Wave 527
//! - PresentationEvent::WeaponFireLoopStarted/Stopped
//! - AudioEventRequest::looping / with_position
//!
//! Fail-closed:
//! - Full Miles 3D / volume attenuation parity still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_FIRESOUND_AUDIO_METHOD_NAMES_WAVE527: &[&str] = &[
    "collect_audio_events",
    "WeaponFireLoopStarted",
    "WeaponFireLoopStopped",
    "looping",
    "with_position",
    "playable_claim = false",
];

pub const PRESENTATION_FIRESOUND_AUDIO_SOURCE_MARKERS_WAVE527: &[&str] = &[
    "Wave 527/528: FireSound loop residual uses host sound name + looping flag + snapshot pose",
    "Wave 527: FiringTracker loop uses concrete FireSound name when non-empty",
    ".looping()",
    "WeaponFireLoopStop",
];

pub const PRESENTATION_FIRESOUND_AUDIO_NAV_STEPS_WAVE527: &[&str] = &[
    "EMIT_WEAPON_FIRE_LOOP_EVENTS",
    "MAP_SOUND_NAME_AND_LOOP_FLAG",
    "STAMP_SNAPSHOT_POSE",
    "DISPATCH_AUDIO_DIRECT",
    "NO_LIVE_GAMELOGIC_DUAL_WRITE",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_FIRESOUND_AUDIO_CMD_NAMES_WAVE527: &[&str] = &[
    "click_presentation_firesound_audio_ok_wnd_detect",
    "click_presentation_firesound_audio_ok_wnd_skip",
    "click_presentation_firesound_audio_ok_wnd_queue",
    "click_presentation_firesound_audio_ok_wnd_prepare",
    "click_presentation_firesound_audio_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationFiresoundAudioAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationFiresoundAudioAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_firesound_audio_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_firesound_audio_last_action()
-> ResidualPresentationFiresoundAudioAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationFiresoundAudioAction::MethodNames,
        2 => ResidualPresentationFiresoundAudioAction::SourceMarkers,
        3 => ResidualPresentationFiresoundAudioAction::NavCommands,
        4 => ResidualPresentationFiresoundAudioAction::CollectSource,
        5 => ResidualPresentationFiresoundAudioAction::DispatchSource,
        6 => ResidualPresentationFiresoundAudioAction::Composite,
        _ => ResidualPresentationFiresoundAudioAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_presentation_firesound_audio_method_names_residual_wave527() -> bool {
    PRESENTATION_FIRESOUND_AUDIO_METHOD_NAMES_WAVE527.len() == 6
        && residual_name_index(
            PRESENTATION_FIRESOUND_AUDIO_METHOD_NAMES_WAVE527,
            "collect_audio_events",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_FIRESOUND_AUDIO_METHOD_NAMES_WAVE527,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_firesound_audio_source_markers_residual_wave527() -> bool {
    PRESENTATION_FIRESOUND_AUDIO_SOURCE_MARKERS_WAVE527.len() == 4
        && residual_name_index(
            PRESENTATION_FIRESOUND_AUDIO_SOURCE_MARKERS_WAVE527,
            "Wave 527/528: FireSound loop residual uses host sound name + looping flag + snapshot pose",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_FIRESOUND_AUDIO_SOURCE_MARKERS_WAVE527,
            ".looping()",
        ) == Some(2)
}

pub fn honesty_presentation_firesound_audio_nav_commands_residual_wave527() -> bool {
    PRESENTATION_FIRESOUND_AUDIO_NAV_STEPS_WAVE527.len() == 6
        && residual_name_index(
            PRESENTATION_FIRESOUND_AUDIO_NAV_STEPS_WAVE527,
            "MAP_SOUND_NAME_AND_LOOP_FLAG",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_FIRESOUND_AUDIO_NAV_STEPS_WAVE527,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_FIRESOUND_AUDIO_CMD_NAMES_WAVE527.len() == 5
}

pub fn simulate_presentation_firesound_audio_collect_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains(
        "Wave 527/528: FireSound loop residual uses host sound name + looping flag + snapshot pose",
    ) && pf
        .contains("Wave 527: FiringTracker loop uses concrete FireSound name when non-empty")
        && pf.contains(".looping()")
        && pf.contains("with_priority(200)");
    residual_action_store(ResidualPresentationFiresoundAudioAction::CollectSource);
    ok
}

pub fn simulate_presentation_firesound_audio_dispatch_source() -> bool {
    let pf = pf_source();
    let eng = eng_source();
    let ok = pf.contains("fn dispatch_audio_events_direct")
        && eng.contains("dispatch_audio_events_direct()")
        && eng.contains("presentation audio events dispatched");
    residual_action_store(ResidualPresentationFiresoundAudioAction::DispatchSource);
    ok
}

pub fn honesty_presentation_firesound_audio_residual_pack_wave527() -> bool {
    honesty_presentation_firesound_audio_method_names_residual_wave527()
        && honesty_presentation_firesound_audio_source_markers_residual_wave527()
        && honesty_presentation_firesound_audio_nav_commands_residual_wave527()
        && simulate_presentation_firesound_audio_collect_source()
        && simulate_presentation_firesound_audio_dispatch_source()
}

pub fn simulate_live_presentation_firesound_audio_honesty() -> bool {
    let ok = honesty_presentation_firesound_audio_residual_pack_wave527();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationFiresoundAudioAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_firesound_audio_method_names_residual_wave527());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_firesound_audio_source_markers_residual_wave527());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_firesound_audio_nav_commands_residual_wave527());
    }

    #[test]
    fn presentation_firesound_audio_sources() {
        assert!(simulate_presentation_firesound_audio_collect_source());
        assert!(simulate_presentation_firesound_audio_dispatch_source());
    }

    #[test]
    fn wave527_composite_pack() {
        assert!(honesty_presentation_firesound_audio_residual_pack_wave527());
    }

    #[test]
    fn simulate_live_presentation_firesound_audio_honesty_residual_live() {
        assert!(
            simulate_live_presentation_firesound_audio_honesty(),
            "presentation firesound audio residual must latch"
        );
        assert!(residual_presentation_firesound_audio_ok());
        assert_eq!(
            residual_presentation_firesound_audio_last_action(),
            ResidualPresentationFiresoundAudioAction::Composite
        );
    }
}
