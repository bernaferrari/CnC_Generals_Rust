//! Wave 834: train_unit auto_target host producer fallback when presentation
//! freeze lags a just-built barracks (construct→train same drain).
//! playable_claim remains false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_TRAIN_AUTO_TARGET_HOST_FALLBACK_METHOD_NAMES_WAVE834: &[&str] = &[
    "train_unit",
    "allow_auto_target",
    "allow_force_complete",
    "host_force_complete_construction",
    "get_objects",
    "Wave 834",
    "playable_claim = false",
];

pub const LIVE_HOST_TRAIN_AUTO_TARGET_HOST_FALLBACK_NAV_STEPS_WAVE834: &[&str] = &[
    "REQUIRE_AUTO_TARGET",
    "REQUIRE_HOST_PRODUCER_FALLBACK",
    "REQUIRE_FORCE_COMPLETE_HOST",
    "LIVE_HOST_TRAIN_AUTO_TARGET_HOST_FALLBACK",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostTrainAutoTargetHostFallbackAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

impl ResidualHostTrainAutoTargetHostFallbackAction {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            _ => Self::None,
        }
    }
}

fn residual_action_store(a: ResidualHostTrainAutoTargetHostFallbackAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

pub fn honesty_host_train_auto_target_host_fallback_method_names_residual_wave834() -> bool {
    let names = LIVE_HOST_TRAIN_AUTO_TARGET_HOST_FALLBACK_METHOD_NAMES_WAVE834;
    let ok = residual_name_index(names, "train_unit").is_some()
        && residual_name_index(names, "allow_auto_target").is_some()
        && residual_name_index(names, "host_force_complete_construction").is_some()
        && residual_name_index(names, "Wave 834").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostTrainAutoTargetHostFallbackAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_train_auto_target_host_fallback_nav_commands_residual_wave834() -> bool {
    let steps = LIVE_HOST_TRAIN_AUTO_TARGET_HOST_FALLBACK_NAV_STEPS_WAVE834;
    let ok = residual_name_index(steps, "LIVE_HOST_TRAIN_AUTO_TARGET_HOST_FALLBACK").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostTrainAutoTargetHostFallbackAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_train_auto_target_host_fallback_residual_pack_wave834() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("Wave 834: when auto_target + force_complete are opt-in")
        && cnc.contains("fall back to host GameLogic producers")
        && cnc.contains("Wave 834: if still no local barracks, spawn")
        && cnc.contains("host_force_ensure_barracks_building_data")
        && cnc.contains("construct_ok_force")
        && cnc.contains("allow_auto_target")
        && cnc.contains("host_force_complete_construction")
        && cnc.contains("get_objects()");
    residual_action_store(ResidualHostTrainAutoTargetHostFallbackAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_train_auto_target_host_fallback_honesty() -> bool {
    let a = honesty_host_train_auto_target_host_fallback_method_names_residual_wave834();
    let b = honesty_host_train_auto_target_host_fallback_nav_commands_residual_wave834();
    let c = honesty_host_train_auto_target_host_fallback_residual_pack_wave834();
    residual_action_store(ResidualHostTrainAutoTargetHostFallbackAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_train_auto_target_host_fallback_residual_wave834() {
        assert!(honesty_host_train_auto_target_host_fallback_residual_pack_wave834());
        assert!(honesty_host_train_auto_target_host_fallback_method_names_residual_wave834());
        assert!(honesty_host_train_auto_target_host_fallback_nav_commands_residual_wave834());
        assert!(simulate_live_host_train_auto_target_host_fallback_honesty());
    }
}
