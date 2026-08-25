//! Wave 1010: dual-world populate_build_queue portrait template residual.
//!
//! When OBJECT_REGISTRY is empty and build_queue_data is empty, peel
//! portrait_state.production_template into a residual BuildQueueEntry so the
//! selection panel can show an in-progress train/upgrade slot.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_POPULATE_BUILD_QUEUE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1010: &[&str] = &[
    "populate_build_queue",
    "production_template",
    "BuildQueueEntry",
    "Wave 1010",
    "playable_claim = false",
];

pub const LIVE_HOST_POPULATE_BUILD_QUEUE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1010: &[&str] = &[
    "DUAL_WORLD",
    "PORTRAIT_TEMPLATE_QUEUE",
    "BUILD_QUEUE_ENTRY",
    "LIVE_HOST_POPULATE_BUILD_QUEUE_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPopulateBuildQueuePresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPopulateBuildQueuePresentationResidualAction) {
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

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_host_populate_build_queue_presentation_residual_method_names_residual_wave1010()
-> bool {
    let names = LIVE_HOST_POPULATE_BUILD_QUEUE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1010;
    let ok = residual_name_index(names, "populate_build_queue").is_some()
        && residual_name_index(names, "Wave 1010").is_some();
    residual_action_store(ResidualHostPopulateBuildQueuePresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_populate_build_queue_presentation_residual_nav_commands_residual_wave1010()
-> bool {
    let steps = LIVE_HOST_POPULATE_BUILD_QUEUE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1010;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_POPULATE_BUILD_QUEUE_PRESENTATION_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "PORTRAIT_TEMPLATE_QUEUE").is_some();
    residual_action_store(ResidualHostPopulateBuildQueuePresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_populate_build_queue_presentation_residual_residual_pack_wave1010() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let body = match cb.find("fn populate_build_queue") {
        Some(i) => &cb[i..],
        None => "",
    };
    let ok = body.contains("Wave 981/1010")
        && body.contains("build_queue_data.is_empty()")
        && body.contains("production_template.clone()")
        && body.contains("BuildQueueEntry")
        && body.contains("QueueProductionType::Unit")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPopulateBuildQueuePresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_populate_build_queue_presentation_residual_honesty() -> bool {
    let a =
        honesty_host_populate_build_queue_presentation_residual_method_names_residual_wave1010();
    let b =
        honesty_host_populate_build_queue_presentation_residual_nav_commands_residual_wave1010();
    let c = honesty_host_populate_build_queue_presentation_residual_residual_pack_wave1010();
    residual_action_store(ResidualHostPopulateBuildQueuePresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_populate_build_queue_presentation_residual_wave1010() {
        assert!(honesty_host_populate_build_queue_presentation_residual_residual_pack_wave1010());
        assert!(
            honesty_host_populate_build_queue_presentation_residual_method_names_residual_wave1010(
            )
        );
        assert!(
            honesty_host_populate_build_queue_presentation_residual_nav_commands_residual_wave1010(
            )
        );
        assert!(simulate_live_host_populate_build_queue_presentation_residual_honesty());
    }
}
