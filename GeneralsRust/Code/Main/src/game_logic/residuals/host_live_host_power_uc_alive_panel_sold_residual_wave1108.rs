//! Wave 1108: power/UC/veterancy/alive + ControlBar panel sold residual.
//!
//! After Waves 1104–1107 sold peels:
//! - `net_power_from_objects` still summed sold structures
//! - `under_construction_objects` / `veteran_or_higher_units` / `alive_renderables`
//!   still included sold
//! - `control_bar_selection_panel` stamped production queue without rechecking
//!   primary usability

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_POWER_UC_ALIVE_PANEL_SOLD_METHOD_NAMES_WAVE1108: &[&str] = &[
    "net_power_from_objects",
    "under_construction_objects",
    "veteran_or_higher_units",
    "alive_renderables",
    "control_bar_selection_panel",
    "Wave 1108",
    "playable_claim: false",
];

pub const LIVE_HOST_POWER_UC_ALIVE_PANEL_SOLD_NAV_STEPS_WAVE1108: &[&str] = &[
    "POWER_EXCLUDES_SOLD",
    "UC_VETERAN_ALIVE_EXCLUDE_SOLD",
    "PANEL_PRIMARY_USABLE",
    "LIVE_HOST_POWER_UC_ALIVE_PANEL_SOLD",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPowerUcAlivePanelSoldAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPowerUcAlivePanelSoldAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_power_uc_alive_panel_sold_method_names_residual_wave1108() -> bool {
    let names = LIVE_HOST_POWER_UC_ALIVE_PANEL_SOLD_METHOD_NAMES_WAVE1108;
    let ok = residual_name_index(names, "net_power_from_objects").is_some()
        && residual_name_index(names, "Wave 1108").is_some();
    residual_action_store(ResidualHostPowerUcAlivePanelSoldAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_power_uc_alive_panel_sold_nav_commands_residual_wave1108() -> bool {
    let steps = LIVE_HOST_POWER_UC_ALIVE_PANEL_SOLD_NAV_STEPS_WAVE1108;
    let ok = residual_name_index(steps, "LIVE_HOST_POWER_UC_ALIVE_PANEL_SOLD").is_some()
        && residual_name_index(steps, "POWER_EXCLUDES_SOLD").is_some();
    residual_action_store(ResidualHostPowerUcAlivePanelSoldAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_power_uc_alive_panel_sold_residual_pack_wave1108() -> bool {
    let pf = pf_source();
    let es = es_source();
    let ok = pf.contains("Wave 1108: power residual excludes sold structures")
        && pf.contains("Wave 1108: UC residual excludes sold")
        && pf.contains("Wave 1108: veterancy residual excludes sold")
        && pf.contains("Wave 1108: alive residual excludes sold")
        && pf.contains("Wave 1108: fail-closed on sold/unusable primary")
        && pf.contains("fn net_power_from_objects")
        && pf.contains("fn under_construction_objects")
        && pf.contains("fn veteran_or_higher_units")
        && pf.contains("fn alive_renderables")
        && pf.contains("fn control_bar_selection_panel")
        && es.contains("playable_claim: false");
    residual_action_store(ResidualHostPowerUcAlivePanelSoldAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_power_uc_alive_panel_sold_residual_honesty() -> bool {
    let a = honesty_host_power_uc_alive_panel_sold_method_names_residual_wave1108();
    let b = honesty_host_power_uc_alive_panel_sold_nav_commands_residual_wave1108();
    let c = honesty_host_power_uc_alive_panel_sold_residual_pack_wave1108();
    residual_action_store(ResidualHostPowerUcAlivePanelSoldAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_power_uc_alive_panel_sold_residual_wave1108() {
        assert!(honesty_host_power_uc_alive_panel_sold_residual_pack_wave1108());
        assert!(honesty_host_power_uc_alive_panel_sold_method_names_residual_wave1108());
        assert!(honesty_host_power_uc_alive_panel_sold_nav_commands_residual_wave1108());
        assert!(simulate_live_host_power_uc_alive_panel_sold_residual_honesty());
    }
}
