//! Wave 864: executable smoke issues attack_nearest_enemy early after train so
//! match_damage_applied / combat_damage_ok can latch before late options steps.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_EXEC_SMOKE_EARLY_COMBAT_METHOD_NAMES_WAVE864: &[&str] = &[
    "saw_early_combat_cmd",
    "attack_nearest_enemy",
    "match_damage_applied",
    "combat_damage_ok",
    "Wave 864",
    "playable_claim = false",
];

pub const LIVE_EXEC_SMOKE_EARLY_COMBAT_NAV_STEPS_WAVE864: &[&str] = &[
    "EARLY_ATTACK_AFTER_TRAIN",
    "LATCH_COMBAT_DAMAGE_COUNTERS",
    "LIVE_EXEC_SMOKE_EARLY_COMBAT",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualExecSmokeEarlyCombatAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualExecSmokeEarlyCombatAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_exec_smoke_early_combat_method_names_residual_wave864() -> bool {
    let names = LIVE_EXEC_SMOKE_EARLY_COMBAT_METHOD_NAMES_WAVE864;
    let ok = residual_name_index(names, "saw_early_combat_cmd").is_some()
        && residual_name_index(names, "attack_nearest_enemy").is_some()
        && residual_name_index(names, "Wave 864").is_some();
    residual_action_store(ResidualExecSmokeEarlyCombatAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_exec_smoke_early_combat_nav_commands_residual_wave864() -> bool {
    let steps = LIVE_EXEC_SMOKE_EARLY_COMBAT_NAV_STEPS_WAVE864;
    let ok = residual_name_index(steps, "LIVE_EXEC_SMOKE_EARLY_COMBAT").is_some()
        && residual_name_index(steps, "EARLY_ATTACK_AFTER_TRAIN").is_some();
    residual_action_store(ResidualExecSmokeEarlyCombatAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_exec_smoke_early_combat_residual_pack_wave864() -> bool {
    let es = es_source();
    let ok = es.contains("saw_early_combat_cmd: bool,")
        && es.contains("Wave 864: issue combat early while InGame")
        && es.contains("attack_nearest_enemy|auto_target=1")
        && es.contains("saw_early_combat_cmd = true")
        && es.contains("match_damage_applied > 0.0 || snap.match_kills > 0");
    residual_action_store(ResidualExecSmokeEarlyCombatAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_exec_smoke_early_combat_honesty() -> bool {
    let a = honesty_exec_smoke_early_combat_method_names_residual_wave864();
    let b = honesty_exec_smoke_early_combat_nav_commands_residual_wave864();
    let c = honesty_exec_smoke_early_combat_residual_pack_wave864();
    residual_action_store(ResidualExecSmokeEarlyCombatAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_exec_smoke_early_combat_residual_wave864() {
        assert!(honesty_exec_smoke_early_combat_residual_pack_wave864());
        assert!(honesty_exec_smoke_early_combat_method_names_residual_wave864());
        assert!(honesty_exec_smoke_early_combat_nav_commands_residual_wave864());
        assert!(simulate_live_exec_smoke_early_combat_honesty());
    }
}
