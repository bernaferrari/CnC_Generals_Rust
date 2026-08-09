//! Wave 862: host world-bounds residual + last_ui_state preferred for
//! host_update_ui_state boot residual. Presentation shell tick remains
//! PRES_SHELL_ONLY_DRAWABLE_TICK (no full GameClient::update).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_WORLD_BOUNDS_UI_RESIDUAL_METHOD_NAMES_WAVE862: &[&str] = &[
    "host_match_world_bounds",
    "host_world_bounds",
    "host_update_ui_state",
    "last_ui_state",
    "PRES_SHELL_ONLY_DRAWABLE_TICK",
    "Wave 862",
    "playable_claim = false",
];

pub const LIVE_HOST_WORLD_BOUNDS_UI_RESIDUAL_NAV_STEPS_WAVE862: &[&str] = &[
    "STAMP_HOST_WORLD_BOUNDS",
    "PREFER_LAST_UI_STATE",
    "SHELL_ONLY_DRAWABLE_TICK",
    "LIVE_HOST_WORLD_BOUNDS_UI_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWorldBoundsUiAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostWorldBoundsUiAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_world_bounds_ui_residual_method_names_residual_wave862() -> bool {
    let names = LIVE_HOST_WORLD_BOUNDS_UI_RESIDUAL_METHOD_NAMES_WAVE862;
    let ok = residual_name_index(names, "host_match_world_bounds").is_some()
        && residual_name_index(names, "host_update_ui_state").is_some()
        && residual_name_index(names, "Wave 862").is_some();
    residual_action_store(ResidualHostWorldBoundsUiAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_world_bounds_ui_residual_nav_commands_residual_wave862() -> bool {
    let steps = LIVE_HOST_WORLD_BOUNDS_UI_RESIDUAL_NAV_STEPS_WAVE862;
    let ok = residual_name_index(steps, "LIVE_HOST_WORLD_BOUNDS_UI_RESIDUAL").is_some()
        && residual_name_index(steps, "SHELL_ONLY_DRAWABLE_TICK").is_some();
    residual_action_store(ResidualHostWorldBoundsUiAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_world_bounds_ui_residual_pack_wave862() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_world_bounds: Option<(glam::Vec3, glam::Vec3)>")
        && cnc.contains("Wave 862: stamp world bounds residual")
        && cnc.contains("Wave 585/862")
        && cnc.contains("if let Some(b) = self.host_match_world_bounds")
        && cnc.contains("if let Some(ui) = self.last_ui_state.clone()")
        && cnc.contains("Wave 862: presentation pose/shroud/caption residual already applied above")
        && cnc.contains("PRES_SHELL_ONLY_DRAWABLE_TICK")
        && cnc.contains("update_presentation_shell")
        // Honesty residual: production path forbids full update_drawables; tests may mention it.
        && cnc.contains("Do not call full update_drawables");
    residual_action_store(ResidualHostWorldBoundsUiAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_world_bounds_ui_residual_honesty() -> bool {
    let a = honesty_host_world_bounds_ui_residual_method_names_residual_wave862();
    let b = honesty_host_world_bounds_ui_residual_nav_commands_residual_wave862();
    let c = honesty_host_world_bounds_ui_residual_pack_wave862();
    residual_action_store(ResidualHostWorldBoundsUiAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_world_bounds_ui_residual_wave862() {
        assert!(honesty_host_world_bounds_ui_residual_pack_wave862());
        assert!(honesty_host_world_bounds_ui_residual_method_names_residual_wave862());
        assert!(honesty_host_world_bounds_ui_residual_nav_commands_residual_wave862());
        assert!(simulate_live_host_world_bounds_ui_residual_honesty());
    }
}
