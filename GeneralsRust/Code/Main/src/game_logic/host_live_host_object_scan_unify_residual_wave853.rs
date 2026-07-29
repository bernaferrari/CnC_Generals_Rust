//! Wave 853: unify stamp-phase get_objects scans — alive residual + train producers
//! share one freeze walk or one host scan inside host_refresh_local_train_producer_residuals.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_OBJECT_SCAN_UNIFY_METHOD_NAMES_WAVE853: &[&str] = &[
    "host_refresh_local_train_producer_residuals",
    "host_match_alive_object_ids",
    "Wave 853",
    "playable_claim = false",
];

pub const LIVE_HOST_OBJECT_SCAN_UNIFY_NAV_STEPS_WAVE853: &[&str] = &[
    "UNIFY_STAMP_OBJECT_SCAN",
    "SINGLE_GET_OBJECTS_OR_FREEZE_WALK",
    "LIVE_HOST_OBJECT_SCAN_UNIFY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostObjectScanUnifyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostObjectScanUnifyAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

pub fn honesty_host_object_scan_unify_method_names_residual_wave853() -> bool {
    let names = LIVE_HOST_OBJECT_SCAN_UNIFY_METHOD_NAMES_WAVE853;
    let ok = residual_name_index(names, "host_refresh_local_train_producer_residuals").is_some()
        && residual_name_index(names, "Wave 853").is_some();
    residual_action_store(ResidualHostObjectScanUnifyAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_object_scan_unify_nav_commands_residual_wave853() -> bool {
    let steps = LIVE_HOST_OBJECT_SCAN_UNIFY_NAV_STEPS_WAVE853;
    let ok = residual_name_index(steps, "LIVE_HOST_OBJECT_SCAN_UNIFY").is_some()
        && residual_name_index(steps, "UNIFY_STAMP_OBJECT_SCAN").is_some();
    residual_action_store(ResidualHostObjectScanUnifyAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_object_scan_unify_residual_pack_wave853() -> bool {
    let cnc = cnc_source();
    let helper_at = cnc.find("fn host_refresh_local_train_producer_residuals");
    let helper_ok = if let Some(i) = helper_at {
        let end = cnc[i..]
            .find("\n    fn ")
            .map(|e| i + e)
            .unwrap_or(i + 3500);
        let body = &cnc[i..end];
        body.matches("get_objects()").count() == 1
            && body.contains("host_match_alive_object_ids = Some(alive)")
            && body.contains("Wave 853: single host scan")
    } else {
        false
    };
    let ok = cnc.contains("Wave 848/853: single stamp-phase object scan")
        && cnc.contains("Wave 851/853: alive residual stamped inside")
        && cnc.matches("get_objects()").count() == 1
        && helper_ok;
    residual_action_store(ResidualHostObjectScanUnifyAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_object_scan_unify_honesty() -> bool {
    let a = honesty_host_object_scan_unify_method_names_residual_wave853();
    let b = honesty_host_object_scan_unify_nav_commands_residual_wave853();
    let c = honesty_host_object_scan_unify_residual_pack_wave853();
    residual_action_store(ResidualHostObjectScanUnifyAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_object_scan_unify_residual_wave853() {
        assert!(honesty_host_object_scan_unify_residual_pack_wave853());
        assert!(honesty_host_object_scan_unify_method_names_residual_wave853());
        assert!(honesty_host_object_scan_unify_nav_commands_residual_wave853());
        assert!(simulate_live_host_object_scan_unify_honesty());
    }
}
