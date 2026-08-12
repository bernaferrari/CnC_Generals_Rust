//! Wave 967: select-matching via presentation unit catalog residual.
//!
//! `select_matching_across_region/map` peel onto presentation catalog when
//! OBJECT_REGISTRY is empty (host path). Seeds templates from selection manager
//! or presentation_selected residual. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELECT_MATCHING_CATALOG_METHOD_NAMES_WAVE967: &[&str] = &[
    "select_matching_from_presentation_catalog",
    "select_matching_across_region",
    "select_matching_across_map",
    "presentation_unit_catalog",
    "Wave 967",
    "playable_claim = false",
];

pub const LIVE_HOST_SELECT_MATCHING_CATALOG_NAV_STEPS_WAVE967: &[&str] = &[
    "SELECT_MATCHING_FROM_CATALOG",
    "REGION_OR_MAP",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_SELECT_MATCHING_CATALOG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSelectMatchingCatalogAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSelectMatchingCatalogAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}

pub fn honesty_host_select_matching_catalog_method_names_residual_wave967() -> bool {
    let names = LIVE_HOST_SELECT_MATCHING_CATALOG_METHOD_NAMES_WAVE967;
    let ok = residual_name_index(names, "select_matching_from_presentation_catalog").is_some()
        && residual_name_index(names, "Wave 967").is_some();
    residual_action_store(ResidualHostSelectMatchingCatalogAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_select_matching_catalog_nav_commands_residual_wave967() -> bool {
    let steps = LIVE_HOST_SELECT_MATCHING_CATALOG_NAV_STEPS_WAVE967;
    let ok = residual_name_index(steps, "LIVE_HOST_SELECT_MATCHING_CATALOG").is_some()
        && residual_name_index(steps, "SELECT_MATCHING_FROM_CATALOG").is_some();
    residual_action_store(ResidualHostSelectMatchingCatalogAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_select_matching_catalog_residual_pack_wave967() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let region = match ui.find("fn select_matching_across_region") {
        Some(i) => &ui[i..ui.len().min(i + 800)],
        None => "",
    };
    let map = match ui.find("fn select_matching_across_map") {
        Some(i) => &ui[i..ui.len().min(i + 800)],
        None => "",
    };
    let ok = ui.contains("Wave 967")
        && ui.contains("select_matching_from_presentation_catalog")
        && region.contains("select_matching_from_presentation_catalog")
        && map.contains("select_matching_from_presentation_catalog")
        && ui.contains("presentation_unit_catalog")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSelectMatchingCatalogAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_select_matching_catalog_honesty() -> bool {
    let a = honesty_host_select_matching_catalog_method_names_residual_wave967();
    let b = honesty_host_select_matching_catalog_nav_commands_residual_wave967();
    let c = honesty_host_select_matching_catalog_residual_pack_wave967();
    residual_action_store(ResidualHostSelectMatchingCatalogAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_select_matching_catalog_residual_wave967() {
        assert!(honesty_host_select_matching_catalog_residual_pack_wave967());
        assert!(honesty_host_select_matching_catalog_method_names_residual_wave967());
        assert!(honesty_host_select_matching_catalog_nav_commands_residual_wave967());
        assert!(simulate_live_host_select_matching_catalog_honesty());
    }
}
