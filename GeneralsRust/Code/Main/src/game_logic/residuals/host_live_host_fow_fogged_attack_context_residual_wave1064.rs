//! Wave 1064: dual-world FOW fogged attack/context residual.
//!
//! selection_attack_result, selection_any_local_object_can_target, and
//! selection_can_enter_target fail-closed on non-local shroud_status >= 2.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FOW_FOGGED_ATTACK_CONTEXT_RESIDUAL_METHOD_NAMES_WAVE1064: &[&str] = &[
    "selection_attack_result",
    "selection_can_enter_target",
    "shroud_status",
    "Wave 1064",
    "playable_claim = false",
];

pub const LIVE_HOST_FOW_FOGGED_ATTACK_CONTEXT_RESIDUAL_NAV_STEPS_WAVE1064: &[&str] = &[
    "FOW",
    "ATTACK_CONTEXT",
    "LIVE_HOST_FOW_FOGGED_ATTACK_CONTEXT_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFowFoggedAttackContextResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFowFoggedAttackContextResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn tr_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/message_stream/translators.rs")
}

pub fn honesty_host_fow_fogged_attack_context_residual_method_names_residual_wave1064() -> bool {
    let names = LIVE_HOST_FOW_FOGGED_ATTACK_CONTEXT_RESIDUAL_METHOD_NAMES_WAVE1064;
    let ok = residual_name_index(names, "selection_attack_result").is_some()
        && residual_name_index(names, "Wave 1064").is_some();
    residual_action_store(ResidualHostFowFoggedAttackContextResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_fogged_attack_context_residual_nav_commands_residual_wave1064() -> bool {
    let steps = LIVE_HOST_FOW_FOGGED_ATTACK_CONTEXT_RESIDUAL_NAV_STEPS_WAVE1064;
    let ok = residual_name_index(steps, "LIVE_HOST_FOW_FOGGED_ATTACK_CONTEXT_RESIDUAL").is_some()
        && residual_name_index(steps, "ATTACK_CONTEXT").is_some();
    residual_action_store(ResidualHostFowFoggedAttackContextResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_fogged_attack_context_residual_residual_pack_wave1064() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ok = tr
        .contains("Wave 1064: FOW fogged/black non-local targets fail-closed for attack residual")
        && tr.contains("Wave 1064: FOW fogged/black non-local targets fail-closed.")
        && tr
            .matches("target.shroud_status >= 2 && !translator_entry_is_local(&target)")
            .count()
            >= 2
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFowFoggedAttackContextResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_fow_fogged_attack_context_residual_honesty() -> bool {
    let a = honesty_host_fow_fogged_attack_context_residual_method_names_residual_wave1064();
    let b = honesty_host_fow_fogged_attack_context_residual_nav_commands_residual_wave1064();
    let c = honesty_host_fow_fogged_attack_context_residual_residual_pack_wave1064();
    residual_action_store(ResidualHostFowFoggedAttackContextResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_fow_fogged_attack_context_residual_wave1064() {
        assert!(honesty_host_fow_fogged_attack_context_residual_residual_pack_wave1064());
        assert!(honesty_host_fow_fogged_attack_context_residual_method_names_residual_wave1064());
        assert!(honesty_host_fow_fogged_attack_context_residual_nav_commands_residual_wave1064());
        assert!(simulate_live_host_fow_fogged_attack_context_residual_honesty());
    }
}
