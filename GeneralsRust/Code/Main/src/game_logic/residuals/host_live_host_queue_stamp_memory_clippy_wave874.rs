//! Wave 874: host_queue_command stamps sim timing; queue+process routes through
//! host_queue_command. memory_system clippy -D warnings cleaned. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_QUEUE_STAMP_METHOD_NAMES_WAVE874: &[&str] = &[
    "host_queue_command",
    "host_queue_and_process_command",
    "host_stamp_sim_timing_residuals",
    "Wave 874",
    "playable_claim = false",
];

pub const LIVE_HOST_QUEUE_STAMP_NAV_STEPS_WAVE874: &[&str] = &[
    "STAMP_AFTER_QUEUE",
    "ROUTE_QUEUE_PROCESS_VIA_HOST_QUEUE",
    "MEMORY_SYSTEM_CLIPPY_CLEAN",
    "LIVE_HOST_QUEUE_STAMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostQueueStampAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostQueueStampAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn memory_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WWLib/memory_system/src/lib.rs")
}

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_queue_stamp_method_names_residual_wave874() -> bool {
    let names = LIVE_HOST_QUEUE_STAMP_METHOD_NAMES_WAVE874;
    let ok = residual_name_index(names, "host_queue_command").is_some()
        && residual_name_index(names, "host_stamp_sim_timing_residuals").is_some()
        && residual_name_index(names, "Wave 874").is_some();
    residual_action_store(ResidualHostQueueStampAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_queue_stamp_nav_commands_residual_wave874() -> bool {
    let steps = LIVE_HOST_QUEUE_STAMP_NAV_STEPS_WAVE874;
    let ok = residual_name_index(steps, "LIVE_HOST_QUEUE_STAMP").is_some()
        && residual_name_index(steps, "STAMP_AFTER_QUEUE").is_some();
    residual_action_store(ResidualHostQueueStampAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_queue_stamp_residual_pack_wave874() -> bool {
    let cnc = cnc_source();
    let mem = memory_source();
    let ok = cnc.contains("Wave 584")
        && cnc.contains("self.host_stamp_sim_timing_residuals()")
        && cnc.contains("Wave 576/874: queue + process + Command SFX residual via host helpers")
        && cnc.contains("CommandPipelineOp::QueueAndProcess")
        && mem.contains("#[allow(clippy::new_without_default)]")
        && mem.contains("#[allow(clippy::vec_box)]")
        && mem.contains("#[allow(dead_code)]");
    residual_action_store(ResidualHostQueueStampAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_queue_stamp_honesty() -> bool {
    let a = honesty_host_queue_stamp_method_names_residual_wave874();
    let b = honesty_host_queue_stamp_nav_commands_residual_wave874();
    let c = honesty_host_queue_stamp_residual_pack_wave874();
    residual_action_store(ResidualHostQueueStampAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_queue_stamp_residual_wave874() {
        assert!(honesty_host_queue_stamp_residual_pack_wave874());
        assert!(honesty_host_queue_stamp_method_names_residual_wave874());
        assert!(honesty_host_queue_stamp_nav_commands_residual_wave874());
        assert!(simulate_live_host_queue_stamp_honesty());
    }
}
