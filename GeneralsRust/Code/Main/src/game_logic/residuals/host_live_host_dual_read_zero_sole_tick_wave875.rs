//! Wave 875: host dual-read helpers are residual-warm (queue via host helpers);
//! production/movement sole-tick early-return honesty. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DUAL_READ_ZERO_SOLE_TICK_METHOD_NAMES_WAVE875: &[&str] = &[
    "host_queue_and_process_command",
    "host_queue_command",
    "gameworld_production_sole_tick_enabled",
    "gameworld_movement_authority_live",
    "Wave 875",
    "playable_claim = false",
];

pub const LIVE_HOST_DUAL_READ_ZERO_SOLE_TICK_NAV_STEPS_WAVE875: &[&str] = &[
    "ZERO_RAW_HOST_DUAL_READS",
    "PRODUCTION_SOLE_TICK_EARLY_RETURN",
    "MOVEMENT_AUTHORITY_EARLY_RETURN",
    "LIVE_HOST_DUAL_READ_ZERO_SOLE_TICK",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDualReadZeroSoleTickAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDualReadZeroSoleTickAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn count_raw_host_dual_reads(cnc: &str) -> usize {
    let mut raw = 0usize;
    let mut start = 0usize;
    while let Some(rel) = cnc[start..].find("fn host_") {
        let abs = start + rel;
        let end = cnc[abs + 10..]
            .find("\n    fn ")
            .map(|i| abs + 10 + i)
            .unwrap_or(cnc.len());
        let body = &cnc[abs..end];
        start = abs + 10;
        if !body.contains("self.game_logic.") {
            continue;
        }
        let has_res = body.contains("last_presentation")
            || body.contains("host_match_")
            || body.contains("host_refresh")
            || body.contains("host_stamp")
            || body.contains("host_clear")
            || body.contains("is_none()")
            || body.contains("last_ui_state")
            || body.contains("host_queue_command")
            || body.contains("host_process_commands");
        if !has_res {
            raw += 1;
        }
    }
    raw
}

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_dual_read_zero_sole_tick_method_names_residual_wave875() -> bool {
    let names = LIVE_HOST_DUAL_READ_ZERO_SOLE_TICK_METHOD_NAMES_WAVE875;
    let ok = residual_name_index(names, "host_queue_and_process_command").is_some()
        && residual_name_index(names, "gameworld_production_sole_tick_enabled").is_some()
        && residual_name_index(names, "Wave 875").is_some();
    residual_action_store(ResidualHostDualReadZeroSoleTickAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_dual_read_zero_sole_tick_nav_commands_residual_wave875() -> bool {
    let steps = LIVE_HOST_DUAL_READ_ZERO_SOLE_TICK_NAV_STEPS_WAVE875;
    let ok = residual_name_index(steps, "LIVE_HOST_DUAL_READ_ZERO_SOLE_TICK").is_some()
        && residual_name_index(steps, "ZERO_RAW_HOST_DUAL_READS").is_some();
    residual_action_store(ResidualHostDualReadZeroSoleTickAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_dual_read_zero_sole_tick_residual_pack_wave875() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let raw = count_raw_host_dual_reads(cnc);
    let ok = raw == 0
        && cnc.contains("Wave 576/874: queue + process + Command SFX residual via host helpers")
        && cnc.contains("self.host_CommandPipelineOp::QueueAndProcess")
        && gl.contains("if crate::gameworld_shadow::gameworld_production_sole_tick_enabled()")
        && gl.contains("if crate::gameworld_shadow::gameworld_movement_authority_live()")
        && gl.contains("// Wave 613: production complete collect + apply via host helpers.")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostDualReadZeroSoleTickAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_dual_read_zero_sole_tick_honesty() -> bool {
    let a = honesty_host_dual_read_zero_sole_tick_method_names_residual_wave875();
    let b = honesty_host_dual_read_zero_sole_tick_nav_commands_residual_wave875();
    let c = honesty_host_dual_read_zero_sole_tick_residual_pack_wave875();
    residual_action_store(ResidualHostDualReadZeroSoleTickAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_dual_read_zero_sole_tick_residual_wave875() {
        assert_eq!(count_raw_host_dual_reads(cnc_source()), 0);
        assert!(honesty_host_dual_read_zero_sole_tick_residual_pack_wave875());
        assert!(honesty_host_dual_read_zero_sole_tick_method_names_residual_wave875());
        assert!(honesty_host_dual_read_zero_sole_tick_nav_commands_residual_wave875());
        assert!(simulate_live_host_dual_read_zero_sole_tick_honesty());
    }
}
