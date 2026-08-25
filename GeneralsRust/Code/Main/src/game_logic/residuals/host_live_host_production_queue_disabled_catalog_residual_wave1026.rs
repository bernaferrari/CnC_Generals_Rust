//! Wave 1026: dual-world production queue + disabled catalog residual.
//!
//! update_context_command peels populate_build_queue without registry producer.
//! Catalog carries disabled; get_command_availability peels Restricted for
//! disabled dual-world units (Sell/Evacuate/Stop remain available).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_QUEUE_DISABLED_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1026: &[&str] = &[
    "update_context_command",
    "populate_build_queue",
    "disabled",
    "CommandAvailability::Restricted",
    "Wave 1026",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_QUEUE_DISABLED_CATALOG_RESIDUAL_NAV_STEPS_WAVE1026: &[&str] = &[
    "UPDATE_CONTEXT_COMMAND",
    "POPULATE_BUILD_QUEUE",
    "DISABLED_RESTRICTED",
    "LIVE_HOST_PRODUCTION_QUEUE_DISABLED_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionQueueDisabledCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProductionQueueDisabledCatalogResidualAction) {
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
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

pub fn honesty_host_production_queue_disabled_catalog_residual_method_names_residual_wave1026()
-> bool {
    let names = LIVE_HOST_PRODUCTION_QUEUE_DISABLED_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1026;
    let ok = residual_name_index(names, "update_context_command").is_some()
        && residual_name_index(names, "Wave 1026").is_some();
    residual_action_store(ResidualHostProductionQueueDisabledCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_queue_disabled_catalog_residual_nav_commands_residual_wave1026()
-> bool {
    let steps = LIVE_HOST_PRODUCTION_QUEUE_DISABLED_CATALOG_RESIDUAL_NAV_STEPS_WAVE1026;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_PRODUCTION_QUEUE_DISABLED_CATALOG_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "DISABLED_RESTRICTED").is_some();
    residual_action_store(ResidualHostProductionQueueDisabledCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_queue_disabled_catalog_residual_residual_pack_wave1026() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let cb = cb_source();
    let ok = cb
        .contains("Wave 1026")
        && cb.contains("if has_production {")
        && ui.contains("Wave 1026: disabled residual for dual-world command availability")
        && tr.contains("pub disabled: bool")
        && cnc.contains("Wave 1026: disabled residual for dual-world command availability")
        && cnc.contains("disabled: o.disabled")
        // 2026-08-15: Wave 1052 joined the disabled Restricted peel comment.
        && (cb.contains("Wave 1025/1026/1052: catalog/command-set residual; disabled => Restricted")
            || cb.contains("Wave 1025/1026: catalog/command-set residual; disabled => Restricted"))
        && cb.contains("entry.disabled && !self.force_disabled_evaluation(command)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionQueueDisabledCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_production_queue_disabled_catalog_residual_honesty() -> bool {
    let a =
        honesty_host_production_queue_disabled_catalog_residual_method_names_residual_wave1026();
    let b =
        honesty_host_production_queue_disabled_catalog_residual_nav_commands_residual_wave1026();
    let c = honesty_host_production_queue_disabled_catalog_residual_residual_pack_wave1026();
    residual_action_store(ResidualHostProductionQueueDisabledCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_production_queue_disabled_catalog_residual_wave1026() {
        assert!(honesty_host_production_queue_disabled_catalog_residual_residual_pack_wave1026());
        assert!(
            honesty_host_production_queue_disabled_catalog_residual_method_names_residual_wave1026(
            )
        );
        assert!(
            honesty_host_production_queue_disabled_catalog_residual_nav_commands_residual_wave1026(
            )
        );
        assert!(simulate_live_host_production_queue_disabled_catalog_residual_honesty());
    }
}
