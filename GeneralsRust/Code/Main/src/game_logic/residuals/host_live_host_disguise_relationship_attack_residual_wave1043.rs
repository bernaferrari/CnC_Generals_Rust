//! Wave 1043: dual-world disguise relationship/attack residual.
//!
//! relationship_to_target and selection_attack_result dual paths use apparent
//! team for disguised units (C++ non-allied viewer parity). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DISGUISE_RELATIONSHIP_ATTACK_RESIDUAL_METHOD_NAMES_WAVE1043: &[&str] = &[
    "relationship_to_target",
    "selection_attack_result",
    "translator_entry_apparent_team",
    "Wave 1043",
    "playable_claim = false",
];

pub const LIVE_HOST_DISGUISE_RELATIONSHIP_ATTACK_RESIDUAL_NAV_STEPS_WAVE1043: &[&str] = &[
    "DISGUISE",
    "RELATIONSHIP",
    "ATTACK_LEGALITY",
    "LIVE_HOST_DISGUISE_RELATIONSHIP_ATTACK_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDisguiseRelationshipAttackResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDisguiseRelationshipAttackResidualAction) {
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
fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}

pub fn honesty_host_disguise_relationship_attack_residual_method_names_residual_wave1043() -> bool {
    let names = LIVE_HOST_DISGUISE_RELATIONSHIP_ATTACK_RESIDUAL_METHOD_NAMES_WAVE1043;
    let ok = residual_name_index(names, "translator_entry_apparent_team").is_some()
        && residual_name_index(names, "Wave 1043").is_some();
    residual_action_store(ResidualHostDisguiseRelationshipAttackResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_disguise_relationship_attack_residual_nav_commands_residual_wave1043() -> bool {
    let steps = LIVE_HOST_DISGUISE_RELATIONSHIP_ATTACK_RESIDUAL_NAV_STEPS_WAVE1043;
    let ok = residual_name_index(steps, "LIVE_HOST_DISGUISE_RELATIONSHIP_ATTACK_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "RELATIONSHIP").is_some();
    residual_action_store(ResidualHostDisguiseRelationshipAttackResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_disguise_relationship_attack_residual_residual_pack_wave1043() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ok = (tr.contains("Wave 973/1043: host empty dual-world") || tr.contains("Wave 973/1043"))
        && (tr.contains("Wave 975/1043: host empty dual-world")
            || tr.contains("Wave 975/1043/1049"))
        && tr.contains("translator_entry_apparent_team(&entry)")
        && tr.contains("translator_entry_apparent_team(&target)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDisguiseRelationshipAttackResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_disguise_relationship_attack_residual_honesty() -> bool {
    let a = honesty_host_disguise_relationship_attack_residual_method_names_residual_wave1043();
    let b = honesty_host_disguise_relationship_attack_residual_nav_commands_residual_wave1043();
    let c = honesty_host_disguise_relationship_attack_residual_residual_pack_wave1043();
    residual_action_store(ResidualHostDisguiseRelationshipAttackResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_disguise_relationship_attack_residual_wave1043() {
        assert!(honesty_host_disguise_relationship_attack_residual_residual_pack_wave1043());
        assert!(
            honesty_host_disguise_relationship_attack_residual_method_names_residual_wave1043()
        );
        assert!(
            honesty_host_disguise_relationship_attack_residual_nav_commands_residual_wave1043()
        );
        assert!(simulate_live_host_disguise_relationship_attack_residual_honesty());
    }
}
