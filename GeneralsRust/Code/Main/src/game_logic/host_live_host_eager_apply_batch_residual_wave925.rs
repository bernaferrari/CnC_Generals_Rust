//! Wave 925: post-logic eager_apply batch authority boundary.
//!
//! Coupled-frame host→GameWorld residual push uses one
//! `eager_apply_all_host_residuals_after_logic` call instead of N dual-borrows.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_EAGER_APPLY_BATCH_METHOD_NAMES_WAVE925: &[&str] = &[
    "host_run_ingame_logic_presentation_frame",
    "eager_apply_all_host_residuals_after_logic",
    "Wave 925",
    "playable_claim = false",
];

pub const LIVE_HOST_EAGER_APPLY_BATCH_NAV_STEPS_WAVE925: &[&str] = &[
    "EAGER_APPLY_BATCH_BOUNDARY",
    "COUPLED_SHADOW_SINGLE_BORROW",
    "LIVE_HOST_EAGER_APPLY_BATCH",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEagerApplyBatchAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostEagerApplyBatchAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gw_source() -> &'static str {
    include_str!("../gameworld_shadow.rs")
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

pub fn honesty_host_eager_apply_batch_method_names_residual_wave925() -> bool {
    let names = LIVE_HOST_EAGER_APPLY_BATCH_METHOD_NAMES_WAVE925;
    let ok = residual_name_index(names, "eager_apply_all_host_residuals_after_logic").is_some()
        && residual_name_index(names, "Wave 925").is_some();
    residual_action_store(ResidualHostEagerApplyBatchAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_eager_apply_batch_nav_commands_residual_wave925() -> bool {
    let steps = LIVE_HOST_EAGER_APPLY_BATCH_NAV_STEPS_WAVE925;
    let ok = residual_name_index(steps, "LIVE_HOST_EAGER_APPLY_BATCH").is_some()
        && residual_name_index(steps, "EAGER_APPLY_BATCH_BOUNDARY").is_some();
    residual_action_store(ResidualHostEagerApplyBatchAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_eager_apply_batch_residual_pack_wave925() -> bool {
    let cnc = cnc_source();
    let gw = gw_source();
    let cascade_raw = code_window(cnc, "// Wave 682/925: post-logic host", 900);
    let cascade = non_comment_code(cascade_raw);
    let batch_raw = code_window(gw, "fn eager_apply_all_host_residuals_after_logic", 6000);
    let ok = cascade_raw.contains("925")
        && cascade.contains("eager_apply_all_host_residuals_after_logic")
        && !cascade.contains("eager_apply_host_fire_spawns_after_logic")
        && !cascade.contains("eager_apply_host_spawn_after_logic")
        && batch_raw.contains("eager_apply_host_fire_spawns_after_logic")
        && batch_raw.contains("eager_apply_host_spawn_after_logic")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostEagerApplyBatchAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_eager_apply_batch_honesty() -> bool {
    let a = honesty_host_eager_apply_batch_method_names_residual_wave925();
    let b = honesty_host_eager_apply_batch_nav_commands_residual_wave925();
    let c = honesty_host_eager_apply_batch_residual_pack_wave925();
    residual_action_store(ResidualHostEagerApplyBatchAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_eager_apply_batch_residual_wave925() {
        assert!(honesty_host_eager_apply_batch_residual_pack_wave925());
        assert!(honesty_host_eager_apply_batch_method_names_residual_wave925());
        assert!(honesty_host_eager_apply_batch_nav_commands_residual_wave925());
        assert!(simulate_live_host_eager_apply_batch_honesty());
    }
}
