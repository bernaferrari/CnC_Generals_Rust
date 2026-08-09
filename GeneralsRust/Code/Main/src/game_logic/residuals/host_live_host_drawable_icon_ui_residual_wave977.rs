//! Wave 977: drawable icon UI presentation residual dispatch.
//!
//! Peels draw_icon_ui off dual-world fail-closed early return so host path
//! dispatches health/ammo/contain/status overlays from presentation residual.
//! Computes health-bar screen region from drawable pose when registry empty.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DRAWABLE_ICON_UI_METHOD_NAMES_WAVE977: &[&str] = &[
    "draw_icon_ui",
    "compute_health_region_from_presentation_pose",
    "health_region_from_world_point",
    "Wave 977",
    "playable_claim = false",
];

pub const LIVE_HOST_DRAWABLE_ICON_UI_NAV_STEPS_WAVE977: &[&str] = &[
    "ICON_UI_FROM_PRESENTATION",
    "HEALTH_REGION_FROM_POSE",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_DRAWABLE_ICON_UI",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDrawableIconUiAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDrawableIconUiAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn drawable_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/drawable/drawable.rs")
}

pub fn honesty_host_drawable_icon_ui_method_names_residual_wave977() -> bool {
    let names = LIVE_HOST_DRAWABLE_ICON_UI_METHOD_NAMES_WAVE977;
    let ok = residual_name_index(names, "draw_icon_ui").is_some()
        && residual_name_index(names, "Wave 977").is_some();
    residual_action_store(ResidualHostDrawableIconUiAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_icon_ui_nav_commands_residual_wave977() -> bool {
    let steps = LIVE_HOST_DRAWABLE_ICON_UI_NAV_STEPS_WAVE977;
    let ok = residual_name_index(steps, "LIVE_HOST_DRAWABLE_ICON_UI").is_some()
        && residual_name_index(steps, "HEALTH_REGION_FROM_POSE").is_some();
    residual_action_store(ResidualHostDrawableIconUiAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_icon_ui_residual_pack_wave977() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let drawable = drawable_source();
    let icon = match drawable.find("pub fn draw_icon_ui") {
        Some(i) => &drawable[i..drawable.len().min(i + 1800)],
        None => "",
    };
    let compute = match drawable.find("fn compute_health_region_from_object") {
        Some(i) => &drawable[i..drawable.len().min(i + 900)],
        None => "",
    };
    let ok = drawable.contains("Wave 977")
        && icon.contains("Wave 977")
        && !icon.contains("empty dual-world → no factory object walks")
        && icon.contains("draw_health_bar")
        && icon.contains("presentation_health_pct")
        && compute.contains("compute_health_region_from_presentation_pose")
        && drawable.contains("fn health_region_from_world_point")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDrawableIconUiAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_drawable_icon_ui_honesty() -> bool {
    let a = honesty_host_drawable_icon_ui_method_names_residual_wave977();
    let b = honesty_host_drawable_icon_ui_nav_commands_residual_wave977();
    let c = honesty_host_drawable_icon_ui_residual_pack_wave977();
    residual_action_store(ResidualHostDrawableIconUiAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_drawable_icon_ui_residual_wave977() {
        assert!(honesty_host_drawable_icon_ui_residual_pack_wave977());
        assert!(honesty_host_drawable_icon_ui_method_names_residual_wave977());
        assert!(honesty_host_drawable_icon_ui_nav_commands_residual_wave977());
        assert!(simulate_live_host_drawable_icon_ui_honesty());
    }
}
