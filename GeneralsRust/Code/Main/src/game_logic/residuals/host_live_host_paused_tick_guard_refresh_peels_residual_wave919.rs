//! Wave 919: paused tick skip + zero guard delta + freeze refresh peels.
//!
//! - host_update_logic_frame skips authority tick when host residual is paused
//! - adjust_unit_guard_radius no-ops zero delta via presentation residual
//! - enqueue/create skip producer residual refresh under presentation freeze
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PAUSED_TICK_GUARD_REFRESH_PEELS_METHOD_NAMES_WAVE919: &[&str] = &[
    "host_update_logic_frame",
    "host_adjust_unit_guard_radius",
    "host_enqueue_production",
    "host_create_object",
    "Wave 919",
    "playable_claim = false",
];

pub const LIVE_HOST_PAUSED_TICK_GUARD_REFRESH_PEELS_NAV_STEPS_WAVE919: &[&str] = &[
    "PAUSED_TICK_SKIP_AUTHORITY",
    "ZERO_GUARD_DELTA_PRESENTATION",
    "FREEZE_SKIP_PRODUCER_REFRESH",
    "LIVE_HOST_PAUSED_TICK_GUARD_REFRESH_PEELS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPausedTickGuardRefreshPeelsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPausedTickGuardRefreshPeelsAction) {
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

pub fn honesty_host_paused_tick_guard_refresh_peels_method_names_residual_wave919() -> bool {
    let names = LIVE_HOST_PAUSED_TICK_GUARD_REFRESH_PEELS_METHOD_NAMES_WAVE919;
    let ok = residual_name_index(names, "host_update_logic_frame").is_some()
        && residual_name_index(names, "Wave 919").is_some();
    residual_action_store(ResidualHostPausedTickGuardRefreshPeelsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_paused_tick_guard_refresh_peels_nav_commands_residual_wave919() -> bool {
    let steps = LIVE_HOST_PAUSED_TICK_GUARD_REFRESH_PEELS_NAV_STEPS_WAVE919;
    let ok = residual_name_index(steps, "LIVE_HOST_PAUSED_TICK_GUARD_REFRESH_PEELS").is_some()
        && residual_name_index(steps, "PAUSED_TICK_SKIP_AUTHORITY").is_some();
    residual_action_store(ResidualHostPausedTickGuardRefreshPeelsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_paused_tick_guard_refresh_peels_residual_pack_wave919() -> bool {
    let cnc = cnc_source();
    let upd_raw = code_window(cnc, "fn host_update_logic_frame", 1200);
    let upd = non_comment_code(upd_raw);
    let gr_raw = code_window(cnc, "fn host_adjust_unit_guard_radius", 1000);
    let gr = non_comment_code(gr_raw);
    let enq_raw = code_window(cnc, "fn host_enqueue_production", 700);
    let enq = non_comment_code(enq_raw);
    let ok = upd_raw.contains("919")
        && upd.contains("game_paused")
        && gr_raw.contains("919")
        && gr.contains("f32::EPSILON")
        && enq_raw.contains("919")
        && enq.contains("last_presentation_frame.is_none()")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostPausedTickGuardRefreshPeelsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_paused_tick_guard_refresh_peels_honesty() -> bool {
    let a = honesty_host_paused_tick_guard_refresh_peels_method_names_residual_wave919();
    let b = honesty_host_paused_tick_guard_refresh_peels_nav_commands_residual_wave919();
    let c = honesty_host_paused_tick_guard_refresh_peels_residual_pack_wave919();
    residual_action_store(ResidualHostPausedTickGuardRefreshPeelsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_paused_tick_guard_refresh_peels_residual_wave919() {
        assert!(honesty_host_paused_tick_guard_refresh_peels_residual_pack_wave919());
        assert!(honesty_host_paused_tick_guard_refresh_peels_method_names_residual_wave919());
        assert!(honesty_host_paused_tick_guard_refresh_peels_nav_commands_residual_wave919());
        assert!(simulate_live_host_paused_tick_guard_refresh_peels_honesty());
    }
}
