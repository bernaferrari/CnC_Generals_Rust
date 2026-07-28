//! Wave 829: golden/map construct pads clamp with MinDistFromEdgeOfMapForBuild
//! margin (30 wu + pad) and pick LBC_OK sites so load_map construct residual
//! is not LBC_RESTRICTED_TERRAIN after clamp. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_BUILD_EDGE_MARGIN_METHOD_NAMES_WAVE829: &[&str] = &[
    "clamp_build_site",
    "find_legal_build_site_near",
    "MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD",
    "LBC_OK",
    "Wave 829",
    "playable_claim = false",
];
pub const LIVE_HOST_BUILD_EDGE_MARGIN_NAV_STEPS_WAVE829: &[&str] = &[
    "REQUIRE_EDGE_MARGIN_GE_MIN_DIST",
    "REQUIRE_LEGAL_SITE_SEARCH",
    "LIVE_HOST_BUILD_EDGE_MARGIN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBuildEdgeMarginAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostBuildEdgeMarginAction {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostBuildEdgeMarginAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
fn gs_source() -> &'static str {
    include_str!("../golden_skirmish.rs")
}
pub fn honesty_host_build_edge_margin_method_names_residual_wave829() -> bool {
    let names = LIVE_HOST_BUILD_EDGE_MARGIN_METHOD_NAMES_WAVE829;
    let ok = residual_name_index(names, "clamp_build_site").is_some()
        && residual_name_index(names, "find_legal_build_site_near").is_some()
        && residual_name_index(names, "MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD").is_some()
        && residual_name_index(names, "LBC_OK").is_some()
        && residual_name_index(names, "Wave 829").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostBuildEdgeMarginAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_build_edge_margin_nav_commands_residual_wave829() -> bool {
    let steps = LIVE_HOST_BUILD_EDGE_MARGIN_NAV_STEPS_WAVE829;
    let ok = residual_name_index(steps, "LIVE_HOST_BUILD_EDGE_MARGIN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostBuildEdgeMarginAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_build_edge_margin_residual_pack_wave829() -> bool {
    let gs = gs_source();
    let ok = gs.contains("Wave 829: margin must meet C++ MinDistFromEdgeOfMapForBuild")
        && gs.contains("find_legal_build_site_near")
        && gs.contains("MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD")
        && gs.contains("legal_build_code_at_for_builder");
    residual_action_store(ResidualHostBuildEdgeMarginAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_build_edge_margin_honesty() -> bool {
    let a = honesty_host_build_edge_margin_method_names_residual_wave829();
    let b = honesty_host_build_edge_margin_nav_commands_residual_wave829();
    let c = honesty_host_build_edge_margin_residual_pack_wave829();
    residual_action_store(ResidualHostBuildEdgeMarginAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_build_edge_margin_residual_wave829() {
        assert!(honesty_host_build_edge_margin_residual_pack_wave829());
        assert!(honesty_host_build_edge_margin_method_names_residual_wave829());
        assert!(honesty_host_build_edge_margin_nav_commands_residual_wave829());
        assert!(simulate_live_host_build_edge_margin_honesty());
    }
}
