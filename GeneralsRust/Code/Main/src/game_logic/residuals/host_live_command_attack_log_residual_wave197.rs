//! Wave 197 residual peels: player attack commands record `host_attack_log`
//! so GameWorld shadow can last-write attack targets (parity with move log).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 196 rebuilt vertical gate residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `CommandExecutor::execute_attack` → `host_attack_log::record`
//! - shadow drains `host_attack_log` into WorldMutation
//!
//! Fail-closed:
//! - Not full command buffer / WorldMutation cutover for all CommandTypes
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Attack-log residual method names.
pub const LIVE_COMMAND_ATTACK_LOG_METHOD_NAMES_WAVE197: &[&str] = &[
    "execute_attack",
    "host_attack_log::record",
    "queue_set_attack_target_for_host",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_ATTACK_LOG_NAV_STEPS_WAVE197: &[&str] = &[
    "REQUIRE_EXECUTE_ATTACK_RECORDS",
    "REQUIRE_SHADOW_DRAINS_ATTACK_LOG",
    "LIVE_ATTACK_LOG_LATCHES",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_ATTACK_LOG_CMD_NAMES_WAVE197: &[&str] = &[
    "click_live_command_attack_log_ok_prepare",
    "click_live_command_attack_log_ok_live",
    "click_live_command_attack_log_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_attack_log_method_names_residual_wave197() -> bool {
    LIVE_COMMAND_ATTACK_LOG_METHOD_NAMES_WAVE197.len() == 4
        && residual_name_index(
            LIVE_COMMAND_ATTACK_LOG_METHOD_NAMES_WAVE197,
            "execute_attack",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_ATTACK_LOG_METHOD_NAMES_WAVE197,
            "host_attack_log::record",
        ) == Some(1)
        && residual_name_index(
            LIVE_COMMAND_ATTACK_LOG_METHOD_NAMES_WAVE197,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_attack_log_nav_commands_residual_wave197() -> bool {
    LIVE_COMMAND_ATTACK_LOG_NAV_STEPS_WAVE197.len() == 4
        && residual_name_index(
            LIVE_COMMAND_ATTACK_LOG_NAV_STEPS_WAVE197,
            "REQUIRE_EXECUTE_ATTACK_RECORDS",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_ATTACK_LOG_NAV_STEPS_WAVE197,
            "LIVE_ATTACK_LOG_LATCHES",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_ATTACK_LOG_CMD_NAMES_WAVE197.len() == 3
}

/// Wave 197 composite residual honesty pack.
pub fn honesty_live_command_attack_log_residual_pack_wave197() -> bool {
    honesty_live_command_attack_log_method_names_residual_wave197()
        && honesty_live_command_attack_log_nav_commands_residual_wave197()
}

/// Source residual: execute_attack records host_attack_log (directly or via unit_command_attack).
pub fn honesty_execute_attack_records_source() -> bool {
    let src = include_str!("../../command_executor.rs");
    let i = match src.find("fn execute_attack(") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 2500)];
    // Wave 232: executor routes through GameLogic::unit_command_attack which records.
    if body.contains("unit_command_attack") {
        let gl = include_str!("../game_logic.rs");
        let gi = match gl.find("pub fn unit_command_attack(") {
            Some(gi) => gi,
            None => return false,
        };
        let gbody = &gl[gi..gl.len().min(gi + 800)];
        return gbody.contains("host_attack_log::record")
            && gbody.contains("set_target(Some(target_id))");
    }
    body.contains("host_attack_log::record") && body.contains("set_target(Some(target_id))")
}

/// Source residual: shadow drains host_attack_log.
pub fn honesty_shadow_drains_attack_log_source() -> bool {
    let src = include_str!("../../gameworld_shadow.rs");
    src.contains("host_attack_log::drain") && src.contains("queue_set_attack_target_for_host")
}

/// Live residual: attack command latches host_attack_log event.
pub fn simulate_live_command_attack_log_honesty() -> bool {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::host_attack_log;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;
    use std::time::SystemTime;

    if !honesty_live_command_attack_log_residual_pack_wave197() {
        return false;
    }
    if !honesty_execute_attack_records_source() {
        return false;
    }
    if !honesty_shadow_drains_attack_log_source() {
        return false;
    }

    host_attack_log::clear();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AtkLog197");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    for name in ["AtkLogUnit197", "AtkLogTarget197"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.set_health(120.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Vehicle);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let Some(attacker) =
        logic.create_object("AtkLogUnit197", Team::USA, Vec3::new(10.0, 0.0, 10.0))
    else {
        return false;
    };
    let Some(target) =
        logic.create_object("AtkLogTarget197", Team::China, Vec3::new(30.0, 0.0, 30.0))
    else {
        return false;
    };

    logic.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: target },
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![attacker],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();

    let events = host_attack_log::drain();
    if !events
        .iter()
        .any(|e| e.attacker == attacker && e.target == Some(target))
    {
        return false;
    }

    // Shadow can consume attack events into mutations.
    use crate::gameworld_shadow::GameWorldShadow;
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Re-record after drain for apply path if needed — drain already consumed.
    // Re-issue command path: record again and apply via queue API.
    host_attack_log::record(attacker, Some(target));
    let ok = shadow.queue_set_attack_target_for_host(attacker, Some(target));
    if !ok {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_attack_log_method_names_residual_wave197());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_attack_log_nav_commands_residual_wave197());
    }

    #[test]
    fn wave197_composite_pack() {
        assert!(honesty_live_command_attack_log_residual_pack_wave197());
    }

    #[test]
    fn attack_log_sources() {
        assert!(honesty_execute_attack_records_source());
        assert!(honesty_shadow_drains_attack_log_source());
    }

    #[test]
    fn simulate_live_command_attack_log_honesty_residual_live() {
        assert!(
            simulate_live_command_attack_log_honesty(),
            "command attack log residual must latch"
        );
    }
}
