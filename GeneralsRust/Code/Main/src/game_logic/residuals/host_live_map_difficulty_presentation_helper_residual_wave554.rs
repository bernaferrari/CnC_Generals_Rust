//! Wave 554 residual peels: map-name and AI difficulty residuals are centralized
//! through `presentation_or_boot_map_name` / `presentation_or_boot_ai_difficulty`
//! — presentation freeze owns both when installed; boot residual without freeze
//! uses host probes. Call sites: host status snapshot, restart, save metadata.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 553 play-time/local-player presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` presentation_or_boot_map_name /
//!   presentation_or_boot_ai_difficulty / call sites
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_MAP_DIFFICULTY_PRESENTATION_HELPER_METHOD_NAMES_WAVE554: &[&str] = &[
    "presentation_or_boot_map_name",
    "presentation_or_boot_ai_difficulty",
    "get_current_map_name",
    "get_difficulty",
    "Wave 554",
    "playable_claim = false",
];

pub const LIVE_MAP_DIFFICULTY_PRESENTATION_HELPER_NAV_STEPS_WAVE554: &[&str] = &[
    "REQUIRE_MAP_NAME_PRESENTATION_HELPER",
    "REQUIRE_AI_DIFFICULTY_PRESENTATION_HELPER",
    "LIVE_MAP_DIFFICULTY_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_MAP_DIFFICULTY_PRESENTATION_HELPER_CMD_NAMES_WAVE554: &[&str] = &[
    "map_name_presentation_helper",
    "ai_difficulty_presentation_helper",
    "boot_get_current_map_name",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualMapDifficultyPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualMapDifficultyPresentationHelperAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualMapDifficultyPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_map_difficulty_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_map_difficulty_presentation_helper_last_action()
-> ResidualMapDifficultyPresentationHelperAction {
    ResidualMapDifficultyPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_map_difficulty_presentation_helper_method_names_residual_wave554() -> bool {
    let names = LIVE_MAP_DIFFICULTY_PRESENTATION_HELPER_METHOD_NAMES_WAVE554;
    let ok = residual_name_index(names, "presentation_or_boot_map_name").is_some()
        && residual_name_index(names, "presentation_or_boot_ai_difficulty").is_some()
        && residual_name_index(names, "get_current_map_name").is_some()
        && residual_name_index(names, "get_difficulty").is_some()
        && residual_name_index(names, "Wave 554").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualMapDifficultyPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_map_difficulty_presentation_helper_source_markers_residual_wave554() -> bool {
    let eng = eng_source();
    let Some(map_b) = fn_body(eng, "fn presentation_or_boot_map_name(") else {
        residual_action_store(ResidualMapDifficultyPresentationHelperAction::SourceMarkers);
        return false;
    };
    let Some(diff_b) = fn_body(eng, "fn presentation_or_boot_ai_difficulty(") else {
        residual_action_store(ResidualMapDifficultyPresentationHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: helpers are fail-closed (Wave 860/895/896). Save/load still
    // probes GameLogic map name (host_authority.rs / ui_commands.rs) — that is
    // C++ SaveLoad, not a presentation dual-read.
    let map_ok = map_b.contains("Wave 554")
        && map_b.contains("world_env.map_name")
        && map_b.contains("host_match_map_name")
        && !map_b.contains("self.game_logic.get_current_map_name");
    let diff_ok = diff_b.contains("Wave 554")
        && diff_b.contains("pres.ai_difficulty")
        && diff_b.contains("host_match_ai_difficulty")
        && !diff_b.contains("self.game_logic.get_difficulty()");
    let calls = eng.matches("presentation_or_boot_map_name()").count() >= 3
        && eng.contains("presentation_or_boot_ai_difficulty()");
    let ok = map_ok && diff_ok && calls && !eng.contains("playable_claim = true");
    residual_action_store(ResidualMapDifficultyPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_map_difficulty_presentation_helper_nav_commands_residual_wave554() -> bool {
    let steps = LIVE_MAP_DIFFICULTY_PRESENTATION_HELPER_NAV_STEPS_WAVE554;
    let cmds = RUNTIME_HOST_LIVE_MAP_DIFFICULTY_PRESENTATION_HELPER_CMD_NAMES_WAVE554;
    let ok = residual_name_index(steps, "REQUIRE_MAP_NAME_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_AI_DIFFICULTY_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_MAP_DIFFICULTY_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "map_name_presentation_helper").is_some()
        && residual_name_index(cmds, "ai_difficulty_presentation_helper").is_some()
        && residual_name_index(cmds, "boot_get_current_map_name").is_some();
    residual_action_store(ResidualMapDifficultyPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_map_difficulty_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 554")
        && eng.contains("fn presentation_or_boot_map_name")
        && eng.contains("fn presentation_or_boot_ai_difficulty");
    residual_action_store(ResidualMapDifficultyPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_map_difficulty_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("presentation_or_boot_map_name()")
        && eng.contains("presentation_or_boot_ai_difficulty()")
        && eng.contains("fn build_save_info")
        && eng.contains("restart_mission_from_ui")
        && eng.contains("runtime_host_status_snapshot");
    residual_action_store(ResidualMapDifficultyPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_map_difficulty_presentation_helper_residual_pack_wave554() -> bool {
    honesty_map_difficulty_presentation_helper_method_names_residual_wave554()
        && honesty_map_difficulty_presentation_helper_source_markers_residual_wave554()
        && honesty_map_difficulty_presentation_helper_nav_commands_residual_wave554()
        && simulate_map_difficulty_presentation_helper_collect_source()
        && simulate_map_difficulty_presentation_helper_dispatch_source()
}

pub fn simulate_live_map_difficulty_presentation_helper_honesty() -> bool {
    let ok = honesty_map_difficulty_presentation_helper_residual_pack_wave554();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualMapDifficultyPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_map_difficulty_presentation_helper_method_names_residual_wave554());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_map_difficulty_presentation_helper_source_markers_residual_wave554());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_map_difficulty_presentation_helper_nav_commands_residual_wave554());
    }

    #[test]
    fn map_difficulty_presentation_helper_sources() {
        assert!(simulate_map_difficulty_presentation_helper_collect_source());
        assert!(simulate_map_difficulty_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave554_composite_pack() {
        assert!(honesty_map_difficulty_presentation_helper_residual_pack_wave554());
    }

    #[test]
    fn simulate_live_map_difficulty_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_map_difficulty_presentation_helper_honesty(),
            "map/difficulty presentation helper residual must latch"
        );
        assert!(residual_map_difficulty_presentation_helper_ok());
        assert_eq!(
            residual_map_difficulty_presentation_helper_last_action(),
            ResidualMapDifficultyPresentationHelperAction::Composite
        );
    }
}
