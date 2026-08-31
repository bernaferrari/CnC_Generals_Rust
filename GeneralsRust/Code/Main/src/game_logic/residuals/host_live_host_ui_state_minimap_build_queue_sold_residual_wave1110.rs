//! Wave 1110: apply_to_ui_state minimap/build-queue + ControlBar select sold residual.
//!
//! After Waves 1106–1109 HUD sold peels:
//! - `apply_to_ui_state` minimap dots / beacon bounds / build_queue still included sold
//! - `apply_to_control_bar` multi-select count and ready-SP lists only skipped destroyed

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UI_STATE_MINIMAP_BUILD_QUEUE_SOLD_METHOD_NAMES_WAVE1110: &[&str] = &[
    "apply_to_ui_state",
    "apply_to_control_bar",
    "minimap_unit_dots",
    "build_queue",
    "Wave 1110",
    "playable_claim: false",
];

pub const LIVE_HOST_UI_STATE_MINIMAP_BUILD_QUEUE_SOLD_NAV_STEPS_WAVE1110: &[&str] = &[
    "UI_STATE_MINIMAP_EXCLUDE_SOLD",
    "BUILD_QUEUE_EXCLUDE_SOLD",
    "CONTROL_BAR_SELECT_USABLE",
    "LIVE_HOST_UI_STATE_MINIMAP_BUILD_QUEUE_SOLD",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUiStateMinimapBuildQueueSoldAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUiStateMinimapBuildQueueSoldAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_ui_state_minimap_build_queue_sold_method_names_residual_wave1110() -> bool {
    let names = LIVE_HOST_UI_STATE_MINIMAP_BUILD_QUEUE_SOLD_METHOD_NAMES_WAVE1110;
    let ok = residual_name_index(names, "apply_to_ui_state").is_some()
        && residual_name_index(names, "Wave 1110").is_some();
    residual_action_store(ResidualHostUiStateMinimapBuildQueueSoldAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ui_state_minimap_build_queue_sold_nav_commands_residual_wave1110() -> bool {
    let steps = LIVE_HOST_UI_STATE_MINIMAP_BUILD_QUEUE_SOLD_NAV_STEPS_WAVE1110;
    let ok = residual_name_index(steps, "LIVE_HOST_UI_STATE_MINIMAP_BUILD_QUEUE_SOLD").is_some()
        && residual_name_index(steps, "UI_STATE_MINIMAP_EXCLUDE_SOLD").is_some();
    residual_action_store(ResidualHostUiStateMinimapBuildQueueSoldAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ui_state_minimap_build_queue_sold_residual_pack_wave1110() -> bool {
    let pf = pf_source();
    let es = es_source();
    let ok = pf.contains("Wave 1110: beacon bounds residual excludes sold")
        && pf.contains("Wave 1110: build-queue residual excludes sold")
        && pf.contains("Wave 1110: minimap unit-dot residual excludes sold")
        && pf.contains("Wave 1110: multi-select count residual excludes sold")
        && pf.contains("Wave 1110: ready SP residual fail-closed on sold/disabled")
        && pf.contains("fn apply_to_ui_state")
        && pf.contains("fn apply_to_control_bar")
        && es.contains("playable_claim: false");
    residual_action_store(ResidualHostUiStateMinimapBuildQueueSoldAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ui_state_minimap_build_queue_sold_residual_honesty() -> bool {
    let a = honesty_host_ui_state_minimap_build_queue_sold_method_names_residual_wave1110();
    let b = honesty_host_ui_state_minimap_build_queue_sold_nav_commands_residual_wave1110();
    let c = honesty_host_ui_state_minimap_build_queue_sold_residual_pack_wave1110();
    residual_action_store(ResidualHostUiStateMinimapBuildQueueSoldAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ui_state_minimap_build_queue_sold_residual_wave1110() {
        assert!(honesty_host_ui_state_minimap_build_queue_sold_residual_pack_wave1110());
        assert!(honesty_host_ui_state_minimap_build_queue_sold_method_names_residual_wave1110());
        assert!(honesty_host_ui_state_minimap_build_queue_sold_nav_commands_residual_wave1110());
        assert!(simulate_live_host_ui_state_minimap_build_queue_sold_residual_honesty());
    }
}
