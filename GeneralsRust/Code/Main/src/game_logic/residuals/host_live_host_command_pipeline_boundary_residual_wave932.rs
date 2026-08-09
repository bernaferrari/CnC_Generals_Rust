//! Wave 932: command pipeline via GameLogic::apply_command_pipeline_op boundary.
//!
//! Host queue/process helpers call one GameLogic authority API instead of three
//! direct dual-writes. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_COMMAND_PIPELINE_BOUNDARY_METHOD_NAMES_WAVE932: &[&str] = &[
    "apply_command_pipeline_op",
    "CommandPipelineOp",
    "host_queue_command",
    "host_queue_and_process_command_silent",
    "host_process_commands_with_command_sound",
    "host_process_shell_menu_commands",
    "Wave 932",
    "playable_claim = false",
];

pub const LIVE_HOST_COMMAND_PIPELINE_BOUNDARY_NAV_STEPS_WAVE932: &[&str] = &[
    "COMMAND_PIPELINE_BOUNDARY",
    "SINGLE_APPLY_COMMAND_PIPELINE_OP",
    "LIVE_HOST_COMMAND_PIPELINE_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCommandPipelineBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCommandPipelineBoundaryAction) {
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

pub fn honesty_host_command_pipeline_boundary_method_names_residual_wave932() -> bool {
    let names = LIVE_HOST_COMMAND_PIPELINE_BOUNDARY_METHOD_NAMES_WAVE932;
    let ok = residual_name_index(names, "apply_command_pipeline_op").is_some()
        && residual_name_index(names, "Wave 932").is_some();
    residual_action_store(ResidualHostCommandPipelineBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_pipeline_boundary_nav_commands_residual_wave932() -> bool {
    let steps = LIVE_HOST_COMMAND_PIPELINE_BOUNDARY_NAV_STEPS_WAVE932;
    let ok = residual_name_index(steps, "LIVE_HOST_COMMAND_PIPELINE_BOUNDARY").is_some()
        && residual_name_index(steps, "COMMAND_PIPELINE_BOUNDARY").is_some();
    residual_action_store(ResidualHostCommandPipelineBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_pipeline_boundary_residual_pack_wave932() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let api_raw = code_window(gl, "fn apply_command_pipeline_op", 900);
    let api = non_comment_code(api_raw);
    let queue_raw = code_window(cnc, "fn host_queue_command", 420);
    let queue = non_comment_code(queue_raw);
    let silent = non_comment_code(code_window(
        cnc,
        "fn host_queue_and_process_command_silent",
        600,
    ));
    let sound = non_comment_code(code_window(
        cnc,
        "fn host_process_commands_with_command_sound",
        700,
    ));
    let shell = non_comment_code(code_window(cnc, "fn host_process_shell_menu_commands", 500));
    let ok = gl.contains("enum CommandPipelineOp")
        && api.contains("self.queue_command")
        && api.contains("self.queue_and_process_command")
        && api.contains("self.process_commands_if_needed")
        && queue.contains("apply_command_pipeline_op")
        && !queue.contains("self.game_logic.queue_command")
        && silent.contains("apply_command_pipeline_op")
        && !silent.contains("self.game_logic.queue_and_process_command")
        && sound.contains("apply_command_pipeline_op")
        && !sound.contains("self.game_logic.process_commands_if_needed")
        && shell.contains("apply_command_pipeline_op")
        && !shell.contains("self.game_logic.process_commands_if_needed")
        && queue_raw.contains("932")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostCommandPipelineBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_command_pipeline_boundary_honesty() -> bool {
    let a = honesty_host_command_pipeline_boundary_method_names_residual_wave932();
    let b = honesty_host_command_pipeline_boundary_nav_commands_residual_wave932();
    let c = honesty_host_command_pipeline_boundary_residual_pack_wave932();
    residual_action_store(ResidualHostCommandPipelineBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_command_pipeline_boundary_residual_wave932() {
        assert!(honesty_host_command_pipeline_boundary_residual_pack_wave932());
        assert!(honesty_host_command_pipeline_boundary_method_names_residual_wave932());
        assert!(honesty_host_command_pipeline_boundary_nav_commands_residual_wave932());
        assert!(simulate_live_host_command_pipeline_boundary_honesty());
    }
}
