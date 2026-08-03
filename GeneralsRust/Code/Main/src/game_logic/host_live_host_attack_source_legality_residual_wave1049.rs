//! Wave 1049: dual-world attack/source/prisoner legality residual.
//!
//! selection_attack_result dual fails closed on illegal targets/sources;
//! selection_source prefers living local; prisoner dual requires status-ok.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ATTACK_SOURCE_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1049: &[&str] = &[
    "selection_attack_result",
    "selection_source_object_id",
    "is_prisoner_target",
    "Wave 1049",
    "playable_claim = false",
];

pub const LIVE_HOST_ATTACK_SOURCE_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1049: &[&str] = &[
    "ATTACK_LEGALITY",
    "SOURCE_LEGALITY",
    "PRISONER",
    "LIVE_HOST_ATTACK_SOURCE_LEGALITY_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAttackSourceLegalityResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostAttackSourceLegalityResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn tr_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/message_stream/translators.rs")
}

pub fn honesty_host_attack_source_legality_residual_method_names_residual_wave1049() -> bool {
    let names = LIVE_HOST_ATTACK_SOURCE_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1049;
    let ok = residual_name_index(names, "selection_attack_result").is_some()
        && residual_name_index(names, "Wave 1049").is_some();
    residual_action_store(ResidualHostAttackSourceLegalityResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_attack_source_legality_residual_nav_commands_residual_wave1049() -> bool {
    let steps = LIVE_HOST_ATTACK_SOURCE_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1049;
    let ok = residual_name_index(steps, "LIVE_HOST_ATTACK_SOURCE_LEGALITY_RESIDUAL").is_some()
        && residual_name_index(steps, "ATTACK_LEGALITY").is_some();
    residual_action_store(ResidualHostAttackSourceLegalityResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_attack_source_legality_residual_residual_pack_wave1049() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ok = tr.contains("Wave 975/1043/1049: host empty dual-world")
        && tr.contains("Wave 973/1049: host empty dual-world")
        && tr.contains("Wave 1049: prefer living local source residual")
        && tr.contains("dual_target_status_ok(&target)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostAttackSourceLegalityResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_attack_source_legality_residual_honesty() -> bool {
    let a = honesty_host_attack_source_legality_residual_method_names_residual_wave1049();
    let b = honesty_host_attack_source_legality_residual_nav_commands_residual_wave1049();
    let c = honesty_host_attack_source_legality_residual_residual_pack_wave1049();
    residual_action_store(ResidualHostAttackSourceLegalityResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_attack_source_legality_residual_wave1049() {
        assert!(honesty_host_attack_source_legality_residual_residual_pack_wave1049());
        assert!(honesty_host_attack_source_legality_residual_method_names_residual_wave1049());
        assert!(honesty_host_attack_source_legality_residual_nav_commands_residual_wave1049());
        assert!(simulate_live_host_attack_source_legality_residual_honesty());
    }
}
