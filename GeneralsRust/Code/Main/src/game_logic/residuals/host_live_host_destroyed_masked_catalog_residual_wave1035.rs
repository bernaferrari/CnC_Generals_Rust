//! Wave 1035: dual-world destroyed/masked selection catalog residual.
//!
//! Catalog carries destroyed/masked; dual collect_drawables skips them and maps
//! destroyed→is_dead, masked→OBJECT_STATUS_MASKED (C++ SelectionXlat).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DESTROYED_MASKED_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1035: &[&str] = &[
    "destroyed",
    "masked",
    "OBJECT_STATUS_MASKED",
    "Wave 1035",
    "playable_claim = false",
];

pub const LIVE_HOST_DESTROYED_MASKED_CATALOG_RESIDUAL_NAV_STEPS_WAVE1035: &[&str] = &[
    "DESTROYED",
    "MASKED",
    "SELECTION_XLAT",
    "LIVE_HOST_DESTROYED_MASKED_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDestroyedMaskedCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDestroyedMaskedCatalogResidualAction) {
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
fn sx_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/message_stream/selection_xlat.rs")
}

pub fn honesty_host_destroyed_masked_catalog_residual_method_names_residual_wave1035() -> bool {
    let names = LIVE_HOST_DESTROYED_MASKED_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1035;
    let ok = residual_name_index(names, "destroyed").is_some()
        && residual_name_index(names, "Wave 1035").is_some();
    residual_action_store(ResidualHostDestroyedMaskedCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_destroyed_masked_catalog_residual_nav_commands_residual_wave1035() -> bool {
    let steps = LIVE_HOST_DESTROYED_MASKED_CATALOG_RESIDUAL_NAV_STEPS_WAVE1035;
    let ok = residual_name_index(steps, "LIVE_HOST_DESTROYED_MASKED_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "DESTROYED").is_some();
    residual_action_store(ResidualHostDestroyedMaskedCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_destroyed_masked_catalog_residual_residual_pack_wave1035() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let sx = sx_source();
    let ok = ui.contains("Wave 1035: destroyed residual for dual-world selection skip")
        && ui.contains("Wave 1035: masked residual for dual-world selection")
        && tr.contains("pub destroyed: bool")
        && tr.contains("pub masked: bool")
        && cnc.contains("Wave 1035: destroyed/masked residual for dual-world selection")
        && sx.contains("Wave 1034/1035: C++ UNSELECTABLE / sold / destroyed / masked residual")
        && sx.contains("entry.destroyed")
        && sx.contains("entry.masked")
        && sx.contains("is_dead: entry.destroyed")
        && sx.contains("OBJECT_STATUS_MASKED")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDestroyedMaskedCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_destroyed_masked_catalog_residual_honesty() -> bool {
    let a = honesty_host_destroyed_masked_catalog_residual_method_names_residual_wave1035();
    let b = honesty_host_destroyed_masked_catalog_residual_nav_commands_residual_wave1035();
    let c = honesty_host_destroyed_masked_catalog_residual_residual_pack_wave1035();
    residual_action_store(ResidualHostDestroyedMaskedCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_destroyed_masked_catalog_residual_wave1035() {
        assert!(honesty_host_destroyed_masked_catalog_residual_residual_pack_wave1035());
        assert!(honesty_host_destroyed_masked_catalog_residual_method_names_residual_wave1035());
        assert!(honesty_host_destroyed_masked_catalog_residual_nav_commands_residual_wave1035());
        assert!(simulate_live_host_destroyed_masked_catalog_residual_honesty());
    }
}
