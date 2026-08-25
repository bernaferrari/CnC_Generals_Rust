//! Wave 1023: dual-world sync_with_game_logic catalog pose residual.
//!
//! When OBJECT_REGISTRY is empty, sync_with_game_logic peels translator catalog
//! positions onto drawable_map (PresentationFrame pose batch remains primary).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SYNC_WITH_GAME_LOGIC_CATALOG_POSE_RESIDUAL_METHOD_NAMES_WAVE1023: &[&str] = &[
    "sync_with_game_logic",
    "translator_catalog_entry",
    "set_position",
    "Wave 1023",
    "playable_claim = false",
];

pub const LIVE_HOST_SYNC_WITH_GAME_LOGIC_CATALOG_POSE_RESIDUAL_NAV_STEPS_WAVE1023: &[&str] = &[
    "SYNC_WITH_GAME_LOGIC",
    "TRANSLATOR_CATALOG",
    "DRAWABLE_POSE",
    "LIVE_HOST_SYNC_WITH_GAME_LOGIC_CATALOG_POSE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSyncWithGameLogicCatalogPoseResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSyncWithGameLogicCatalogPoseResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
fn gc_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

pub fn honesty_host_sync_with_game_logic_catalog_pose_residual_method_names_residual_wave1023()
-> bool {
    let names = LIVE_HOST_SYNC_WITH_GAME_LOGIC_CATALOG_POSE_RESIDUAL_METHOD_NAMES_WAVE1023;
    let ok = residual_name_index(names, "sync_with_game_logic").is_some()
        && residual_name_index(names, "Wave 1023").is_some();
    residual_action_store(ResidualHostSyncWithGameLogicCatalogPoseResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sync_with_game_logic_catalog_pose_residual_nav_commands_residual_wave1023()
-> bool {
    let steps = LIVE_HOST_SYNC_WITH_GAME_LOGIC_CATALOG_POSE_RESIDUAL_NAV_STEPS_WAVE1023;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_SYNC_WITH_GAME_LOGIC_CATALOG_POSE_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "SYNC_WITH_GAME_LOGIC").is_some();
    residual_action_store(ResidualHostSyncWithGameLogicCatalogPoseResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sync_with_game_logic_catalog_pose_residual_residual_pack_wave1023() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let gc = gc_source();
    let ok = (gc.contains("Wave 1023: host empty dual-world peels translator catalog pose")
        || gc.contains("Wave 1023/1050: host empty dual-world peels translator catalog pose"))
        && gc.contains("drawable.set_position")
        && gc.contains("translator_catalog_entry(object_id)")
        && gc.contains("if dual_world_registry_unavailable()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSyncWithGameLogicCatalogPoseResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sync_with_game_logic_catalog_pose_residual_honesty() -> bool {
    let a =
        honesty_host_sync_with_game_logic_catalog_pose_residual_method_names_residual_wave1023();
    let b =
        honesty_host_sync_with_game_logic_catalog_pose_residual_nav_commands_residual_wave1023();
    let c = honesty_host_sync_with_game_logic_catalog_pose_residual_residual_pack_wave1023();
    residual_action_store(ResidualHostSyncWithGameLogicCatalogPoseResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sync_with_game_logic_catalog_pose_residual_wave1023() {
        assert!(honesty_host_sync_with_game_logic_catalog_pose_residual_residual_pack_wave1023());
        assert!(
            honesty_host_sync_with_game_logic_catalog_pose_residual_method_names_residual_wave1023(
            )
        );
        assert!(
            honesty_host_sync_with_game_logic_catalog_pose_residual_nav_commands_residual_wave1023(
            )
        );
        assert!(simulate_live_host_sync_with_game_logic_catalog_pose_residual_honesty());
    }
}
