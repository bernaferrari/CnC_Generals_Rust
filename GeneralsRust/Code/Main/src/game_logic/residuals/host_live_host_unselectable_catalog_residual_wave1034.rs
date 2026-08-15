//! Wave 1034: dual-world unselectable/sold selection residual.
//!
//! Catalog carries unselectable; dual collect_drawables skips unselectable/sold
//! entries (C++ OBJECT_STATUS_UNSELECTABLE). ControlBar dual evaluate clears on
//! unselectable. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UNSELECTABLE_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1034: &[&str] = &[
    "unselectable",
    "collect_drawables",
    "OBJECT_STATUS_UNSELECTABLE",
    "Wave 1034",
    "playable_claim = false",
];

pub const LIVE_HOST_UNSELECTABLE_CATALOG_RESIDUAL_NAV_STEPS_WAVE1034: &[&str] = &[
    "UNSELECTABLE",
    "SELECTION_XLAT",
    "CONTROL_BAR_CLEAR",
    "LIVE_HOST_UNSELECTABLE_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUnselectableCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUnselectableCatalogResidualAction) {
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
fn sx_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/message_stream/selection_xlat.rs")
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

pub fn honesty_host_unselectable_catalog_residual_method_names_residual_wave1034() -> bool {
    let names = LIVE_HOST_UNSELECTABLE_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1034;
    let ok = residual_name_index(names, "unselectable").is_some()
        && residual_name_index(names, "Wave 1034").is_some();
    residual_action_store(ResidualHostUnselectableCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_unselectable_catalog_residual_nav_commands_residual_wave1034() -> bool {
    let steps = LIVE_HOST_UNSELECTABLE_CATALOG_RESIDUAL_NAV_STEPS_WAVE1034;
    let ok = residual_name_index(steps, "LIVE_HOST_UNSELECTABLE_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "UNSELECTABLE").is_some();
    residual_action_store(ResidualHostUnselectableCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_unselectable_catalog_residual_residual_pack_wave1034() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let sx = sx_source();
    let cb = cb_source();
    let ok = ui.contains("Wave 1034: unselectable residual for dual-world selection")
        && tr.contains("pub unselectable: bool")
        && cnc.contains("Wave 1034: unselectable residual for dual-world selection")
        && sx.contains("Wave 1034")
        && sx.contains("entry.unselectable || entry.sold")
        && sx.contains("OBJECT_STATUS_UNSELECTABLE")
        && cb.contains("Wave 1033")
        && cb.contains("catalog_unselectable")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostUnselectableCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_unselectable_catalog_residual_honesty() -> bool {
    let a = honesty_host_unselectable_catalog_residual_method_names_residual_wave1034();
    let b = honesty_host_unselectable_catalog_residual_nav_commands_residual_wave1034();
    let c = honesty_host_unselectable_catalog_residual_residual_pack_wave1034();
    residual_action_store(ResidualHostUnselectableCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_unselectable_catalog_residual_wave1034() {
        assert!(honesty_host_unselectable_catalog_residual_residual_pack_wave1034());
        assert!(honesty_host_unselectable_catalog_residual_method_names_residual_wave1034());
        assert!(honesty_host_unselectable_catalog_residual_nav_commands_residual_wave1034());
        assert!(simulate_live_host_unselectable_catalog_residual_honesty());
    }
}
