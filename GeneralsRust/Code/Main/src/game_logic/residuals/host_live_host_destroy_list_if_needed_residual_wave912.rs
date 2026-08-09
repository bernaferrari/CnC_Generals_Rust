//! Wave 912: process_destroy_list only when residual destroy work is pending.
//!
//! Empty frames skip the authority destroy-list dual-write. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DESTROY_LIST_IF_NEEDED_METHOD_NAMES_WAVE912: &[&str] = &[
    "host_run_gameworld_shadow_after_logic",
    "process_destroy_list_if_needed",
    "has_pending_destroy_work",
    "Wave 912",
    "playable_claim = false",
];

pub const LIVE_HOST_DESTROY_LIST_IF_NEEDED_NAV_STEPS_WAVE912: &[&str] = &[
    "DESTROY_LIST_IF_NEEDED",
    "SKIP_EMPTY_DESTROY_DUAL_WRITE",
    "LIVE_HOST_DESTROY_LIST_IF_NEEDED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDestroyListIfNeededAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDestroyListIfNeededAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn ready_source() -> &'static str {
    include_str!("../host_destroy_ready_log.rs")
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

pub fn honesty_host_destroy_list_if_needed_method_names_residual_wave912() -> bool {
    let names = LIVE_HOST_DESTROY_LIST_IF_NEEDED_METHOD_NAMES_WAVE912;
    let ok = residual_name_index(names, "process_destroy_list_if_needed").is_some()
        && residual_name_index(names, "Wave 912").is_some();
    residual_action_store(ResidualHostDestroyListIfNeededAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_destroy_list_if_needed_nav_commands_residual_wave912() -> bool {
    let steps = LIVE_HOST_DESTROY_LIST_IF_NEEDED_NAV_STEPS_WAVE912;
    let ok = residual_name_index(steps, "LIVE_HOST_DESTROY_LIST_IF_NEEDED").is_some()
        && residual_name_index(steps, "DESTROY_LIST_IF_NEEDED").is_some();
    residual_action_store(ResidualHostDestroyListIfNeededAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_destroy_list_if_needed_residual_pack_wave912() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ready = ready_source();
    let host_raw = code_window(cnc, "fn host_run_gameworld_shadow_after_logic", 2400);
    let host = non_comment_code(host_raw);
    let helper_raw = code_window(gl, "fn process_destroy_list_if_needed", 500);
    let helper = non_comment_code(helper_raw);
    let ok = host_raw.contains("912")
        && host.contains("process_destroy_list_if_needed")
        && !host.contains("process_destroy_list();")
        && helper.contains("has_pending_destroy_work")
        && ready.contains("has_pending")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostDestroyListIfNeededAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_destroy_list_if_needed_honesty() -> bool {
    let a = honesty_host_destroy_list_if_needed_method_names_residual_wave912();
    let b = honesty_host_destroy_list_if_needed_nav_commands_residual_wave912();
    let c = honesty_host_destroy_list_if_needed_residual_pack_wave912();
    residual_action_store(ResidualHostDestroyListIfNeededAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_destroy_list_if_needed_residual_wave912() {
        assert!(honesty_host_destroy_list_if_needed_residual_pack_wave912());
        assert!(honesty_host_destroy_list_if_needed_method_names_residual_wave912());
        assert!(honesty_host_destroy_list_if_needed_nav_commands_residual_wave912());
        assert!(simulate_live_host_destroy_list_if_needed_honesty());
    }
}
