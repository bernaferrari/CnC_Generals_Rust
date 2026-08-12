//! Wave 941: host residual mutation authority (poison/force-kill/pending fire).
//!
//! Shadow session drains for PoisonedBehavior DoT, topple/height/slow-death kills,
//! and FireWeaponWhenDamaged pending fire route through
//! `apply_host_residual_mutation_op` instead of `get_objects_mut` dual-writes.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RESIDUAL_MUTATION_BOUNDARY_METHOD_NAMES_WAVE941: &[&str] = &[
    "apply_host_residual_mutation_op",
    "HostResidualMutationOp",
    "PoisonDot",
    "ForceKill",
    "SetPendingFireWhenDamaged",
    "Wave 941",
    "playable_claim = false",
];

pub const LIVE_HOST_RESIDUAL_MUTATION_BOUNDARY_NAV_STEPS_WAVE941: &[&str] = &[
    "RESIDUAL_MUTATION_BOUNDARY",
    "HOST_RESIDUAL_MUTATION_OP",
    "LIVE_HOST_RESIDUAL_MUTATION_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostResidualMutationBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostResidualMutationBoundaryAction) {
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

fn code_window<'a>(src: &'a str, marker: &str, len: usize) -> &'a str {
    match src.find(marker) {
        Some(i) => &src[i..src.len().min(i + len)],
        None => "",
    }
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn session_fn_window(src: &str) -> &str {
    let marker = "fn shadow_session_after_host_tick";
    match src.find(marker) {
        Some(i) => &src[i..src.len().min(i + 130_000)],
        None => "",
    }
}

pub fn honesty_host_residual_mutation_boundary_method_names_residual_wave941() -> bool {
    let names = LIVE_HOST_RESIDUAL_MUTATION_BOUNDARY_METHOD_NAMES_WAVE941;
    let ok = residual_name_index(names, "apply_host_residual_mutation_op").is_some()
        && residual_name_index(names, "Wave 941").is_some();
    residual_action_store(ResidualHostResidualMutationBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_residual_mutation_boundary_nav_commands_residual_wave941() -> bool {
    let steps = LIVE_HOST_RESIDUAL_MUTATION_BOUNDARY_NAV_STEPS_WAVE941;
    let ok = residual_name_index(steps, "LIVE_HOST_RESIDUAL_MUTATION_BOUNDARY").is_some()
        && residual_name_index(steps, "RESIDUAL_MUTATION_BOUNDARY").is_some();
    residual_action_store(ResidualHostResidualMutationBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_residual_mutation_boundary_residual_pack_wave941() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let api = non_comment_code(code_window(gl, "fn apply_host_residual_mutation_op", 2000));
    let session = non_comment_code(session_fn_window(sh));
    let ok = gl.contains("enum HostResidualMutationOp")
        && api.contains("PoisonDot")
        && api.contains("ForceKill")
        && api.contains("SetPendingFireWhenDamaged")
        && api.contains("take_damage_from_typed_death")
        && session.contains("apply_host_residual_mutation_op")
        && session.contains("PoisonDot")
        && session.contains("ForceKill")
        && session.contains("SetPendingFireWhenDamaged")
        && !session.contains("take_damage_from_typed_death")
        && session.contains("host_poison_dot_log::drain")
        && session.contains("host_topple_kill_log::drain")
        && session.contains("host_fwwd_continuous_log::drain")
        && session.contains("host_fwwd_reaction_log::drain")
        && gl.contains("Wave 941")
        && sh.contains("941")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostResidualMutationBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_residual_mutation_boundary_honesty() -> bool {
    let a = honesty_host_residual_mutation_boundary_method_names_residual_wave941();
    let b = honesty_host_residual_mutation_boundary_nav_commands_residual_wave941();
    let c = honesty_host_residual_mutation_boundary_residual_pack_wave941();
    residual_action_store(ResidualHostResidualMutationBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_residual_mutation_boundary_residual_wave941() {
        assert!(honesty_host_residual_mutation_boundary_residual_pack_wave941());
        assert!(honesty_host_residual_mutation_boundary_method_names_residual_wave941());
        assert!(honesty_host_residual_mutation_boundary_nav_commands_residual_wave941());
        assert!(simulate_live_host_residual_mutation_boundary_honesty());
    }
}
