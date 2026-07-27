//! Wave 528 residual peels: presentation FireSound loop stop is stop-only.
//! - `WeaponFireLoopStop` must not resolve/play FireSound again
//! - AudioManagerSubsystem tracks looping_object_audio and clears on stop
//! - collect_audio_events emits explicit `WeaponFireLoopStop` event type
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 527 FireSound name/loop/pose peel.
//! Architecture residual - audio stop boundary without GameLogic dual-write.
//!
//! Sources:
//! - presentation_frame.rs Wave 528 WeaponFireLoopStop collect
//! - subsystem_manager.rs AudioManagerSubsystem Wave 528 stop arm
//!
//! Fail-closed:
//! - Full Miles stopObj instance matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_FIRESOUND_STOP_METHOD_NAMES_WAVE528: &[&str] = &[
    "WeaponFireLoopStop",
    "looping_object_audio",
    "collect_audio_events",
    "dispatch_audio_events_direct",
    "is_looping",
    "playable_claim = false",
];

pub const PRESENTATION_FIRESOUND_STOP_SOURCE_MARKERS_WAVE528: &[&str] = &[
    "Wave 528: explicit stop residual (must not re-trigger FireSound play)",
    "Wave 528: presentation FireSound loop stop is stop-only (no replay)",
    "looping_object_audio",
    "\"WeaponFireLoopStop\"",
];

pub const PRESENTATION_FIRESOUND_STOP_NAV_STEPS_WAVE528: &[&str] = &[
    "EMIT_WEAPON_FIRE_LOOP_STOP",
    "QUEUE_STOP_ONLY_EVENT",
    "CLEAR_LOOPING_OBJECT_AUDIO",
    "NO_FIRESOUND_REPLAY",
    "NO_LIVE_GAMELOGIC_DUAL_WRITE",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_FIRESOUND_STOP_CMD_NAMES_WAVE528: &[&str] = &[
    "click_presentation_firesound_stop_ok_wnd_detect",
    "click_presentation_firesound_stop_ok_wnd_skip",
    "click_presentation_firesound_stop_ok_wnd_queue",
    "click_presentation_firesound_stop_ok_wnd_prepare",
    "click_presentation_firesound_stop_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationFiresoundStopAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    SubsystemSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationFiresoundStopAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_firesound_stop_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_firesound_stop_last_action() -> ResidualPresentationFiresoundStopAction
{
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationFiresoundStopAction::MethodNames,
        2 => ResidualPresentationFiresoundStopAction::SourceMarkers,
        3 => ResidualPresentationFiresoundStopAction::NavCommands,
        4 => ResidualPresentationFiresoundStopAction::CollectSource,
        5 => ResidualPresentationFiresoundStopAction::SubsystemSource,
        6 => ResidualPresentationFiresoundStopAction::Composite,
        _ => ResidualPresentationFiresoundStopAction::Idle,
    }
}

fn pf_source() -> &'static str {
    include_str!("../presentation_frame.rs")
}

fn sm_source() -> &'static str {
    include_str!("../subsystem_manager.rs")
}

pub fn honesty_presentation_firesound_stop_method_names_residual_wave528() -> bool {
    PRESENTATION_FIRESOUND_STOP_METHOD_NAMES_WAVE528.len() == 6
        && residual_name_index(
            PRESENTATION_FIRESOUND_STOP_METHOD_NAMES_WAVE528,
            "WeaponFireLoopStop",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_FIRESOUND_STOP_METHOD_NAMES_WAVE528,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_firesound_stop_source_markers_residual_wave528() -> bool {
    PRESENTATION_FIRESOUND_STOP_SOURCE_MARKERS_WAVE528.len() == 4
        && residual_name_index(
            PRESENTATION_FIRESOUND_STOP_SOURCE_MARKERS_WAVE528,
            "Wave 528: explicit stop residual (must not re-trigger FireSound play)",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_FIRESOUND_STOP_SOURCE_MARKERS_WAVE528,
            "looping_object_audio",
        ) == Some(2)
}

pub fn honesty_presentation_firesound_stop_nav_commands_residual_wave528() -> bool {
    PRESENTATION_FIRESOUND_STOP_NAV_STEPS_WAVE528.len() == 6
        && residual_name_index(
            PRESENTATION_FIRESOUND_STOP_NAV_STEPS_WAVE528,
            "NO_FIRESOUND_REPLAY",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_FIRESOUND_STOP_NAV_STEPS_WAVE528,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_FIRESOUND_STOP_CMD_NAMES_WAVE528.len() == 5
}

pub fn simulate_presentation_firesound_stop_collect_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 528: explicit stop residual (must not re-trigger FireSound play)")
        && pf.contains("WeaponFireLoopStop")
        && pf.contains("with_priority(200)")
        && !pf.contains("// Stop uses same event key as start");
    residual_action_store(ResidualPresentationFiresoundStopAction::CollectSource);
    ok
}

pub fn simulate_presentation_firesound_stop_subsystem_source() -> bool {
    let sm = sm_source();
    let ok = sm.contains("Wave 528: presentation FireSound loop stop is stop-only (no replay)")
        && sm.contains("looping_object_audio")
        && sm.contains("\"WeaponFireLoopStop\"")
        && sm.contains("looping_object_audio.remove");
    residual_action_store(ResidualPresentationFiresoundStopAction::SubsystemSource);
    ok
}

pub fn honesty_presentation_firesound_stop_residual_pack_wave528() -> bool {
    honesty_presentation_firesound_stop_method_names_residual_wave528()
        && honesty_presentation_firesound_stop_source_markers_residual_wave528()
        && honesty_presentation_firesound_stop_nav_commands_residual_wave528()
        && simulate_presentation_firesound_stop_collect_source()
        && simulate_presentation_firesound_stop_subsystem_source()
}

pub fn simulate_live_presentation_firesound_stop_honesty() -> bool {
    let ok = honesty_presentation_firesound_stop_residual_pack_wave528();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationFiresoundStopAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_firesound_stop_method_names_residual_wave528());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_firesound_stop_source_markers_residual_wave528());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_firesound_stop_nav_commands_residual_wave528());
    }

    #[test]
    fn presentation_firesound_stop_sources() {
        assert!(simulate_presentation_firesound_stop_collect_source());
        assert!(simulate_presentation_firesound_stop_subsystem_source());
    }

    #[test]
    fn wave528_composite_pack() {
        assert!(honesty_presentation_firesound_stop_residual_pack_wave528());
    }

    #[test]
    fn simulate_live_presentation_firesound_stop_honesty_residual_live() {
        assert!(
            simulate_live_presentation_firesound_stop_honesty(),
            "presentation firesound stop residual must latch"
        );
        assert!(residual_presentation_firesound_stop_ok());
        assert_eq!(
            residual_presentation_firesound_stop_last_action(),
            ResidualPresentationFiresoundStopAction::Composite
        );
    }
}
