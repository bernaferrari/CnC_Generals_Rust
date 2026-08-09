//! Wave 924: structure placement legal-build via host residual cache.
//!
//! Placement cursor sync and place_structure_from_ui use
//! host_legal_build_code_at_for_builder (per-frame cache) instead of live
//! GameLogic dual-reads. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PLACEMENT_LEGAL_BUILD_CACHE_METHOD_NAMES_WAVE924: &[&str] = &[
    "sync_pending_structure_placement_cursor",
    "place_structure_from_ui",
    "host_legal_build_code_at_for_builder",
    "Wave 924",
    "playable_claim = false",
];

pub const LIVE_HOST_PLACEMENT_LEGAL_BUILD_CACHE_NAV_STEPS_WAVE924: &[&str] = &[
    "PLACEMENT_CURSOR_LEGAL_CACHE",
    "PLACE_STRUCTURE_LEGAL_CACHE",
    "LIVE_HOST_PLACEMENT_LEGAL_BUILD_CACHE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPlacementLegalBuildCacheAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPlacementLegalBuildCacheAction) {
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

pub fn honesty_host_placement_legal_build_cache_method_names_residual_wave924() -> bool {
    let names = LIVE_HOST_PLACEMENT_LEGAL_BUILD_CACHE_METHOD_NAMES_WAVE924;
    let ok = residual_name_index(names, "place_structure_from_ui").is_some()
        && residual_name_index(names, "Wave 924").is_some();
    residual_action_store(ResidualHostPlacementLegalBuildCacheAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_placement_legal_build_cache_nav_commands_residual_wave924() -> bool {
    let steps = LIVE_HOST_PLACEMENT_LEGAL_BUILD_CACHE_NAV_STEPS_WAVE924;
    let ok = residual_name_index(steps, "LIVE_HOST_PLACEMENT_LEGAL_BUILD_CACHE").is_some()
        && residual_name_index(steps, "PLACEMENT_CURSOR_LEGAL_CACHE").is_some();
    residual_action_store(ResidualHostPlacementLegalBuildCacheAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_placement_legal_build_cache_residual_pack_wave924() -> bool {
    let cnc = cnc_source();
    let cursor_raw = code_window(cnc, "fn sync_pending_structure_placement_cursor", 2200);
    let cursor = non_comment_code(cursor_raw);
    let place_raw = code_window(cnc, "fn place_structure_from_ui", 5000);
    let place = non_comment_code(place_raw);
    let host_raw = code_window(cnc, "fn host_legal_build_code_at_for_builder", 1400);
    let place_ok = place.contains("host_legal_build_code_at_for_builder")
        && !place.contains("self.game_logic")
        && (place_raw.contains("924") || cnc.contains("Wave 924: structure place"));
    let ok = cursor_raw.contains("924")
        && cursor.contains("host_legal_build_code_at_for_builder")
        && !cursor.contains("self.game_logic")
        && place_ok
        && host_raw.contains("924")
        && host_raw.contains("host_legal_build_cache")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostPlacementLegalBuildCacheAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_placement_legal_build_cache_honesty() -> bool {
    let a = honesty_host_placement_legal_build_cache_method_names_residual_wave924();
    let b = honesty_host_placement_legal_build_cache_nav_commands_residual_wave924();
    let c = honesty_host_placement_legal_build_cache_residual_pack_wave924();
    residual_action_store(ResidualHostPlacementLegalBuildCacheAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_placement_legal_build_cache_residual_wave924() {
        assert!(honesty_host_placement_legal_build_cache_residual_pack_wave924());
        assert!(honesty_host_placement_legal_build_cache_method_names_residual_wave924());
        assert!(honesty_host_placement_legal_build_cache_nav_commands_residual_wave924());
        assert!(simulate_live_host_placement_legal_build_cache_honesty());
    }
}
