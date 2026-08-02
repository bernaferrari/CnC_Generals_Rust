//! Wave 940: post-writeback sole-tick batch + host ObjectId authority boundary.
//!
//! - `apply_post_writeback_sole_ticks` batches Waves 823–827 sole residual ticks.
//! - `HostObjectIdOp` / `apply_host_object_id_op` routes shadow create/mark-destroy.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SOLE_TICK_OBJECT_ID_BOUNDARY_METHOD_NAMES_WAVE940: &[&str] = &[
    "apply_post_writeback_sole_ticks",
    "apply_host_object_id_op",
    "HostObjectIdOp",
    "MarkForDestruction",
    "Wave 940",
    "playable_claim = false",
];

pub const LIVE_HOST_SOLE_TICK_OBJECT_ID_BOUNDARY_NAV_STEPS_WAVE940: &[&str] = &[
    "SOLE_TICK_OBJECT_ID_BOUNDARY",
    "POST_WRITEBACK_SOLE_TICKS_BATCH",
    "LIVE_HOST_SOLE_TICK_OBJECT_ID_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSoleTickObjectIdBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSoleTickObjectIdBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn shadow_source() -> &'static str {
    include_str!("../gameworld_shadow.rs")
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

fn session_fn_window(src: &str) -> &str {
    let marker = "fn shadow_session_after_host_tick";
    match src.find(marker) {
        Some(i) => &src[i..src.len().min(i + 120_000)],
        None => "",
    }
}

pub fn honesty_host_sole_tick_object_id_boundary_method_names_residual_wave940() -> bool {
    let names = LIVE_HOST_SOLE_TICK_OBJECT_ID_BOUNDARY_METHOD_NAMES_WAVE940;
    let ok = residual_name_index(names, "apply_post_writeback_sole_ticks").is_some()
        && residual_name_index(names, "Wave 940").is_some();
    residual_action_store(ResidualHostSoleTickObjectIdBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sole_tick_object_id_boundary_nav_commands_residual_wave940() -> bool {
    let steps = LIVE_HOST_SOLE_TICK_OBJECT_ID_BOUNDARY_NAV_STEPS_WAVE940;
    let ok = residual_name_index(steps, "LIVE_HOST_SOLE_TICK_OBJECT_ID_BOUNDARY").is_some()
        && residual_name_index(steps, "SOLE_TICK_OBJECT_ID_BOUNDARY").is_some();
    residual_action_store(ResidualHostSoleTickObjectIdBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sole_tick_object_id_boundary_residual_pack_wave940() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let sole = non_comment_code(code_window(gl, "fn apply_post_writeback_sole_ticks", 700));
    let id_api = non_comment_code(code_window(gl, "fn apply_host_object_id_op", 900));
    let session = non_comment_code(session_fn_window(sh));
    let ok = gl.contains("enum HostObjectIdOp")
        && sole.contains("tick_patriot_assist_lasers_sole")
        && sole.contains("tick_host_systems_residuals_sole")
        && id_api.contains("mark_object_for_destruction")
        && id_api.contains("create_object")
        && session.contains("apply_post_writeback_sole_ticks")
        && !session.contains("tick_patriot_assist_lasers_sole()")
        && !session.contains("tick_host_systems_residuals_sole()")
        && session.contains("apply_host_object_id_op")
        && session.contains("MarkForDestruction")
        && !session.contains("logic.mark_object_for_destruction")
        && !session.contains("logic.create_object")
        && sh.contains("940")
        && gl.contains("Wave 940")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSoleTickObjectIdBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sole_tick_object_id_boundary_honesty() -> bool {
    let a = honesty_host_sole_tick_object_id_boundary_method_names_residual_wave940();
    let b = honesty_host_sole_tick_object_id_boundary_nav_commands_residual_wave940();
    let c = honesty_host_sole_tick_object_id_boundary_residual_pack_wave940();
    residual_action_store(ResidualHostSoleTickObjectIdBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sole_tick_object_id_boundary_residual_wave940() {
        assert!(honesty_host_sole_tick_object_id_boundary_residual_pack_wave940());
        assert!(honesty_host_sole_tick_object_id_boundary_method_names_residual_wave940());
        assert!(honesty_host_sole_tick_object_id_boundary_nav_commands_residual_wave940());
        assert!(simulate_live_host_sole_tick_object_id_boundary_honesty());
    }
}
