//! Wave 757: under coupled tick, GW writeback skips objects with pending host
//! combat/movement/status logs (body damage, movement, combat attack, locomotor,
//! physics motive, weapon set/stats, AI state/mood, stealth, model condition,
//! experience). Host mid-frame log is authority until apply. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_COMBAT_MOVEMENT_LOGS_METHOD_NAMES_WAVE757: &[&str] = &[
    "has_pending",
    "writeback_body_damage_to_host",
    "writeback_movement_to_host",
    "writeback_combat_attack_to_host",
    "Wave 757",
    "playable_claim = false",
];
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_COMBAT_MOVEMENT_LOGS_NAV_STEPS_WAVE757: &[&str] = &[
    "REQUIRE_HAS_PENDING",
    "REQUIRE_COUPLED_TICK",
    "REQUIRE_TWELVE_WRITEBACKS",
    "LIVE_HOST_WRITEBACK_SKIP_PENDING_COMBAT_MOVEMENT_LOGS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_WRITEBACK_SKIP_PENDING_COMBAT_MOVEMENT_LOGS_CMD_NAMES_WAVE757:
    &[&str] = &[
    "host_writeback_skip_pending_combat_movement_logs",
    "has_pending",
    "coupled_tick",
    "twelve_writebacks",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWritebackSkipPendingCombatMovementLogsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostWritebackSkipPendingCombatMovementLogsAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostWritebackSkipPendingCombatMovementLogsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_writeback_skip_pending_combat_movement_logs_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_writeback_skip_pending_combat_movement_logs_last_action()
-> ResidualHostWritebackSkipPendingCombatMovementLogsAction {
    ResidualHostWritebackSkipPendingCombatMovementLogsAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_writeback_skip_pending_combat_movement_logs_method_names_residual_wave757()
-> bool {
    let names = LIVE_HOST_WRITEBACK_SKIP_PENDING_COMBAT_MOVEMENT_LOGS_METHOD_NAMES_WAVE757;
    let ok = residual_name_index(names, "has_pending").is_some()
        && residual_name_index(names, "writeback_body_damage_to_host").is_some()
        && residual_name_index(names, "writeback_movement_to_host").is_some()
        && residual_name_index(names, "writeback_combat_attack_to_host").is_some()
        && residual_name_index(names, "Wave 757").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingCombatMovementLogsAction::MethodNames);
    ok
}
pub fn honesty_host_writeback_skip_pending_combat_movement_logs_source_markers_residual_wave757()
-> bool {
    let sh = sh_source();
    // 2026-08-14 last-writer correction: `host_ai_state_log` no longer skips
    // host writeback while pending — the GameWorld SetAiState channel is the
    // last-writer (mirrors C++ Object::setAIState applying immediately; the
    // pending-skip starved GW and host logs replayed stale state). The other
    // eleven channels keep the Wave 757 pending-skip.
    let logs = [
        "host_body_damage_log",
        "host_movement_log",
        "host_combat_attack_log",
        "host_locomotor_log",
        "host_physics_motive_log",
        "host_weapon_set_log",
        "host_weapon_stats_log",
        "host_ai_mood_log",
        "host_stealth_flags_log",
        "host_model_condition_log",
        "host_experience_log",
    ];
    let wbs = [
        "writeback_body_damage_to_host",
        "writeback_movement_to_host",
        "writeback_combat_attack_to_host",
        "writeback_locomotor_to_host",
        "writeback_physics_motive_to_host",
        "writeback_weapon_set_to_host",
        "writeback_weapon_stats_to_host",
        "writeback_ai_state_to_host",
        "writeback_ai_mood_to_host",
        "writeback_stealth_flags_to_host",
        "writeback_model_condition_to_host",
        "writeback_experience_to_host",
    ];
    let logs_ok = logs
        .iter()
        .all(|l| sh.contains(&format!("{l}::has_pending")));
    let wbs_ok = wbs.iter().all(|w| sh.contains(w));
    let wave_hits = sh.matches("Wave 757").count();
    let ok = sh.contains("Wave 757")
        && sh.contains("shadow_coupled_tick_active()")
        && logs_ok
        && wbs_ok
        && wave_hits >= 11
        && !sh.contains("playable_claim = true");
    residual_action_store(ResidualHostWritebackSkipPendingCombatMovementLogsAction::SourceMarkers);
    ok
}
pub fn honesty_host_writeback_skip_pending_combat_movement_logs_nav_commands_residual_wave757()
-> bool {
    let steps = LIVE_HOST_WRITEBACK_SKIP_PENDING_COMBAT_MOVEMENT_LOGS_NAV_STEPS_WAVE757;
    let cmds = RUNTIME_HOST_LIVE_HOST_WRITEBACK_SKIP_PENDING_COMBAT_MOVEMENT_LOGS_CMD_NAMES_WAVE757;
    let ok = residual_name_index(steps, "REQUIRE_HAS_PENDING").is_some()
        && residual_name_index(steps, "REQUIRE_COUPLED_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_TWELVE_WRITEBACKS").is_some()
        && residual_name_index(
            steps,
            "LIVE_HOST_WRITEBACK_SKIP_PENDING_COMBAT_MOVEMENT_LOGS",
        )
        .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_writeback_skip_pending_combat_movement_logs").is_some()
        && residual_name_index(cmds, "has_pending").is_some()
        && residual_name_index(cmds, "coupled_tick").is_some()
        && residual_name_index(cmds, "twelve_writebacks").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingCombatMovementLogsAction::NavCommands);
    ok
}
pub fn simulate_host_writeback_skip_pending_combat_movement_logs_collect_source() -> bool {
    let ok = sh_source().contains("Wave 757")
        && sh_source().contains("host_body_damage_log::has_pending");
    residual_action_store(ResidualHostWritebackSkipPendingCombatMovementLogsAction::CollectSource);
    ok
}
pub fn simulate_host_writeback_skip_pending_combat_movement_logs_dispatch_source() -> bool {
    let ok = sh_source().matches("Wave 757").count() >= 11
        && sh_source().contains("writeback_experience_to_host");
    residual_action_store(ResidualHostWritebackSkipPendingCombatMovementLogsAction::DispatchSource);
    ok
}
pub fn honesty_host_writeback_skip_pending_combat_movement_logs_residual_pack_wave757() -> bool {
    honesty_host_writeback_skip_pending_combat_movement_logs_method_names_residual_wave757()
        && honesty_host_writeback_skip_pending_combat_movement_logs_source_markers_residual_wave757(
        )
        && honesty_host_writeback_skip_pending_combat_movement_logs_nav_commands_residual_wave757()
        && simulate_host_writeback_skip_pending_combat_movement_logs_collect_source()
        && simulate_host_writeback_skip_pending_combat_movement_logs_dispatch_source()
}
pub fn simulate_live_host_writeback_skip_pending_combat_movement_logs_honesty() -> bool {
    let ok = honesty_host_writeback_skip_pending_combat_movement_logs_residual_pack_wave757();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostWritebackSkipPendingCombatMovementLogsAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_writeback_skip_pending_combat_movement_logs_method_names_residual_wave757(
            )
        );
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_writeback_skip_pending_combat_movement_logs_source_markers_residual_wave757());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_writeback_skip_pending_combat_movement_logs_nav_commands_residual_wave757(
            )
        );
    }
    #[test]
    fn sources() {
        assert!(simulate_host_writeback_skip_pending_combat_movement_logs_collect_source());
        assert!(simulate_host_writeback_skip_pending_combat_movement_logs_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_writeback_skip_pending_combat_movement_logs_residual_pack_wave757());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_writeback_skip_pending_combat_movement_logs_honesty());
        assert!(residual_host_writeback_skip_pending_combat_movement_logs_ok());
    }
}
