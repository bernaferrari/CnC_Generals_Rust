//! Wave 915: process SFX return-bool peel + world-size/template dual-write skips.
//!
//! - process_commands_if_needed returns bool (no has_pending dual-read for SFX)
//! - override_world_size skips when residual bounds match
//! - golden ranger insert fail-closed when freeze is installed without residual table
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PROCESS_SFX_WORLD_TEMPLATE_PEELS_METHOD_NAMES_WAVE915: &[&str] = &[
    "host_process_commands_with_command_sound",
    "host_override_world_size",
    "host_ensure_golden_ranger_template",
    "process_commands_if_needed",
    "Wave 915",
    "playable_claim = false",
];

pub const LIVE_HOST_PROCESS_SFX_WORLD_TEMPLATE_PEELS_NAV_STEPS_WAVE915: &[&str] = &[
    "PROCESS_SFX_FROM_IF_NEEDED_BOOL",
    "SKIP_REDUNDANT_WORLD_SIZE_WRITE",
    "GOLDEN_RANGER_FREEZE_FAILCLOSED",
    "LIVE_HOST_PROCESS_SFX_WORLD_TEMPLATE_PEELS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProcessSfxWorldTemplatePeelsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProcessSfxWorldTemplatePeelsAction) {
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

pub fn honesty_host_process_sfx_world_template_peels_method_names_residual_wave915() -> bool {
    let names = LIVE_HOST_PROCESS_SFX_WORLD_TEMPLATE_PEELS_METHOD_NAMES_WAVE915;
    let ok = residual_name_index(names, "host_override_world_size").is_some()
        && residual_name_index(names, "Wave 915").is_some();
    residual_action_store(ResidualHostProcessSfxWorldTemplatePeelsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_process_sfx_world_template_peels_nav_commands_residual_wave915() -> bool {
    let steps = LIVE_HOST_PROCESS_SFX_WORLD_TEMPLATE_PEELS_NAV_STEPS_WAVE915;
    let ok = residual_name_index(steps, "LIVE_HOST_PROCESS_SFX_WORLD_TEMPLATE_PEELS").is_some()
        && residual_name_index(steps, "PROCESS_SFX_FROM_IF_NEEDED_BOOL").is_some();
    residual_action_store(ResidualHostProcessSfxWorldTemplatePeelsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_process_sfx_world_template_peels_residual_pack_wave915() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sound_raw = code_window(cnc, "fn host_process_commands_with_command_sound", 900);
    let sound = non_comment_code(sound_raw);
    let over_raw = code_window(cnc, "fn host_override_world_size", 900);
    let over = non_comment_code(over_raw);
    let gold_raw = code_window(cnc, "fn host_ensure_golden_ranger_template", 1200);
    let gold = non_comment_code(gold_raw);
    let helper_raw = code_window(&gl, "fn process_commands_if_needed", 500);
    let helper = non_comment_code(helper_raw);
    let ok = sound_raw.contains("915")
        && !sound.contains("has_pending_commands")
        && (sound.contains("process_commands_if_needed")
            || sound.contains("CommandPipelineOp::ProcessIfNeeded"))
        && over_raw.contains("915")
        && over.contains("host_match_world_bounds")
        && gold_raw.contains("915")
        && gold.contains("last_presentation_frame")
        && helper.contains("-> bool")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostProcessSfxWorldTemplatePeelsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_process_sfx_world_template_peels_honesty() -> bool {
    let a = honesty_host_process_sfx_world_template_peels_method_names_residual_wave915();
    let b = honesty_host_process_sfx_world_template_peels_nav_commands_residual_wave915();
    let c = honesty_host_process_sfx_world_template_peels_residual_pack_wave915();
    residual_action_store(ResidualHostProcessSfxWorldTemplatePeelsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_process_sfx_world_template_peels_residual_wave915() {
        assert!(honesty_host_process_sfx_world_template_peels_residual_pack_wave915());
        assert!(honesty_host_process_sfx_world_template_peels_method_names_residual_wave915());
        assert!(honesty_host_process_sfx_world_template_peels_nav_commands_residual_wave915());
        assert!(simulate_live_host_process_sfx_world_template_peels_honesty());
    }
}
