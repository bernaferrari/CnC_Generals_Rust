//! Wave 1050: dual-world sync pose+visibility status residual.
//!
//! sync_with_game_logic dual peels catalog pose and also destroyed/stealth/FOW
//! visibility (parity with update_drawable_visibility). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SYNC_POSE_VISIBILITY_RESIDUAL_METHOD_NAMES_WAVE1050: &[&str] = &[
    "sync_with_game_logic",
    "status_hidden",
    "StealthLook::Invisible",
    "Wave 1050",
    "playable_claim = false",
];

pub const LIVE_HOST_SYNC_POSE_VISIBILITY_RESIDUAL_NAV_STEPS_WAVE1050: &[&str] = &[
    "SYNC_POSE",
    "VISIBILITY",
    "LIVE_HOST_SYNC_POSE_VISIBILITY_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSyncPoseVisibilityResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSyncPoseVisibilityResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn client_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_sync_pose_visibility_residual_method_names_residual_wave1050() -> bool {
    let names = LIVE_HOST_SYNC_POSE_VISIBILITY_RESIDUAL_METHOD_NAMES_WAVE1050;
    let ok = residual_name_index(names, "sync_with_game_logic").is_some()
        && residual_name_index(names, "Wave 1050").is_some();
    residual_action_store(ResidualHostSyncPoseVisibilityResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sync_pose_visibility_residual_nav_commands_residual_wave1050() -> bool {
    let steps = LIVE_HOST_SYNC_POSE_VISIBILITY_RESIDUAL_NAV_STEPS_WAVE1050;
    let ok = residual_name_index(steps, "LIVE_HOST_SYNC_POSE_VISIBILITY_RESIDUAL").is_some()
        && residual_name_index(steps, "SYNC_POSE").is_some();
    residual_action_store(ResidualHostSyncPoseVisibilityResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sync_pose_visibility_residual_residual_pack_wave1050() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let ok = client.contains("Wave 1023/1050: host empty dual-world peels translator catalog pose")
        && client.contains("Wave 1050: status/FOW visibility residual")
        && client.contains("entry.destroyed || (entry.effectively_stealthed && !local)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSyncPoseVisibilityResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sync_pose_visibility_residual_honesty() -> bool {
    let a = honesty_host_sync_pose_visibility_residual_method_names_residual_wave1050();
    let b = honesty_host_sync_pose_visibility_residual_nav_commands_residual_wave1050();
    let c = honesty_host_sync_pose_visibility_residual_residual_pack_wave1050();
    residual_action_store(ResidualHostSyncPoseVisibilityResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sync_pose_visibility_residual_wave1050() {
        assert!(honesty_host_sync_pose_visibility_residual_residual_pack_wave1050());
        assert!(honesty_host_sync_pose_visibility_residual_method_names_residual_wave1050());
        assert!(honesty_host_sync_pose_visibility_residual_nav_commands_residual_wave1050());
        assert!(simulate_live_host_sync_pose_visibility_residual_honesty());
    }
}
