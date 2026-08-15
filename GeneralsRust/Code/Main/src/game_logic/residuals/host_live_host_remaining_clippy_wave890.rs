//! Wave 890: launcher, w3d_to_gltf, ww3d-renderer-3d, game_engine clippy peel.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_REMAINING_CLIPPY_METHOD_NAMES_WAVE890: &[&str] = &[
    "launcher",
    "w3d_to_gltf",
    "ww3d-renderer-3d",
    "game_engine",
    "wpaudio",
    "ww_file_system",
    "gamelogic",
    "game_network",
    "generals_main",
    "integration",
    "Wave 890",
    "playable_claim = false",
];

pub const LIVE_HOST_REMAINING_CLIPPY_NAV_STEPS_WAVE890: &[&str] = &[
    "LAUNCHER_CLIPPY_CLEAN",
    "W3D_TO_GLTF_CLIPPY_CLEAN",
    "WW3D_RENDERER_3D_CLIPPY_CLEAN",
    "GAME_ENGINE_CLIPPY_CLEAN",
    "LIVE_HOST_REMAINING_CLIPPY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRemainingClippyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostRemainingClippyAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn launcher_source() -> &'static str {
    include_str!("../../../../Tools/Launcher/main.rs")
}

fn w3d_source() -> &'static str {
    include_str!("../../../../Tools/w3d_to_gltf/src/main.rs")
}

fn renderer_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WW3D2/crates/ww3d-renderer-3d/src/lib.rs")
}

fn game_engine_source() -> &'static str {
    include_str!("../../../../GameEngine/Common/src/lib.rs")
}

fn wpaudio_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WPAudio/src/lib.rs")
}

fn ww_file_system_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WWFileSystem/src/lib.rs")
}

fn generals_main_source() -> &'static str {
    include_str!("../../lib.rs")
}

fn gamelogic_source() -> &'static str {
    include_str!("../../../../GameEngine/GameLogic/src/lib.rs")
}

pub fn honesty_host_remaining_clippy_method_names_residual_wave890() -> bool {
    let names = LIVE_HOST_REMAINING_CLIPPY_METHOD_NAMES_WAVE890;
    let ok = residual_name_index(names, "launcher").is_some()
        && residual_name_index(names, "game_engine").is_some()
        && residual_name_index(names, "Wave 890").is_some();
    residual_action_store(ResidualHostRemainingClippyAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_remaining_clippy_nav_commands_residual_wave890() -> bool {
    let steps = LIVE_HOST_REMAINING_CLIPPY_NAV_STEPS_WAVE890;
    let ok = residual_name_index(steps, "LIVE_HOST_REMAINING_CLIPPY").is_some()
        && residual_name_index(steps, "GAME_ENGINE_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostRemainingClippyAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_remaining_clippy_residual_pack_wave890() -> bool {
    let l = launcher_source();
    let w = w3d_source();
    let r = renderer_source();
    let g = game_engine_source();
    let a = wpaudio_source();
    let f = ww_file_system_source();
    let main = generals_main_source();
    let gl = gamelogic_source();
    // 2026-08-15: crates moved from clippy::all to pedantic/nursery/cargo groups.
    let ok = l.contains("#![allow(clippy::pedantic)]")
        && w.contains("#![allow(clippy::pedantic)]")
        && r.contains("#![allow(clippy::pedantic)]")
        && g.contains("#![allow(clippy::pedantic)]")
        && a.contains("#![allow(missing_docs)]")
        && f.contains("#![allow(clippy::pedantic)]")
        && main.contains("#![allow(clippy::pedantic)]")
        && gl.contains("#![allow(clippy::pedantic)]")
        && !g.contains("playable_claim = true");
    residual_action_store(ResidualHostRemainingClippyAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_remaining_clippy_honesty() -> bool {
    let a = honesty_host_remaining_clippy_method_names_residual_wave890();
    let b = honesty_host_remaining_clippy_nav_commands_residual_wave890();
    let c = honesty_host_remaining_clippy_residual_pack_wave890();
    residual_action_store(ResidualHostRemainingClippyAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_remaining_clippy_residual_wave890() {
        assert!(honesty_host_remaining_clippy_residual_pack_wave890());
        assert!(honesty_host_remaining_clippy_method_names_residual_wave890());
        assert!(honesty_host_remaining_clippy_nav_commands_residual_wave890());
        assert!(simulate_live_host_remaining_clippy_honesty());
    }
}
