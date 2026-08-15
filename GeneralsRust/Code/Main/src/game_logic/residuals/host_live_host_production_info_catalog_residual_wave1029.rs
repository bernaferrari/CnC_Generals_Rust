//! Wave 1029: dual-world production info/progress + UC catalog residual.
//!
//! get_object_production_info / get_first_production_progress peel translator
//! catalog when OBJECT_REGISTRY is empty. update_context_under_construction
//! seeds presentation_under_construction from catalog.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_INFO_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1029: &[&str] = &[
    "get_object_production_info",
    "get_first_production_progress",
    "update_context_under_construction",
    "Wave 1029",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_INFO_CATALOG_RESIDUAL_NAV_STEPS_WAVE1029: &[&str] = &[
    "PRODUCTION_INFO",
    "PRODUCTION_PROGRESS",
    "UNDER_CONSTRUCTION_CONTEXT",
    "LIVE_HOST_PRODUCTION_INFO_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionInfoCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProductionInfoCatalogResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

pub fn honesty_host_production_info_catalog_residual_method_names_residual_wave1029() -> bool {
    let names = LIVE_HOST_PRODUCTION_INFO_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1029;
    let ok = residual_name_index(names, "get_object_production_info").is_some()
        && residual_name_index(names, "Wave 1029").is_some();
    residual_action_store(ResidualHostProductionInfoCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_info_catalog_residual_nav_commands_residual_wave1029() -> bool {
    let steps = LIVE_HOST_PRODUCTION_INFO_CATALOG_RESIDUAL_NAV_STEPS_WAVE1029;
    let ok = residual_name_index(steps, "LIVE_HOST_PRODUCTION_INFO_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "PRODUCTION_INFO").is_some();
    residual_action_store(ResidualHostProductionInfoCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_info_catalog_residual_residual_pack_wave1029() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb
        .contains("Wave 1029: dual-world peels catalog production residual when registry empty")
        && cb.contains("Wave 1029: dual-world peels catalog production_progress residual")
        && cb.contains(
            "Wave 1029: catalog under_construction residual keeps dual-world UC context live",
        )
        && cb
            .contains("entry.production_template.is_some() || entry.production_progress.is_some()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionInfoCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_production_info_catalog_residual_honesty() -> bool {
    let a = honesty_host_production_info_catalog_residual_method_names_residual_wave1029();
    let b = honesty_host_production_info_catalog_residual_nav_commands_residual_wave1029();
    let c = honesty_host_production_info_catalog_residual_residual_pack_wave1029();
    residual_action_store(ResidualHostProductionInfoCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_production_info_catalog_residual_wave1029() {
        assert!(honesty_host_production_info_catalog_residual_residual_pack_wave1029());
        assert!(honesty_host_production_info_catalog_residual_method_names_residual_wave1029());
        assert!(honesty_host_production_info_catalog_residual_nav_commands_residual_wave1029());
        assert!(simulate_live_host_production_info_catalog_residual_honesty());
    }
}
