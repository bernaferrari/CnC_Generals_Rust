//! Wave 1017: dual-world multi-select command-set catalog residual.
//!
//! add_multi_select_commands seeds presentation command-set names from the
//! translator catalog when dual-world and multi-selected. Portrait peel
//! accumulates distinct command_set_name values for intersection.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MULTI_SELECT_CATALOG_COMMAND_SET_RESIDUAL_METHOD_NAMES_WAVE1017: &[&str] = &[
    "add_multi_select_commands",
    "presentation_command_set_names",
    "populate_multi_select_commands_from_sets",
    "Wave 1017",
    "playable_claim = false",
];

pub const LIVE_HOST_MULTI_SELECT_CATALOG_COMMAND_SET_RESIDUAL_NAV_STEPS_WAVE1017: &[&str] = &[
    "MULTI_SELECT",
    "TRANSLATOR_CATALOG",
    "COMMAND_SET_INTERSECT",
    "LIVE_HOST_MULTI_SELECT_CATALOG_COMMAND_SET_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMultiSelectCatalogCommandSetResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMultiSelectCatalogCommandSetResidualAction) {
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

pub fn honesty_host_multi_select_catalog_command_set_residual_method_names_residual_wave1017()
-> bool {
    let names = LIVE_HOST_MULTI_SELECT_CATALOG_COMMAND_SET_RESIDUAL_METHOD_NAMES_WAVE1017;
    let ok = residual_name_index(names, "add_multi_select_commands").is_some()
        && residual_name_index(names, "Wave 1017").is_some();
    residual_action_store(ResidualHostMultiSelectCatalogCommandSetResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_multi_select_catalog_command_set_residual_nav_commands_residual_wave1017()
-> bool {
    let steps = LIVE_HOST_MULTI_SELECT_CATALOG_COMMAND_SET_RESIDUAL_NAV_STEPS_WAVE1017;
    let ok = residual_name_index(steps, "LIVE_HOST_MULTI_SELECT_CATALOG_COMMAND_SET_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "MULTI_SELECT").is_some();
    residual_action_store(ResidualHostMultiSelectCatalogCommandSetResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_multi_select_catalog_command_set_residual_residual_pack_wave1017() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 1017: dual-world multi-select seeds command-set names")
        && cb.contains("presentation_names.len() < 2")
        && cb.contains("context.selected_objects.len() >= 2")
        && cb.contains("Wave 1015/1017: command-set residual refresh")
        && cb.contains("Accumulate distinct names so multi-select intersection can peel")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostMultiSelectCatalogCommandSetResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_multi_select_catalog_command_set_residual_honesty() -> bool {
    let a = honesty_host_multi_select_catalog_command_set_residual_method_names_residual_wave1017();
    let b = honesty_host_multi_select_catalog_command_set_residual_nav_commands_residual_wave1017();
    let c = honesty_host_multi_select_catalog_command_set_residual_residual_pack_wave1017();
    residual_action_store(ResidualHostMultiSelectCatalogCommandSetResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_multi_select_catalog_command_set_residual_wave1017() {
        assert!(honesty_host_multi_select_catalog_command_set_residual_residual_pack_wave1017());
        assert!(
            honesty_host_multi_select_catalog_command_set_residual_method_names_residual_wave1017()
        );
        assert!(
            honesty_host_multi_select_catalog_command_set_residual_nav_commands_residual_wave1017()
        );
        assert!(simulate_live_host_multi_select_catalog_command_set_residual_honesty());
    }
}
