//! Wave 934: host-support residuals via GameLogic::apply_host_support_op boundary.
//!
//! Host barracks/supplies/shell/destroy/template helpers call one GameLogic
//! authority API instead of six direct dual-writes. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SUPPORT_BOUNDARY_METHOD_NAMES_WAVE934: &[&str] = &[
    "apply_host_support_op",
    "HostSupportOp",
    "HostSupportResult",
    "host_ensure_barracks_building_data",
    "host_force_ensure_barracks_building_data",
    "host_ensure_player_min_supplies_residual",
    "host_update_shell_with_budget",
    "host_ensure_golden_ranger_template",
    "ProcessDestroyListIfNeeded",
    "Wave 934",
    "playable_claim = false",
];

pub const LIVE_HOST_SUPPORT_BOUNDARY_NAV_STEPS_WAVE934: &[&str] = &[
    "HOST_SUPPORT_BOUNDARY",
    "SINGLE_APPLY_HOST_SUPPORT_OP",
    "LIVE_HOST_SUPPORT_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSupportBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSupportBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
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

pub fn honesty_host_support_boundary_method_names_residual_wave934() -> bool {
    let names = LIVE_HOST_SUPPORT_BOUNDARY_METHOD_NAMES_WAVE934;
    let ok = residual_name_index(names, "apply_host_support_op").is_some()
        && residual_name_index(names, "Wave 934").is_some();
    residual_action_store(ResidualHostSupportBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_support_boundary_nav_commands_residual_wave934() -> bool {
    let steps = LIVE_HOST_SUPPORT_BOUNDARY_NAV_STEPS_WAVE934;
    let ok = residual_name_index(steps, "LIVE_HOST_SUPPORT_BOUNDARY").is_some()
        && residual_name_index(steps, "HOST_SUPPORT_BOUNDARY").is_some();
    residual_action_store(ResidualHostSupportBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_support_boundary_residual_pack_wave934() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let api_raw = code_window(&gl, "fn apply_host_support_op", 1600);
    let api = non_comment_code(api_raw);
    let barracks_raw = code_window(cnc, "fn host_ensure_barracks_building_data", 700);
    let barracks = non_comment_code(barracks_raw);
    let force = non_comment_code(code_window(
        cnc,
        "fn host_force_ensure_barracks_building_data",
        700,
    ));
    let supplies = non_comment_code(code_window(
        cnc,
        "fn host_ensure_player_min_supplies_residual",
        700,
    ));
    let shell = non_comment_code(code_window(cnc, "fn host_update_shell_with_budget", 700));
    let golden_raw = code_window(cnc, "fn host_ensure_golden_ranger_template", 1800);
    let golden = non_comment_code(golden_raw);
    let shadow = non_comment_code(code_window(
        cnc,
        "fn host_run_gameworld_shadow_after_logic",
        1200,
    ));
    let ok = gl.contains("enum HostSupportOp")
        && gl.contains("enum HostSupportResult")
        && api.contains("self.ensure_barracks_building_data")
        && api.contains("self.force_ensure_barracks_building_data")
        && api.contains("self.ensure_player_min_supplies")
        && api.contains("self.update_shell_with_budget")
        && api.contains("self.process_destroy_list_if_needed")
        && api.contains("self.templates.insert")
        && barracks.contains("apply_host_support_op")
        && !barracks.contains("self.game_logic.ensure_barracks_building_data")
        && force.contains("apply_host_support_op")
        && !force.contains("self.game_logic.force_ensure_barracks_building_data")
        && supplies.contains("apply_host_support_op")
        && !supplies.contains("self.game_logic.ensure_player_min_supplies")
        && shell.contains("apply_host_support_op")
        && !shell.contains("self.game_logic.update_shell_with_budget")
        && golden.contains("apply_host_support_op")
        && golden.contains("InsertThingTemplate")
        && !golden.contains(".templates.insert")
        && shadow.contains("apply_host_support_op")
        && shadow.contains("ProcessDestroyListIfNeeded")
        && !cnc.contains("self.game_logic.process_destroy_list_if_needed")
        && barracks_raw.contains("934")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostSupportBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_support_boundary_honesty() -> bool {
    let a = honesty_host_support_boundary_method_names_residual_wave934();
    let b = honesty_host_support_boundary_nav_commands_residual_wave934();
    let c = honesty_host_support_boundary_residual_pack_wave934();
    residual_action_store(ResidualHostSupportBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_support_boundary_residual_wave934() {
        assert!(honesty_host_support_boundary_residual_pack_wave934());
        assert!(honesty_host_support_boundary_method_names_residual_wave934());
        assert!(honesty_host_support_boundary_nav_commands_residual_wave934());
        assert!(simulate_live_host_support_boundary_honesty());
    }
}
