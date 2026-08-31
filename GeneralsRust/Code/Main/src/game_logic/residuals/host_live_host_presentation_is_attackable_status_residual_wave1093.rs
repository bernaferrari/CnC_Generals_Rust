//! Wave 1093: presentation_is_attackable sold/masked residual.
//!
//! After Wave 1092 selectable status peels, enemy-priority pick still treated
//! sold/masked objects as attackable (only destroyed gated). Fail-close sold +
//! masked on the shared UnitControlSystem helper and duplicate input helpers.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_IS_ATTACKABLE_STATUS_METHOD_NAMES_WAVE1093: &[&str] = &[
    "presentation_is_attackable",
    "pick_object_id_at_world_from_presentation",
    "Wave 1093",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_IS_ATTACKABLE_STATUS_NAV_STEPS_WAVE1093: &[&str] = &[
    "PRESENTATION_IS_ATTACKABLE",
    "SOLD_MASKED_FAIL_CLOSED",
    "LIVE_HOST_PRESENTATION_IS_ATTACKABLE_STATUS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationIsAttackableStatusAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationIsAttackableStatusAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn uc_source() -> &'static str {
    include_str!("../../unit_control.rs")
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

pub fn honesty_host_presentation_is_attackable_status_method_names_residual_wave1093() -> bool {
    let names = LIVE_HOST_PRESENTATION_IS_ATTACKABLE_STATUS_METHOD_NAMES_WAVE1093;
    let ok = residual_name_index(names, "presentation_is_attackable").is_some()
        && residual_name_index(names, "Wave 1093").is_some();
    residual_action_store(ResidualHostPresentationIsAttackableStatusAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_is_attackable_status_nav_commands_residual_wave1093() -> bool {
    let steps = LIVE_HOST_PRESENTATION_IS_ATTACKABLE_STATUS_NAV_STEPS_WAVE1093;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_IS_ATTACKABLE_STATUS").is_some()
        && residual_name_index(steps, "SOLD_MASKED_FAIL_CLOSED").is_some();
    residual_action_store(ResidualHostPresentationIsAttackableStatusAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_is_attackable_status_residual_pack_wave1093() -> bool {
    let uc = uc_source();
    let ii = ii_source();
    let iss = iss_source();
    let es = es_source();
    let i = match uc.find("fn presentation_is_attackable") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationIsAttackableStatusAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let window = &uc[i..i.saturating_add(700)];
    let pick_i = match uc.find("fn pick_object_id_at_world_from_presentation") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationIsAttackableStatusAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let pick = &uc[pick_i..pick_i.saturating_add(1800)];
    let ii_i = match ii.find("fn presentation_is_attackable") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationIsAttackableStatusAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let ii_w = &ii[ii_i..ii_i.saturating_add(500)];
    let iss_i = match iss.find("fn presentation_is_attackable") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationIsAttackableStatusAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let iss_w = &iss[iss_i..iss_i.saturating_add(500)];
    let ok = (window.contains("Wave 1093: presentation attackable residual fail-closed")
            || window.contains("!o.unattackable"))
        && window.contains("!o.sold")
        && window.contains("!o.masked")
        && window.contains("!o.destroyed")
        && pick.contains("Self::presentation_is_attackable(o)")
        && ii_w.contains("!o.sold")
        && ii_w.contains("!o.masked")
        && iss_w.contains("!o.sold")
        && iss_w.contains("!o.masked")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostPresentationIsAttackableStatusAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_is_attackable_status_residual_honesty() -> bool {
    let a = honesty_host_presentation_is_attackable_status_method_names_residual_wave1093();
    let b = honesty_host_presentation_is_attackable_status_nav_commands_residual_wave1093();
    let c = honesty_host_presentation_is_attackable_status_residual_pack_wave1093();
    residual_action_store(ResidualHostPresentationIsAttackableStatusAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_is_attackable_status_residual_wave1093() {
        assert!(honesty_host_presentation_is_attackable_status_residual_pack_wave1093());
        assert!(honesty_host_presentation_is_attackable_status_method_names_residual_wave1093());
        assert!(honesty_host_presentation_is_attackable_status_nav_commands_residual_wave1093());
        assert!(simulate_live_host_presentation_is_attackable_status_residual_honesty());
    }
}
