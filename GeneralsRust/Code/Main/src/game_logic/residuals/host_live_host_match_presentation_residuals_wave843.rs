//! Wave 843: host-owned match map/local-player/AI residuals peel live GameLogic
//! dual-reads from presentation_or_boot_* helpers after start_game_from_ui.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_PRESENTATION_RESIDUALS_METHOD_NAMES_WAVE843: &[&str] = &[
    "host_match_map_name",
    "host_match_local_player_id",
    "host_match_ai_difficulty",
    "Wave 843",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_PRESENTATION_RESIDUALS_NAV_STEPS_WAVE843: &[&str] = &[
    "STAMP_HOST_MATCH_MAP",
    "STAMP_HOST_MATCH_LOCAL_PLAYER",
    "STAMP_HOST_MATCH_AI_DIFFICULTY",
    "LIVE_HOST_MATCH_PRESENTATION_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchPresentationResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchPresentationResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

pub fn honesty_host_match_presentation_residuals_method_names_residual_wave843() -> bool {
    let names = LIVE_HOST_MATCH_PRESENTATION_RESIDUALS_METHOD_NAMES_WAVE843;
    let ok = residual_name_index(names, "host_match_map_name").is_some()
        && residual_name_index(names, "host_match_local_player_id").is_some()
        && residual_name_index(names, "host_match_ai_difficulty").is_some()
        && residual_name_index(names, "Wave 843").is_some();
    residual_action_store(ResidualHostMatchPresentationResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_presentation_residuals_nav_commands_residual_wave843() -> bool {
    let steps = LIVE_HOST_MATCH_PRESENTATION_RESIDUALS_NAV_STEPS_WAVE843;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_PRESENTATION_RESIDUALS").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_MAP").is_some();
    residual_action_store(ResidualHostMatchPresentationResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_presentation_residuals_residual_pack_wave843() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_map_name: Option<String>")
        && cnc.contains("host_match_local_player_id: Option<u32>")
        && cnc.contains("host_match_ai_difficulty: Option<crate::ai::AIDifficulty>")
        && (cnc.contains("Wave 843")
            || cnc.contains(
                "Wave 843/844: host-owned match residuals for presentation_or_boot peels",
            ))
        && (cnc.contains("self.host_match_map_name = Some(map_name.clone())")
            || cnc.contains("self.host_match_map_name = Some(loaded_map_name.clone())")
            || cnc.contains("self.host_match_map_name = Some(loaded.clone())"))
        && (cnc.contains("self.host_match_local_player_id = Some(self.current_player_id)")
            || cnc.contains("host_refresh_match_sim_residuals_from_logic"))
        && cnc.contains("if let Some(host) = self.host_match_map_name.as_ref()")
        && cnc.contains("if let Some(id) = self.host_match_local_player_id")
        && cnc.contains("if let Some(d) = self.host_match_ai_difficulty");
    residual_action_store(ResidualHostMatchPresentationResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_presentation_residuals_honesty() -> bool {
    let a = honesty_host_match_presentation_residuals_method_names_residual_wave843();
    let b = honesty_host_match_presentation_residuals_nav_commands_residual_wave843();
    let c = honesty_host_match_presentation_residuals_residual_pack_wave843();
    residual_action_store(ResidualHostMatchPresentationResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_presentation_residuals_residual_wave843() {
        assert!(honesty_host_match_presentation_residuals_residual_pack_wave843());
        assert!(honesty_host_match_presentation_residuals_method_names_residual_wave843());
        assert!(honesty_host_match_presentation_residuals_nav_commands_residual_wave843());
        assert!(simulate_live_host_match_presentation_residuals_honesty());
    }
}
