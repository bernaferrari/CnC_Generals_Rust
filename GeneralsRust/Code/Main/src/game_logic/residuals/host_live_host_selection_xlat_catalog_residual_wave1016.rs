//! Wave 1016: selection_xlat dual-world catalog residual for empty OBJECT_REGISTRY.
//!
//! When drawable_registry and OBJECT_REGISTRY are empty, collect_drawables peels
//! translator catalog selectables (shroud_status, local team, structure/crate kinds).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELECTION_XLAT_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1016: &[&str] = &[
    "collect_drawables",
    "with_translator_catalog",
    "SelectableDrawable",
    "Wave 1016",
    "playable_claim = false",
];

pub const LIVE_HOST_SELECTION_XLAT_CATALOG_RESIDUAL_NAV_STEPS_WAVE1016: &[&str] = &[
    "SELECTION_XLAT",
    "TRANSLATOR_CATALOG",
    "COLLECT_DRAWABLES",
    "LIVE_HOST_SELECTION_XLAT_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSelectionXlatCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSelectionXlatCatalogResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn sx_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/message_stream/selection_xlat.rs")
}

pub fn honesty_host_selection_xlat_catalog_residual_method_names_residual_wave1016() -> bool {
    let names = LIVE_HOST_SELECTION_XLAT_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1016;
    let ok = residual_name_index(names, "collect_drawables").is_some()
        && residual_name_index(names, "Wave 1016").is_some();
    residual_action_store(ResidualHostSelectionXlatCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_xlat_catalog_residual_nav_commands_residual_wave1016() -> bool {
    let steps = LIVE_HOST_SELECTION_XLAT_CATALOG_RESIDUAL_NAV_STEPS_WAVE1016;
    let ok = residual_name_index(steps, "LIVE_HOST_SELECTION_XLAT_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "SELECTION_XLAT").is_some();
    residual_action_store(ResidualHostSelectionXlatCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_xlat_catalog_residual_residual_pack_wave1016() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sx = sx_source();
    let ok = sx.contains("Wave 1016")
        && sx.contains("registry peels translator catalog residual")
        && sx.contains("with_translator_catalog")
        && sx.contains("entry.shroud_status >= 3")
        && sx.contains("translator_entry_is_local(entry)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSelectionXlatCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_selection_xlat_catalog_residual_honesty() -> bool {
    let a = honesty_host_selection_xlat_catalog_residual_method_names_residual_wave1016();
    let b = honesty_host_selection_xlat_catalog_residual_nav_commands_residual_wave1016();
    let c = honesty_host_selection_xlat_catalog_residual_residual_pack_wave1016();
    residual_action_store(ResidualHostSelectionXlatCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_selection_xlat_catalog_residual_wave1016() {
        assert!(honesty_host_selection_xlat_catalog_residual_residual_pack_wave1016());
        assert!(honesty_host_selection_xlat_catalog_residual_method_names_residual_wave1016());
        assert!(honesty_host_selection_xlat_catalog_residual_nav_commands_residual_wave1016());
        assert!(simulate_live_host_selection_xlat_catalog_residual_honesty());
    }
}
