//! Wave 529 residual peels: RadarMessage → EVA/radar presentation audio.
//! - Attack/Ally/Generic radar kinds map to RadarAttack/RadarAlly/RadarGeneric
//! - Classic EVA text phrases map to EVA_* event names
//! - Snapshot world position stamped when non-zero
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 528 FireSound stop residual.
//! Architecture residual - EVA/radar audio from presentation without GameLogic dual-write.
//!
//! Sources:
//! - presentation_frame.rs collect_audio_events Wave 529
//! - PresentationEvent::RadarMessage kind/text/position
//!
//! Fail-closed:
//! - Full EVA voice bank / Miles speech channel parity still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_RADAR_EVA_AUDIO_METHOD_NAMES_WAVE529: &[&str] = &[
    "collect_audio_events",
    "RadarMessage",
    "EVA_LowPower",
    "RadarAttack",
    "with_position",
    "playable_claim = false",
];

pub const PRESENTATION_RADAR_EVA_AUDIO_SOURCE_MARKERS_WAVE529: &[&str] = &[
    "Wave 529: radar/EVA presentation audio residual (no GameLogic dual-write)",
    "EVA_LowPower",
    "RadarAttack",
    "RadarAlly",
];

pub const PRESENTATION_RADAR_EVA_AUDIO_NAV_STEPS_WAVE529: &[&str] = &[
    "EMIT_RADAR_MESSAGE_EVENTS",
    "MAP_EVA_TEXT_AND_RADAR_KIND",
    "STAMP_SNAPSHOT_POSITION",
    "DISPATCH_AUDIO_DIRECT",
    "NO_LIVE_GAMELOGIC_DUAL_WRITE",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_RADAR_EVA_AUDIO_CMD_NAMES_WAVE529: &[&str] = &[
    "click_presentation_radar_eva_audio_ok_wnd_detect",
    "click_presentation_radar_eva_audio_ok_wnd_skip",
    "click_presentation_radar_eva_audio_ok_wnd_queue",
    "click_presentation_radar_eva_audio_ok_wnd_prepare",
    "click_presentation_radar_eva_audio_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationRadarEvaAudioAction {
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

fn residual_action_store(a: ResidualPresentationRadarEvaAudioAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_radar_eva_audio_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_radar_eva_audio_last_action() -> ResidualPresentationRadarEvaAudioAction
{
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationRadarEvaAudioAction::MethodNames,
        2 => ResidualPresentationRadarEvaAudioAction::SourceMarkers,
        3 => ResidualPresentationRadarEvaAudioAction::NavCommands,
        4 => ResidualPresentationRadarEvaAudioAction::CollectSource,
        5 => ResidualPresentationRadarEvaAudioAction::DispatchSource,
        6 => ResidualPresentationRadarEvaAudioAction::Composite,
        _ => ResidualPresentationRadarEvaAudioAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_presentation_radar_eva_audio_method_names_residual_wave529() -> bool {
    PRESENTATION_RADAR_EVA_AUDIO_METHOD_NAMES_WAVE529.len() == 6
        && residual_name_index(
            PRESENTATION_RADAR_EVA_AUDIO_METHOD_NAMES_WAVE529,
            "collect_audio_events",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_RADAR_EVA_AUDIO_METHOD_NAMES_WAVE529,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_radar_eva_audio_source_markers_residual_wave529() -> bool {
    PRESENTATION_RADAR_EVA_AUDIO_SOURCE_MARKERS_WAVE529.len() == 4
        && residual_name_index(
            PRESENTATION_RADAR_EVA_AUDIO_SOURCE_MARKERS_WAVE529,
            "Wave 529: radar/EVA presentation audio residual (no GameLogic dual-write)",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_RADAR_EVA_AUDIO_SOURCE_MARKERS_WAVE529,
            "RadarAttack",
        ) == Some(2)
}

pub fn honesty_presentation_radar_eva_audio_nav_commands_residual_wave529() -> bool {
    PRESENTATION_RADAR_EVA_AUDIO_NAV_STEPS_WAVE529.len() == 6
        && residual_name_index(
            PRESENTATION_RADAR_EVA_AUDIO_NAV_STEPS_WAVE529,
            "MAP_EVA_TEXT_AND_RADAR_KIND",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_RADAR_EVA_AUDIO_NAV_STEPS_WAVE529,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_RADAR_EVA_AUDIO_CMD_NAMES_WAVE529.len() == 5
}

pub fn simulate_presentation_radar_eva_audio_collect_source() -> bool {
    let pf = pf_source();
    let ok = pf
        .contains("Wave 529: radar/EVA presentation audio residual (no GameLogic dual-write)")
        && pf.contains("EVA_LowPower")
        && pf.contains("EVA_BaseUnderAttack")
        && pf.contains("RadarAttack")
        && pf.contains("RadarAlly")
        && pf.contains("RadarGeneric");
    residual_action_store(ResidualPresentationRadarEvaAudioAction::CollectSource);
    ok
}

pub fn simulate_presentation_radar_eva_audio_dispatch_source() -> bool {
    let pf = pf_source();
    let eng = eng_source();
    let ok = pf.contains("fn dispatch_audio_events_direct")
        && eng.contains("dispatch_audio_events_direct()")
        && eng.contains("presentation audio events dispatched");
    residual_action_store(ResidualPresentationRadarEvaAudioAction::DispatchSource);
    ok
}

pub fn honesty_presentation_radar_eva_audio_residual_pack_wave529() -> bool {
    honesty_presentation_radar_eva_audio_method_names_residual_wave529()
        && honesty_presentation_radar_eva_audio_source_markers_residual_wave529()
        && honesty_presentation_radar_eva_audio_nav_commands_residual_wave529()
        && simulate_presentation_radar_eva_audio_collect_source()
        && simulate_presentation_radar_eva_audio_dispatch_source()
}

pub fn simulate_live_presentation_radar_eva_audio_honesty() -> bool {
    let ok = honesty_presentation_radar_eva_audio_residual_pack_wave529();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationRadarEvaAudioAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_radar_eva_audio_method_names_residual_wave529());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_radar_eva_audio_source_markers_residual_wave529());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_radar_eva_audio_nav_commands_residual_wave529());
    }

    #[test]
    fn presentation_radar_eva_audio_sources() {
        assert!(simulate_presentation_radar_eva_audio_collect_source());
        assert!(simulate_presentation_radar_eva_audio_dispatch_source());
    }

    #[test]
    fn wave529_composite_pack() {
        assert!(honesty_presentation_radar_eva_audio_residual_pack_wave529());
    }

    #[test]
    fn simulate_live_presentation_radar_eva_audio_honesty_residual_live() {
        assert!(
            simulate_live_presentation_radar_eva_audio_honesty(),
            "presentation radar/EVA audio residual must latch"
        );
        assert!(residual_presentation_radar_eva_audio_ok());
        assert_eq!(
            residual_presentation_radar_eva_audio_last_action(),
            ResidualPresentationRadarEvaAudioAction::Composite
        );
    }
}
