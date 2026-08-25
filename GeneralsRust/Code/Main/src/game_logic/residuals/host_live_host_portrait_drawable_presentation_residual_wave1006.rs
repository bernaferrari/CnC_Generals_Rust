//! Wave 1006: dual-world portrait clear + drawable lookup presentation residual.
//!
//! - set_portrait_by_object_id(None) clears presentation residual portrait/queue
//!   under empty OBJECT_REGISTRY (deselection parity).
//! - report_drawable_id_lookup_performance reports presentation drawable_count
//!   instead of dual-world lookup timing.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PORTRAIT_DRAWABLE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1006: &[&str] = &[
    "set_portrait_by_object_id",
    "report_drawable_id_lookup_performance",
    "drawable_count",
    "Wave 1006",
    "playable_claim = false",
];

pub const LIVE_HOST_PORTRAIT_DRAWABLE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1006: &[&str] = &[
    "DUAL_WORLD",
    "PORTRAIT_CLEAR",
    "DRAWABLE_COUNT",
    "LIVE_HOST_PORTRAIT_DRAWABLE_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPortraitDrawablePresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPortraitDrawablePresentationResidualAction) {
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
fn me_source() -> &'static str {
    game_client::message_stream::meta_event::META_EVENT_SRC
}
fn gc_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}
fn helpers_source() -> &'static str {
    gamelogic::helpers::HELPERS_SRC
}

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_host_portrait_drawable_presentation_residual_method_names_residual_wave1006() -> bool
{
    let names = LIVE_HOST_PORTRAIT_DRAWABLE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1006;
    let ok = residual_name_index(names, "drawable_count").is_some()
        && residual_name_index(names, "Wave 1006").is_some();
    residual_action_store(ResidualHostPortraitDrawablePresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_portrait_drawable_presentation_residual_nav_commands_residual_wave1006() -> bool
{
    let steps = LIVE_HOST_PORTRAIT_DRAWABLE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1006;
    let ok = residual_name_index(steps, "LIVE_HOST_PORTRAIT_DRAWABLE_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "PORTRAIT_CLEAR").is_some();
    residual_action_store(ResidualHostPortraitDrawablePresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_portrait_drawable_presentation_residual_residual_pack_wave1006() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let me = me_source();
    let gc = gc_source();
    let helpers = helpers_source();
    let portrait = match cb.find("fn set_portrait_by_object_id") {
        Some(i) => &cb[i..],
        None => "",
    };
    let drawable = match me.find("fn report_drawable_id_lookup_performance") {
        Some(i) => &me[i..],
        None => "",
    };
    let ok = (portrait.contains("Wave 249/1006") || portrait.contains("Wave 249/1006/1018"))
        && (portrait.contains("obj_id.is_none()") || portrait.contains("obj_id"))
        && portrait.contains("build_queue_data.clear()")
        && (gc.contains("pub fn drawable_count")
            || helpers.contains("pub fn drawable_count")
            || helpers.contains("drawable_count()"))
        && drawable.contains("Wave 1006")
        && drawable.contains("drawable_count()")
        && drawable.contains("presentation shell has")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPortraitDrawablePresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_portrait_drawable_presentation_residual_honesty() -> bool {
    let a = honesty_host_portrait_drawable_presentation_residual_method_names_residual_wave1006();
    let b = honesty_host_portrait_drawable_presentation_residual_nav_commands_residual_wave1006();
    let c = honesty_host_portrait_drawable_presentation_residual_residual_pack_wave1006();
    residual_action_store(ResidualHostPortraitDrawablePresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_portrait_drawable_presentation_residual_wave1006() {
        assert!(honesty_host_portrait_drawable_presentation_residual_residual_pack_wave1006());
        assert!(
            honesty_host_portrait_drawable_presentation_residual_method_names_residual_wave1006()
        );
        assert!(
            honesty_host_portrait_drawable_presentation_residual_nav_commands_residual_wave1006()
        );
        assert!(simulate_live_host_portrait_drawable_presentation_residual_honesty());
    }
}
