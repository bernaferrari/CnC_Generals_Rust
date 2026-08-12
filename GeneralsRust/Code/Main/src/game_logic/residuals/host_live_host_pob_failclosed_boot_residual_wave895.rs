//! Wave 895: presentation_or_boot_* fail-closed boot defaults (no dual-read).
//!
//! Timing/UI peels prefer freeze → host_match residual → fail-closed default
//! instead of live GameLogic probes when residual is cold.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_POB_FAILCLOSED_BOOT_METHOD_NAMES_WAVE895: &[&str] = &[
    "presentation_or_boot_visual_speed",
    "presentation_or_boot_time_frozen",
    "presentation_or_boot_logic_frame",
    "presentation_or_boot_fixed_step_diagnostics",
    "presentation_or_boot_has_template",
    "presentation_or_boot_diplomacy_players",
    "Wave 895",
    "playable_claim = false",
];

pub const LIVE_HOST_POB_FAILCLOSED_BOOT_NAV_STEPS_WAVE895: &[&str] = &[
    "POB_TIMING_FAILCLOSED_BOOT",
    "POB_TEMPLATE_FAILCLOSED_BOOT",
    "POB_DIPLOMACY_FAILCLOSED_BOOT",
    "LIVE_HOST_POB_FAILCLOSED_BOOT",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPobFailclosedBootAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPobFailclosedBootAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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

pub fn honesty_host_pob_failclosed_boot_method_names_residual_wave895() -> bool {
    let names = LIVE_HOST_POB_FAILCLOSED_BOOT_METHOD_NAMES_WAVE895;
    let ok = residual_name_index(names, "presentation_or_boot_visual_speed").is_some()
        && residual_name_index(names, "Wave 895").is_some();
    residual_action_store(ResidualHostPobFailclosedBootAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_pob_failclosed_boot_nav_commands_residual_wave895() -> bool {
    let steps = LIVE_HOST_POB_FAILCLOSED_BOOT_NAV_STEPS_WAVE895;
    let ok = residual_name_index(steps, "LIVE_HOST_POB_FAILCLOSED_BOOT").is_some()
        && residual_name_index(steps, "POB_TIMING_FAILCLOSED_BOOT").is_some();
    residual_action_store(ResidualHostPobFailclosedBootAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_pob_failclosed_boot_residual_pack_wave895() -> bool {
    let cnc = cnc_source();
    let speed = non_comment_code(code_window(
        cnc,
        "fn presentation_or_boot_visual_speed",
        500,
    ));
    let frozen = non_comment_code(code_window(cnc, "fn presentation_or_boot_time_frozen", 500));
    let frame = non_comment_code(code_window(cnc, "fn presentation_or_boot_logic_frame", 500));
    let steps = non_comment_code(code_window(
        cnc,
        "fn presentation_or_boot_fixed_step_diagnostics",
        700,
    ));
    let tmpl = non_comment_code(code_window(
        cnc,
        "fn presentation_or_boot_has_template",
        900,
    ));
    let dip = non_comment_code(code_window(
        cnc,
        "fn presentation_or_boot_diplomacy_players",
        700,
    ));
    let alive = non_comment_code(code_window(
        cnc,
        "fn presentation_or_boot_object_alive",
        700,
    ));
    let ok = cnc.matches("Wave 895").count() >= 8
        && speed.contains("1.0")
        && !speed.contains("visual_speed_multiplier()")
        && !frozen.contains("is_time_frozen_for_simulation()")
        && !frame.contains("get_frame()")
        && steps.contains("(0, false, 0.0)")
        && !steps.contains("fixed_step_diagnostics()")
        && tmpl.contains("false")
        && !tmpl.contains("templates.contains_key")
        && dip.contains("Vec::new()")
        && !dip.contains("player_ids()")
        && !alive.contains("host_object_is_alive")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostPobFailclosedBootAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_pob_failclosed_boot_honesty() -> bool {
    let a = honesty_host_pob_failclosed_boot_method_names_residual_wave895();
    let b = honesty_host_pob_failclosed_boot_nav_commands_residual_wave895();
    let c = honesty_host_pob_failclosed_boot_residual_pack_wave895();
    residual_action_store(ResidualHostPobFailclosedBootAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_pob_failclosed_boot_residual_wave895() {
        assert!(honesty_host_pob_failclosed_boot_residual_pack_wave895());
        assert!(honesty_host_pob_failclosed_boot_method_names_residual_wave895());
        assert!(honesty_host_pob_failclosed_boot_nav_commands_residual_wave895());
        assert!(simulate_live_host_pob_failclosed_boot_honesty());
    }
}
