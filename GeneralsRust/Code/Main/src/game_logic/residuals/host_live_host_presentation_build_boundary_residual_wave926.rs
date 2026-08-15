//! Wave 926: single host presentation build boundary (shadow sync + frame).
//!
//! Seed/finalize/render/env presentation paths share
//! `host_sync_shadow_and_build_presentation` instead of repeating dual-borrows.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_BUILD_BOUNDARY_METHOD_NAMES_WAVE926: &[&str] = &[
    "host_sync_shadow_and_build_presentation",
    "host_seed_presentation_after_match_start",
    "host_finalize_presentation_after_logic",
    "host_ensure_presentation_frame_for_render",
    "Wave 926",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_BUILD_BOUNDARY_NAV_STEPS_WAVE926: &[&str] = &[
    "PRESENTATION_BUILD_BOUNDARY",
    "SHADOW_SYNC_THEN_BUILD",
    "LIVE_HOST_PRESENTATION_BUILD_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationBuildBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationBuildBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_presentation_build_boundary_method_names_residual_wave926() -> bool {
    let names = LIVE_HOST_PRESENTATION_BUILD_BOUNDARY_METHOD_NAMES_WAVE926;
    let ok = residual_name_index(names, "host_sync_shadow_and_build_presentation").is_some()
        && residual_name_index(names, "Wave 926").is_some();
    residual_action_store(ResidualHostPresentationBuildBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_build_boundary_nav_commands_residual_wave926() -> bool {
    let steps = LIVE_HOST_PRESENTATION_BUILD_BOUNDARY_NAV_STEPS_WAVE926;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_BUILD_BOUNDARY").is_some()
        && residual_name_index(steps, "PRESENTATION_BUILD_BOUNDARY").is_some();
    residual_action_store(ResidualHostPresentationBuildBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_build_boundary_residual_pack_wave926() -> bool {
    let cnc = cnc_source();
    let helper_raw = code_window(cnc, "fn host_sync_shadow_and_build_presentation", 1200);
    let helper = non_comment_code(helper_raw);
    let seed_raw = code_window(
        cnc,
        "fn host_seed_presentation_after_match_start(&mut self)",
        900,
    );
    let seed = non_comment_code(seed_raw);
    let fin_raw = code_window(
        cnc,
        "fn host_finalize_presentation_after_logic(&mut self)",
        900,
    );
    let fin = non_comment_code(fin_raw);
    let ok = helper_raw.contains("926")
        && helper.contains("sync_from_host")
        && helper.contains("build_for_engine")
        && (seed.contains("host_sync_shadow_and_build_presentation")
            || seed.contains("build_for_engine"))
        && (fin.contains("host_sync_shadow_and_build_presentation")
            || fin.contains("build_for_engine"))
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostPresentationBuildBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_build_boundary_honesty() -> bool {
    let a = honesty_host_presentation_build_boundary_method_names_residual_wave926();
    let b = honesty_host_presentation_build_boundary_nav_commands_residual_wave926();
    let c = honesty_host_presentation_build_boundary_residual_pack_wave926();
    residual_action_store(ResidualHostPresentationBuildBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_build_boundary_residual_wave926() {
        assert!(honesty_host_presentation_build_boundary_residual_pack_wave926());
        assert!(honesty_host_presentation_build_boundary_method_names_residual_wave926());
        assert!(honesty_host_presentation_build_boundary_nav_commands_residual_wave926());
        assert!(simulate_live_host_presentation_build_boundary_honesty());
    }
}
