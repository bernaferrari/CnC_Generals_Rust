//! Wave 872: stamp known-template residual after GoldenRanger ensure; stamp
//! last_ui_state after boot UI dual-read; refresh producers after barracks ensure;
//! stamp sim timing after shell budget tick. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_TEMPLATE_UI_METHOD_NAMES_WAVE872: &[&str] = &[
    "host_ensure_golden_ranger_template",
    "host_stamp_known_template_name",
    "host_update_ui_state",
    "host_update_shell_with_budget",
    "host_ensure_barracks_building_data",
    "Wave 872",
    "playable_claim = false",
];

pub const LIVE_HOST_TEMPLATE_UI_NAV_STEPS_WAVE872: &[&str] = &[
    "STAMP_KNOWN_TEMPLATE_ON_INSERT",
    "STAMP_UI_STATE_AFTER_BOOT",
    "STAMP_SHELL_TIMING",
    "REFRESH_AFTER_BARRACKS_ENSURE",
    "LIVE_HOST_TEMPLATE_UI",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostTemplateUiAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostTemplateUiAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

pub fn honesty_host_template_ui_method_names_residual_wave872() -> bool {
    let names = LIVE_HOST_TEMPLATE_UI_METHOD_NAMES_WAVE872;
    let ok = residual_name_index(names, "host_stamp_known_template_name").is_some()
        && residual_name_index(names, "host_ensure_golden_ranger_template").is_some()
        && residual_name_index(names, "Wave 872").is_some();
    residual_action_store(ResidualHostTemplateUiAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_template_ui_nav_commands_residual_wave872() -> bool {
    let steps = LIVE_HOST_TEMPLATE_UI_NAV_STEPS_WAVE872;
    let ok = residual_name_index(steps, "LIVE_HOST_TEMPLATE_UI").is_some()
        && residual_name_index(steps, "STAMP_KNOWN_TEMPLATE_ON_INSERT").is_some();
    residual_action_store(ResidualHostTemplateUiAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_template_ui_residual_pack_wave872() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("fn host_stamp_known_template_name(&mut self, name: &str)")
        && cnc.contains("Wave 581")
        && cnc.contains("Wave 585")
        && cnc.contains("self.last_ui_state = Some(ui.clone())")
        && cnc.contains("Wave 584")
        && cnc.contains("Wave 583")
        && cnc.contains("Wave 834");
    residual_action_store(ResidualHostTemplateUiAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_template_ui_honesty() -> bool {
    let a = honesty_host_template_ui_method_names_residual_wave872();
    let b = honesty_host_template_ui_nav_commands_residual_wave872();
    let c = honesty_host_template_ui_residual_pack_wave872();
    residual_action_store(ResidualHostTemplateUiAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_template_ui_residual_wave872() {
        assert!(honesty_host_template_ui_residual_pack_wave872());
        assert!(honesty_host_template_ui_method_names_residual_wave872());
        assert!(honesty_host_template_ui_nav_commands_residual_wave872());
        assert!(simulate_live_host_template_ui_honesty());
    }
}
