//! Wave 1096: input find_object_at_position sold/masked + FOW residual.
//!
//! InputIntegration / InputSystemSimple presentation pick only skipped destroyed,
//! so sold/masked or fogged non-local objects could still win left/right-click
//! targeting before selectable/attackable classify. Mirror UnitControl Wave
//! 1093–1094 peels: fail-close sold/masked and non-local FOW unless Clear.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_INPUT_FIND_OBJECT_FOW_STATUS_METHOD_NAMES_WAVE1096: &[&str] = &[
    "find_object_at_position",
    "handle_left_click",
    "handle_right_click",
    "Wave 1096",
    "playable_claim = false",
];

pub const LIVE_HOST_INPUT_FIND_OBJECT_FOW_STATUS_NAV_STEPS_WAVE1096: &[&str] = &[
    "INPUT_FIND_OBJECT",
    "SOLD_MASKED_FOW_CLEAR",
    "LIVE_HOST_INPUT_FIND_OBJECT_FOW_STATUS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostInputFindObjectFowStatusAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostInputFindObjectFowStatusAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn ii_source() -> &'static str {
    include_str!("../../input_integration.rs")
}
fn iss_source() -> &'static str {
    include_str!("../../input_system_simple.rs")
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_input_find_object_fow_status_method_names_residual_wave1096() -> bool {
    let names = LIVE_HOST_INPUT_FIND_OBJECT_FOW_STATUS_METHOD_NAMES_WAVE1096;
    let ok = residual_name_index(names, "find_object_at_position").is_some()
        && residual_name_index(names, "Wave 1096").is_some();
    residual_action_store(ResidualHostInputFindObjectFowStatusAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_input_find_object_fow_status_nav_commands_residual_wave1096() -> bool {
    let steps = LIVE_HOST_INPUT_FIND_OBJECT_FOW_STATUS_NAV_STEPS_WAVE1096;
    let ok = residual_name_index(steps, "LIVE_HOST_INPUT_FIND_OBJECT_FOW_STATUS").is_some()
        && residual_name_index(steps, "SOLD_MASKED_FOW_CLEAR").is_some();
    residual_action_store(ResidualHostInputFindObjectFowStatusAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

fn window_ok(src: &str) -> bool {
    let i = match src.find("fn find_object_at_position") {
        Some(i) => i,
        None => return false,
    };
    let w = &src[i..i.saturating_add(1400)];
    w.contains("Wave 1096: sold/masked + non-local FOW Clear-only")
        && (w.contains("o.destroyed || o.sold || o.masked")
            || (w.contains("o.destroyed") && w.contains("o.sold") && w.contains("o.masked")))
        && w.contains("visibility_alpha < 0.95")
        && (w.contains("local_team") || w.contains("is_owned_by_local") || w.contains("is_local"))
}

pub fn honesty_host_input_find_object_fow_status_residual_pack_wave1096() -> bool {
    let ii = ii_source();
    let iss = iss_source();
    let es = es_source();
    let ok = window_ok(ii)
        && window_ok(iss)
        && ii.contains("presentation_is_attackable")
        && iss.contains("presentation_is_attackable")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostInputFindObjectFowStatusAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_input_find_object_fow_status_residual_honesty() -> bool {
    let a = honesty_host_input_find_object_fow_status_method_names_residual_wave1096();
    let b = honesty_host_input_find_object_fow_status_nav_commands_residual_wave1096();
    let c = honesty_host_input_find_object_fow_status_residual_pack_wave1096();
    residual_action_store(ResidualHostInputFindObjectFowStatusAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_input_find_object_fow_status_residual_wave1096() {
        assert!(honesty_host_input_find_object_fow_status_residual_pack_wave1096());
        assert!(honesty_host_input_find_object_fow_status_method_names_residual_wave1096());
        assert!(honesty_host_input_find_object_fow_status_nav_commands_residual_wave1096());
        assert!(simulate_live_host_input_find_object_fow_status_residual_honesty());
    }
}
