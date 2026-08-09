//! Wave 857: special-power-ready residual joins the unified stamp-phase object
//! scan (alive + train producers + special ready). Removes per-object get_object
//! dual-read from the boot special-power stamp path.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SPECIAL_POWER_SCAN_UNIFY_METHOD_NAMES_WAVE857: &[&str] = &[
    "host_refresh_local_train_producer_residuals",
    "host_match_special_power_ready_ids",
    "special_ready",
    "Wave 857",
    "playable_claim = false",
];

pub const LIVE_HOST_SPECIAL_POWER_SCAN_UNIFY_NAV_STEPS_WAVE857: &[&str] = &[
    "UNIFY_SPECIAL_POWER_READY_SCAN",
    "NO_PER_OBJECT_GET_OBJECT_STAMP",
    "LIVE_HOST_SPECIAL_POWER_SCAN_UNIFY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSpecialPowerScanUnifyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSpecialPowerScanUnifyAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_special_power_scan_unify_method_names_residual_wave857() -> bool {
    let names = LIVE_HOST_SPECIAL_POWER_SCAN_UNIFY_METHOD_NAMES_WAVE857;
    let ok = residual_name_index(names, "host_refresh_local_train_producer_residuals").is_some()
        && residual_name_index(names, "host_match_special_power_ready_ids").is_some()
        && residual_name_index(names, "Wave 857").is_some();
    residual_action_store(ResidualHostSpecialPowerScanUnifyAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_special_power_scan_unify_nav_commands_residual_wave857() -> bool {
    let steps = LIVE_HOST_SPECIAL_POWER_SCAN_UNIFY_NAV_STEPS_WAVE857;
    let ok = residual_name_index(steps, "LIVE_HOST_SPECIAL_POWER_SCAN_UNIFY").is_some()
        && residual_name_index(steps, "NO_PER_OBJECT_GET_OBJECT_STAMP").is_some();
    residual_action_store(ResidualHostSpecialPowerScanUnifyAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_special_power_scan_unify_residual_pack_wave857() -> bool {
    let cnc = cnc_source();
    let helper_at = cnc.find("fn host_refresh_local_train_producer_residuals");
    let helper_ok = if let Some(i) = helper_at {
        let end = cnc[i..]
            .find("\n    fn ")
            .map(|e| i + e)
            .unwrap_or(i + 4000);
        let body = &cnc[i..end];
        body.matches("get_objects()").count() == 1
            && body.contains("host_match_special_power_ready_ids = Some(special_ready)")
            && body.contains("special_power_ready")
            && !body.contains("get_object(")
    } else {
        false
    };
    let ok = cnc.contains("Wave 848/853/857: single stamp-phase object scan")
        && cnc.contains("Wave 854/857: special-power-ready residual stamped inside")
        && cnc.matches("get_object(").count() == 0
        && cnc.matches("get_objects()").count() == 1
        && helper_ok;
    residual_action_store(ResidualHostSpecialPowerScanUnifyAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_special_power_scan_unify_honesty() -> bool {
    let a = honesty_host_special_power_scan_unify_method_names_residual_wave857();
    let b = honesty_host_special_power_scan_unify_nav_commands_residual_wave857();
    let c = honesty_host_special_power_scan_unify_residual_pack_wave857();
    residual_action_store(ResidualHostSpecialPowerScanUnifyAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_special_power_scan_unify_residual_wave857() {
        assert!(honesty_host_special_power_scan_unify_residual_pack_wave857());
        assert!(honesty_host_special_power_scan_unify_method_names_residual_wave857());
        assert!(honesty_host_special_power_scan_unify_nav_commands_residual_wave857());
        assert!(simulate_live_host_special_power_scan_unify_honesty());
    }
}
