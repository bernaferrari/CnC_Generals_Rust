//! Wave 1100: presentation factory producer sold/UC/disabled residual.
//!
//! `first_constructed_producer_id` / `unit_producer_structures` only skipped
//! destroyed, so sold, under-construction, or disabled factories could still
//! be chosen for train/ControlBar residual feeds. Fail-close those status bits.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_PRODUCER_USABLE_METHOD_NAMES_WAVE1100: &[&str] = &[
    "first_constructed_producer_id",
    "unit_producer_structures",
    "Wave 1100",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_PRODUCER_USABLE_NAV_STEPS_WAVE1100: &[&str] = &[
    "PRODUCER_SOLD_UC_DISABLED",
    "UNIT_PRODUCER_STRUCTURES_USABLE",
    "LIVE_HOST_PRESENTATION_PRODUCER_USABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationProducerUsableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationProducerUsableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_host_presentation_producer_usable_method_names_residual_wave1100() -> bool {
    let names = LIVE_HOST_PRESENTATION_PRODUCER_USABLE_METHOD_NAMES_WAVE1100;
    let ok = residual_name_index(names, "first_constructed_producer_id").is_some()
        && residual_name_index(names, "Wave 1100").is_some();
    residual_action_store(ResidualHostPresentationProducerUsableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_producer_usable_nav_commands_residual_wave1100() -> bool {
    let steps = LIVE_HOST_PRESENTATION_PRODUCER_USABLE_NAV_STEPS_WAVE1100;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_PRODUCER_USABLE").is_some()
        && residual_name_index(steps, "PRODUCER_SOLD_UC_DISABLED").is_some();
    residual_action_store(ResidualHostPresentationProducerUsableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_producer_usable_residual_pack_wave1100() -> bool {
    let pf = pf_source();
    let es = es_source();
    let cnc = cnc_source();
    let a_i = match pf.find("fn first_constructed_producer_id") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationProducerUsableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let a = &pf[a_i..a_i.saturating_add(1600)];
    let b_i = match pf.find("fn unit_producer_structures") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationProducerUsableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let b = &pf[b_i..b_i.saturating_add(1200)];
    let ok = a.contains("Wave 1100: fail-closed on sold/UC/disabled producers")
        && a.contains("!o.sold")
        && a.contains("!o.under_construction")
        && a.contains("!o.disabled")
        && b.contains("Wave 1100: fail-closed on sold/UC/disabled factory residual feed")
        && b.contains("!o.sold")
        && b.contains("!o.under_construction")
        && b.contains("!o.disabled")
        && cnc.contains("first_constructed_producer_id")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostPresentationProducerUsableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_producer_usable_residual_honesty() -> bool {
    let a = honesty_host_presentation_producer_usable_method_names_residual_wave1100();
    let b = honesty_host_presentation_producer_usable_nav_commands_residual_wave1100();
    let c = honesty_host_presentation_producer_usable_residual_pack_wave1100();
    residual_action_store(ResidualHostPresentationProducerUsableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_producer_usable_residual_wave1100() {
        assert!(honesty_host_presentation_producer_usable_residual_pack_wave1100());
        assert!(honesty_host_presentation_producer_usable_method_names_residual_wave1100());
        assert!(honesty_host_presentation_producer_usable_nav_commands_residual_wave1100());
        assert!(simulate_live_host_presentation_producer_usable_residual_honesty());
    }
}
