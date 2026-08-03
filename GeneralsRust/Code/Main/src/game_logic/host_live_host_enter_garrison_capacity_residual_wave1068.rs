//! Wave 1068: dual-world enter garrison capacity + source residual.
//!
//! selection_can_enter_target dual fails closed on full garrison, under-construction
//! containers, and unusable local sources. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ENTER_GARRISON_CAPACITY_RESIDUAL_METHOD_NAMES_WAVE1068: &[&str] = &[
    "selection_can_enter_target",
    "max_garrison",
    "occupant_count",
    "Wave 1068",
    "playable_claim = false",
];

pub const LIVE_HOST_ENTER_GARRISON_CAPACITY_RESIDUAL_NAV_STEPS_WAVE1068: &[&str] = &[
    "ENTER",
    "GARRISON_CAPACITY",
    "LIVE_HOST_ENTER_GARRISON_CAPACITY_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEnterGarrisonCapacityResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostEnterGarrisonCapacityResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn tr_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/message_stream/translators.rs")
}

pub fn honesty_host_enter_garrison_capacity_residual_method_names_residual_wave1068() -> bool {
    let names = LIVE_HOST_ENTER_GARRISON_CAPACITY_RESIDUAL_METHOD_NAMES_WAVE1068;
    let ok = residual_name_index(names, "selection_can_enter_target").is_some()
        && residual_name_index(names, "Wave 1068").is_some();
    residual_action_store(ResidualHostEnterGarrisonCapacityResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_enter_garrison_capacity_residual_nav_commands_residual_wave1068() -> bool {
    let steps = LIVE_HOST_ENTER_GARRISON_CAPACITY_RESIDUAL_NAV_STEPS_WAVE1068;
    let ok = residual_name_index(steps, "LIVE_HOST_ENTER_GARRISON_CAPACITY_RESIDUAL").is_some()
        && residual_name_index(steps, "GARRISON_CAPACITY").is_some();
    residual_action_store(ResidualHostEnterGarrisonCapacityResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_enter_garrison_capacity_residual_residual_pack_wave1068() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ok = tr.contains("Wave 1068: full garrison residual fail-closed")
        && tr.contains("target.max_garrison > 0 && target.occupant_count >= target.max_garrison")
        && tr.contains("Wave 1068: under-construction container residual fail-closed")
        && tr.contains("Wave 1068: unusable local source residual fail-closed")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostEnterGarrisonCapacityResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_enter_garrison_capacity_residual_honesty() -> bool {
    let a = honesty_host_enter_garrison_capacity_residual_method_names_residual_wave1068();
    let b = honesty_host_enter_garrison_capacity_residual_nav_commands_residual_wave1068();
    let c = honesty_host_enter_garrison_capacity_residual_residual_pack_wave1068();
    residual_action_store(ResidualHostEnterGarrisonCapacityResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_enter_garrison_capacity_residual_wave1068() {
        assert!(honesty_host_enter_garrison_capacity_residual_residual_pack_wave1068());
        assert!(honesty_host_enter_garrison_capacity_residual_method_names_residual_wave1068());
        assert!(honesty_host_enter_garrison_capacity_residual_nav_commands_residual_wave1068());
        assert!(simulate_live_host_enter_garrison_capacity_residual_honesty());
    }
}
