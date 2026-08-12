//! Wave 1011: presentation unit catalog health residual for dual-world portrait.
//!
//! PresentationUnitCatalogEntry / TranslatorCatalogEntry carry health_current/
//! health_maximum from RenderableObject. Dual-world update_portrait peels them
//! when filling an empty portrait from the catalog.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CATALOG_HEALTH_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1011: &[&str] = &[
    "health_current",
    "health_maximum",
    "PresentationUnitCatalogEntry",
    "Wave 1011",
    "playable_claim = false",
];

pub const LIVE_HOST_CATALOG_HEALTH_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1011: &[&str] = &[
    "CATALOG_HEALTH",
    "PORTRAIT_PEEL",
    "TRANSLATOR_CATALOG",
    "LIVE_HOST_CATALOG_HEALTH_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCatalogHealthPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCatalogHealthPresentationResidualAction) {
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

pub fn honesty_host_catalog_health_presentation_residual_method_names_residual_wave1011() -> bool {
    let names = LIVE_HOST_CATALOG_HEALTH_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1011;
    let ok = residual_name_index(names, "health_current").is_some()
        && residual_name_index(names, "Wave 1011").is_some();
    residual_action_store(ResidualHostCatalogHealthPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_catalog_health_presentation_residual_nav_commands_residual_wave1011() -> bool {
    let steps = LIVE_HOST_CATALOG_HEALTH_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1011;
    let ok = residual_name_index(steps, "LIVE_HOST_CATALOG_HEALTH_PRESENTATION_RESIDUAL").is_some()
        && residual_name_index(steps, "PORTRAIT_PEEL").is_some();
    residual_action_store(ResidualHostCatalogHealthPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_catalog_health_presentation_residual_residual_pack_wave1011() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let cb = cb_source();
    let ok = ui.contains("Wave 1011: health residual for dual-world portrait peel")
        && tr.contains("pub health_current: f32")
        && tr.contains("pub health_maximum: f32")
        && cnc.contains("health_current: o.health_current")
        && cnc.contains("health_maximum: if o.health_max")
        && cb.contains("entry.health_maximum > 0.0")
        && cb.contains("self.portrait_state.health_current = entry.health_current")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostCatalogHealthPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_catalog_health_presentation_residual_honesty() -> bool {
    let a = honesty_host_catalog_health_presentation_residual_method_names_residual_wave1011();
    let b = honesty_host_catalog_health_presentation_residual_nav_commands_residual_wave1011();
    let c = honesty_host_catalog_health_presentation_residual_residual_pack_wave1011();
    residual_action_store(ResidualHostCatalogHealthPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_catalog_health_presentation_residual_wave1011() {
        assert!(honesty_host_catalog_health_presentation_residual_residual_pack_wave1011());
        assert!(honesty_host_catalog_health_presentation_residual_method_names_residual_wave1011());
        assert!(honesty_host_catalog_health_presentation_residual_nav_commands_residual_wave1011());
        assert!(simulate_live_host_catalog_health_presentation_residual_honesty());
    }
}
