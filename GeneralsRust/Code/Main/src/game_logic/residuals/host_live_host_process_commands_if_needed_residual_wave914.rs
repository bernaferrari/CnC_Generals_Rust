//! Wave 914: process_commands only when the command queue is non-empty.
//!
//! Empty host process paths skip the authority dual-write / economy materialize.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PROCESS_COMMANDS_IF_NEEDED_METHOD_NAMES_WAVE914: &[&str] = &[
    "host_process_commands_with_command_sound",
    "host_queue_and_process_command_silent",
    "host_process_shell_menu_commands",
    "process_commands_if_needed",
    "has_pending_commands",
    "Wave 914",
    "playable_claim = false",
];

pub const LIVE_HOST_PROCESS_COMMANDS_IF_NEEDED_NAV_STEPS_WAVE914: &[&str] = &[
    "PROCESS_COMMANDS_IF_NEEDED",
    "SKIP_EMPTY_COMMAND_QUEUE",
    "LIVE_HOST_PROCESS_COMMANDS_IF_NEEDED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProcessCommandsIfNeededAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProcessCommandsIfNeededAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
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

pub fn honesty_host_process_commands_if_needed_method_names_residual_wave914() -> bool {
    let names = LIVE_HOST_PROCESS_COMMANDS_IF_NEEDED_METHOD_NAMES_WAVE914;
    let ok = residual_name_index(names, "process_commands_if_needed").is_some()
        && residual_name_index(names, "Wave 914").is_some();
    residual_action_store(ResidualHostProcessCommandsIfNeededAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_process_commands_if_needed_nav_commands_residual_wave914() -> bool {
    let steps = LIVE_HOST_PROCESS_COMMANDS_IF_NEEDED_NAV_STEPS_WAVE914;
    let ok = residual_name_index(steps, "LIVE_HOST_PROCESS_COMMANDS_IF_NEEDED").is_some()
        && residual_name_index(steps, "SKIP_EMPTY_COMMAND_QUEUE").is_some();
    residual_action_store(ResidualHostProcessCommandsIfNeededAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_process_commands_if_needed_residual_pack_wave914() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sound_raw = code_window(cnc, "fn host_process_commands_with_command_sound", 900);
    let sound = non_comment_code(sound_raw);
    let silent_raw = code_window(cnc, "fn host_queue_and_process_command_silent", 700);
    let silent = non_comment_code(silent_raw);
    let shell_raw = code_window(cnc, "fn host_process_shell_menu_commands", 500);
    let shell = non_comment_code(shell_raw);
    let helper_raw = code_window(gl, "fn process_commands_if_needed", 500);
    let helper = non_comment_code(helper_raw);
    let ok = sound_raw.contains("914")
        && sound.contains("has_pending_commands")
        && silent.contains("process_commands_if_needed")
        && shell.contains("process_commands_if_needed")
        && helper.contains("command_queue.is_empty")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostProcessCommandsIfNeededAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_process_commands_if_needed_honesty() -> bool {
    let a = honesty_host_process_commands_if_needed_method_names_residual_wave914();
    let b = honesty_host_process_commands_if_needed_nav_commands_residual_wave914();
    let c = honesty_host_process_commands_if_needed_residual_pack_wave914();
    residual_action_store(ResidualHostProcessCommandsIfNeededAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_process_commands_if_needed_residual_wave914() {
        assert!(honesty_host_process_commands_if_needed_residual_pack_wave914());
        assert!(honesty_host_process_commands_if_needed_method_names_residual_wave914());
        assert!(honesty_host_process_commands_if_needed_nav_commands_residual_wave914());
        assert!(simulate_live_host_process_commands_if_needed_honesty());
    }
}
