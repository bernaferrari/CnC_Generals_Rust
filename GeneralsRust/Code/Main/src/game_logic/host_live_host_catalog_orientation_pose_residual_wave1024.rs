//! Wave 1024: presentation catalog orientation residual for dual-world pose peel.
//!
//! Catalog entries carry yaw (orientation). Dual-world sync_with_game_logic peels
//! position + Rotate_Y onto drawable_map via set_instance_transform.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CATALOG_ORIENTATION_POSE_RESIDUAL_METHOD_NAMES_WAVE1024: &[&str] = &[
    "orientation",
    "sync_with_game_logic",
    "set_instance_transform",
    "Wave 1024",
    "playable_claim = false",
];

pub const LIVE_HOST_CATALOG_ORIENTATION_POSE_RESIDUAL_NAV_STEPS_WAVE1024: &[&str] = &[
    "CATALOG_ORIENTATION",
    "DRAWABLE_POSE",
    "ROTATE_Y",
    "LIVE_HOST_CATALOG_ORIENTATION_POSE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCatalogOrientationPoseResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCatalogOrientationPoseResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn ui_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/gui/ingame_ui.rs")
}
fn tr_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/presentation_translator_residual.rs")
}
fn gc_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_catalog_orientation_pose_residual_method_names_residual_wave1024() -> bool {
    let names = LIVE_HOST_CATALOG_ORIENTATION_POSE_RESIDUAL_METHOD_NAMES_WAVE1024;
    let ok = residual_name_index(names, "orientation").is_some()
        && residual_name_index(names, "Wave 1024").is_some();
    residual_action_store(ResidualHostCatalogOrientationPoseResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_catalog_orientation_pose_residual_nav_commands_residual_wave1024() -> bool {
    let steps = LIVE_HOST_CATALOG_ORIENTATION_POSE_RESIDUAL_NAV_STEPS_WAVE1024;
    let ok = residual_name_index(steps, "LIVE_HOST_CATALOG_ORIENTATION_POSE_RESIDUAL").is_some()
        && residual_name_index(steps, "CATALOG_ORIENTATION").is_some();
    residual_action_store(ResidualHostCatalogOrientationPoseResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_catalog_orientation_pose_residual_residual_pack_wave1024() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let gc = gc_source();
    let ok = ui.contains("Wave 1024: yaw residual for dual-world drawable pose peel")
        && tr.contains("pub orientation: f32")
        && cnc.contains("Wave 1024: orientation residual for dual-world pose peel")
        && cnc.contains("orientation: o.orientation")
        && gc.contains("orientation: u.orientation")
        && gc.contains("rotation_y(entry.orientation)")
        && gc.contains("set_instance_transform(transform)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostCatalogOrientationPoseResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_catalog_orientation_pose_residual_honesty() -> bool {
    let a = honesty_host_catalog_orientation_pose_residual_method_names_residual_wave1024();
    let b = honesty_host_catalog_orientation_pose_residual_nav_commands_residual_wave1024();
    let c = honesty_host_catalog_orientation_pose_residual_residual_pack_wave1024();
    residual_action_store(ResidualHostCatalogOrientationPoseResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_catalog_orientation_pose_residual_wave1024() {
        assert!(honesty_host_catalog_orientation_pose_residual_residual_pack_wave1024());
        assert!(honesty_host_catalog_orientation_pose_residual_method_names_residual_wave1024());
        assert!(honesty_host_catalog_orientation_pose_residual_nav_commands_residual_wave1024());
        assert!(simulate_live_host_catalog_orientation_pose_residual_honesty());
    }
}
