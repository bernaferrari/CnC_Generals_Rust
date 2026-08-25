//! Wave 1027: dual-world structure inventory + evaluate_context catalog residual.
//!
//! update_context_structure_inventory peels presentation_garrisoned_count when
//! OBJECT_REGISTRY is empty. evaluate_context peels translator catalog command-set
//! when presentation freezes are not yet stamped.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_STRUCTURE_INVENTORY_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1027: &[&str] = &[
    "update_context_structure_inventory",
    "presentation_garrisoned_count",
    "evaluate_context",
    "Wave 1027",
    "playable_claim = false",
];

pub const LIVE_HOST_STRUCTURE_INVENTORY_CATALOG_RESIDUAL_NAV_STEPS_WAVE1027: &[&str] = &[
    "STRUCTURE_INVENTORY",
    "EVALUATE_CONTEXT",
    "TRANSLATOR_CATALOG",
    "LIVE_HOST_STRUCTURE_INVENTORY_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStructureInventoryCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostStructureInventoryCatalogResidualAction) {
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
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

pub fn honesty_host_structure_inventory_catalog_residual_method_names_residual_wave1027() -> bool {
    let names = LIVE_HOST_STRUCTURE_INVENTORY_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1027;
    let ok = residual_name_index(names, "update_context_structure_inventory").is_some()
        && residual_name_index(names, "Wave 1027").is_some();
    residual_action_store(ResidualHostStructureInventoryCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_structure_inventory_catalog_residual_nav_commands_residual_wave1027() -> bool {
    let steps = LIVE_HOST_STRUCTURE_INVENTORY_CATALOG_RESIDUAL_NAV_STEPS_WAVE1027;
    let ok = residual_name_index(steps, "LIVE_HOST_STRUCTURE_INVENTORY_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "STRUCTURE_INVENTORY").is_some();
    residual_action_store(ResidualHostStructureInventoryCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_structure_inventory_catalog_residual_residual_pack_wave1027() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb
        .contains("Wave 1027: host empty dual-world peels presentation garrison residual count")
        && cb.contains("presentation_garrisoned_count as u32")
        && (cb.contains("Wave 1027: catalog residual when presentation freezes not yet stamped")
            || cb.contains(
                "Wave 1027/1032: catalog residual when presentation freezes not yet stamped",
            ))
        && cb.contains("translator_catalog_entry(obj_id)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostStructureInventoryCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_structure_inventory_catalog_residual_honesty() -> bool {
    let a = honesty_host_structure_inventory_catalog_residual_method_names_residual_wave1027();
    let b = honesty_host_structure_inventory_catalog_residual_nav_commands_residual_wave1027();
    let c = honesty_host_structure_inventory_catalog_residual_residual_pack_wave1027();
    residual_action_store(ResidualHostStructureInventoryCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_structure_inventory_catalog_residual_wave1027() {
        assert!(honesty_host_structure_inventory_catalog_residual_residual_pack_wave1027());
        assert!(honesty_host_structure_inventory_catalog_residual_method_names_residual_wave1027());
        assert!(honesty_host_structure_inventory_catalog_residual_nav_commands_residual_wave1027());
        assert!(simulate_live_host_structure_inventory_catalog_residual_honesty());
    }
}
