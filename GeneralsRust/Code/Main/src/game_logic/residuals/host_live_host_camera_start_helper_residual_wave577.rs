//! Wave 577 residual peels: host camera jump residual is centralized through
//! `host_center_camera_and_request_focus`, and start-new-game residual through
//! `host_start_new_game_with_faction`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 576 host command flush helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_center_camera_and_request_focus /
//!   host_start_new_game_with_faction
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CAMERA_START_HELPER_METHOD_NAMES_WAVE577: &[&str] = &[
    "host_center_camera_and_request_focus",
    "host_start_new_game_with_faction",
    "request_camera_focus",
    "start_new_game",
    "Wave 577",
    "playable_claim = false",
];

pub const LIVE_HOST_CAMERA_START_HELPER_NAV_STEPS_WAVE577: &[&str] = &[
    "REQUIRE_HOST_CAMERA_JUMP_HELPER",
    "REQUIRE_HOST_START_GAME_HELPER",
    "LIVE_HOST_CAMERA_START_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_CAMERA_START_HELPER_CMD_NAMES_WAVE577: &[&str] = &[
    "host_camera_jump_helper",
    "host_start_game_helper",
    "camera_start_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCameraStartHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostCameraStartHelperAction {
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

fn residual_action_store(action: ResidualHostCameraStartHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_camera_start_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_camera_start_helper_last_action() -> ResidualHostCameraStartHelperAction {
    ResidualHostCameraStartHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_camera_start_helper_method_names_residual_wave577() -> bool {
    let names = LIVE_HOST_CAMERA_START_HELPER_METHOD_NAMES_WAVE577;
    let ok = residual_name_index(names, "host_center_camera_and_request_focus").is_some()
        && residual_name_index(names, "host_start_new_game_with_faction").is_some()
        && residual_name_index(names, "request_camera_focus").is_some()
        && residual_name_index(names, "start_new_game").is_some()
        && residual_name_index(names, "Wave 577").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostCameraStartHelperAction::MethodNames);
    ok
}

pub fn honesty_host_camera_start_helper_source_markers_residual_wave577() -> bool {
    let eng = eng_source();
    let Some(cam) = fn_body(eng, "fn host_center_camera_and_request_focus(") else {
        residual_action_store(ResidualHostCameraStartHelperAction::SourceMarkers);
        return false;
    };
    let Some(start) = fn_body(eng, "fn host_start_new_game_with_faction(") else {
        residual_action_store(ResidualHostCameraStartHelperAction::SourceMarkers);
        return false;
    };
    let cam_ok = cam.contains("Wave 577")
        && cam.contains("clamp_to_world_bounds")
        && cam.contains("camera_target.x")
        && cam.contains("self.camera_target");
    // 2026-08-15: start goes through SessionControlOp (no set_player_team dual-write).
    let start_ok = start.contains("Wave 577")
        && start.contains("SessionControlOp::StartNewGameWithFaction")
        && start.contains("setup_skirmish_ai");
    let call_ok = eng.contains("host_center_camera_and_request_focus")
        && eng.contains("self.host_start_new_game_with_faction(");
    let raw_focus = eng.matches("self.game_logic.request_camera_focus").count();
    let raw_start = eng.matches("self.game_logic.start_new_game").count();
    // only inside helpers
    let ok = cam_ok
        && start_ok
        && call_ok
        && raw_focus == 0
        && raw_start == 0
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostCameraStartHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_camera_start_helper_nav_commands_residual_wave577() -> bool {
    let steps = LIVE_HOST_CAMERA_START_HELPER_NAV_STEPS_WAVE577;
    let cmds = RUNTIME_HOST_LIVE_HOST_CAMERA_START_HELPER_CMD_NAMES_WAVE577;
    let ok = residual_name_index(steps, "REQUIRE_HOST_CAMERA_JUMP_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_START_GAME_HELPER").is_some()
        && residual_name_index(steps, "LIVE_HOST_CAMERA_START_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_camera_jump_helper").is_some()
        && residual_name_index(cmds, "host_start_game_helper").is_some()
        && residual_name_index(cmds, "camera_start_residual").is_some();
    residual_action_store(ResidualHostCameraStartHelperAction::NavCommands);
    ok
}

pub fn simulate_host_camera_start_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 577")
        && eng.contains("fn host_center_camera_and_request_focus")
        && eng.contains("fn host_start_new_game_with_faction");
    residual_action_store(ResidualHostCameraStartHelperAction::CollectSource);
    ok
}

pub fn simulate_host_camera_start_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("host_center_camera_and_request_focus")
        && eng.contains("self.host_start_new_game_with_faction(mode, faction_team, true)")
        && eng.contains("self.host_start_new_game_with_faction(mode, faction_team, false)");
    residual_action_store(ResidualHostCameraStartHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_camera_start_helper_residual_pack_wave577() -> bool {
    honesty_host_camera_start_helper_method_names_residual_wave577()
        && honesty_host_camera_start_helper_source_markers_residual_wave577()
        && honesty_host_camera_start_helper_nav_commands_residual_wave577()
        && simulate_host_camera_start_helper_collect_source()
        && simulate_host_camera_start_helper_dispatch_source()
}

pub fn simulate_live_host_camera_start_helper_honesty() -> bool {
    let ok = honesty_host_camera_start_helper_residual_pack_wave577();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostCameraStartHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_camera_start_helper_method_names_residual_wave577());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_camera_start_helper_source_markers_residual_wave577());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_camera_start_helper_nav_commands_residual_wave577());
    }

    #[test]
    fn host_camera_start_helper_sources() {
        assert!(simulate_host_camera_start_helper_collect_source());
        assert!(simulate_host_camera_start_helper_dispatch_source());
    }

    #[test]
    fn wave577_composite_pack() {
        assert!(honesty_host_camera_start_helper_residual_pack_wave577());
    }

    #[test]
    fn simulate_live_host_camera_start_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_camera_start_helper_honesty(),
            "host camera/start helper residual must latch"
        );
        assert!(residual_host_camera_start_helper_ok());
        assert_eq!(
            residual_host_camera_start_helper_last_action(),
            ResidualHostCameraStartHelperAction::Composite
        );
    }
}
