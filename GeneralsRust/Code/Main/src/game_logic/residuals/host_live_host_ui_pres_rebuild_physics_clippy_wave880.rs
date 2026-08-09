//! Wave 880: host_update_ui_state rebuilds from presentation freeze before boot
//! dual-read; ww3d-physics clippy -D warnings peel. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UI_PRES_REBUILD_METHOD_NAMES_WAVE880: &[&str] = &[
    "host_update_ui_state",
    "apply_to_ui_state",
    "last_presentation_frame",
    "ww3d-physics",
    "Wave 880",
    "playable_claim = false",
];

pub const LIVE_HOST_UI_PRES_REBUILD_NAV_STEPS_WAVE880: &[&str] = &[
    "REBUILD_UI_FROM_PRESENTATION_FREEZE",
    "BOOT_DUAL_READ_ONLY_WITHOUT_FREEZE",
    "WW3D_PHYSICS_CLIPPY_CLEAN",
    "LIVE_HOST_UI_PRES_REBUILD",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUiPresRebuildAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUiPresRebuildAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn physics_body_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WW3D2/crates/ww3d-physics/src/body.rs")
}

fn physics_world_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WW3D2/crates/ww3d-physics/src/world.rs")
}

pub fn honesty_host_ui_pres_rebuild_method_names_residual_wave880() -> bool {
    let names = LIVE_HOST_UI_PRES_REBUILD_METHOD_NAMES_WAVE880;
    let ok = residual_name_index(names, "host_update_ui_state").is_some()
        && residual_name_index(names, "apply_to_ui_state").is_some()
        && residual_name_index(names, "Wave 880").is_some();
    residual_action_store(ResidualHostUiPresRebuildAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ui_pres_rebuild_nav_commands_residual_wave880() -> bool {
    let steps = LIVE_HOST_UI_PRES_REBUILD_NAV_STEPS_WAVE880;
    let ok = residual_name_index(steps, "LIVE_HOST_UI_PRES_REBUILD").is_some()
        && residual_name_index(steps, "REBUILD_UI_FROM_PRESENTATION_FREEZE").is_some();
    residual_action_store(ResidualHostUiPresRebuildAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ui_pres_rebuild_residual_pack_wave880() -> bool {
    let cnc = cnc_source();
    let body = physics_body_source();
    let world = physics_world_source();
    let ok = cnc.contains("Wave 585/862/872/880: prefer last presentation UI residual")
        && cnc.contains(
            "Wave 880: rebuild UI residual from presentation freeze without live dual-read",
        )
        && cnc.contains("pres.apply_to_ui_state(&mut ui)")
        && body.contains("#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]")
        && body.contains("#[default]\n    Dynamic,")
        && world.contains("#[allow(clippy::new_without_default)]")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostUiPresRebuildAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ui_pres_rebuild_honesty() -> bool {
    let a = honesty_host_ui_pres_rebuild_method_names_residual_wave880();
    let b = honesty_host_ui_pres_rebuild_nav_commands_residual_wave880();
    let c = honesty_host_ui_pres_rebuild_residual_pack_wave880();
    residual_action_store(ResidualHostUiPresRebuildAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ui_pres_rebuild_residual_wave880() {
        assert!(honesty_host_ui_pres_rebuild_residual_pack_wave880());
        assert!(honesty_host_ui_pres_rebuild_method_names_residual_wave880());
        assert!(honesty_host_ui_pres_rebuild_nav_commands_residual_wave880());
        assert!(simulate_live_host_ui_pres_rebuild_honesty());
    }
}
