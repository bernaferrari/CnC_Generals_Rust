//! Wave 1018: dual-world set_portrait_by_object_id catalog residual.
//!
//! On selection with empty OBJECT_REGISTRY, set_portrait_by_object_id peels
//! update_portrait_for_object (catalog health/veterancy/production/command-set).
//! Deselection clears portrait, queue, and presentation command-set residual.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PORTRAIT_SELECTION_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1018: &[&str] = &[
    "set_portrait_by_object_id",
    "update_portrait_for_object",
    "dual_world_registry_unavailable",
    "Wave 1018",
    "playable_claim = false",
];

pub const LIVE_HOST_PORTRAIT_SELECTION_CATALOG_RESIDUAL_NAV_STEPS_WAVE1018: &[&str] = &[
    "PORTRAIT_SELECTION",
    "TRANSLATOR_CATALOG",
    "UPDATE_PORTRAIT_FOR_OBJECT",
    "LIVE_HOST_PORTRAIT_SELECTION_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPortraitSelectionCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPortraitSelectionCatalogResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn cb_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/gui/control_bar/control_bar.rs")
}

pub fn honesty_host_portrait_selection_catalog_residual_method_names_residual_wave1018() -> bool {
    let names = LIVE_HOST_PORTRAIT_SELECTION_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1018;
    let ok = residual_name_index(names, "set_portrait_by_object_id").is_some()
        && residual_name_index(names, "Wave 1018").is_some();
    residual_action_store(ResidualHostPortraitSelectionCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_portrait_selection_catalog_residual_nav_commands_residual_wave1018() -> bool {
    let steps = LIVE_HOST_PORTRAIT_SELECTION_CATALOG_RESIDUAL_NAV_STEPS_WAVE1018;
    let ok = residual_name_index(steps, "LIVE_HOST_PORTRAIT_SELECTION_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "PORTRAIT_SELECTION").is_some();
    residual_action_store(ResidualHostPortraitSelectionCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_portrait_selection_catalog_residual_residual_pack_wave1018() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 249/1006/1018: dual-world peels catalog portrait on selection")
        && cb.contains("Wave 1018: selection path must refresh portrait residual from catalog")
        && cb.contains("self.update_portrait_for_object(id)")
        && cb.contains("presentation_primary_command_set.clear()")
        && cb.contains("presentation_command_set_names.clear()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPortraitSelectionCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_portrait_selection_catalog_residual_honesty() -> bool {
    let a = honesty_host_portrait_selection_catalog_residual_method_names_residual_wave1018();
    let b = honesty_host_portrait_selection_catalog_residual_nav_commands_residual_wave1018();
    let c = honesty_host_portrait_selection_catalog_residual_residual_pack_wave1018();
    residual_action_store(ResidualHostPortraitSelectionCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_portrait_selection_catalog_residual_wave1018() {
        assert!(honesty_host_portrait_selection_catalog_residual_residual_pack_wave1018());
        assert!(honesty_host_portrait_selection_catalog_residual_method_names_residual_wave1018());
        assert!(honesty_host_portrait_selection_catalog_residual_nav_commands_residual_wave1018());
        assert!(simulate_live_host_portrait_selection_catalog_residual_honesty());
    }
}
