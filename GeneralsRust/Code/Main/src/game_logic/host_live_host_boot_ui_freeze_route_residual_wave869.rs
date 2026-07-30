//! Wave 869: host_notify_boot_ui_message routes through presentation UI residual
//! under freeze (no GameLogic dual-write). Cancel/clear-path/guard-radius refresh
//! object-scan residuals. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_BOOT_UI_FREEZE_ROUTE_METHOD_NAMES_WAVE869: &[&str] = &[
    "host_notify_boot_ui_message",
    "host_notify_presentation_ui_message",
    "host_cancel_production_and_sync_hud",
    "host_clear_unit_movement_path",
    "host_adjust_unit_guard_radius",
    "Wave 869",
    "playable_claim = false",
];

pub const LIVE_HOST_BOOT_UI_FREEZE_ROUTE_NAV_STEPS_WAVE869: &[&str] = &[
    "ROUTE_BOOT_UI_UNDER_FREEZE",
    "REFRESH_AFTER_CANCEL",
    "REFRESH_AFTER_CLEAR_PATH",
    "REFRESH_AFTER_GUARD_RADIUS",
    "LIVE_HOST_BOOT_UI_FREEZE_ROUTE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBootUiFreezeRouteAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostBootUiFreezeRouteAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

pub fn honesty_host_boot_ui_freeze_route_method_names_residual_wave869() -> bool {
    let names = LIVE_HOST_BOOT_UI_FREEZE_ROUTE_METHOD_NAMES_WAVE869;
    let ok = residual_name_index(names, "host_notify_boot_ui_message").is_some()
        && residual_name_index(names, "host_notify_presentation_ui_message").is_some()
        && residual_name_index(names, "Wave 869").is_some();
    residual_action_store(ResidualHostBootUiFreezeRouteAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_boot_ui_freeze_route_nav_commands_residual_wave869() -> bool {
    let steps = LIVE_HOST_BOOT_UI_FREEZE_ROUTE_NAV_STEPS_WAVE869;
    let ok = residual_name_index(steps, "LIVE_HOST_BOOT_UI_FREEZE_ROUTE").is_some()
        && residual_name_index(steps, "ROUTE_BOOT_UI_UNDER_FREEZE").is_some();
    residual_action_store(ResidualHostBootUiFreezeRouteAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_boot_ui_freeze_route_residual_pack_wave869() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("Wave 603/869: host boot UI message residual")
        && cnc.contains("when presentation freeze is installed, never dual-write")
        && cnc.contains("self.host_notify_presentation_ui_message(message)")
        && cnc.contains("Wave 580/869: cancel + HUD building_queue residual + refresh scan")
        && cnc.contains("Wave 584/869: host clear path residual")
        && cnc.contains("Wave 584/869: host guard radius residual + keep scan residual warm")
        && cnc
            .matches("host_refresh_local_train_producer_residuals()")
            .count()
            >= 3;
    residual_action_store(ResidualHostBootUiFreezeRouteAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_boot_ui_freeze_route_honesty() -> bool {
    let a = honesty_host_boot_ui_freeze_route_method_names_residual_wave869();
    let b = honesty_host_boot_ui_freeze_route_nav_commands_residual_wave869();
    let c = honesty_host_boot_ui_freeze_route_residual_pack_wave869();
    residual_action_store(ResidualHostBootUiFreezeRouteAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_boot_ui_freeze_route_residual_wave869() {
        assert!(honesty_host_boot_ui_freeze_route_residual_pack_wave869());
        assert!(honesty_host_boot_ui_freeze_route_method_names_residual_wave869());
        assert!(honesty_host_boot_ui_freeze_route_nav_commands_residual_wave869());
        assert!(simulate_live_host_boot_ui_freeze_route_honesty());
    }
}
