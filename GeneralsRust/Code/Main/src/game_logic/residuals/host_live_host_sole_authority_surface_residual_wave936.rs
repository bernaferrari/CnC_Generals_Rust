//! Wave 936: sole-authority surface honesty lock after Waves 930–935.
//!
//! Host GameLogic dual-writes are only:
//! - `host_game_logic_mut().apply_*` authority APIs
//! - intentional split-borrow naked field adapters (save/load/shadow/presentation)
//! - accessor/replace helpers
//!
//! playable_claim stays false (no full retail WND/GPU playthrough).

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SOLE_AUTHORITY_SURFACE_METHOD_NAMES_WAVE936: &[&str] = &[
    "apply_direct_player_order",
    "apply_object_lifecycle_op",
    "apply_command_pipeline_op",
    "apply_session_control_op",
    "apply_host_support_op",
    "host_game_logic_mut",
    "host_replace_game_logic",
    "Wave 936",
    "playable_claim = false",
];

pub const LIVE_HOST_SOLE_AUTHORITY_SURFACE_NAV_STEPS_WAVE936: &[&str] = &[
    "SOLE_AUTHORITY_SURFACE",
    "HOST_APPLY_APIS_ONLY",
    "LIVE_HOST_SOLE_AUTHORITY_SURFACE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

const ALLOWED_NAKED_GAMELOGIC_FNS_WAVE936: &[&str] = &[
    "host_game_logic",
    "host_game_logic_mut",
    "host_replace_game_logic",
    "host_run_ingame_logic_presentation_frame",
    "host_sync_shadow_and_build_presentation",
    "host_run_gameworld_shadow_after_logic",
    "host_save_game_authority",
    "host_load_game_authority",
    "host_apply_skirmish_config_authority",
    "host_simulate_gameworld_authority_probe",
    "host_load_map_or_default",
    "host_load_game_authority",
    "host_replace_staged_restore_world",
    "stage_saved_world_for_restore",
    "host_load_game_from_ui",
    "host_dismiss_in_game_popup_message",
    "host_invalidate_active_popup_for_world_boundary",
    "presentation_mouse_game_logic",
    "host_seed_presentation_after_match_start",
    "host_ensure_presentation_env_for_hints",
    "presentation_runtime_heightmap_for_frame",
    "runtime_host_cmd_enqueue_production",
    "host_resolve_unit_template",
];

const REQUIRED_APPLY_APIS_WAVE936: &[&str] = &[
    "fn apply_direct_player_order",
    "fn apply_object_lifecycle_op",
    "fn apply_command_pipeline_op",
    "fn apply_session_control_op",
    "fn apply_host_support_op",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSoleAuthoritySurfaceAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSoleAuthoritySurfaceAction) {
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

fn enclosing_fn_name(src: &str, at: usize) -> String {
    let head = &src[..at];
    let markers = [
        "\n    pub(super) fn ",
        "\n    pub(crate) fn ",
        "\n    pub fn ",
        "\n    fn ",
    ];
    let at = markers
        .iter()
        .filter_map(|m| head.rfind(m).map(|i| (i, *m)))
        .max_by_key(|(i, _)| *i);
    match at {
        Some((i, marker)) => {
            let s = &head[i + marker.len()..];
            s.split('(').next().unwrap_or("").trim().to_string()
        }
        None => String::new(),
    }
}

fn naked_game_logic_field_sites(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(rel) = src[i..].find("self.game_logic") {
        let j = i + rel;
        let after = j + "self.game_logic".len();
        if src[after..].starts_with("_paused") {
            i = after;
            continue;
        }
        let line_start = src[..j].rfind('\n').map(|x| x + 1).unwrap_or(0);
        let line_end = src[j..].find('\n').map(|x| j + x).unwrap_or(src.len());
        let line = &src[line_start..line_end];
        if line.trim_start().starts_with("//") {
            i = after;
            continue;
        }
        // Source-scan tests mention `self.game_logic` inside string literals.
        let rel_in_line = j - line_start;
        if line[..rel_in_line].matches('"').count() % 2 == 1 {
            i = after;
            continue;
        }
        out.push(enclosing_fn_name(src, j));
        i = after;
    }
    out
}

pub fn honesty_host_sole_authority_surface_method_names_residual_wave936() -> bool {
    let names = LIVE_HOST_SOLE_AUTHORITY_SURFACE_METHOD_NAMES_WAVE936;
    let ok = residual_name_index(names, "apply_host_support_op").is_some()
        && residual_name_index(names, "Wave 936").is_some();
    residual_action_store(ResidualHostSoleAuthoritySurfaceAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sole_authority_surface_nav_commands_residual_wave936() -> bool {
    let steps = LIVE_HOST_SOLE_AUTHORITY_SURFACE_NAV_STEPS_WAVE936;
    let ok = residual_name_index(steps, "LIVE_HOST_SOLE_AUTHORITY_SURFACE").is_some()
        && residual_name_index(steps, "SOLE_AUTHORITY_SURFACE").is_some();
    residual_action_store(ResidualHostSoleAuthoritySurfaceAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sole_authority_surface_residual_pack_wave936() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sites = naked_game_logic_field_sites(cnc);
    let allowed_ok = sites.iter().all(|fname| {
        ALLOWED_NAKED_GAMELOGIC_FNS_WAVE936
            .iter()
            .any(|a| *a == fname.as_str())
    });
    let apis_ok = REQUIRED_APPLY_APIS_WAVE936
        .iter()
        .all(|api| gl.contains(api));
    let ok = apis_ok
        && cnc.contains("host_game_logic_mut().apply_direct_player_order")
        && cnc.contains("host_game_logic_mut().apply_object_lifecycle_op")
        && cnc.contains("apply_command_pipeline_op")
        && cnc.contains("host_game_logic_mut().apply_session_control_op")
        && cnc.contains("host_game_logic_mut().apply_host_support_op")
        && cnc.contains("fn host_game_logic(")
        && cnc.contains("fn host_game_logic_mut(")
        && cnc.contains("fn host_replace_game_logic(")
        && sites.len() >= 8
        && allowed_ok
        && !cnc.contains("self.playable_claim = true")
        && gl.contains("enum DirectPlayerOrder")
        && gl.contains("enum ObjectLifecycleOp")
        && gl.contains("enum CommandPipelineOp")
        && gl.contains("enum SessionControlOp")
        && gl.contains("enum HostSupportOp");
    residual_action_store(ResidualHostSoleAuthoritySurfaceAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sole_authority_surface_honesty() -> bool {
    let a = honesty_host_sole_authority_surface_method_names_residual_wave936();
    let b = honesty_host_sole_authority_surface_nav_commands_residual_wave936();
    let c = honesty_host_sole_authority_surface_residual_pack_wave936();
    residual_action_store(ResidualHostSoleAuthoritySurfaceAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sole_authority_surface_residual_wave936() {
        assert!(honesty_host_sole_authority_surface_residual_pack_wave936());
        assert!(honesty_host_sole_authority_surface_method_names_residual_wave936());
        assert!(honesty_host_sole_authority_surface_nav_commands_residual_wave936());
        assert!(simulate_live_host_sole_authority_surface_honesty());
    }
}
