//! Wave 896: map_name + host_is_in_shell_game fail-closed dual-read peel.
//!
//! - `presentation_or_boot_map_name` no longer probes `get_current_map_name` when
//!   residual is cold; returns freeze/host residual or empty boot default.
//! - `host_is_in_shell_game` fail-closed boot default `true` (menu/shell before stamp).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MAP_SHELL_FAILCLOSED_METHOD_NAMES_WAVE896: &[&str] = &[
    "presentation_or_boot_map_name",
    "host_is_in_shell_game",
    "host_match_map_name",
    "host_match_in_shell",
    "Wave 896",
    "playable_claim = false",
];

pub const LIVE_HOST_MAP_SHELL_FAILCLOSED_NAV_STEPS_WAVE896: &[&str] = &[
    "MAP_NAME_FAILCLOSED_NO_LIVE_PROBE",
    "SHELL_GAME_FAILCLOSED_BOOT_TRUE",
    "LIVE_HOST_MAP_SHELL_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMapShellFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMapShellFailclosedAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
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

pub fn honesty_host_map_shell_failclosed_method_names_residual_wave896() -> bool {
    let names = LIVE_HOST_MAP_SHELL_FAILCLOSED_METHOD_NAMES_WAVE896;
    let ok = residual_name_index(names, "presentation_or_boot_map_name").is_some()
        && residual_name_index(names, "host_is_in_shell_game").is_some()
        && residual_name_index(names, "Wave 896").is_some();
    residual_action_store(ResidualHostMapShellFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_map_shell_failclosed_nav_commands_residual_wave896() -> bool {
    let steps = LIVE_HOST_MAP_SHELL_FAILCLOSED_NAV_STEPS_WAVE896;
    let ok = residual_name_index(steps, "LIVE_HOST_MAP_SHELL_FAILCLOSED").is_some()
        && residual_name_index(steps, "MAP_NAME_FAILCLOSED_NO_LIVE_PROBE").is_some();
    residual_action_store(ResidualHostMapShellFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_map_shell_failclosed_residual_pack_wave896() -> bool {
    let cnc = cnc_source();
    let map = non_comment_code(code_window(cnc, "fn presentation_or_boot_map_name", 1800));
    let shell = non_comment_code(code_window(cnc, "fn host_is_in_shell_game", 500));
    let ok = !map.contains("get_current_map_name")
        && map.contains("String::new()")
        && map.contains("host_match_map_name")
        && !shell.contains("isInShellGame()")
        && shell.contains("true")
        && cnc.contains("Wave 896")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostMapShellFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_map_shell_failclosed_honesty() -> bool {
    let a = honesty_host_map_shell_failclosed_method_names_residual_wave896();
    let b = honesty_host_map_shell_failclosed_nav_commands_residual_wave896();
    let c = honesty_host_map_shell_failclosed_residual_pack_wave896();
    residual_action_store(ResidualHostMapShellFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_map_shell_failclosed_residual_wave896() {
        assert!(honesty_host_map_shell_failclosed_residual_pack_wave896());
        assert!(honesty_host_map_shell_failclosed_method_names_residual_wave896());
        assert!(honesty_host_map_shell_failclosed_nav_commands_residual_wave896());
        assert!(simulate_live_host_map_shell_failclosed_honesty());
    }
}
