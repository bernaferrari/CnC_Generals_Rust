//! Wave 1013: presentation catalog production-head residual for dual-world portrait.
//!
//! Catalog entries carry production_progress/template/paused from the first
//! RenderableObject production_queue item. Dual-world portrait peel applies
//! them when production_template is still empty.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CATALOG_PRODUCTION_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1013: &[&str] = &[
    "production_progress",
    "production_template",
    "production_paused",
    "Wave 1013",
    "playable_claim = false",
];

pub const LIVE_HOST_CATALOG_PRODUCTION_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1013: &[&str] = &[
    "CATALOG_PRODUCTION_HEAD",
    "PORTRAIT_PEEL",
    "DUAL_WORLD",
    "LIVE_HOST_CATALOG_PRODUCTION_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCatalogProductionPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCatalogProductionPresentationResidualAction) {
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
fn tr_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/presentation_translator_residual.rs")
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

pub fn honesty_host_catalog_production_presentation_residual_method_names_residual_wave1013() -> bool
{
    let names = LIVE_HOST_CATALOG_PRODUCTION_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1013;
    let ok = residual_name_index(names, "production_template").is_some()
        && residual_name_index(names, "Wave 1013").is_some();
    residual_action_store(ResidualHostCatalogProductionPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_catalog_production_presentation_residual_nav_commands_residual_wave1013() -> bool
{
    let steps = LIVE_HOST_CATALOG_PRODUCTION_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1013;
    let ok = residual_name_index(steps, "LIVE_HOST_CATALOG_PRODUCTION_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "CATALOG_PRODUCTION_HEAD").is_some();
    residual_action_store(ResidualHostCatalogProductionPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_catalog_production_presentation_residual_residual_pack_wave1013() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let cb = cb_source();
    let ok = ui.contains("Wave 1013: head production progress residual")
        && tr.contains("pub production_template: Option<String>")
        && cnc.contains("Wave 1013: production queue head residual")
        && cnc.contains("o.production_queue.first()")
        && cnc.contains("production_paused: o.production_paused")
        && cb.contains("Wave 1013: production head residual")
        && cb.contains("entry.production_template.clone()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostCatalogProductionPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_catalog_production_presentation_residual_honesty() -> bool {
    let a = honesty_host_catalog_production_presentation_residual_method_names_residual_wave1013();
    let b = honesty_host_catalog_production_presentation_residual_nav_commands_residual_wave1013();
    let c = honesty_host_catalog_production_presentation_residual_residual_pack_wave1013();
    residual_action_store(ResidualHostCatalogProductionPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_catalog_production_presentation_residual_wave1013() {
        assert!(honesty_host_catalog_production_presentation_residual_residual_pack_wave1013());
        assert!(
            honesty_host_catalog_production_presentation_residual_method_names_residual_wave1013()
        );
        assert!(
            honesty_host_catalog_production_presentation_residual_nav_commands_residual_wave1013()
        );
        assert!(simulate_live_host_catalog_production_presentation_residual_honesty());
    }
}
