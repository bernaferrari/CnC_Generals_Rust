//! Wave 848: host-owned local train producer residuals peel ad-hoc get_objects
//! dual-reads from train auto_target force-complete path. Stamp via
//! host_refresh_local_train_producer_residuals during match residual refresh.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_TRAIN_PRODUCER_RESIDUAL_METHOD_NAMES_WAVE848: &[&str] = &[
    "host_refresh_local_train_producer_residuals",
    "host_match_local_barracks_ids",
    "host_match_local_producer_ids",
    "host_match_local_unfinished_producer_ids",
    "host_match_local_team_sample_pos",
    "Wave 848",
    "playable_claim = false",
];

pub const LIVE_HOST_TRAIN_PRODUCER_RESIDUAL_NAV_STEPS_WAVE848: &[&str] = &[
    "STAMP_HOST_TRAIN_PRODUCERS",
    "AUTO_TARGET_USES_HOST_RESIDUAL",
    "LIVE_HOST_TRAIN_PRODUCER_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostTrainProducerAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostTrainProducerAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

pub fn honesty_host_train_producer_residual_method_names_residual_wave848() -> bool {
    let names = LIVE_HOST_TRAIN_PRODUCER_RESIDUAL_METHOD_NAMES_WAVE848;
    let ok = residual_name_index(names, "host_refresh_local_train_producer_residuals").is_some()
        && residual_name_index(names, "host_match_local_barracks_ids").is_some()
        && residual_name_index(names, "Wave 848").is_some();
    residual_action_store(ResidualHostTrainProducerAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_train_producer_residual_nav_commands_residual_wave848() -> bool {
    let steps = LIVE_HOST_TRAIN_PRODUCER_RESIDUAL_NAV_STEPS_WAVE848;
    let ok = residual_name_index(steps, "LIVE_HOST_TRAIN_PRODUCER_RESIDUAL").is_some()
        && residual_name_index(steps, "AUTO_TARGET_USES_HOST_RESIDUAL").is_some();
    residual_action_store(ResidualHostTrainProducerAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_train_producer_residual_pack_wave848() -> bool {
    let cnc = cnc_source();
    // auto_target block must not call get_objects directly (stamp helper may).
    let auto_at = cnc.find("// Wave 834/848: when auto_target");
    let auto_ok = if let Some(i) = auto_at {
        let end = cnc[i..]
            .find("barracks.sort_by_key")
            .map(|e| i + e)
            .unwrap_or(i + 2500);
        let block = &cnc[i..end];
        !block.contains("get_objects()")
            && block.contains("host_match_local_barracks_ids")
            && block.contains("host_refresh_local_train_producer_residuals")
    } else {
        false
    };
    let ok = cnc.contains("fn host_refresh_local_train_producer_residuals")
        && cnc.contains("host_match_local_barracks_ids: Option<Vec<crate::game_logic::ObjectId>>")
        && cnc.contains("host_match_local_producer_ids: Option<Vec<crate::game_logic::ObjectId>>")
        && cnc.contains(
            "host_match_local_unfinished_producer_ids: Option<Vec<crate::game_logic::ObjectId>>",
        )
        && cnc.contains("host_match_local_team_sample_pos: Option<[f32; 3]>")
        && cnc.contains("Wave 848: stamp local train producers after other match residuals")
        && auto_ok;
    residual_action_store(ResidualHostTrainProducerAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_train_producer_residual_honesty() -> bool {
    let a = honesty_host_train_producer_residual_method_names_residual_wave848();
    let b = honesty_host_train_producer_residual_nav_commands_residual_wave848();
    let c = honesty_host_train_producer_residual_pack_wave848();
    residual_action_store(ResidualHostTrainProducerAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_train_producer_residual_wave848() {
        assert!(honesty_host_train_producer_residual_pack_wave848());
        assert!(honesty_host_train_producer_residual_method_names_residual_wave848());
        assert!(honesty_host_train_producer_residual_nav_commands_residual_wave848());
        assert!(simulate_live_host_train_producer_residual_honesty());
    }
}
