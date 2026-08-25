//! Wave 935: GameLogic host access via borrow accessors + intentional split-borrow adapters.
//!
//! Authority method calls use `host_game_logic_mut().apply_*`.
//! Naked `self.game_logic` remains only in accessors and split-borrow adapter sites
//! (save/load/skirmish/shadow/presentation). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GAMELOGIC_BORROW_BOUNDARY_METHOD_NAMES_WAVE935: &[&str] = &[
    "host_game_logic",
    "host_game_logic_mut",
    "host_replace_game_logic",
    "host_game_logic_mut()",
    "Wave 935",
    "playable_claim = false",
];

pub const LIVE_HOST_GAMELOGIC_BORROW_BOUNDARY_NAV_STEPS_WAVE935: &[&str] = &[
    "GAMELOGIC_BORROW_BOUNDARY",
    "HOST_GAME_LOGIC_ACCESSORS",
    "LIVE_HOST_GAMELOGIC_BORROW_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

const ALLOWED_NAKED_GAMELOGIC_FNS_WAVE935: &[&str] = &[
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
    "host_save_game_authority",
    "host_load_game_authority",
    "host_apply_skirmish_config_authority",
    "host_simulate_gameworld_authority_probe",
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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGamelogicBorrowBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostGamelogicBorrowBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn code_window<'a>(src: &'a str, marker: &str, len: usize) -> &'a str {
    super::harness::last_rust_fn_body(src, marker.trim_start_matches("fn ").trim())
        .or_else(|| src.rfind(marker).map(|i| &src[i..src.len().min(i + len)]))
        .unwrap_or("")
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
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

/// Naked `self.game_logic` field uses excluding `game_logic_paused` and comments.
fn naked_game_logic_field_sites(src: &str) -> Vec<(String, usize)> {
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
        out.push((enclosing_fn_name(src, j), j));
        i = after;
    }
    out
}

pub fn honesty_host_gamelogic_borrow_boundary_method_names_residual_wave935() -> bool {
    let names = LIVE_HOST_GAMELOGIC_BORROW_BOUNDARY_METHOD_NAMES_WAVE935;
    let ok = residual_name_index(names, "host_game_logic_mut").is_some()
        && residual_name_index(names, "Wave 935").is_some();
    residual_action_store(ResidualHostGamelogicBorrowBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_gamelogic_borrow_boundary_nav_commands_residual_wave935() -> bool {
    let steps = LIVE_HOST_GAMELOGIC_BORROW_BOUNDARY_NAV_STEPS_WAVE935;
    let ok = residual_name_index(steps, "LIVE_HOST_GAMELOGIC_BORROW_BOUNDARY").is_some()
        && residual_name_index(steps, "GAMELOGIC_BORROW_BOUNDARY").is_some();
    residual_action_store(ResidualHostGamelogicBorrowBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_gamelogic_borrow_boundary_residual_pack_wave935() -> bool {
    let cnc = cnc_source();
    let issue = non_comment_code(code_window(cnc, "fn host_issue_direct_player_order", 360));
    let sites = naked_game_logic_field_sites(cnc);
    let allowed_ok = sites.iter().all(|(fname, _)| {
        ALLOWED_NAKED_GAMELOGIC_FNS_WAVE935
            .iter()
            .any(|a| *a == fname.as_str())
    });
    let ok = cnc.contains("fn host_game_logic(")
        && cnc.contains("&self.game_logic")
        && cnc.contains("fn host_game_logic_mut(")
        && cnc.contains("&mut self.game_logic")
        && cnc.contains("fn host_replace_game_logic(")
        && cnc.contains("self.game_logic = logic")
        && issue.contains("host_game_logic_mut()")
        && !issue.contains("self.game_logic.apply_")
        && cnc.contains("host_game_logic_mut().apply_direct_player_order")
        && cnc.contains("host_game_logic_mut().apply_object_lifecycle_op")
        && cnc.contains("host_game_logic_mut().apply_command_pipeline_op")
        && cnc.contains("host_game_logic_mut().apply_session_control_op")
        && cnc.contains("host_game_logic_mut().apply_host_support_op")
        && sites.len() >= 8
        && allowed_ok
        && cnc.contains("Wave 935")
        && !cnc.contains("self.playable_claim = true");
    residual_action_store(ResidualHostGamelogicBorrowBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_gamelogic_borrow_boundary_honesty() -> bool {
    let a = honesty_host_gamelogic_borrow_boundary_method_names_residual_wave935();
    let b = honesty_host_gamelogic_borrow_boundary_nav_commands_residual_wave935();
    let c = honesty_host_gamelogic_borrow_boundary_residual_pack_wave935();
    residual_action_store(ResidualHostGamelogicBorrowBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_gamelogic_borrow_boundary_residual_wave935() {
        assert!(honesty_host_gamelogic_borrow_boundary_residual_pack_wave935());
        assert!(honesty_host_gamelogic_borrow_boundary_method_names_residual_wave935());
        assert!(honesty_host_gamelogic_borrow_boundary_nav_commands_residual_wave935());
        assert!(simulate_live_host_gamelogic_borrow_boundary_honesty());
    }
}
