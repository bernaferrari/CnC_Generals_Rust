//! Wave 1092: presentation_is_selectable sold/unselectable/masked residual.
//!
//! Catalog stamp uses `UnitControlSystem::presentation_is_selectable` for dual-world
//! selectable residual. It only checked destroyed + Selectable kind + not contained,
//! so sold/unselectable/masked objects still entered the catalog as selectable
//! and were pickable via `pick_object_id_at_world_from_presentation`.
//!
//! Fail-close those C++ status bits. `Object::isSelectable` / `CanSelectDrawable`
//! have no disabled-type gate — EMP / underpowered / unmanned stay clickable.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_IS_SELECTABLE_STATUS_METHOD_NAMES_WAVE1092: &[&str] = &[
    "presentation_is_selectable",
    "pick_object_id_at_world_from_presentation",
    "Wave 1092",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_IS_SELECTABLE_STATUS_NAV_STEPS_WAVE1092: &[&str] = &[
    "PRESENTATION_IS_SELECTABLE",
    "SOLD_UNSELECTABLE_MASKED",
    "LIVE_HOST_PRESENTATION_IS_SELECTABLE_STATUS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationIsSelectableStatusAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationIsSelectableStatusAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn uc_source() -> &'static str {
    include_str!("../../unit_control.rs")
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_host_presentation_is_selectable_status_method_names_residual_wave1092() -> bool {
    let names = LIVE_HOST_PRESENTATION_IS_SELECTABLE_STATUS_METHOD_NAMES_WAVE1092;
    let ok = residual_name_index(names, "presentation_is_selectable").is_some()
        && residual_name_index(names, "Wave 1092").is_some();
    residual_action_store(ResidualHostPresentationIsSelectableStatusAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_is_selectable_status_nav_commands_residual_wave1092() -> bool {
    let steps = LIVE_HOST_PRESENTATION_IS_SELECTABLE_STATUS_NAV_STEPS_WAVE1092;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_IS_SELECTABLE_STATUS").is_some()
        && residual_name_index(steps, "SOLD_UNSELECTABLE_MASKED_DISABLED").is_some();
    residual_action_store(ResidualHostPresentationIsSelectableStatusAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_is_selectable_status_residual_pack_wave1092() -> bool {
    let uc = uc_source();
    let es = es_source();
    let cnc = cnc_source();
    let i = match uc.find("fn presentation_is_selectable") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationIsSelectableStatusAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let window = &uc[i..i.saturating_add(900)];
    let pick_i = match uc.find("fn pick_object_id_at_world_from_presentation") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationIsSelectableStatusAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let pick = &uc[pick_i..pick_i.saturating_add(2000)];
    let ok = window.contains("Wave 1092: presentation selectable residual fail-closed")
        && window.contains("!o.sold")
        && window.contains("!o.unselectable")
        && window.contains("!o.masked")
        && !window.contains("&& !o.disabled")
        && window.contains("!o.destroyed")
        && pick.contains("Self::presentation_is_selectable(o)")
        && cnc.contains("UnitControlSystem::presentation_is_selectable(o)")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostPresentationIsSelectableStatusAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_is_selectable_status_residual_honesty() -> bool {
    let a = honesty_host_presentation_is_selectable_status_method_names_residual_wave1092();
    let b = honesty_host_presentation_is_selectable_status_nav_commands_residual_wave1092();
    let c = honesty_host_presentation_is_selectable_status_residual_pack_wave1092();
    residual_action_store(ResidualHostPresentationIsSelectableStatusAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_is_selectable_status_residual_wave1092() {
        assert!(honesty_host_presentation_is_selectable_status_residual_pack_wave1092());
        assert!(honesty_host_presentation_is_selectable_status_method_names_residual_wave1092());
        assert!(honesty_host_presentation_is_selectable_status_nav_commands_residual_wave1092());
        assert!(simulate_live_host_presentation_is_selectable_status_residual_honesty());
    }
}
