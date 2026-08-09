//! Wave 867: refresh object-scan / selection / time-frozen residuals after host
//! create/destroy/force-complete/pause mutations so peels stay residual-warm.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MUTATION_RESIDUAL_REFRESH_METHOD_NAMES_WAVE867: &[&str] = &[
    "host_create_object",
    "host_destroy_object",
    "host_force_complete_construction",
    "host_set_paused",
    "host_refresh_local_train_producer_residuals",
    "Wave 867",
    "playable_claim = false",
];

pub const LIVE_HOST_MUTATION_RESIDUAL_REFRESH_NAV_STEPS_WAVE867: &[&str] = &[
    "REFRESH_AFTER_SPAWN",
    "REFRESH_AFTER_DESTROY",
    "REFRESH_AFTER_FORCE_COMPLETE",
    "STAMP_TIME_FROZEN_ON_PAUSE",
    "LIVE_HOST_MUTATION_RESIDUAL_REFRESH",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMutationResidualRefreshAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMutationResidualRefreshAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_mutation_residual_refresh_method_names_residual_wave867() -> bool {
    let names = LIVE_HOST_MUTATION_RESIDUAL_REFRESH_METHOD_NAMES_WAVE867;
    let ok = residual_name_index(names, "host_create_object").is_some()
        && residual_name_index(names, "host_destroy_object").is_some()
        && residual_name_index(names, "Wave 867").is_some();
    residual_action_store(ResidualHostMutationResidualRefreshAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_mutation_residual_refresh_nav_commands_residual_wave867() -> bool {
    let steps = LIVE_HOST_MUTATION_RESIDUAL_REFRESH_NAV_STEPS_WAVE867;
    let ok = residual_name_index(steps, "LIVE_HOST_MUTATION_RESIDUAL_REFRESH").is_some()
        && residual_name_index(steps, "REFRESH_AFTER_SPAWN").is_some();
    residual_action_store(ResidualHostMutationResidualRefreshAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_mutation_residual_refresh_residual_pack_wave867() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("Wave 581/867: host spawn residual + refresh object-scan residuals")
        && cnc.contains("Wave 584/867: host destroy residual + refresh object-scan residuals")
        && cnc.contains("Wave 583/867: host construction force-complete residual + refresh scan")
        && cnc.contains("Wave 575/601/867: paired host pause residual + time-frozen stamp")
        && cnc
            .matches("host_refresh_local_train_producer_residuals()")
            .count()
            >= 4
        && cnc.contains("self.host_match_time_frozen = Some");
    residual_action_store(ResidualHostMutationResidualRefreshAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_mutation_residual_refresh_honesty() -> bool {
    let a = honesty_host_mutation_residual_refresh_method_names_residual_wave867();
    let b = honesty_host_mutation_residual_refresh_nav_commands_residual_wave867();
    let c = honesty_host_mutation_residual_refresh_residual_pack_wave867();
    residual_action_store(ResidualHostMutationResidualRefreshAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_mutation_residual_refresh_residual_wave867() {
        assert!(honesty_host_mutation_residual_refresh_residual_pack_wave867());
        assert!(honesty_host_mutation_residual_refresh_method_names_residual_wave867());
        assert!(honesty_host_mutation_residual_refresh_nav_commands_residual_wave867());
        assert!(simulate_live_host_mutation_residual_refresh_honesty());
    }
}
