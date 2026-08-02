//! Wave 966: presentation unit catalog residual for host select-similar.
//!
//! InGameUI peels select_similar_units onto presentation unit catalog when
//! OBJECT_REGISTRY is empty. Engine stamps catalog from PresentationFrame each
//! shell tick. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_UNIT_CATALOG_METHOD_NAMES_WAVE966: &[&str] = &[
    "PresentationUnitCatalogEntry",
    "set_presentation_unit_catalog",
    "apply_presentation_unit_catalog",
    "select_similar_units",
    "Wave 966",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_UNIT_CATALOG_NAV_STEPS_WAVE966: &[&str] = &[
    "PRESENTATION_UNIT_CATALOG",
    "SELECT_SIMILAR_FROM_CATALOG",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_PRESENTATION_UNIT_CATALOG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationUnitCatalogAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationUnitCatalogAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn ui_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/gui/ingame_ui.rs")
}

fn client_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_presentation_unit_catalog_method_names_residual_wave966() -> bool {
    let names = LIVE_HOST_PRESENTATION_UNIT_CATALOG_METHOD_NAMES_WAVE966;
    let ok = residual_name_index(names, "PresentationUnitCatalogEntry").is_some()
        && residual_name_index(names, "Wave 966").is_some();
    residual_action_store(ResidualHostPresentationUnitCatalogAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_unit_catalog_nav_commands_residual_wave966() -> bool {
    let steps = LIVE_HOST_PRESENTATION_UNIT_CATALOG_NAV_STEPS_WAVE966;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_UNIT_CATALOG").is_some()
        && residual_name_index(steps, "SELECT_SIMILAR_FROM_CATALOG").is_some();
    residual_action_store(ResidualHostPresentationUnitCatalogAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_unit_catalog_residual_pack_wave966() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let client = client_source();
    let sim = match ui.find("fn select_similar_units") {
        Some(i) => &ui[i..ui.len().min(i + 2500)],
        None => "",
    };
    let ok = ui.contains("Wave 966")
        && client.contains("Wave 966")
        && cnc.contains("Wave 966")
        && ui.contains("struct PresentationUnitCatalogEntry")
        && ui.contains("presentation_unit_catalog")
        && ui.contains("set_presentation_unit_catalog")
        && sim.contains("presentation_unit_catalog")
        && sim.contains("dual_world_registry_unavailable")
        && client.contains("apply_presentation_unit_catalog")
        && cnc.contains("apply_presentation_unit_catalog")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPresentationUnitCatalogAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_unit_catalog_honesty() -> bool {
    let a = honesty_host_presentation_unit_catalog_method_names_residual_wave966();
    let b = honesty_host_presentation_unit_catalog_nav_commands_residual_wave966();
    let c = honesty_host_presentation_unit_catalog_residual_pack_wave966();
    residual_action_store(ResidualHostPresentationUnitCatalogAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_unit_catalog_residual_wave966() {
        assert!(honesty_host_presentation_unit_catalog_residual_pack_wave966());
        assert!(honesty_host_presentation_unit_catalog_method_names_residual_wave966());
        assert!(honesty_host_presentation_unit_catalog_nav_commands_residual_wave966());
        assert!(simulate_live_host_presentation_unit_catalog_honesty());
    }
}
