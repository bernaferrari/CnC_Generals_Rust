//! Historical wave198: guard belongs to the canonical Main host object.
//! Guard shadow replay and reverse writeback are retired; adjacent channels remain separately tracked.
/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Guard-log residual method names.
pub const LIVE_COMMAND_GUARD_LOG_METHOD_NAMES_WAVE198: &[&str] = &[
    "execute_guard",
    "canonical_guard_command",
    "immutable_guard_projection",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_GUARD_LOG_NAV_STEPS_WAVE198: &[&str] = &[
    "REQUIRE_CANONICAL_GUARD_COMMAND",
    "REQUIRE_IMMUTABLE_GUARD_PROJECTION",
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
            "canonical_guard_command",
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
            "REQUIRE_CANONICAL_GUARD_COMMAND",
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

/// Source residual: execute_guard reaches the canonical host guard command.
pub fn honesty_execute_guard_records_source() -> bool {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = match src.find("fn execute_guard(") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..];
    // Wave 232: guard enters through the canonical unit_command_guard_full.
    if body.contains("unit_command_guard_full") {
        // 2026-08-15: scan host plus extra world_* splits.
        let gl = super::host_logic_scan_src();
        let gi = match gl.find("pub fn unit_command_guard_full") {
            Some(gi) => gi,
            None => return false,
        };
        let gbody = &gl[gi..];
        return gbody.contains("unit.set_ai_state(ai_state.clone())")
            && gbody.contains("set_guard_position")
            && gbody.contains("set_guard_target");
    }
    body.contains("unit_command_guard_full")
        && body.contains("set_guard_position")
        && body.contains("set_guard_target")
}

/// Source residual: stop/patrol clear canonical guard; shadow cannot write it back.
pub fn honesty_shadow_drains_guard_log_source() -> bool {
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let gw = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let stop_i = ce.find("fn execute_stop").unwrap_or(0);
    let stop = &ce[stop_i..];
    // Wave 232: stop clears guard via the canonical unit_command_stop.
    let stop_ok = stop.contains("unit_command_stop")
        || (stop.contains("unit_command_stop") && {
            // 2026-08-15: scan host plus extra world_* splits.
            let gl = super::host_logic_scan_src();
            let gi = gl.find("pub fn unit_command_stop").unwrap_or(0);
            let gbody = &gl[gi..];
            gbody.contains("unit.set_ai_state(ai_state.clone())")
        });
    stop_ok
        && !gw.contains("pub fn apply_host_guard_events")
        && !gw.contains("pub fn writeback_guard_to_host")
}

/// Live residual: guard command changes the actual owner and immutable projection.
pub fn simulate_live_command_guard_log_honesty() -> bool {
    use crate::command_system::{CommandType, GameCommand, GuardTarget, ModifierKeys};
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

    let Some(owner) = logic.host_object(unit) else {
        return false;
    };
    if owner.guard_position != Some(dest)
        || owner.guard_target.is_some()
        || owner.guard_radius <= 0.0
    {
        return false;
    }
    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(entity) = shadow.entity_for_host(unit) else {
        return false;
    };
    #[cfg(test)]
    {
        let copy = shadow.world_mut().world_mut().entity_mut(entity).unwrap();
        copy.guard_position = Some([900.0, 0.0, 900.0]);
        copy.guard_radius = 1.0;
    }
    if logic.host_object(unit).unwrap().guard_position != Some(dest) {
        return false;
    }
    shadow.sync_from_host(&logic);
    if shadow.world().entity(entity).unwrap().guard_position != Some([dest.x, dest.y, dest.z]) {
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
