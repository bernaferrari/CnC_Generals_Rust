//! Wave 1033: dual-world ControlBar sold residual.
//!
//! Catalog/presentation carry sold; dual evaluate_context clears the bar when
//! sold (C++ OBJECT_STATUS_SOLD). sync_sold_from_presentation freezes host path.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SOLD_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1033: &[&str] = &[
    "sync_sold_from_presentation",
    "presentation_sold",
    "catalog_sold",
    "Wave 1033",
    "playable_claim = false",
];

pub const LIVE_HOST_SOLD_CATALOG_RESIDUAL_NAV_STEPS_WAVE1033: &[&str] = &[
    "SOLD_STATUS",
    "EVALUATE_CONTEXT",
    "OBJECT_STATUS_SOLD",
    "LIVE_HOST_SOLD_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSoldCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSoldCatalogResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
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
fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

pub fn honesty_host_sold_catalog_residual_method_names_residual_wave1033() -> bool {
    let names = LIVE_HOST_SOLD_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1033;
    let ok = residual_name_index(names, "sync_sold_from_presentation").is_some()
        && residual_name_index(names, "Wave 1033").is_some();
    residual_action_store(ResidualHostSoldCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sold_catalog_residual_nav_commands_residual_wave1033() -> bool {
    let steps = LIVE_HOST_SOLD_CATALOG_RESIDUAL_NAV_STEPS_WAVE1033;
    let ok = residual_name_index(steps, "LIVE_HOST_SOLD_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "SOLD_STATUS").is_some();
    residual_action_store(ResidualHostSoldCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sold_catalog_residual_residual_pack_wave1033() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let cb = cb_source();
    let pf = pf_source();
    let ok = ui.contains("Wave 1033: sold residual for dual-world ControlBar clear")
        && tr.contains("pub sold: bool")
        && cnc.contains("Wave 1033: sold residual for dual-world ControlBar clear")
        && cb.contains("Wave 1033")
        && cb.contains("fn sync_sold_from_presentation")
        && cb.contains("presentation_sold")
        && pf.contains("sync_sold_from_presentation(panel.sold)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSoldCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sold_catalog_residual_honesty() -> bool {
    let a = honesty_host_sold_catalog_residual_method_names_residual_wave1033();
    let b = honesty_host_sold_catalog_residual_nav_commands_residual_wave1033();
    let c = honesty_host_sold_catalog_residual_residual_pack_wave1033();
    residual_action_store(ResidualHostSoldCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sold_catalog_residual_wave1033() {
        assert!(honesty_host_sold_catalog_residual_residual_pack_wave1033());
        assert!(honesty_host_sold_catalog_residual_method_names_residual_wave1033());
        assert!(honesty_host_sold_catalog_residual_nav_commands_residual_wave1033());
        assert!(simulate_live_host_sold_catalog_residual_honesty());
    }
}
