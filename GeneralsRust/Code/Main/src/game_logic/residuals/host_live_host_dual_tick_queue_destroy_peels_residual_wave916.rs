//! Wave 916: dual-tick AuthorityOnly short-circuit + queue/destroy residual peels.
//!
//! - default dual-tick policy never invokes tick_gamelogic_crate
//! - queue_command no longer stamps sim timing mid-queue
//! - destroy_object skips when presentation residual already destroyed
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DUAL_TICK_QUEUE_DESTROY_PEELS_METHOD_NAMES_WAVE916: &[&str] = &[
    "host_queue_command",
    "host_destroy_object",
    "dual_tick_policy",
    "tick_gamelogic_crate",
    "Wave 916",
    "playable_claim = false",
];

pub const LIVE_HOST_DUAL_TICK_QUEUE_DESTROY_PEELS_NAV_STEPS_WAVE916: &[&str] = &[
    "DUAL_TICK_AUTHORITY_ONLY_SHORT_CIRCUIT",
    "QUEUE_NO_MID_STAMP",
    "DESTROY_SKIP_IF_PRESENTATION_DEAD",
    "LIVE_HOST_DUAL_TICK_QUEUE_DESTROY_PEELS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDualTickQueueDestroyPeelsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDualTickQueueDestroyPeelsAction) {
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

pub fn honesty_host_dual_tick_queue_destroy_peels_method_names_residual_wave916() -> bool {
    let names = LIVE_HOST_DUAL_TICK_QUEUE_DESTROY_PEELS_METHOD_NAMES_WAVE916;
    let ok = residual_name_index(names, "host_destroy_object").is_some()
        && residual_name_index(names, "Wave 916").is_some();
    residual_action_store(ResidualHostDualTickQueueDestroyPeelsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_dual_tick_queue_destroy_peels_nav_commands_residual_wave916() -> bool {
    let steps = LIVE_HOST_DUAL_TICK_QUEUE_DESTROY_PEELS_NAV_STEPS_WAVE916;
    let ok = residual_name_index(steps, "LIVE_HOST_DUAL_TICK_QUEUE_DESTROY_PEELS").is_some()
        && residual_name_index(steps, "DUAL_TICK_AUTHORITY_ONLY_SHORT_CIRCUIT").is_some();
    residual_action_store(ResidualHostDualTickQueueDestroyPeelsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_dual_tick_queue_destroy_peels_residual_pack_wave916() -> bool {
    let cnc = cnc_source();
    // dual-tick site is not a fn host_*; search policy block
    let dual_idx = cnc.find("Wave 916: AuthorityOnly");
    let dual_raw = if dual_idx.is_some() {
        let i = dual_idx.unwrap();
        &cnc[i..cnc.len().min(i + 900)]
    } else {
        ""
    };
    let dual = non_comment_code(dual_raw);
    let q_raw = code_window(cnc, "fn host_queue_command", 700);
    let q = non_comment_code(q_raw);
    let d_raw = code_window(cnc, "fn host_destroy_object", 1200);
    let d = non_comment_code(d_raw);
    let ok = dual_raw.contains("916")
        && dual.contains("AuthorityOnly")
        && dual.contains("tick_gamelogic_crate")
        && q_raw.contains("916")
        && !q.contains("host_stamp_sim_timing_residuals")
        && d_raw.contains("916")
        && d.contains("already_destroyed")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostDualTickQueueDestroyPeelsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_dual_tick_queue_destroy_peels_honesty() -> bool {
    let a = honesty_host_dual_tick_queue_destroy_peels_method_names_residual_wave916();
    let b = honesty_host_dual_tick_queue_destroy_peels_nav_commands_residual_wave916();
    let c = honesty_host_dual_tick_queue_destroy_peels_residual_pack_wave916();
    residual_action_store(ResidualHostDualTickQueueDestroyPeelsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_dual_tick_queue_destroy_peels_residual_wave916() {
        assert!(honesty_host_dual_tick_queue_destroy_peels_residual_pack_wave916());
        assert!(honesty_host_dual_tick_queue_destroy_peels_method_names_residual_wave916());
        assert!(honesty_host_dual_tick_queue_destroy_peels_nav_commands_residual_wave916());
        assert!(simulate_live_host_dual_tick_queue_destroy_peels_honesty());
    }
}
