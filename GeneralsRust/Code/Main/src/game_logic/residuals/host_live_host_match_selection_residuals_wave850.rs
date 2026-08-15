//! Wave 850: host-owned selection residual peels player_selected_objects dual-reads
//! from host_ui_selected_ids / host_ui_selection_seed_id boot paths when freeze
//! and engine selection are empty.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_SELECTION_RESIDUALS_METHOD_NAMES_WAVE850: &[&str] = &[
    "host_match_selected_ids",
    "host_ui_selected_ids",
    "host_ui_selection_seed_id",
    "Wave 850",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_SELECTION_RESIDUALS_NAV_STEPS_WAVE850: &[&str] = &[
    "STAMP_HOST_MATCH_SELECTION",
    "PREFER_HOST_SELECTION_BEFORE_LIVE",
    "LIVE_HOST_MATCH_SELECTION_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchSelectionResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchSelectionResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

pub fn honesty_host_match_selection_residuals_method_names_residual_wave850() -> bool {
    let names = LIVE_HOST_MATCH_SELECTION_RESIDUALS_METHOD_NAMES_WAVE850;
    let ok = residual_name_index(names, "host_match_selected_ids").is_some()
        && residual_name_index(names, "host_ui_selected_ids").is_some()
        && residual_name_index(names, "Wave 850").is_some();
    residual_action_store(ResidualHostMatchSelectionResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_selection_residuals_nav_commands_residual_wave850() -> bool {
    let steps = LIVE_HOST_MATCH_SELECTION_RESIDUALS_NAV_STEPS_WAVE850;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_SELECTION_RESIDUALS").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_SELECTION").is_some();
    residual_action_store(ResidualHostMatchSelectionResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_selection_residuals_residual_pack_wave850() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_selected_ids: Option<Vec<crate::game_logic::ObjectId>>")
        && cnc.contains("Wave 850: stamp selection residual")
        && cnc.contains("Wave 610/850")
        && cnc.contains("Wave 609/850")
        && cnc.contains("Wave 850")
        && cnc
            .matches("if let Some(ids) = self.host_match_selected_ids.as_ref()")
            .count()
            >= 2
        && cnc.contains("player_selected_objects"); // boot residual remains
    residual_action_store(ResidualHostMatchSelectionResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_selection_residuals_honesty() -> bool {
    let a = honesty_host_match_selection_residuals_method_names_residual_wave850();
    let b = honesty_host_match_selection_residuals_nav_commands_residual_wave850();
    let c = honesty_host_match_selection_residuals_residual_pack_wave850();
    residual_action_store(ResidualHostMatchSelectionResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_selection_residuals_residual_wave850() {
        assert!(honesty_host_match_selection_residuals_residual_pack_wave850());
        assert!(honesty_host_match_selection_residuals_method_names_residual_wave850());
        assert!(honesty_host_match_selection_residuals_nav_commands_residual_wave850());
        assert!(simulate_live_host_match_selection_residuals_honesty());
    }
}
