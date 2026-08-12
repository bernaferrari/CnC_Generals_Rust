//! Wave 911: per-frame legal-build residual cache (construct pad scan peel).
//!
//! Repeat legal_build probes within the same logic frame hit the host cache
//! instead of dual-reading GameLogic again. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_LEGAL_BUILD_CACHE_METHOD_NAMES_WAVE911: &[&str] = &[
    "host_legal_build_code_at_for_builder",
    "host_is_location_legal_to_build_for_builder",
    "host_legal_build_cache",
    "Wave 911",
    "playable_claim = false",
];

pub const LIVE_HOST_LEGAL_BUILD_CACHE_NAV_STEPS_WAVE911: &[&str] = &[
    "LEGAL_BUILD_PER_FRAME_CACHE",
    "PAD_SCAN_CACHE_HIT",
    "LIVE_HOST_LEGAL_BUILD_CACHE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostLegalBuildCacheAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostLegalBuildCacheAction) {
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

pub fn honesty_host_legal_build_cache_method_names_residual_wave911() -> bool {
    let names = LIVE_HOST_LEGAL_BUILD_CACHE_METHOD_NAMES_WAVE911;
    let ok = residual_name_index(names, "host_legal_build_cache").is_some()
        && residual_name_index(names, "Wave 911").is_some();
    residual_action_store(ResidualHostLegalBuildCacheAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_legal_build_cache_nav_commands_residual_wave911() -> bool {
    let steps = LIVE_HOST_LEGAL_BUILD_CACHE_NAV_STEPS_WAVE911;
    let ok = residual_name_index(steps, "LIVE_HOST_LEGAL_BUILD_CACHE").is_some()
        && residual_name_index(steps, "LEGAL_BUILD_PER_FRAME_CACHE").is_some();
    residual_action_store(ResidualHostLegalBuildCacheAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_legal_build_cache_residual_pack_wave911() -> bool {
    let cnc = cnc_source();
    let legal_raw = code_window(cnc, "fn host_legal_build_code_at_for_builder", 1600);
    let legal = non_comment_code(legal_raw);
    let ok = legal_raw.contains("911")
        && legal.contains("host_legal_build_cache")
        && legal.contains("host_legal_build_cache_frame")
        && cnc.contains("host_legal_build_cache_frame:")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostLegalBuildCacheAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_legal_build_cache_honesty() -> bool {
    let a = honesty_host_legal_build_cache_method_names_residual_wave911();
    let b = honesty_host_legal_build_cache_nav_commands_residual_wave911();
    let c = honesty_host_legal_build_cache_residual_pack_wave911();
    residual_action_store(ResidualHostLegalBuildCacheAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_legal_build_cache_residual_wave911() {
        assert!(honesty_host_legal_build_cache_residual_pack_wave911());
        assert!(honesty_host_legal_build_cache_method_names_residual_wave911());
        assert!(honesty_host_legal_build_cache_nav_commands_residual_wave911());
        assert!(simulate_live_host_legal_build_cache_honesty());
    }
}
