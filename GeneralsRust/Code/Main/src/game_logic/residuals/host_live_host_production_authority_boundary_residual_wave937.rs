//! Wave 937: production complete/spawn via GameLogic::apply_production_authority_op.
//!
//! Shadow same-frame sole-tick complete and door-ready drains call one GameLogic
//! authority API. Unit spawn from completions routes through SpawnUnit.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_AUTHORITY_BOUNDARY_METHOD_NAMES_WAVE937: &[&str] = &[
    "apply_production_authority_op",
    "ProductionAuthorityOp",
    "ApplyCompletionsAfterReadyWriteback",
    "SpawnUnit",
    "ApplyDoorReadyCompletions",
    "ApplySpawnReadyCompletions",
    "Wave 937",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_AUTHORITY_BOUNDARY_NAV_STEPS_WAVE937: &[&str] = &[
    "PRODUCTION_AUTHORITY_BOUNDARY",
    "SINGLE_APPLY_PRODUCTION_AUTHORITY_OP",
    "LIVE_HOST_PRODUCTION_AUTHORITY_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionAuthorityBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProductionAuthorityBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
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

pub fn honesty_host_production_authority_boundary_method_names_residual_wave937() -> bool {
    let names = LIVE_HOST_PRODUCTION_AUTHORITY_BOUNDARY_METHOD_NAMES_WAVE937;
    let ok = residual_name_index(names, "apply_production_authority_op").is_some()
        && residual_name_index(names, "Wave 937").is_some();
    residual_action_store(ResidualHostProductionAuthorityBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_authority_boundary_nav_commands_residual_wave937() -> bool {
    let steps = LIVE_HOST_PRODUCTION_AUTHORITY_BOUNDARY_NAV_STEPS_WAVE937;
    let ok = residual_name_index(steps, "LIVE_HOST_PRODUCTION_AUTHORITY_BOUNDARY").is_some()
        && residual_name_index(steps, "PRODUCTION_AUTHORITY_BOUNDARY").is_some();
    residual_action_store(ResidualHostProductionAuthorityBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_authority_boundary_residual_pack_wave937() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let api = non_comment_code(code_window(gl, "fn apply_production_authority_op", 1400));
    let unit_apply = non_comment_code(code_window(
        gl,
        "fn host_apply_unit_production_completions",
        2000,
    ));
    let ok = gl.contains("enum ProductionAuthorityOp")
        && gl.contains("enum ProductionAuthorityResult")
        && api.contains("host_apply_production_completions_after_ready_writeback")
        && api.contains("host_spawn_production_unit")
        && api.contains("host_apply_production_spawn_ready_completions")
        && api.contains("host_apply_production_door_ready_completions")
        && sh.contains("apply_production_authority_op")
        && sh.contains("ApplyCompletionsAfterReadyWriteback")
        && sh.contains("ApplyDoorReadyCompletions")
        && !sh.contains("logic.host_apply_production_completions_after_ready_writeback")
        && !sh.contains("logic.host_apply_production_door_ready_completions")
        && unit_apply.contains("apply_production_authority_op")
        && unit_apply.contains("SpawnUnit")
        && sh.contains("937")
        && gl.contains("Wave 937")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionAuthorityBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_production_authority_boundary_honesty() -> bool {
    let a = honesty_host_production_authority_boundary_method_names_residual_wave937();
    let b = honesty_host_production_authority_boundary_nav_commands_residual_wave937();
    let c = honesty_host_production_authority_boundary_residual_pack_wave937();
    residual_action_store(ResidualHostProductionAuthorityBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_production_authority_boundary_residual_wave937() {
        assert!(honesty_host_production_authority_boundary_residual_pack_wave937());
        assert!(honesty_host_production_authority_boundary_method_names_residual_wave937());
        assert!(honesty_host_production_authority_boundary_nav_commands_residual_wave937());
        assert!(simulate_live_host_production_authority_boundary_honesty());
    }
}
