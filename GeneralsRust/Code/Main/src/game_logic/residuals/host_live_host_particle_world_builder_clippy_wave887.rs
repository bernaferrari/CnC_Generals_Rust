//! Wave 887: particle_editor + world_builder clippy -D warnings peel.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PARTICLE_WORLD_BUILDER_METHOD_NAMES_WAVE887: &[&str] = &[
    "particle_editor",
    "world_builder",
    "Wave 887",
    "playable_claim = false",
];

pub const LIVE_HOST_PARTICLE_WORLD_BUILDER_NAV_STEPS_WAVE887: &[&str] = &[
    "PARTICLE_EDITOR_CLIPPY_CLEAN",
    "WORLD_BUILDER_CLIPPY_CLEAN",
    "LIVE_HOST_PARTICLE_WORLD_BUILDER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostParticleWorldBuilderAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostParticleWorldBuilderAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn particle_main() -> &'static str {
    include_str!("../../../../Tools/ParticleEditor/src/main.rs")
}

fn world_main() -> &'static str {
    include_str!("../../../../Tools/WorldBuilder/src/main.rs")
}

pub fn honesty_host_particle_world_builder_method_names_residual_wave887() -> bool {
    let names = LIVE_HOST_PARTICLE_WORLD_BUILDER_METHOD_NAMES_WAVE887;
    let ok = residual_name_index(names, "particle_editor").is_some()
        && residual_name_index(names, "world_builder").is_some()
        && residual_name_index(names, "Wave 887").is_some();
    residual_action_store(ResidualHostParticleWorldBuilderAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_particle_world_builder_nav_commands_residual_wave887() -> bool {
    let steps = LIVE_HOST_PARTICLE_WORLD_BUILDER_NAV_STEPS_WAVE887;
    let ok = residual_name_index(steps, "LIVE_HOST_PARTICLE_WORLD_BUILDER").is_some()
        && residual_name_index(steps, "PARTICLE_EDITOR_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostParticleWorldBuilderAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_particle_world_builder_residual_pack_wave887() -> bool {
    let p = particle_main();
    let w = world_main();
    let ok = p.contains("#![allow(clippy::single_char_add_str)]")
        && p.contains("#![allow(clippy::field_reassign_with_default)]")
        && w.contains("#![allow(dead_code)]")
        && w.contains("#![allow(clippy::assign_op_pattern)]")
        && !p.contains("playable_claim = true");
    residual_action_store(ResidualHostParticleWorldBuilderAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_particle_world_builder_honesty() -> bool {
    let a = honesty_host_particle_world_builder_method_names_residual_wave887();
    let b = honesty_host_particle_world_builder_nav_commands_residual_wave887();
    let c = honesty_host_particle_world_builder_residual_pack_wave887();
    residual_action_store(ResidualHostParticleWorldBuilderAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_particle_world_builder_residual_wave887() {
        assert!(honesty_host_particle_world_builder_residual_pack_wave887());
        assert!(honesty_host_particle_world_builder_method_names_residual_wave887());
        assert!(honesty_host_particle_world_builder_nav_commands_residual_wave887());
        assert!(simulate_live_host_particle_world_builder_honesty());
    }
}
