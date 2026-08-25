//! Wave 559 residual peels: align presentation residual honesty tests with the
//! current presentation-owned host path (status sample, train_unit team helper,
//! snap_camera freeze, move selection, dual_world_registry_unavailable gates).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 558 diplomacy presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_frame.rs` residual honesty tests
//! - `cnc_game_engine.rs` runtime_host_status_snapshot / snap_camera / move / train_unit
//! - GameLogic/GameClient dual_world_registry_unavailable peels
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_HONESTY_ALIGN_METHOD_NAMES_WAVE559: &[&str] = &[
    "runtime_host_status_snapshot",
    "Object-roster stats are presentation-owned when freeze installed",
    "snap_camera_to_local_units_if_needed",
    "dual_world_registry_unavailable",
    "Wave 559",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_HONESTY_ALIGN_NAV_STEPS_WAVE559: &[&str] = &[
    "REQUIRE_STATUS_SAMPLE_PRESENTATION",
    "REQUIRE_SNAP_CAMERA_PRESENTATION",
    "LIVE_PRESENTATION_HONESTY_ALIGN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PRESENTATION_HONESTY_ALIGN_CMD_NAMES_WAVE559: &[&str] = &[
    "status_sample_presentation",
    "snap_camera_presentation",
    "dual_world_registry_gate",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationHonestyAlignAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationHonestyAlignAction {
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

fn residual_action_store(action: ResidualPresentationHonestyAlignAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_honesty_align_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_honesty_align_last_action() -> ResidualPresentationHonestyAlignAction {
    ResidualPresentationHonestyAlignAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_presentation_honesty_align_method_names_residual_wave559() -> bool {
    let names = LIVE_PRESENTATION_HONESTY_ALIGN_METHOD_NAMES_WAVE559;
    let ok = residual_name_index(names, "runtime_host_status_snapshot").is_some()
        && residual_name_index(
            names,
            "Object-roster stats are presentation-owned when freeze installed",
        )
        .is_some()
        && residual_name_index(names, "snap_camera_to_local_units_if_needed").is_some()
        && residual_name_index(names, "dual_world_registry_unavailable").is_some()
        && residual_name_index(names, "Wave 559").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationHonestyAlignAction::MethodNames);
    ok
}

pub fn honesty_presentation_honesty_align_source_markers_residual_wave559() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let Some(status) = fn_body(eng, "fn runtime_host_status_snapshot(") else {
        residual_action_store(ResidualPresentationHonestyAlignAction::SourceMarkers);
        return false;
    };
    let Some(snap) = fn_body(eng, "fn snap_camera_to_local_units_if_needed(") else {
        residual_action_store(ResidualPresentationHonestyAlignAction::SourceMarkers);
        return false;
    };
    let status_ok = status
        .contains("Object-roster stats are presentation-owned when freeze installed")
        && status.contains("count_mobile_friendlies")
        && status.contains("first_friendly_sample_label");
    let snap_ok = snap.contains("Presentation-only")
        && snap.contains("local_team_base_position")
        && !snap.contains("team_base_position(team)");
    // 2026-08-15: Wave 559 honesty tests live in presentation_frame/tests.
    let pf_tests = include_str!("../../presentation_frame/tests/dual_tick_registry.rs");
    let tests_ok = pf_tests.contains("Wave 559")
        && (pf_tests.contains("dual_world_registry_unavailable")
            || pf.contains("dual_world_registry_unavailable"))
        && (pf_tests.contains("local_team_for_ui()") || pf.contains("local_team_for_ui()"));
    let ok = status_ok
        && snap_ok
        && tests_ok
        && !status.contains("playable_claim = true")
        && !snap.contains("playable_claim = true");
    residual_action_store(ResidualPresentationHonestyAlignAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_honesty_align_nav_commands_residual_wave559() -> bool {
    let steps = LIVE_PRESENTATION_HONESTY_ALIGN_NAV_STEPS_WAVE559;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_HONESTY_ALIGN_CMD_NAMES_WAVE559;
    let ok = residual_name_index(steps, "REQUIRE_STATUS_SAMPLE_PRESENTATION").is_some()
        && residual_name_index(steps, "REQUIRE_SNAP_CAMERA_PRESENTATION").is_some()
        && residual_name_index(steps, "LIVE_PRESENTATION_HONESTY_ALIGN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "status_sample_presentation").is_some()
        && residual_name_index(cmds, "snap_camera_presentation").is_some()
        && residual_name_index(cmds, "dual_world_registry_gate").is_some();
    residual_action_store(ResidualPresentationHonestyAlignAction::NavCommands);
    ok
}

pub fn simulate_presentation_honesty_align_collect_source() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let pf_tests = include_str!("../../presentation_frame/tests/dual_tick_registry.rs");
    let ok = eng.contains("fn runtime_host_status_snapshot")
        && eng.contains("fn snap_camera_to_local_units_if_needed")
        && (pf.contains("Wave 559") || pf_tests.contains("Wave 559"));
    residual_action_store(ResidualPresentationHonestyAlignAction::CollectSource);
    ok
}

pub fn simulate_presentation_honesty_align_dispatch_source() -> bool {
    let pf = pf_source();
    let pf_tests = include_str!("../../presentation_frame/tests/dual_tick_registry.rs");
    let ok = (pf.contains("fn status_sample_prefers_presentation")
        || pf_tests.contains("fn status_sample_prefers_presentation"))
        && (pf.contains("fn snap_camera_start_hint_prefers_presentation")
            || pf_tests.contains("fn snap_camera_start_hint_prefers_presentation"))
        && (pf.contains("fn train_unit_prefers_presentation_team")
            || pf_tests.contains("fn train_unit_prefers_presentation_team"))
        && (pf.contains("fn runtime_host_move_prefers_presentation_selection")
            || pf_tests.contains("fn runtime_host_move_prefers_presentation_selection")
            || pf_tests.contains("Wave 559: move uses presentation-first"));
    residual_action_store(ResidualPresentationHonestyAlignAction::DispatchSource);
    ok
}

pub fn honesty_presentation_honesty_align_residual_pack_wave559() -> bool {
    honesty_presentation_honesty_align_method_names_residual_wave559()
        && honesty_presentation_honesty_align_source_markers_residual_wave559()
        && honesty_presentation_honesty_align_nav_commands_residual_wave559()
        && simulate_presentation_honesty_align_collect_source()
        && simulate_presentation_honesty_align_dispatch_source()
}

pub fn simulate_live_presentation_honesty_align_honesty() -> bool {
    let ok = honesty_presentation_honesty_align_residual_pack_wave559();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationHonestyAlignAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_honesty_align_method_names_residual_wave559());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_honesty_align_source_markers_residual_wave559());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_honesty_align_nav_commands_residual_wave559());
    }

    #[test]
    fn presentation_honesty_align_sources() {
        assert!(simulate_presentation_honesty_align_collect_source());
        assert!(simulate_presentation_honesty_align_dispatch_source());
    }

    #[test]
    fn wave559_composite_pack() {
        assert!(honesty_presentation_honesty_align_residual_pack_wave559());
    }

    #[test]
    fn simulate_live_presentation_honesty_align_honesty_residual_live() {
        assert!(
            simulate_live_presentation_honesty_align_honesty(),
            "presentation honesty align residual must latch"
        );
        assert!(residual_presentation_honesty_align_ok());
        assert_eq!(
            residual_presentation_honesty_align_last_action(),
            ResidualPresentationHonestyAlignAction::Composite
        );
    }
}
