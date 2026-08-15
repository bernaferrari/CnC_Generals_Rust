//! Wave 922: load_map_or_fallback + queue_and_process_command authority boundaries.
//!
//! Host match-load and silent queue+process each use one GameLogic call.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_LOAD_QUEUE_PROCESS_BOUNDARIES_METHOD_NAMES_WAVE922: &[&str] = &[
    "host_load_map_or_default",
    "host_queue_and_process_command_silent",
    "load_map_or_fallback",
    "queue_and_process_command",
    "Wave 922",
    "playable_claim = false",
];

pub const LIVE_HOST_LOAD_QUEUE_PROCESS_BOUNDARIES_NAV_STEPS_WAVE922: &[&str] = &[
    "LOAD_MAP_OR_FALLBACK_BOUNDARY",
    "QUEUE_AND_PROCESS_COMMAND_BOUNDARY",
    "LIVE_HOST_LOAD_QUEUE_PROCESS_BOUNDARIES",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostLoadQueueProcessBoundariesAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostLoadQueueProcessBoundariesAction) {
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

pub fn honesty_host_load_queue_process_boundaries_method_names_residual_wave922() -> bool {
    let names = LIVE_HOST_LOAD_QUEUE_PROCESS_BOUNDARIES_METHOD_NAMES_WAVE922;
    let ok = residual_name_index(names, "load_map_or_fallback").is_some()
        && residual_name_index(names, "Wave 922").is_some();
    residual_action_store(ResidualHostLoadQueueProcessBoundariesAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_load_queue_process_boundaries_nav_commands_residual_wave922() -> bool {
    let steps = LIVE_HOST_LOAD_QUEUE_PROCESS_BOUNDARIES_NAV_STEPS_WAVE922;
    let ok = residual_name_index(steps, "LIVE_HOST_LOAD_QUEUE_PROCESS_BOUNDARIES").is_some()
        && residual_name_index(steps, "LOAD_MAP_OR_FALLBACK_BOUNDARY").is_some();
    residual_action_store(ResidualHostLoadQueueProcessBoundariesAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_load_queue_process_boundaries_residual_pack_wave922() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let load_raw = code_window(cnc, "fn host_load_map_or_default", 1100);
    let load = non_comment_code(load_raw);
    let silent_raw = code_window(cnc, "fn host_queue_and_process_command_silent", 700);
    let silent = non_comment_code(silent_raw);
    let ok = load_raw.contains("922")
        && load.contains("load_map_or_fallback")
        && !load.contains(".load_map(")
        && silent_raw.contains("922")
        && silent.contains("queue_and_process_command")
        && !silent.contains(".queue_command(")
        && gl.contains("fn load_map_or_fallback")
        && gl.contains("fn queue_and_process_command")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostLoadQueueProcessBoundariesAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_load_queue_process_boundaries_honesty() -> bool {
    let a = honesty_host_load_queue_process_boundaries_method_names_residual_wave922();
    let b = honesty_host_load_queue_process_boundaries_nav_commands_residual_wave922();
    let c = honesty_host_load_queue_process_boundaries_residual_pack_wave922();
    residual_action_store(ResidualHostLoadQueueProcessBoundariesAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_load_queue_process_boundaries_residual_wave922() {
        assert!(honesty_host_load_queue_process_boundaries_residual_pack_wave922());
        assert!(honesty_host_load_queue_process_boundaries_method_names_residual_wave922());
        assert!(honesty_host_load_queue_process_boundaries_nav_commands_residual_wave922());
        assert!(simulate_live_host_load_queue_process_boundaries_honesty());
    }
}
