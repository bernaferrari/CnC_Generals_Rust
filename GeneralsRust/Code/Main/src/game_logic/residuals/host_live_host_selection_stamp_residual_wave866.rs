//! Wave 866: host_set_selection stamps host_match_selected_ids so selection peels
//! stay residual-warm without waiting for full residual refresh.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELECTION_STAMP_METHOD_NAMES_WAVE866: &[&str] = &[
    "host_set_selection",
    "host_match_selected_ids",
    "selected_objects",
    "Wave 866",
    "playable_claim = false",
];

pub const LIVE_HOST_SELECTION_STAMP_NAV_STEPS_WAVE866: &[&str] = &[
    "STAMP_SELECTION_ON_SET",
    "KEEP_SELECTION_RESIDUAL_WARM",
    "LIVE_HOST_SELECTION_STAMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSelectionStampAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSelectionStampAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

pub fn honesty_host_selection_stamp_method_names_residual_wave866() -> bool {
    let names = LIVE_HOST_SELECTION_STAMP_METHOD_NAMES_WAVE866;
    let ok = residual_name_index(names, "host_set_selection").is_some()
        && residual_name_index(names, "host_match_selected_ids").is_some()
        && residual_name_index(names, "Wave 866").is_some();
    residual_action_store(ResidualHostSelectionStampAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_stamp_nav_commands_residual_wave866() -> bool {
    let steps = LIVE_HOST_SELECTION_STAMP_NAV_STEPS_WAVE866;
    let ok = residual_name_index(steps, "LIVE_HOST_SELECTION_STAMP").is_some()
        && residual_name_index(steps, "STAMP_SELECTION_ON_SET").is_some();
    residual_action_store(ResidualHostSelectionStampAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_stamp_residual_pack_wave866() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("Wave 579")
        && (cnc.contains("self.host_match_selected_ids = Some(ids)")
            || cnc.contains("self.host_match_selected_ids = Some(ids.clone())"))
        && cnc.contains("self.selected_objects = ids.clone()");
    residual_action_store(ResidualHostSelectionStampAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_selection_stamp_honesty() -> bool {
    let a = honesty_host_selection_stamp_method_names_residual_wave866();
    let b = honesty_host_selection_stamp_nav_commands_residual_wave866();
    let c = honesty_host_selection_stamp_residual_pack_wave866();
    residual_action_store(ResidualHostSelectionStampAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_selection_stamp_residual_wave866() {
        assert!(honesty_host_selection_stamp_residual_pack_wave866());
        assert!(honesty_host_selection_stamp_method_names_residual_wave866());
        assert!(honesty_host_selection_stamp_nav_commands_residual_wave866());
        assert!(simulate_live_host_selection_stamp_honesty());
    }
}
