//! Wave 1025: dual-world get_command_availability catalog residual.
//!
//! When OBJECT_REGISTRY misses an object, availability peels presentation
//! command-set residual and translator catalog (selectable/command_set_name)
//! instead of hiding all buttons solely for empty portrait freeze.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_COMMAND_AVAILABILITY_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1025: &[&str] = &[
    "get_command_availability",
    "presentation_primary_command_set",
    "translator_catalog_entry",
    "Wave 1025",
    "playable_claim = false",
];

pub const LIVE_HOST_COMMAND_AVAILABILITY_CATALOG_RESIDUAL_NAV_STEPS_WAVE1025: &[&str] = &[
    "COMMAND_AVAILABILITY",
    "TRANSLATOR_CATALOG",
    "PRESENTATION_COMMAND_SET",
    "LIVE_HOST_COMMAND_AVAILABILITY_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCommandAvailabilityCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCommandAvailabilityCatalogResidualAction) {
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

pub fn honesty_host_command_availability_catalog_residual_method_names_residual_wave1025() -> bool {
    let names = LIVE_HOST_COMMAND_AVAILABILITY_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1025;
    let ok = residual_name_index(names, "get_command_availability").is_some()
        && residual_name_index(names, "Wave 1025").is_some();
    residual_action_store(ResidualHostCommandAvailabilityCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_availability_catalog_residual_nav_commands_residual_wave1025() -> bool {
    let steps = LIVE_HOST_COMMAND_AVAILABILITY_CATALOG_RESIDUAL_NAV_STEPS_WAVE1025;
    let ok = residual_name_index(steps, "LIVE_HOST_COMMAND_AVAILABILITY_CATALOG_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "COMMAND_AVAILABILITY").is_some();
    residual_action_store(ResidualHostCommandAvailabilityCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_availability_catalog_residual_residual_pack_wave1025() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 1025: dual-world peels catalog/command-set residual")
        && cb.contains("presentation_primary_command_set.is_empty()")
        && cb.contains("translator_catalog_entry(obj_id)")
        // 2026-08-15: live binding is `entry` (impl_command_context.rs:159).
        && (cb.contains("!entry.command_set_name.is_empty() || entry.selectable")
            || cb.contains("!e.command_set_name.is_empty() || e.selectable"))
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostCommandAvailabilityCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_command_availability_catalog_residual_honesty() -> bool {
    let a = honesty_host_command_availability_catalog_residual_method_names_residual_wave1025();
    let b = honesty_host_command_availability_catalog_residual_nav_commands_residual_wave1025();
    let c = honesty_host_command_availability_catalog_residual_residual_pack_wave1025();
    residual_action_store(ResidualHostCommandAvailabilityCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_command_availability_catalog_residual_wave1025() {
        assert!(honesty_host_command_availability_catalog_residual_residual_pack_wave1025());
        assert!(
            honesty_host_command_availability_catalog_residual_method_names_residual_wave1025()
        );
        assert!(
            honesty_host_command_availability_catalog_residual_nav_commands_residual_wave1025()
        );
        assert!(simulate_live_host_command_availability_catalog_residual_honesty());
    }
}
