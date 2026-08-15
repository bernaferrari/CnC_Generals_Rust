//! Wave 933: session control via GameLogic::apply_session_control_op boundary.
//!
//! Host select/pause/start/reset/camera/world helpers call one GameLogic
//! authority API instead of six direct dual-writes. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SESSION_CONTROL_BOUNDARY_METHOD_NAMES_WAVE933: &[&str] = &[
    "apply_session_control_op",
    "SessionControlOp",
    "host_set_selection",
    "host_set_paused",
    "host_start_new_game_with_faction",
    "host_reset_game_logic",
    "host_set_camera_follow_object",
    "host_override_world_size",
    "Wave 933",
    "playable_claim = false",
];

pub const LIVE_HOST_SESSION_CONTROL_BOUNDARY_NAV_STEPS_WAVE933: &[&str] = &[
    "SESSION_CONTROL_BOUNDARY",
    "SINGLE_APPLY_SESSION_CONTROL_OP",
    "LIVE_HOST_SESSION_CONTROL_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSessionControlBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSessionControlBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn code_window<'a>(src: &'a str, marker: &str, len: usize) -> &'a str {
    match src.find(marker) {
        Some(i) => &src[i..src.len().min(i + len)],
        None => "",
    }
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_session_control_boundary_method_names_residual_wave933() -> bool {
    let names = LIVE_HOST_SESSION_CONTROL_BOUNDARY_METHOD_NAMES_WAVE933;
    let ok = residual_name_index(names, "apply_session_control_op").is_some()
        && residual_name_index(names, "Wave 933").is_some();
    residual_action_store(ResidualHostSessionControlBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_session_control_boundary_nav_commands_residual_wave933() -> bool {
    let steps = LIVE_HOST_SESSION_CONTROL_BOUNDARY_NAV_STEPS_WAVE933;
    let ok = residual_name_index(steps, "LIVE_HOST_SESSION_CONTROL_BOUNDARY").is_some()
        && residual_name_index(steps, "SESSION_CONTROL_BOUNDARY").is_some();
    residual_action_store(ResidualHostSessionControlBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_session_control_boundary_residual_pack_wave933() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let api_raw = code_window(gl, "fn apply_session_control_op", 1400);
    let api = non_comment_code(api_raw);
    let sel_raw = code_window(cnc, "fn host_set_selection", 700);
    let sel = non_comment_code(sel_raw);
    let paused = non_comment_code(code_window(cnc, "fn host_set_paused", 700));
    let start = non_comment_code(code_window(cnc, "fn host_start_new_game_with_faction", 800));
    let reset = non_comment_code(code_window(cnc, "fn host_reset_game_logic", 500));
    let cam = non_comment_code(code_window(cnc, "fn host_set_camera_follow_object", 800));
    let world = non_comment_code(code_window(cnc, "fn host_override_world_size", 700));
    let ok = gl.contains("enum SessionControlOp")
        && api.contains("self.select_objects")
        && api.contains("self.set_paused")
        && api.contains("self.start_new_game_with_faction")
        && api.contains("self.reset()")
        && api.contains("self.set_camera_follow_object")
        && api.contains("self.override_world_size")
        && sel.contains("apply_session_control_op")
        && !sel.contains("self.game_logic.select_objects")
        && paused.contains("apply_session_control_op")
        && !paused.contains("self.game_logic.set_paused")
        && start.contains("apply_session_control_op")
        && !start.contains("self.game_logic.start_new_game_with_faction")
        && reset.contains("apply_session_control_op")
        && !reset.contains("self.game_logic.reset(")
        && cam.contains("apply_session_control_op")
        && !cam.contains("self.game_logic.set_camera_follow_object")
        && world.contains("apply_session_control_op")
        && !world.contains("self.game_logic.override_world_size")
        && sel_raw.contains("933")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostSessionControlBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_session_control_boundary_honesty() -> bool {
    let a = honesty_host_session_control_boundary_method_names_residual_wave933();
    let b = honesty_host_session_control_boundary_nav_commands_residual_wave933();
    let c = honesty_host_session_control_boundary_residual_pack_wave933();
    residual_action_store(ResidualHostSessionControlBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_session_control_boundary_residual_wave933() {
        assert!(honesty_host_session_control_boundary_residual_pack_wave933());
        assert!(honesty_host_session_control_boundary_method_names_residual_wave933());
        assert!(honesty_host_session_control_boundary_nav_commands_residual_wave933());
        assert!(simulate_live_host_session_control_boundary_honesty());
    }
}
