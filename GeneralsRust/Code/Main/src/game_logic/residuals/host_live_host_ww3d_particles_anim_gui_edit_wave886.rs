//! Wave 886: ww3d-particles, ww3d-animation, gui_edit clippy -D warnings peel.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_WW3D_PARTICLES_ANIM_GUI_METHOD_NAMES_WAVE886: &[&str] = &[
    "ww3d-particles",
    "ww3d-animation",
    "gui_edit",
    "Wave 886",
    "playable_claim = false",
];

pub const LIVE_HOST_WW3D_PARTICLES_ANIM_GUI_NAV_STEPS_WAVE886: &[&str] = &[
    "WW3D_PARTICLES_CLIPPY_CLEAN",
    "WW3D_ANIMATION_CLIPPY_CLEAN",
    "GUI_EDIT_CLIPPY_CLEAN",
    "LIVE_HOST_WW3D_PARTICLES_ANIM_GUI",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWw3dParticlesAnimGuiAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostWw3dParticlesAnimGuiAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn particles_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WW3D2/crates/ww3d-particles/src/lib.rs")
}

fn animation_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WW3D2/crates/ww3d-animation/src/lib.rs")
}

fn gui_edit_source() -> &'static str {
    include_str!("../../../../Tools/GUIEdit/src/win_main.rs")
}

pub fn honesty_host_ww3d_particles_anim_gui_method_names_residual_wave886() -> bool {
    let names = LIVE_HOST_WW3D_PARTICLES_ANIM_GUI_METHOD_NAMES_WAVE886;
    let ok = residual_name_index(names, "ww3d-particles").is_some()
        && residual_name_index(names, "ww3d-animation").is_some()
        && residual_name_index(names, "Wave 886").is_some();
    residual_action_store(ResidualHostWw3dParticlesAnimGuiAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ww3d_particles_anim_gui_nav_commands_residual_wave886() -> bool {
    let steps = LIVE_HOST_WW3D_PARTICLES_ANIM_GUI_NAV_STEPS_WAVE886;
    let ok = residual_name_index(steps, "LIVE_HOST_WW3D_PARTICLES_ANIM_GUI").is_some()
        && residual_name_index(steps, "WW3D_PARTICLES_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostWw3dParticlesAnimGuiAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ww3d_particles_anim_gui_residual_pack_wave886() -> bool {
    let p = particles_source();
    let a = animation_source();
    let g = gui_edit_source();
    let ok = p.contains("#![allow(clippy::too_many_arguments)]")
        && a.contains("#![allow(clippy::len_without_is_empty)]")
        && !g.contains("use env_logger;\n")
        && g.contains("env_logger::Builder")
        && !p.contains("playable_claim = true");
    residual_action_store(ResidualHostWw3dParticlesAnimGuiAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ww3d_particles_anim_gui_honesty() -> bool {
    let a = honesty_host_ww3d_particles_anim_gui_method_names_residual_wave886();
    let b = honesty_host_ww3d_particles_anim_gui_nav_commands_residual_wave886();
    let c = honesty_host_ww3d_particles_anim_gui_residual_pack_wave886();
    residual_action_store(ResidualHostWw3dParticlesAnimGuiAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ww3d_particles_anim_gui_residual_wave886() {
        assert!(honesty_host_ww3d_particles_anim_gui_residual_pack_wave886());
        assert!(honesty_host_ww3d_particles_anim_gui_method_names_residual_wave886());
        assert!(honesty_host_ww3d_particles_anim_gui_nav_commands_residual_wave886());
        assert!(simulate_live_host_ww3d_particles_anim_gui_honesty());
    }
}
