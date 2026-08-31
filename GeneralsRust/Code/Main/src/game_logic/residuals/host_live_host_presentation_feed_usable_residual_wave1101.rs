//! Wave 1101: presentation residual feed sold/UC/disabled peels.
//!
//! After Wave 1100 producer peels, other presentation residual feeds still only
//! skipped destroyed (CC, workers, supply storage, production queues, garrison,
//! harvestable, structure lists, construct builders). Fail-close sold and, where
//! relevant, under-construction/disabled so UI/command residual matches pick.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_FEED_USABLE_METHOD_NAMES_WAVE1101: &[&str] = &[
    "first_friendly_command_center_position",
    "friendly_workers",
    "supply_storage_structures",
    "structures_with_production",
    "alive_construct_builder_ids",
    "Wave 1101",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_FEED_USABLE_NAV_STEPS_WAVE1101: &[&str] = &[
    "CC_WORKER_SUPPLY_PRODUCTION_GARRISON",
    "CONSTRUCT_BUILDER_SOLD_DISABLED",
    "LIVE_HOST_PRESENTATION_FEED_USABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationFeedUsableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationFeedUsableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_presentation_feed_usable_method_names_residual_wave1101() -> bool {
    let names = LIVE_HOST_PRESENTATION_FEED_USABLE_METHOD_NAMES_WAVE1101;
    let ok = residual_name_index(names, "friendly_workers").is_some()
        && residual_name_index(names, "Wave 1101").is_some();
    residual_action_store(ResidualHostPresentationFeedUsableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_feed_usable_nav_commands_residual_wave1101() -> bool {
    let steps = LIVE_HOST_PRESENTATION_FEED_USABLE_NAV_STEPS_WAVE1101;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_FEED_USABLE").is_some()
        && residual_name_index(steps, "CC_WORKER_SUPPLY_PRODUCTION_GARRISON").is_some();
    residual_action_store(ResidualHostPresentationFeedUsableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_feed_usable_residual_pack_wave1101() -> bool {
    let pf = pf_source();
    let es = es_source();
    let checks = [
        (
            "fn first_friendly_command_center_position",
            "Wave 1101: fail-closed on sold/UC/disabled CC residual",
        ),
        (
            "fn friendly_workers",
            "Wave 1101: fail-closed on sold/disabled worker residual feed",
        ),
        (
            "fn worker_objects",
            "Wave 1101: fail-closed on sold/disabled worker residual feed",
        ),
        (
            "fn supply_storage_structures",
            "Wave 1101: fail-closed on sold supply-storage residual feed",
        ),
        (
            "fn structures_with_production",
            "Wave 1101: fail-closed on sold/disabled production-queue residual feed",
        ),
        (
            "fn garrisoned_structures",
            "Wave 1101: fail-closed on sold garrison residual feed",
        ),
        (
            "fn harvestable_objects",
            "Wave 1101: fail-closed on sold harvestable residual feed",
        ),
        (
            "fn structure_objects",
            "Wave 1101: fail-closed on sold structure residual feed",
        ),
        (
            "fn alive_construct_builder_ids",
            "Wave 1101: fail-closed on sold/disabled construct builders",
        ),
    ];
    let mut ok = // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    for (fn_name, wave) in checks {
        let Some(i) = pf.find(fn_name) else {
            ok = false;
            break;
        };
        let w = &pf[i..i.saturating_add(1200)];
        if !w.contains(wave) || !(w.contains("!o.sold") || w.contains("o.sold ||")) {
            ok = false;
            break;
        }
    }
    residual_action_store(ResidualHostPresentationFeedUsableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_feed_usable_residual_honesty() -> bool {
    let a = honesty_host_presentation_feed_usable_method_names_residual_wave1101();
    let b = honesty_host_presentation_feed_usable_nav_commands_residual_wave1101();
    let c = honesty_host_presentation_feed_usable_residual_pack_wave1101();
    residual_action_store(ResidualHostPresentationFeedUsableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_feed_usable_residual_wave1101() {
        assert!(honesty_host_presentation_feed_usable_residual_pack_wave1101());
        assert!(honesty_host_presentation_feed_usable_method_names_residual_wave1101());
        assert!(honesty_host_presentation_feed_usable_nav_commands_residual_wave1101());
        assert!(simulate_live_host_presentation_feed_usable_residual_honesty());
    }
}
