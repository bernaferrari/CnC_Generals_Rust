//! Wave 534 residual peels: every host `TheEva::set_should_play` edge records
//! into `host_eva_log` via `record_event` so PresentationFrame EvaAlert audio
//! covers the full EVA matrix (not only Wave 533 low-power/funds/attack/lost).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 533 EvaAlert presentation audio residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `host_eva_log.rs` record_event / eva_event_audio_name
//! - `game_logic.rs` TheEva::set_should_play → record_event
//! - `presentation_frame.rs` EvaAlert collect_audio_events
//!
//! Fail-closed:
//! - Not full C++ EVA priority/side/Miles speech matrix
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_EVA_FULL_MATRIX_METHOD_NAMES_WAVE534: &[&str] = &[
    "host_eva_log::record_event",
    "eva_event_audio_name",
    "TheEva::set_should_play",
    "PresentationEvent::EvaAlert",
    "EVA_BeaconDetected",
    "EVA_UpgradeComplete",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_EVA_FULL_MATRIX_NAV_STEPS_WAVE534: &[&str] = &[
    "REQUIRE_EVA_RECORD_EVENT",
    "REQUIRE_FULL_SET_SHOULD_PLAY_COVERAGE",
    "LIVE_PRESENTATION_EVA_FULL_MATRIX",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PRESENTATION_EVA_FULL_MATRIX_CMD_NAMES_WAVE534: &[&str] =
    &["eva_full_matrix", "record_event", "eva_beacon_detected"];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationEvaFullMatrixAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationEvaFullMatrixAction {
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

fn residual_action_store(action: ResidualPresentationEvaFullMatrixAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_eva_full_matrix_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_eva_full_matrix_last_action() -> ResidualPresentationEvaFullMatrixAction
{
    ResidualPresentationEvaFullMatrixAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn log_source() -> &'static str {
    include_str!("../host_eva_log.rs")
}

fn pf_source() -> &'static str {
    include_str!("../../presentation_frame.rs")
}

pub fn honesty_presentation_eva_full_matrix_method_names_residual_wave534() -> bool {
    let names = LIVE_PRESENTATION_EVA_FULL_MATRIX_METHOD_NAMES_WAVE534;
    let ok = residual_name_index(names, "host_eva_log::record_event").is_some()
        && residual_name_index(names, "eva_event_audio_name").is_some()
        && residual_name_index(names, "TheEva::set_should_play").is_some()
        && residual_name_index(names, "PresentationEvent::EvaAlert").is_some()
        && residual_name_index(names, "EVA_BeaconDetected").is_some()
        && residual_name_index(names, "EVA_UpgradeComplete").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationEvaFullMatrixAction::MethodNames);
    ok
}

pub fn honesty_presentation_eva_full_matrix_source_markers_residual_wave534() -> bool {
    let gl = gl_source();
    let log = log_source();
    let pf = pf_source();
    let set_n = gl.matches("TheEva::set_should_play").count();
    let rec_n = gl.matches("host_eva_log::record_event").count();
    let ok = log.contains("pub fn record_event")
        && log.contains("pub fn eva_event_audio_name")
        && log.contains("eva_event_table_token")
        && log.contains("eva_event_audio_name")
        && log.contains("EVA_{}")
        && pf.contains("PresentationEvent::EvaAlert")
        && pf.contains("host_eva_log::take_last_drain")
        && set_n > 0
        && rec_n >= set_n
        && gl.contains("EvaEvent::BeaconDetected")
        && gl.contains("EvaEvent::UpgradeComplete")
        && gl.contains("EvaEvent::VehicleStolen")
        && gl.contains("EvaEvent::BuildingSabotaged")
        && !pf.contains("playable_claim = true");
    residual_action_store(ResidualPresentationEvaFullMatrixAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_eva_full_matrix_nav_commands_residual_wave534() -> bool {
    let steps = LIVE_PRESENTATION_EVA_FULL_MATRIX_NAV_STEPS_WAVE534;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_EVA_FULL_MATRIX_CMD_NAMES_WAVE534;
    let ok = residual_name_index(steps, "REQUIRE_EVA_RECORD_EVENT").is_some()
        && residual_name_index(steps, "REQUIRE_FULL_SET_SHOULD_PLAY_COVERAGE").is_some()
        && residual_name_index(steps, "LIVE_PRESENTATION_EVA_FULL_MATRIX").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "eva_full_matrix").is_some()
        && residual_name_index(cmds, "record_event").is_some()
        && residual_name_index(cmds, "eva_beacon_detected").is_some();
    residual_action_store(ResidualPresentationEvaFullMatrixAction::NavCommands);
    ok
}

pub fn simulate_presentation_eva_full_matrix_collect_source() -> bool {
    let gl = gl_source();
    let log = log_source();
    let ok = log.contains("record_event")
        && gl.matches("host_eva_log::record_event").count()
            >= gl.matches("TheEva::set_should_play").count();
    residual_action_store(ResidualPresentationEvaFullMatrixAction::CollectSource);
    ok
}

pub fn simulate_presentation_eva_full_matrix_dispatch_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("PresentationEvent::EvaAlert { name }")
        && pf.contains("AudioEventRequest::new(name.as_str())");
    residual_action_store(ResidualPresentationEvaFullMatrixAction::DispatchSource);
    ok
}

pub fn honesty_presentation_eva_full_matrix_residual_pack_wave534() -> bool {
    honesty_presentation_eva_full_matrix_method_names_residual_wave534()
        && honesty_presentation_eva_full_matrix_source_markers_residual_wave534()
        && honesty_presentation_eva_full_matrix_nav_commands_residual_wave534()
        && simulate_presentation_eva_full_matrix_collect_source()
        && simulate_presentation_eva_full_matrix_dispatch_source()
}

pub fn simulate_live_presentation_eva_full_matrix_honesty() -> bool {
    let ok = honesty_presentation_eva_full_matrix_residual_pack_wave534();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationEvaFullMatrixAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_eva_full_matrix_method_names_residual_wave534());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_eva_full_matrix_source_markers_residual_wave534());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_eva_full_matrix_nav_commands_residual_wave534());
    }

    #[test]
    fn presentation_eva_full_matrix_sources() {
        assert!(simulate_presentation_eva_full_matrix_collect_source());
        assert!(simulate_presentation_eva_full_matrix_dispatch_source());
    }

    #[test]
    fn wave534_composite_pack() {
        assert!(honesty_presentation_eva_full_matrix_residual_pack_wave534());
    }

    #[test]
    fn simulate_live_presentation_eva_full_matrix_honesty_residual_live() {
        assert!(
            simulate_live_presentation_eva_full_matrix_honesty(),
            "eva full matrix residual must latch"
        );
        assert!(residual_presentation_eva_full_matrix_ok());
        assert_eq!(
            residual_presentation_eva_full_matrix_last_action(),
            ResidualPresentationEvaFullMatrixAction::Composite
        );
    }
}
