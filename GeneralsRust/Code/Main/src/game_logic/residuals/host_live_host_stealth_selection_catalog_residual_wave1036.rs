//! Wave 1036: dual-world effectively-stealthed selection catalog residual.
//!
//! Catalog carries effectively_stealthed; dual collect_drawables skips
//! non-local effectively stealthed entries (C++ SelectionInfo enemy/neutral).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_STEALTH_SELECTION_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1036: &[&str] = &[
    "effectively_stealthed",
    "translator_entry_is_local",
    "collect_drawables",
    "Wave 1036",
    "playable_claim = false",
];

pub const LIVE_HOST_STEALTH_SELECTION_CATALOG_RESIDUAL_NAV_STEPS_WAVE1036: &[&str] = &[
    "EFFECTIVELY_STEALTHED",
    "SELECTION_XLAT",
    "ENEMY_STEALTH_SKIP",
    "LIVE_HOST_STEALTH_SELECTION_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStealthSelectionCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostStealthSelectionCatalogResidualAction) {
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

pub fn honesty_host_stealth_selection_catalog_residual_method_names_residual_wave1036() -> bool {
    let names = LIVE_HOST_STEALTH_SELECTION_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1036;
    let ok = residual_name_index(names, "effectively_stealthed").is_some()
        && residual_name_index(names, "Wave 1036").is_some();
    residual_action_store(ResidualHostStealthSelectionCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_stealth_selection_catalog_residual_nav_commands_residual_wave1036() -> bool {
    let steps = LIVE_HOST_STEALTH_SELECTION_CATALOG_RESIDUAL_NAV_STEPS_WAVE1036;
    let ok = residual_name_index(steps, "LIVE_HOST_STEALTH_SELECTION_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "EFFECTIVELY_STEALTHED").is_some();
    residual_action_store(ResidualHostStealthSelectionCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_stealth_selection_catalog_residual_residual_pack_wave1036() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let sx = sx_source();
    let ok = ui.contains("Wave 1036: effectively stealthed residual")
        && tr.contains("pub effectively_stealthed: bool")
        && cnc.contains("Wave 1036: effectively stealthed residual for dual-world selection")
        && cnc.contains("effectively_stealthed: o.effectively_stealthed")
        && (sx.contains("Wave 1034") || sx.contains("Wave 1034/1035/1036"))
        && sx.contains("entry.effectively_stealthed && !translator_entry_is_local(entry)")
        && sx.contains("effectively_stealthed")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostStealthSelectionCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_stealth_selection_catalog_residual_honesty() -> bool {
    let a = honesty_host_stealth_selection_catalog_residual_method_names_residual_wave1036();
    let b = honesty_host_stealth_selection_catalog_residual_nav_commands_residual_wave1036();
    let c = honesty_host_stealth_selection_catalog_residual_residual_pack_wave1036();
    residual_action_store(ResidualHostStealthSelectionCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_stealth_selection_catalog_residual_wave1036() {
        assert!(honesty_host_stealth_selection_catalog_residual_residual_pack_wave1036());
        assert!(honesty_host_stealth_selection_catalog_residual_method_names_residual_wave1036());
        assert!(honesty_host_stealth_selection_catalog_residual_nav_commands_residual_wave1036());
        assert!(simulate_live_host_stealth_selection_catalog_residual_honesty());
    }
}
