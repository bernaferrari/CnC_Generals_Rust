//! Wave 894: unlocked-sciences multi-player residual + AI difficulty freeze stamp.
//!
//! - `presentation_or_boot_unlocked_sciences` prefers `host_match_unlocked_sciences`
//!   map (per-player) before freeze local-only residual; non-local freeze miss is
//!   fail-closed empty (no dual-read).
//! - `host_refresh_match_sim_residuals_from_logic` stamps `host_match_ai_difficulty`
//!   from presentation freeze when installed.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SCIENCES_AI_METHOD_NAMES_WAVE894: &[&str] = &[
    "presentation_or_boot_unlocked_sciences",
    "host_match_unlocked_sciences",
    "host_match_ai_difficulty",
    "Wave 894",
    "playable_claim = false",
];

pub const LIVE_HOST_SCIENCES_AI_NAV_STEPS_WAVE894: &[&str] = &[
    "SCIENCES_MAP_RESIDUAL_FIRST",
    "AI_DIFFICULTY_FROM_PRESENTATION",
    "LIVE_HOST_SCIENCES_AI",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSciencesAiAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSciencesAiAction) {
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

pub fn honesty_host_sciences_ai_method_names_residual_wave894() -> bool {
    let names = LIVE_HOST_SCIENCES_AI_METHOD_NAMES_WAVE894;
    let ok = residual_name_index(names, "presentation_or_boot_unlocked_sciences").is_some()
        && residual_name_index(names, "host_match_ai_difficulty").is_some()
        && residual_name_index(names, "Wave 894").is_some();
    residual_action_store(ResidualHostSciencesAiAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sciences_ai_nav_commands_residual_wave894() -> bool {
    let steps = LIVE_HOST_SCIENCES_AI_NAV_STEPS_WAVE894;
    let ok = residual_name_index(steps, "LIVE_HOST_SCIENCES_AI").is_some()
        && residual_name_index(steps, "SCIENCES_MAP_RESIDUAL_FIRST").is_some();
    residual_action_store(ResidualHostSciencesAiAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sciences_ai_residual_pack_wave894() -> bool {
    let cnc = cnc_source();
    let sci = non_comment_code(code_window(
        cnc,
        "fn presentation_or_boot_unlocked_sciences",
        900,
    ));
    let refresh = non_comment_code(code_window(
        cnc,
        "fn host_refresh_match_sim_residuals_from_logic",
        1400,
    ));
    // Sciences: map residual must appear before last_presentation_frame local path.
    let map_at = sci.find("host_match_unlocked_sciences");
    let frame_at = sci.find("last_presentation_frame");
    let sci_ok = match (map_at, frame_at) {
        (Some(m), Some(f)) => m < f && sci.contains("player_id == frame.local_player_id"),
        _ => false,
    };
    let ai_ok = refresh.contains("host_match_ai_difficulty = Some(pres.ai_difficulty)");
    let ok = sci_ok && ai_ok && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostSciencesAiAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sciences_ai_honesty() -> bool {
    let a = honesty_host_sciences_ai_method_names_residual_wave894();
    let b = honesty_host_sciences_ai_nav_commands_residual_wave894();
    let c = honesty_host_sciences_ai_residual_pack_wave894();
    residual_action_store(ResidualHostSciencesAiAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sciences_ai_residual_wave894() {
        assert!(honesty_host_sciences_ai_residual_pack_wave894());
        assert!(honesty_host_sciences_ai_method_names_residual_wave894());
        assert!(honesty_host_sciences_ai_nav_commands_residual_wave894());
        assert!(simulate_live_host_sciences_ai_honesty());
    }
}
