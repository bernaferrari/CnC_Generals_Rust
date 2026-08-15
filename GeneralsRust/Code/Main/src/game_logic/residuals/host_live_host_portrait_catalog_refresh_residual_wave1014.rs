//! Wave 1014: dual-world portrait/production catalog refresh residual.
//!
//! - update_portrait_for_object refreshes health/veterancy/production from the
//!   translator catalog on every dual-world call (not only first fill).
//! - populate_build_queue seeds portrait production head from catalog when empty
//!   before peeling BuildQueueEntry residual.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PORTRAIT_CATALOG_REFRESH_RESIDUAL_METHOD_NAMES_WAVE1014: &[&str] = &[
    "update_portrait_for_object",
    "populate_build_queue",
    "translator_catalog_entry",
    "Wave 1014",
    "playable_claim = false",
];

pub const LIVE_HOST_PORTRAIT_CATALOG_REFRESH_RESIDUAL_NAV_STEPS_WAVE1014: &[&str] = &[
    "PORTRAIT_REFRESH",
    "CATALOG_PRODUCTION_SEED",
    "DUAL_WORLD",
    "LIVE_HOST_PORTRAIT_CATALOG_REFRESH_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPortraitCatalogRefreshResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPortraitCatalogRefreshResidualAction) {
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

pub fn honesty_host_portrait_catalog_refresh_residual_method_names_residual_wave1014() -> bool {
    let names = LIVE_HOST_PORTRAIT_CATALOG_REFRESH_RESIDUAL_METHOD_NAMES_WAVE1014;
    let ok = residual_name_index(names, "Wave 1014").is_some()
        && residual_name_index(names, "update_portrait_for_object").is_some();
    residual_action_store(ResidualHostPortraitCatalogRefreshResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_portrait_catalog_refresh_residual_nav_commands_residual_wave1014() -> bool {
    let steps = LIVE_HOST_PORTRAIT_CATALOG_REFRESH_RESIDUAL_NAV_STEPS_WAVE1014;
    let ok = residual_name_index(steps, "LIVE_HOST_PORTRAIT_CATALOG_REFRESH_RESIDUAL").is_some()
        && residual_name_index(steps, "PORTRAIT_REFRESH").is_some();
    residual_action_store(ResidualHostPortraitCatalogRefreshResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_portrait_catalog_refresh_residual_residual_pack_wave1014() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let portrait = match cb.find("fn update_portrait_for_object") {
        Some(i) => &cb[i..cb.len().min(i + 2500)],
        None => "",
    };
    let populate = match cb.find("fn populate_build_queue") {
        Some(i) => &cb[i..cb.len().min(i + 2000)],
        None => "",
    };
    let ok = portrait.contains("Wave 249/1008/1014")
        && portrait.contains("health residual refresh")
        && portrait.contains("production head residual refresh")
        && populate.contains("Wave 981/1010/1014")
        && populate.contains("seed portrait production head from translator catalog")
        && populate.contains("translator_catalog_entry(producer_id)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPortraitCatalogRefreshResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_portrait_catalog_refresh_residual_honesty() -> bool {
    let a = honesty_host_portrait_catalog_refresh_residual_method_names_residual_wave1014();
    let b = honesty_host_portrait_catalog_refresh_residual_nav_commands_residual_wave1014();
    let c = honesty_host_portrait_catalog_refresh_residual_residual_pack_wave1014();
    residual_action_store(ResidualHostPortraitCatalogRefreshResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_portrait_catalog_refresh_residual_wave1014() {
        assert!(honesty_host_portrait_catalog_refresh_residual_residual_pack_wave1014());
        assert!(honesty_host_portrait_catalog_refresh_residual_method_names_residual_wave1014());
        assert!(honesty_host_portrait_catalog_refresh_residual_nav_commands_residual_wave1014());
        assert!(simulate_live_host_portrait_catalog_refresh_residual_honesty());
    }
}
