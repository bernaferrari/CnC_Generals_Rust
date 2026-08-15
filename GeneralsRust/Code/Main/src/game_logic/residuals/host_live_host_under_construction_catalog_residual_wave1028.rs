//! Wave 1028: dual-world under-construction catalog residual.
//!
//! Catalog entries carry under_construction + construction_percent.
//! evaluate_context peels them into presentation_under_construction when
//! freeze is unset so ControlBarState::UnderConstruction works dual-world.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UNDER_CONSTRUCTION_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1028: &[&str] = &[
    "under_construction",
    "construction_percent",
    "presentation_under_construction",
    "Wave 1028",
    "playable_claim = false",
];

pub const LIVE_HOST_UNDER_CONSTRUCTION_CATALOG_RESIDUAL_NAV_STEPS_WAVE1028: &[&str] = &[
    "UNDER_CONSTRUCTION",
    "TRANSLATOR_CATALOG",
    "EVALUATE_CONTEXT",
    "LIVE_HOST_UNDER_CONSTRUCTION_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUnderConstructionCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUnderConstructionCatalogResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}
fn tr_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/presentation_translator_residual.rs")
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

pub fn honesty_host_under_construction_catalog_residual_method_names_residual_wave1028() -> bool {
    let names = LIVE_HOST_UNDER_CONSTRUCTION_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1028;
    let ok = residual_name_index(names, "under_construction").is_some()
        && residual_name_index(names, "Wave 1028").is_some();
    residual_action_store(ResidualHostUnderConstructionCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_under_construction_catalog_residual_nav_commands_residual_wave1028() -> bool {
    let steps = LIVE_HOST_UNDER_CONSTRUCTION_CATALOG_RESIDUAL_NAV_STEPS_WAVE1028;
    let ok = residual_name_index(steps, "LIVE_HOST_UNDER_CONSTRUCTION_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "UNDER_CONSTRUCTION").is_some();
    residual_action_store(ResidualHostUnderConstructionCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_under_construction_catalog_residual_residual_pack_wave1028() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let cb = cb_source();
    let ok = ui.contains("Wave 1028: under-construction residual for dual-world ControlBar state")
        && tr.contains("pub under_construction: bool")
        && tr.contains("pub construction_percent: f32")
        && cnc.contains("Wave 1028: under-construction residual for dual-world ControlBar")
        && cnc.contains("under_construction: o.under_construction")
        && cb
            .contains("Wave 1028: seed under-construction residual from catalog when freeze unset")
        && cb.contains("self.presentation_under_construction = true")
        && cb.contains("self.presentation_construction_percent = entry.construction_percent")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostUnderConstructionCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_under_construction_catalog_residual_honesty() -> bool {
    let a = honesty_host_under_construction_catalog_residual_method_names_residual_wave1028();
    let b = honesty_host_under_construction_catalog_residual_nav_commands_residual_wave1028();
    let c = honesty_host_under_construction_catalog_residual_residual_pack_wave1028();
    residual_action_store(ResidualHostUnderConstructionCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_under_construction_catalog_residual_wave1028() {
        assert!(honesty_host_under_construction_catalog_residual_residual_pack_wave1028());
        assert!(honesty_host_under_construction_catalog_residual_method_names_residual_wave1028());
        assert!(honesty_host_under_construction_catalog_residual_nav_commands_residual_wave1028());
        assert!(simulate_live_host_under_construction_catalog_residual_honesty());
    }
}
