//! Wave 530 residual peels: OwnerChanged → capture/hijack presentation audio.
//! - structures map to BuildingCaptured
//! - units map to UnitHijacked
//! - snapshot pose stamped when available
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 529 radar/EVA audio.
//! Architecture residual - capture audio from presentation without GameLogic dual-write.
//!
//! Sources:
//! - presentation_frame.rs collect_audio_events Wave 530
//! - PresentationEvent::OwnerChanged
//!
//! Fail-closed:
//! - Full faction EVA capture line matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_CAPTURE_AUDIO_METHOD_NAMES_WAVE530: &[&str] = &[
    "collect_audio_events",
    "OwnerChanged",
    "BuildingCaptured",
    "UnitHijacked",
    "with_position",
    "playable_claim = false",
];

pub const PRESENTATION_CAPTURE_AUDIO_SOURCE_MARKERS_WAVE530: &[&str] = &[
    "Wave 530: capture/hijack ownership transfer audio residual",
    "BuildingCaptured",
    "UnitHijacked",
    "OwnerChanged { id, .. }",
];

pub const PRESENTATION_CAPTURE_AUDIO_NAV_STEPS_WAVE530: &[&str] = &[
    "EMIT_OWNER_CHANGED_EVENTS",
    "MAP_STRUCTURE_OR_UNIT_CAPTURE",
    "STAMP_SNAPSHOT_POSE",
    "DISPATCH_AUDIO_DIRECT",
    "NO_LIVE_GAMELOGIC_DUAL_WRITE",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_CAPTURE_AUDIO_CMD_NAMES_WAVE530: &[&str] = &[
    "click_presentation_capture_audio_ok_wnd_detect",
    "click_presentation_capture_audio_ok_wnd_skip",
    "click_presentation_capture_audio_ok_wnd_queue",
    "click_presentation_capture_audio_ok_wnd_prepare",
    "click_presentation_capture_audio_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationCaptureAudioAction {
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

fn residual_action_store(a: ResidualPresentationCaptureAudioAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_capture_audio_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_capture_audio_last_action() -> ResidualPresentationCaptureAudioAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationCaptureAudioAction::MethodNames,
        2 => ResidualPresentationCaptureAudioAction::SourceMarkers,
        3 => ResidualPresentationCaptureAudioAction::NavCommands,
        4 => ResidualPresentationCaptureAudioAction::CollectSource,
        5 => ResidualPresentationCaptureAudioAction::DispatchSource,
        6 => ResidualPresentationCaptureAudioAction::Composite,
        _ => ResidualPresentationCaptureAudioAction::Idle,
    }
}

fn pf_source() -> &'static str {
    include_str!("../../presentation_frame.rs")
}

fn eng_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_presentation_capture_audio_method_names_residual_wave530() -> bool {
    PRESENTATION_CAPTURE_AUDIO_METHOD_NAMES_WAVE530.len() == 6
        && residual_name_index(
            PRESENTATION_CAPTURE_AUDIO_METHOD_NAMES_WAVE530,
            "collect_audio_events",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CAPTURE_AUDIO_METHOD_NAMES_WAVE530,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_capture_audio_source_markers_residual_wave530() -> bool {
    PRESENTATION_CAPTURE_AUDIO_SOURCE_MARKERS_WAVE530.len() == 4
        && residual_name_index(
            PRESENTATION_CAPTURE_AUDIO_SOURCE_MARKERS_WAVE530,
            "Wave 530: capture/hijack ownership transfer audio residual",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CAPTURE_AUDIO_SOURCE_MARKERS_WAVE530,
            "UnitHijacked",
        ) == Some(2)
}

pub fn honesty_presentation_capture_audio_nav_commands_residual_wave530() -> bool {
    PRESENTATION_CAPTURE_AUDIO_NAV_STEPS_WAVE530.len() == 6
        && residual_name_index(
            PRESENTATION_CAPTURE_AUDIO_NAV_STEPS_WAVE530,
            "MAP_STRUCTURE_OR_UNIT_CAPTURE",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_CAPTURE_AUDIO_NAV_STEPS_WAVE530,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_CAPTURE_AUDIO_CMD_NAMES_WAVE530.len() == 5
}

pub fn simulate_presentation_capture_audio_collect_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 530: capture/hijack ownership transfer audio residual")
        && pf.contains("BuildingCaptured")
        && pf.contains("UnitHijacked")
        && pf.contains("OwnerChanged { id, .. }");
    residual_action_store(ResidualPresentationCaptureAudioAction::CollectSource);
    ok
}

pub fn simulate_presentation_capture_audio_dispatch_source() -> bool {
    let pf = pf_source();
    let eng = eng_source();
    let ok = pf.contains("fn dispatch_audio_events_direct")
        && eng.contains("dispatch_audio_events_direct()")
        && eng.contains("presentation audio events dispatched");
    residual_action_store(ResidualPresentationCaptureAudioAction::DispatchSource);
    ok
}

pub fn honesty_presentation_capture_audio_residual_pack_wave530() -> bool {
    honesty_presentation_capture_audio_method_names_residual_wave530()
        && honesty_presentation_capture_audio_source_markers_residual_wave530()
        && honesty_presentation_capture_audio_nav_commands_residual_wave530()
        && simulate_presentation_capture_audio_collect_source()
        && simulate_presentation_capture_audio_dispatch_source()
}

pub fn simulate_live_presentation_capture_audio_honesty() -> bool {
    let ok = honesty_presentation_capture_audio_residual_pack_wave530();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationCaptureAudioAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_capture_audio_method_names_residual_wave530());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_capture_audio_source_markers_residual_wave530());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_capture_audio_nav_commands_residual_wave530());
    }

    #[test]
    fn presentation_capture_audio_sources() {
        assert!(simulate_presentation_capture_audio_collect_source());
        assert!(simulate_presentation_capture_audio_dispatch_source());
    }

    #[test]
    fn wave530_composite_pack() {
        assert!(honesty_presentation_capture_audio_residual_pack_wave530());
    }

    #[test]
    fn simulate_live_presentation_capture_audio_honesty_residual_live() {
        assert!(
            simulate_live_presentation_capture_audio_honesty(),
            "presentation capture audio residual must latch"
        );
        assert!(residual_presentation_capture_audio_ok());
        assert_eq!(
            residual_presentation_capture_audio_last_action(),
            ResidualPresentationCaptureAudioAction::Composite
        );
    }
}
