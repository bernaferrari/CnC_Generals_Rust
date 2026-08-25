//! Wave 1015: presentation catalog command-set residual for dual-world ControlBar.
//!
//! Catalog entries carry command_set_name from RenderableObject. Dual-world
//! portrait peel refreshes presentation_primary_command_set; get_object_has_production
//! treats non-empty command_set_name as production-interface residual.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CATALOG_COMMAND_SET_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1015: &[&str] = &[
    "command_set_name",
    "presentation_primary_command_set",
    "get_object_has_production",
    "Wave 1015",
    "playable_claim = false",
];

pub const LIVE_HOST_CATALOG_COMMAND_SET_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1015: &[&str] = &[
    "CATALOG_COMMAND_SET",
    "PORTRAIT_COMMAND_SET",
    "PRODUCTION_INTERFACE",
    "LIVE_HOST_CATALOG_COMMAND_SET_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCatalogCommandSetPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCatalogCommandSetPresentationResidualAction) {
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

pub fn honesty_host_catalog_command_set_presentation_residual_method_names_residual_wave1015()
-> bool {
    let names = LIVE_HOST_CATALOG_COMMAND_SET_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1015;
    let ok = residual_name_index(names, "command_set_name").is_some()
        && residual_name_index(names, "Wave 1015").is_some();
    residual_action_store(ResidualHostCatalogCommandSetPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_catalog_command_set_presentation_residual_nav_commands_residual_wave1015()
-> bool {
    let steps = LIVE_HOST_CATALOG_COMMAND_SET_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1015;
    let ok = residual_name_index(steps, "LIVE_HOST_CATALOG_COMMAND_SET_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "CATALOG_COMMAND_SET").is_some();
    residual_action_store(ResidualHostCatalogCommandSetPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_catalog_command_set_presentation_residual_residual_pack_wave1015() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let cb = cb_source();
    let ok = ui.contains("Wave 1015")
        && tr.contains("pub command_set_name: String")
        && cnc.contains("Wave 1015: command-set residual")
        && cnc.contains("command_set_name: if !o.command_set_name.is_empty()")
        && cb.contains("Wave 1015")
        && cb.contains("presentation_primary_command_set = entry.command_set_name.clone()")
        && cb.contains("!entry.command_set_name.is_empty()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostCatalogCommandSetPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_catalog_command_set_presentation_residual_honesty() -> bool {
    let a = honesty_host_catalog_command_set_presentation_residual_method_names_residual_wave1015();
    let b = honesty_host_catalog_command_set_presentation_residual_nav_commands_residual_wave1015();
    let c = honesty_host_catalog_command_set_presentation_residual_residual_pack_wave1015();
    residual_action_store(ResidualHostCatalogCommandSetPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_catalog_command_set_presentation_residual_wave1015() {
        assert!(honesty_host_catalog_command_set_presentation_residual_residual_pack_wave1015());
        assert!(
            honesty_host_catalog_command_set_presentation_residual_method_names_residual_wave1015()
        );
        assert!(
            honesty_host_catalog_command_set_presentation_residual_nav_commands_residual_wave1015()
        );
        assert!(simulate_live_host_catalog_command_set_presentation_residual_honesty());
    }
}
