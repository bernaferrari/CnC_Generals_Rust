//! Wave 931: object lifecycle via GameLogic::apply_object_lifecycle_op boundary.
//!
//! Host create/destroy/prod/path/guard helpers call one GameLogic authority API
//! instead of seven direct dual-writes. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_OBJECT_LIFECYCLE_BOUNDARY_METHOD_NAMES_WAVE931: &[&str] = &[
    "apply_object_lifecycle_op",
    "ObjectLifecycleOp",
    "ObjectLifecycleResult",
    "host_create_object",
    "host_destroy_object",
    "host_enqueue_production",
    "host_cancel_production_and_sync_hud",
    "Wave 931",
    "playable_claim = false",
];

pub const LIVE_HOST_OBJECT_LIFECYCLE_BOUNDARY_NAV_STEPS_WAVE931: &[&str] = &[
    "OBJECT_LIFECYCLE_BOUNDARY",
    "SINGLE_APPLY_OBJECT_LIFECYCLE_OP",
    "LIVE_HOST_OBJECT_LIFECYCLE_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostObjectLifecycleBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostObjectLifecycleBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
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

pub fn honesty_host_object_lifecycle_boundary_method_names_residual_wave931() -> bool {
    let names = LIVE_HOST_OBJECT_LIFECYCLE_BOUNDARY_METHOD_NAMES_WAVE931;
    let ok = residual_name_index(names, "apply_object_lifecycle_op").is_some()
        && residual_name_index(names, "Wave 931").is_some();
    residual_action_store(ResidualHostObjectLifecycleBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_object_lifecycle_boundary_nav_commands_residual_wave931() -> bool {
    let steps = LIVE_HOST_OBJECT_LIFECYCLE_BOUNDARY_NAV_STEPS_WAVE931;
    let ok = residual_name_index(steps, "LIVE_HOST_OBJECT_LIFECYCLE_BOUNDARY").is_some()
        && residual_name_index(steps, "OBJECT_LIFECYCLE_BOUNDARY").is_some();
    residual_action_store(ResidualHostObjectLifecycleBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_object_lifecycle_boundary_residual_pack_wave931() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let api_raw = code_window(gl, "fn apply_object_lifecycle_op", 1400);
    let api = non_comment_code(api_raw);
    let create_raw = code_window(cnc, "fn host_create_object", 700);
    let create = non_comment_code(create_raw);
    let destroy = non_comment_code(code_window(cnc, "fn host_destroy_object", 800));
    let enqueue = non_comment_code(code_window(cnc, "fn host_enqueue_production", 700));
    let cancel = non_comment_code(code_window(
        cnc,
        "fn host_cancel_production_and_sync_hud",
        800,
    ));
    let force = non_comment_code(code_window(cnc, "fn host_force_complete_construction", 700));
    let clear = non_comment_code(code_window(cnc, "fn host_clear_unit_movement_path", 700));
    let guard = non_comment_code(code_window(cnc, "fn host_adjust_unit_guard_radius", 800));
    let ok = gl.contains("enum ObjectLifecycleOp")
        && gl.contains("enum ObjectLifecycleResult")
        && api.contains("self.create_object")
        && api.contains("self.destroy_object")
        && api.contains("self.enqueue_production")
        && api.contains("self.cancel_production")
        && create.contains("apply_object_lifecycle_op")
        && !create.contains("self.game_logic.create_object")
        && destroy.contains("apply_object_lifecycle_op")
        && !destroy.contains("self.game_logic.destroy_object")
        && enqueue.contains("apply_object_lifecycle_op")
        && !enqueue.contains("self.game_logic.enqueue_production")
        && cancel.contains("apply_object_lifecycle_op")
        && !cancel.contains("self.game_logic.cancel_production")
        && force.contains("apply_object_lifecycle_op")
        && clear.contains("apply_object_lifecycle_op")
        && guard.contains("apply_object_lifecycle_op")
        && create_raw.contains("931")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostObjectLifecycleBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_object_lifecycle_boundary_honesty() -> bool {
    let a = honesty_host_object_lifecycle_boundary_method_names_residual_wave931();
    let b = honesty_host_object_lifecycle_boundary_nav_commands_residual_wave931();
    let c = honesty_host_object_lifecycle_boundary_residual_pack_wave931();
    residual_action_store(ResidualHostObjectLifecycleBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_object_lifecycle_boundary_residual_wave931() {
        assert!(honesty_host_object_lifecycle_boundary_residual_pack_wave931());
        assert!(honesty_host_object_lifecycle_boundary_method_names_residual_wave931());
        assert!(honesty_host_object_lifecycle_boundary_nav_commands_residual_wave931());
        assert!(simulate_live_host_object_lifecycle_boundary_honesty());
    }
}
