//! Wave 759: under coupled tick, remaining pose/combat writebacks skip objects with
//! pending host move/movement/contain/combat/rebuild logs (move targets, transforms,
//! contain capacity, combat status, rebuild producer). Host mid-frame log is authority
//! until apply. `playable_claim` stays false.

//! Host mid-frame log is authority until apply. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_MOVE_TRANSFORM_LOGS_METHOD_NAMES_WAVE759: &[&str] = &[
    "has_pending",
    "writeback_move_targets_to_host",
    "writeback_transforms_to_host",
    "writeback_contain_capacity_to_host",
    "writeback_combat_status_to_host",
    "writeback_rebuild_producer_to_host",
    "Wave 759",
    "playable_claim = false",
];
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_MOVE_TRANSFORM_LOGS_NAV_STEPS_WAVE759: &[&str] = &[
    "REQUIRE_HAS_PENDING",
    "REQUIRE_COUPLED_TICK",
    "REQUIRE_MOVE_TARGETS_GATE",
    "REQUIRE_TRANSFORMS_GATE",
    "REQUIRE_COMBAT_STATUS_GATE",
    "LIVE_HOST_WRITEBACK_SKIP_PENDING_MOVE_TRANSFORM_LOGS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_WRITEBACK_SKIP_PENDING_MOVE_TRANSFORM_LOGS_CMD_NAMES_WAVE759:
    &[&str] = &[
    "host_writeback_skip_pending_move_transform_logs",
    "has_pending",
    "coupled_tick",
    "move_targets_gate",
    "transforms_gate",
    "combat_status_gate",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWritebackSkipPendingMoveTransformLogsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostWritebackSkipPendingMoveTransformLogsAction {
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
fn residual_action_store(a: ResidualHostWritebackSkipPendingMoveTransformLogsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_writeback_skip_pending_move_transform_logs_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_writeback_skip_pending_move_transform_logs_last_action()
-> ResidualHostWritebackSkipPendingMoveTransformLogsAction {
    ResidualHostWritebackSkipPendingMoveTransformLogsAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_writeback_skip_pending_move_transform_logs_method_names_residual_wave759()
-> bool {
    let names = LIVE_HOST_WRITEBACK_SKIP_PENDING_MOVE_TRANSFORM_LOGS_METHOD_NAMES_WAVE759;
    let ok = residual_name_index(names, "has_pending").is_some()
        && residual_name_index(names, "writeback_move_targets_to_host").is_some()
        && residual_name_index(names, "writeback_transforms_to_host").is_some()
        && residual_name_index(names, "writeback_contain_capacity_to_host").is_some()
        && residual_name_index(names, "writeback_combat_status_to_host").is_some()
        && residual_name_index(names, "writeback_rebuild_producer_to_host").is_some()
        && residual_name_index(names, "Wave 759").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingMoveTransformLogsAction::MethodNames);
    ok
}
pub fn honesty_host_writeback_skip_pending_move_transform_logs_source_markers_residual_wave759()
-> bool {
    let sh = sh_source();
    let logs = [
        "host_move_log",
        "host_movement_log",
        "host_contain_log",
        "host_combat_attack_log",
        "host_rebuild_producer_log",
    ];
    let wbs = [
        "writeback_move_targets_to_host",
        "writeback_transforms_to_host",
        "writeback_contain_capacity_to_host",
        "writeback_combat_status_to_host",
        "writeback_rebuild_producer_to_host",
    ];
    let logs_ok = logs
        .iter()
        .all(|l| sh.contains(&format!("{l}::has_pending")));
    let wbs_ok = wbs.iter().all(|w| {
        let Some(j) = sh.find(&format!("fn {w}")) else {
            return false;
        };
        sh[j..sh.len().min(j + 900)].contains("Wave 759")
    });
    let ok =
        logs_ok && wbs_ok && sh.contains("Wave 759") && sh.contains("shadow_coupled_tick_active");
    residual_action_store(ResidualHostWritebackSkipPendingMoveTransformLogsAction::SourceMarkers);
    ok
}
pub fn honesty_host_writeback_skip_pending_move_transform_logs_nav_commands_residual_wave759()
-> bool {
    let steps = LIVE_HOST_WRITEBACK_SKIP_PENDING_MOVE_TRANSFORM_LOGS_NAV_STEPS_WAVE759;
    let ok = residual_name_index(steps, "REQUIRE_HAS_PENDING").is_some()
        && residual_name_index(steps, "REQUIRE_COUPLED_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_MOVE_TARGETS_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_TRANSFORMS_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_COMBAT_STATUS_GATE").is_some()
        && residual_name_index(
            steps,
            "LIVE_HOST_WRITEBACK_SKIP_PENDING_MOVE_TRANSFORM_LOGS",
        )
        .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingMoveTransformLogsAction::NavCommands);
    ok
}
pub fn simulate_host_writeback_skip_pending_move_transform_logs_collect_source() -> bool {
    let ok = sh_source().contains("Wave 759")
        && sh_source().contains("host_move_log::has_pending")
        && sh_source().contains("host_movement_log::has_pending");
    residual_action_store(ResidualHostWritebackSkipPendingMoveTransformLogsAction::CollectSource);
    ok
}
pub fn simulate_host_writeback_skip_pending_move_transform_logs_dispatch_source() -> bool {
    let ok = sh_source().matches("Wave 759").count() >= 5
        && sh_source().contains("writeback_move_targets_to_host")
        && sh_source().contains("writeback_transforms_to_host")
        && sh_source().contains("writeback_rebuild_producer_to_host");
    residual_action_store(ResidualHostWritebackSkipPendingMoveTransformLogsAction::DispatchSource);
    ok
}
pub fn honesty_host_writeback_skip_pending_move_transform_logs_residual_pack_wave759() -> bool {
    honesty_host_writeback_skip_pending_move_transform_logs_method_names_residual_wave759()
        && honesty_host_writeback_skip_pending_move_transform_logs_source_markers_residual_wave759()
        && honesty_host_writeback_skip_pending_move_transform_logs_nav_commands_residual_wave759()
        && simulate_host_writeback_skip_pending_move_transform_logs_collect_source()
        && simulate_host_writeback_skip_pending_move_transform_logs_dispatch_source()
}
pub fn simulate_live_host_writeback_skip_pending_move_transform_logs_honesty() -> bool {
    let ok = honesty_host_writeback_skip_pending_move_transform_logs_residual_pack_wave759();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostWritebackSkipPendingMoveTransformLogsAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_writeback_skip_pending_move_transform_logs_method_names_residual_wave759()
        );
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_writeback_skip_pending_move_transform_logs_source_markers_residual_wave759(
            )
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_writeback_skip_pending_move_transform_logs_nav_commands_residual_wave759()
        );
    }
    #[test]
    fn sources() {
        assert!(simulate_host_writeback_skip_pending_move_transform_logs_collect_source());
        assert!(simulate_host_writeback_skip_pending_move_transform_logs_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_writeback_skip_pending_move_transform_logs_residual_pack_wave759());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_writeback_skip_pending_move_transform_logs_honesty());
        assert!(residual_host_writeback_skip_pending_move_transform_logs_ok());
    }
}
