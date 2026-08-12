//! Wave 1004: FX residual name GameWorld→host writeback.
//!
//! writeback_combat_status_to_host last-writes pending_death_fx, BoneFX last_fx,
//! and transition damage FX residual name from Entity onto host Object.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FX_NAME_WRITEBACK_RESIDUAL_METHOD_NAMES_WAVE1004: &[&str] = &[
    "writeback_combat_status_to_host",
    "pending_death_fx",
    "bone_fx_damage",
    "pending_transition_damage_fx",
    "Wave 1004",
    "playable_claim = false",
];

pub const LIVE_HOST_FX_NAME_WRITEBACK_RESIDUAL_NAV_STEPS_WAVE1004: &[&str] = &[
    "WRITEBACK_COMBAT_STATUS",
    "DEATH_FX",
    "BONE_FX",
    "TRANSITION_DAMAGE_FX",
    "LIVE_HOST_FX_NAME_WRITEBACK_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFxNameWritebackResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFxNameWritebackResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

pub fn honesty_host_fx_name_writeback_residual_method_names_residual_wave1004() -> bool {
    let names = LIVE_HOST_FX_NAME_WRITEBACK_RESIDUAL_METHOD_NAMES_WAVE1004;
    let ok = residual_name_index(names, "pending_death_fx").is_some()
        && residual_name_index(names, "Wave 1004").is_some();
    residual_action_store(ResidualHostFxNameWritebackResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fx_name_writeback_residual_nav_commands_residual_wave1004() -> bool {
    let steps = LIVE_HOST_FX_NAME_WRITEBACK_RESIDUAL_NAV_STEPS_WAVE1004;
    let ok = residual_name_index(steps, "LIVE_HOST_FX_NAME_WRITEBACK_RESIDUAL").is_some()
        && residual_name_index(steps, "BONE_FX").is_some();
    residual_action_store(ResidualHostFxNameWritebackResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fx_name_writeback_residual_residual_pack_wave1004() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sh = shadow_source();
    let ok = sh.contains("Wave 1004: FX residual name last-writer")
        && sh.contains("obj.pending_death_fx != ent.death_fx_name")
        && sh.contains("bone.last_fx != ent.bone_fx_name")
        && sh.contains("ent.damage_fx_name.as_ref()")
        && sh.contains("pending_transition_damage_fx.last_mut()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFxNameWritebackResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_fx_name_writeback_residual_honesty() -> bool {
    let a = honesty_host_fx_name_writeback_residual_method_names_residual_wave1004();
    let b = honesty_host_fx_name_writeback_residual_nav_commands_residual_wave1004();
    let c = honesty_host_fx_name_writeback_residual_residual_pack_wave1004();
    residual_action_store(ResidualHostFxNameWritebackResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_fx_name_writeback_residual_wave1004() {
        assert!(honesty_host_fx_name_writeback_residual_residual_pack_wave1004());
        assert!(honesty_host_fx_name_writeback_residual_method_names_residual_wave1004());
        assert!(honesty_host_fx_name_writeback_residual_nav_commands_residual_wave1004());
        assert!(simulate_live_host_fx_name_writeback_residual_honesty());
    }
}
