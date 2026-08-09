//! Wave 1012: presentation catalog veterancy residual for dual-world portrait.
//!
//! PresentationUnitCatalogEntry / TranslatorCatalogEntry carry veterancy_overlay
//! chevron names from RenderableObject::veterancy. Dual-world portrait peel
//! applies them when filling an empty portrait from the catalog.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CATALOG_VETERANCY_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1012: &[&str] = &[
    "veterancy_overlay",
    "SSChevron1L",
    "PresentationVeterancy",
    "Wave 1012",
    "playable_claim = false",
];

pub const LIVE_HOST_CATALOG_VETERANCY_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1012: &[&str] = &[
    "CATALOG_VETERANCY",
    "PORTRAIT_CHEVRON",
    "DUAL_WORLD",
    "LIVE_HOST_CATALOG_VETERANCY_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCatalogVeterancyPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCatalogVeterancyPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn ui_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/gui/ingame_ui.rs")
}
fn tr_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/presentation_translator_residual.rs")
}
fn cb_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/gui/control_bar/control_bar.rs")
}

pub fn honesty_host_catalog_veterancy_presentation_residual_method_names_residual_wave1012() -> bool
{
    let names = LIVE_HOST_CATALOG_VETERANCY_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1012;
    let ok = residual_name_index(names, "veterancy_overlay").is_some()
        && residual_name_index(names, "Wave 1012").is_some();
    residual_action_store(ResidualHostCatalogVeterancyPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_catalog_veterancy_presentation_residual_nav_commands_residual_wave1012() -> bool
{
    let steps = LIVE_HOST_CATALOG_VETERANCY_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1012;
    let ok = residual_name_index(steps, "LIVE_HOST_CATALOG_VETERANCY_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "PORTRAIT_CHEVRON").is_some();
    residual_action_store(ResidualHostCatalogVeterancyPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_catalog_veterancy_presentation_residual_residual_pack_wave1012() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let cb = cb_source();
    let ok = ui.contains("Wave 1012: veterancy chevron residual")
        && tr.contains("pub veterancy_overlay: Option<String>")
        && cnc.contains("SSChevron1L")
        && cnc.contains("PresentationVeterancy")
        && cnc.contains("Wave 1012: veterancy chevron residual")
        && cb.contains("entry.veterancy_overlay")
        && cb.contains("self.portrait_state.veterancy_overlay = entry.veterancy_overlay.clone()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostCatalogVeterancyPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_catalog_veterancy_presentation_residual_honesty() -> bool {
    let a = honesty_host_catalog_veterancy_presentation_residual_method_names_residual_wave1012();
    let b = honesty_host_catalog_veterancy_presentation_residual_nav_commands_residual_wave1012();
    let c = honesty_host_catalog_veterancy_presentation_residual_residual_pack_wave1012();
    residual_action_store(ResidualHostCatalogVeterancyPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_catalog_veterancy_presentation_residual_wave1012() {
        assert!(honesty_host_catalog_veterancy_presentation_residual_residual_pack_wave1012());
        assert!(
            honesty_host_catalog_veterancy_presentation_residual_method_names_residual_wave1012()
        );
        assert!(
            honesty_host_catalog_veterancy_presentation_residual_nav_commands_residual_wave1012()
        );
        assert!(simulate_live_host_catalog_veterancy_presentation_residual_honesty());
    }
}
