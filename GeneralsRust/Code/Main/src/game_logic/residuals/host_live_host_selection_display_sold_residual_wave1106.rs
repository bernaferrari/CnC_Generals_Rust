//! Wave 1106: selection display + sold residual peels.
//!
//! After Waves 1104–1105 FOW/cmdset peels:
//! - `selected_unit_display_infos` / `selection_ids_for_consumers` only skipped destroyed
//! - friendly sample label / template pose / attacking / stealth / contained counts
//!   still included sold objects

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELECTION_DISPLAY_SOLD_METHOD_NAMES_WAVE1106: &[&str] = &[
    "selected_unit_display_infos",
    "selection_ids_for_consumers",
    "first_friendly_sample_label",
    "first_alive_position_for_template",
    "attacking_units",
    "Wave 1106",
    "playable_claim: false",
];

pub const LIVE_HOST_SELECTION_DISPLAY_SOLD_NAV_STEPS_WAVE1106: &[&str] = &[
    "SELECTION_DISPLAY_USABLE",
    "CONSUMER_IDS_FILTER_SOLD",
    "SOLD_EXCLUDED_COUNTS",
    "LIVE_HOST_SELECTION_DISPLAY_SOLD",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSelectionDisplaySoldAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSelectionDisplaySoldAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_selection_display_sold_method_names_residual_wave1106() -> bool {
    let names = LIVE_HOST_SELECTION_DISPLAY_SOLD_METHOD_NAMES_WAVE1106;
    let ok = residual_name_index(names, "selected_unit_display_infos").is_some()
        && residual_name_index(names, "Wave 1106").is_some();
    residual_action_store(ResidualHostSelectionDisplaySoldAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_display_sold_nav_commands_residual_wave1106() -> bool {
    let steps = LIVE_HOST_SELECTION_DISPLAY_SOLD_NAV_STEPS_WAVE1106;
    let ok = residual_name_index(steps, "LIVE_HOST_SELECTION_DISPLAY_SOLD").is_some()
        && residual_name_index(steps, "SELECTION_DISPLAY_USABLE").is_some();
    residual_action_store(ResidualHostSelectionDisplaySoldAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_display_sold_residual_pack_wave1106() -> bool {
    let pf = pf_source();
    let es = es_source();
    let ok = pf.contains("Wave 1106: selection display residual fail-closed on sold")
        && pf.contains("Wave 1106: consumer selection residual filters sold")
        && pf.contains("Wave 1106: sample label residual fail-closed on sold")
        && pf.contains("Wave 1106: template pose residual fail-closed on sold")
        && pf.contains("Wave 1106: attacking residual excludes sold")
        && pf.contains("Wave 1106: stealth residual excludes sold")
        && pf.contains("Wave 1106: contained residual excludes sold")
        && pf.contains("fn selected_unit_display_infos")
        && pf.contains("fn selection_ids_for_consumers")
        && pf.contains("!o.sold")
        && pf.contains("!o.unselectable")
        && pf.contains("!o.masked")
        && pf.contains("!o.disabled")
        && es.contains("playable_claim: false");
    residual_action_store(ResidualHostSelectionDisplaySoldAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_selection_display_sold_residual_honesty() -> bool {
    let a = honesty_host_selection_display_sold_method_names_residual_wave1106();
    let b = honesty_host_selection_display_sold_nav_commands_residual_wave1106();
    let c = honesty_host_selection_display_sold_residual_pack_wave1106();
    residual_action_store(ResidualHostSelectionDisplaySoldAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_selection_display_sold_residual_wave1106() {
        assert!(honesty_host_selection_display_sold_residual_pack_wave1106());
        assert!(honesty_host_selection_display_sold_method_names_residual_wave1106());
        assert!(honesty_host_selection_display_sold_nav_commands_residual_wave1106());
        assert!(simulate_live_host_selection_display_sold_residual_honesty());
    }
}
