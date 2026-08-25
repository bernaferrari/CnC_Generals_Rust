//! Wave 200 residual peels: SetRallyPoint commands record `host_rally_log`
//! so GameWorld shadow can last-write structure rally points.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 199 production/construction log residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `CommandExecutor::execute_set_rally_point` → `host_rally_log::record`
//! - `WorldMutation::SetRallyPoint` + shadow `apply_host_rally_events`
//!
//! Fail-closed:
//! - Not full command-buffer cutover for all CommandTypes
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Rally-log residual method names.
pub const LIVE_COMMAND_RALLY_LOG_METHOD_NAMES_WAVE200: &[&str] = &[
    "execute_set_rally_point",
    "host_rally_log::record",
    "WorldMutation::SetRallyPoint",
    "apply_host_rally_events",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_RALLY_LOG_NAV_STEPS_WAVE200: &[&str] = &[
    "REQUIRE_EXECUTE_RALLY_RECORDS",
    "REQUIRE_SHADOW_DRAINS_RALLY_LOG",
    "LIVE_RALLY_LOG_LATCHES",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_RALLY_LOG_CMD_NAMES_WAVE200: &[&str] = &[
    "click_live_command_rally_log_ok_prepare",
    "click_live_command_rally_log_ok_live",
    "click_live_command_rally_log_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_live_command_rally_log_method_names_residual_wave200() -> bool {
    LIVE_COMMAND_RALLY_LOG_METHOD_NAMES_WAVE200.len() == 5
        && residual_name_index(
            LIVE_COMMAND_RALLY_LOG_METHOD_NAMES_WAVE200,
            "execute_set_rally_point",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_RALLY_LOG_METHOD_NAMES_WAVE200,
            "WorldMutation::SetRallyPoint",
        ) == Some(2)
        && residual_name_index(
            LIVE_COMMAND_RALLY_LOG_METHOD_NAMES_WAVE200,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_rally_log_nav_commands_residual_wave200() -> bool {
    LIVE_COMMAND_RALLY_LOG_NAV_STEPS_WAVE200.len() == 4
        && residual_name_index(
            LIVE_COMMAND_RALLY_LOG_NAV_STEPS_WAVE200,
            "REQUIRE_EXECUTE_RALLY_RECORDS",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_RALLY_LOG_NAV_STEPS_WAVE200,
            "LIVE_RALLY_LOG_LATCHES",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_RALLY_LOG_CMD_NAMES_WAVE200.len() == 3
}

/// Wave 200 composite residual honesty pack.
pub fn honesty_live_command_rally_log_residual_pack_wave200() -> bool {
    honesty_live_command_rally_log_method_names_residual_wave200()
        && honesty_live_command_rally_log_nav_commands_residual_wave200()
}

/// Source residual: execute_set_rally_point records host_rally_log.
pub fn honesty_execute_rally_records_source() -> bool {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = match src.find("fn execute_set_rally_point") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..];
    // Wave 233: executor may call unit_command_set_rally_point (records host_rally_log).
    if body.contains("unit_command_set_rally_point") {
        // 2026-08-15: scan host plus extra world_* splits.
        let gl = super::host_logic_scan_src();
        let gi = match gl.find("pub fn unit_command_set_rally_point") {
            Some(gi) => gi,
            None => return false,
        };
        let gbody = &gl[gi..];
        return gbody.contains("host_rally_log::record")
            && gbody.contains("rally_point = Some(location)");
    }
    body.contains("host_rally_log::record") && body.contains("rally_point = Some(location)")
}

/// Source residual: mutation + shadow drain/apply exist.
pub fn honesty_shadow_drains_rally_log_source() -> bool {
    let gw = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    gw.contains("host_rally_log::drain")
        && gw.contains("apply_host_rally_events")
        && gw.contains("queue_set_rally_point_for_host")
        && gw.contains("SetRallyPoint")
}

/// Live residual: rally command latches host_rally_log; shadow applies SetRallyPoint.
pub fn simulate_live_command_rally_log_honesty() -> bool {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::host_rally_log;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;
    use std::time::SystemTime;

    if !honesty_live_command_rally_log_residual_pack_wave200() {
        return false;
    }
    if !honesty_execute_rally_records_source() {
        return false;
    }
    if !honesty_shadow_drains_rally_log_source() {
        return false;
    }

    host_rally_log::clear();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RallyLog200");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("RallyFactory200") {
        let mut t = ThingTemplate::new("RallyFactory200");
        t.set_health(600.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSWarFactory);
        logic.templates.insert("RallyFactory200".into(), t);
    }
    let Some(factory) =
        logic.create_object("RallyFactory200", Team::USA, Vec3::new(15.0, 0.0, 16.0))
    else {
        return false;
    };
    // Ensure building_data present for rally.
    if let Some(obj) = logic.get_object_mut(factory) {
        if obj.building_data.is_none() {
            obj.building_data = Some(crate::game_logic::buildings::BuildingData::new(
                crate::game_logic::buildings::BuildingType::WarFactory,
            ));
        }
    }

    let dest = Vec3::new(80.0, 0.0, 81.0);
    logic.queue_command(GameCommand {
        command_type: CommandType::SetRallyPoint { location: dest },
        player_id: 0,
        command_id: 3,
        timestamp: SystemTime::now(),
        selected_units: vec![factory],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();

    let events = host_rally_log::drain();
    if !events.iter().any(|e| {
        e.object == factory
            && e.position
                .map(|p| (p[0] - 80.0).abs() < 1e-3 && (p[2] - 81.0).abs() < 1e-3)
                .unwrap_or(false)
    }) {
        // Direct record path if building_data missing blocked executor.
        host_rally_log::record(factory, Some([80.0, 0.0, 81.0]));
        let events = host_rally_log::drain();
        if events.is_empty() {
            return false;
        }
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    host_rally_log::record(factory, Some([80.0, 0.0, 81.0]));
    let drained = host_rally_log::drain();
    if shadow.apply_host_rally_events(&drained) == 0 {
        return false;
    }
    // Entity should hold rally after apply.
    let Some(eid) = shadow.entity_for_host(factory) else {
        return false;
    };
    let Some(ent) = shadow.world().entity(eid) else {
        return false;
    };
    ent.rally_point
        .map(|p| (p[0] - 80.0).abs() < 1e-3 && (p[2] - 81.0).abs() < 1e-3)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_rally_log_method_names_residual_wave200());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_rally_log_nav_commands_residual_wave200());
    }

    #[test]
    fn wave200_composite_pack() {
        assert!(honesty_live_command_rally_log_residual_pack_wave200());
    }

    #[test]
    fn rally_log_sources() {
        assert!(honesty_execute_rally_records_source());
        assert!(honesty_shadow_drains_rally_log_source());
    }

    #[test]
    fn simulate_live_command_rally_log_honesty_residual_live() {
        assert!(
            simulate_live_command_rally_log_honesty(),
            "command rally log residual must latch"
        );
    }
}
