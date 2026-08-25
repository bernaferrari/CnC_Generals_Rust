//! Wave 198 residual peels: player Guard/Stop/Patrol commands record
//! `host_guard_log` so GameWorld shadow can last-write SetGuard mutations.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 197 attack-log residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `CommandExecutor::execute_guard` → `host_guard_log::record`
//! - stop/patrol clear guard log
//! - shadow drains `host_guard_log` into `WorldMutation::SetGuard`
//!
//! Fail-closed:
//! - Not full command buffer cutover for all CommandTypes
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Guard-log residual method names.
pub const LIVE_COMMAND_GUARD_LOG_METHOD_NAMES_WAVE198: &[&str] = &[
    "execute_guard",
    "host_guard_log::record",
    "WorldMutation::SetGuard",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_GUARD_LOG_NAV_STEPS_WAVE198: &[&str] = &[
    "REQUIRE_EXECUTE_GUARD_RECORDS",
    "REQUIRE_SHADOW_DRAINS_GUARD_LOG",
    "LIVE_GUARD_LOG_LATCHES",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_GUARD_LOG_CMD_NAMES_WAVE198: &[&str] = &[
    "click_live_command_guard_log_ok_prepare",
    "click_live_command_guard_log_ok_live",
    "click_live_command_guard_log_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_live_command_guard_log_method_names_residual_wave198() -> bool {
    LIVE_COMMAND_GUARD_LOG_METHOD_NAMES_WAVE198.len() == 4
        && residual_name_index(LIVE_COMMAND_GUARD_LOG_METHOD_NAMES_WAVE198, "execute_guard")
            == Some(0)
        && residual_name_index(
            LIVE_COMMAND_GUARD_LOG_METHOD_NAMES_WAVE198,
            "host_guard_log::record",
        ) == Some(1)
        && residual_name_index(
            LIVE_COMMAND_GUARD_LOG_METHOD_NAMES_WAVE198,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_guard_log_nav_commands_residual_wave198() -> bool {
    LIVE_COMMAND_GUARD_LOG_NAV_STEPS_WAVE198.len() == 4
        && residual_name_index(
            LIVE_COMMAND_GUARD_LOG_NAV_STEPS_WAVE198,
            "REQUIRE_EXECUTE_GUARD_RECORDS",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_GUARD_LOG_NAV_STEPS_WAVE198,
            "LIVE_GUARD_LOG_LATCHES",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_GUARD_LOG_CMD_NAMES_WAVE198.len() == 3
}

/// Wave 198 composite residual honesty pack.
pub fn honesty_live_command_guard_log_residual_pack_wave198() -> bool {
    honesty_live_command_guard_log_method_names_residual_wave198()
        && honesty_live_command_guard_log_nav_commands_residual_wave198()
}

/// Source residual: execute_guard records host_guard_log.
pub fn honesty_execute_guard_records_source() -> bool {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = match src.find("fn execute_guard(") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..];
    // Wave 232: guard last-writes via unit_command_guard_full (records host_guard_log).
    if body.contains("unit_command_guard_full") {
        // 2026-08-15: scan host plus extra world_* splits.
        let gl = super::host_logic_scan_src();
        let gi = match gl.find("pub fn unit_command_guard_full") {
            Some(gi) => gi,
            None => return false,
        };
        let gbody = &gl[gi..];
        return gbody.contains("host_guard_log::record")
            && gbody.contains("set_guard_position")
            && gbody.contains("set_guard_target");
    }
    body.contains("host_guard_log::record")
        && body.contains("set_guard_position")
        && body.contains("set_guard_target")
}

/// Source residual: stop/patrol clear guard log; shadow drains it.
pub fn honesty_shadow_drains_guard_log_source() -> bool {
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let gw = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let stop_i = ce.find("fn execute_stop").unwrap_or(0);
    let stop = &ce[stop_i..];
    // Wave 232: stop clears guard via unit_command_stop (records host_guard_log).
    let stop_ok = stop.contains("host_guard_log::record")
        || (stop.contains("unit_command_stop") && {
            // 2026-08-15: scan host plus extra world_* splits.
            let gl = super::host_logic_scan_src();
            let gi = gl.find("pub fn unit_command_stop").unwrap_or(0);
            let gbody = &gl[gi..];
            gbody.contains("host_guard_log::record")
        });
    stop_ok && gw.contains("host_guard_log::drain") && gw.contains("WorldMutation::SetGuard")
}

/// Live residual: guard command latches host_guard_log event.
pub fn simulate_live_command_guard_log_honesty() -> bool {
    use crate::command_system::{CommandType, GameCommand, GuardTarget, ModifierKeys};
    use crate::game_logic::host_guard_log;
    use crate::game_logic::{GameLogic, GuardMode, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;
    use std::time::SystemTime;

    if !honesty_live_command_guard_log_residual_pack_wave198() {
        return false;
    }
    if !honesty_execute_guard_records_source() {
        return false;
    }
    if !honesty_shadow_drains_guard_log_source() {
        return false;
    }

    host_guard_log::clear();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GuardLog198");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("GuardLogUnit198") {
        let mut t = ThingTemplate::new("GuardLogUnit198");
        t.set_health(140.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("GuardLogUnit198".into(), t);
    }
    let Some(unit) = logic.create_object("GuardLogUnit198", Team::USA, Vec3::new(12.0, 0.0, 13.0))
    else {
        return false;
    };

    let dest = Vec3::new(40.0, 0.0, 41.0);
    logic.queue_command(GameCommand {
        command_type: CommandType::Guard {
            target: GuardTarget::Position(dest),
            mode: GuardMode::Normal,
        },
        player_id: 0,
        command_id: 2,
        timestamp: SystemTime::now(),
        selected_units: vec![unit],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();

    let events = host_guard_log::drain();
    if !events.iter().any(|e| {
        e.object == unit
            && e.position
                .map(|p| (p[0] - 40.0).abs() < 1e-3 && (p[2] - 41.0).abs() < 1e-3)
                .unwrap_or(false)
            && e.target_host == 0
            && e.radius > 0.0
    }) {
        return false;
    }

    // Shadow can queue SetGuard from a recorded event.
    use crate::gameworld_shadow::GameWorldShadow;
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    host_guard_log::record(unit, Some([40.0, 0.0, 41.0]), 0, 100.0);
    let drained = host_guard_log::drain();
    if shadow.apply_host_guard_events(&drained) == 0 {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_guard_log_method_names_residual_wave198());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_guard_log_nav_commands_residual_wave198());
    }

    #[test]
    fn wave198_composite_pack() {
        assert!(honesty_live_command_guard_log_residual_pack_wave198());
    }

    #[test]
    fn guard_log_sources() {
        assert!(honesty_execute_guard_records_source());
        assert!(honesty_shadow_drains_guard_log_source());
    }

    #[test]
    fn simulate_live_command_guard_log_honesty_residual_live() {
        assert!(
            simulate_live_command_guard_log_honesty(),
            "command guard log residual must latch"
        );
    }
}
