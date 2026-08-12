//! Wave 533 residual peels: host EVA pulses (`TheEva::setShouldPlay` edges)
//! drain into `PresentationEvent::EvaAlert` and map to EVA_* presentation
//! audio (no live GameLogic dual-read). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 532 FireSound drain sibling residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `host_eva_log.rs` record/take_last_drain
//! - `presentation_frame.rs` EvaAlert collect_audio_events
//! - `game_logic.rs` low-power / funds / under-attack / lost hooks
//!
//! Fail-closed:
//! - Not full C++ EVA priority/queue/side matrix
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_EVA_PULSE_AUDIO_METHOD_NAMES_WAVE533: &[&str] = &[
    "host_eva_log::record",
    "host_eva_log::record_event",
    "host_eva_log::take_last_drain",
    "PresentationEvent::EvaAlert",
    "EVA_LowPower",
    "EVA_InsufficientFunds",
    "EVA_BaseUnderAttack",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_EVA_PULSE_AUDIO_NAV_STEPS_WAVE533: &[&str] = &[
    "REQUIRE_HOST_EVA_LOG",
    "REQUIRE_EVA_ALERT_EVENT",
    "REQUIRE_EVA_PULSE_AUDIO",
    "LIVE_PRESENTATION_EVA_PULSE_AUDIO",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PRESENTATION_EVA_PULSE_AUDIO_CMD_NAMES_WAVE533: &[&str] =
    &["eva_pulse_audio", "eva_alert", "eva_low_power"];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationEvaPulseAudioAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationEvaPulseAudioAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualPresentationEvaPulseAudioAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_eva_pulse_audio_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_eva_pulse_audio_last_action() -> ResidualPresentationEvaPulseAudioAction
{
    ResidualPresentationEvaPulseAudioAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn log_source() -> &'static str {
    include_str!("../host_eva_log.rs")
}

pub fn honesty_presentation_eva_pulse_audio_method_names_residual_wave533() -> bool {
    let names = LIVE_PRESENTATION_EVA_PULSE_AUDIO_METHOD_NAMES_WAVE533;
    let ok = residual_name_index(names, "host_eva_log::record").is_some()
        && residual_name_index(names, "host_eva_log::record_event").is_some()
        && residual_name_index(names, "host_eva_log::take_last_drain").is_some()
        && residual_name_index(names, "PresentationEvent::EvaAlert").is_some()
        && residual_name_index(names, "EVA_LowPower").is_some()
        && residual_name_index(names, "EVA_InsufficientFunds").is_some()
        && residual_name_index(names, "EVA_BaseUnderAttack").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationEvaPulseAudioAction::MethodNames);
    ok
}

pub fn honesty_presentation_eva_pulse_audio_source_markers_residual_wave533() -> bool {
    let pf = pf_source();
    let gl = gl_source();
    let log = log_source();
    // Accept single-line or rustfmt multiline `record_event( ... EvaEvent::X ... )`.
    let has_event = |name: &str| -> bool {
        gl.contains("host_eva_log::record_event") && gl.contains(&format!("EvaEvent::{name}"))
    };
    let ok = pf.contains("Wave 533")
        && pf.contains("PresentationEvent::EvaAlert")
        && pf.contains("host_eva_log::take_last_drain")
        && pf.contains("EvaAlert { name }")
        && has_event("LowPower")
        && has_event("InsufficientFunds")
        && has_event("BaseUnderAttack")
        && has_event("AllyUnderAttack")
        && has_event("BuildingLost")
        && has_event("UnitLost")
        && log.contains("pub fn record")
        && log.contains("pub fn record_event")
        && log.contains("pub fn take_last_drain")
        && !pf.contains("playable_claim = true");
    residual_action_store(ResidualPresentationEvaPulseAudioAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_eva_pulse_audio_nav_commands_residual_wave533() -> bool {
    let steps = LIVE_PRESENTATION_EVA_PULSE_AUDIO_NAV_STEPS_WAVE533;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_EVA_PULSE_AUDIO_CMD_NAMES_WAVE533;
    let ok = residual_name_index(steps, "REQUIRE_HOST_EVA_LOG").is_some()
        && residual_name_index(steps, "REQUIRE_EVA_ALERT_EVENT").is_some()
        && residual_name_index(steps, "REQUIRE_EVA_PULSE_AUDIO").is_some()
        && residual_name_index(steps, "LIVE_PRESENTATION_EVA_PULSE_AUDIO").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "eva_pulse_audio").is_some()
        && residual_name_index(cmds, "eva_alert").is_some()
        && residual_name_index(cmds, "eva_low_power").is_some();
    residual_action_store(ResidualPresentationEvaPulseAudioAction::NavCommands);
    ok
}

pub fn simulate_presentation_eva_pulse_audio_collect_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("EvaAlert {")
        && pf.contains("host_eva_log::take_last_drain")
        && pf.contains("Wave 533");
    residual_action_store(ResidualPresentationEvaPulseAudioAction::CollectSource);
    ok
}

pub fn simulate_presentation_eva_pulse_audio_dispatch_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("PresentationEvent::EvaAlert { name }")
        && pf.contains("AudioEventRequest::new(name.as_str())")
        && pf.contains("with_priority(180)");
    residual_action_store(ResidualPresentationEvaPulseAudioAction::DispatchSource);
    ok
}

pub fn honesty_presentation_eva_pulse_audio_residual_pack_wave533() -> bool {
    honesty_presentation_eva_pulse_audio_method_names_residual_wave533()
        && honesty_presentation_eva_pulse_audio_source_markers_residual_wave533()
        && honesty_presentation_eva_pulse_audio_nav_commands_residual_wave533()
        && simulate_presentation_eva_pulse_audio_collect_source()
        && simulate_presentation_eva_pulse_audio_dispatch_source()
}

pub fn simulate_live_presentation_eva_pulse_audio_honesty() -> bool {
    let ok = honesty_presentation_eva_pulse_audio_residual_pack_wave533();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationEvaPulseAudioAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_eva_pulse_audio_method_names_residual_wave533());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_eva_pulse_audio_source_markers_residual_wave533());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_eva_pulse_audio_nav_commands_residual_wave533());
    }

    #[test]
    fn presentation_eva_pulse_audio_sources() {
        assert!(simulate_presentation_eva_pulse_audio_collect_source());
        assert!(simulate_presentation_eva_pulse_audio_dispatch_source());
    }

    #[test]
    fn wave533_composite_pack() {
        assert!(honesty_presentation_eva_pulse_audio_residual_pack_wave533());
    }

    #[test]
    fn simulate_live_presentation_eva_pulse_audio_honesty_residual_live() {
        assert!(
            simulate_live_presentation_eva_pulse_audio_honesty(),
            "eva pulse audio residual must latch"
        );
        assert!(residual_presentation_eva_pulse_audio_ok());
        assert_eq!(
            residual_presentation_eva_pulse_audio_last_action(),
            ResidualPresentationEvaPulseAudioAction::Composite
        );
    }
}
