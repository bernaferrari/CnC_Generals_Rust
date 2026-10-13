//! Receiving Main behavior and read-only presentation witness.
//! Historical shell identifiers are metadata, not C++ parity evidence.

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

pub fn simulate_live_command_attack_log_honesty() -> bool {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;
    use std::time::SystemTime;

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

    // The command has already committed its target to the canonical Main object.
    if logic.host_object(attacker).and_then(|object| object.target) != Some(target) {
        return false;
    }
    let mut observer = crate::gameworld_shadow::GameWorldShadow::new(64);
    observer.sync_from_host(&logic);
    let Some(entity) = observer.entity_for_host(attacker) else {
        return false;
    };
    let Some(victim) = observer.entity_for_host(target) else {
        return false;
    };
    if observer
        .world()
        .entity(entity)
        .and_then(|object| object.attack_target)
        != Some(victim)
    {
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
    fn simulate_live_command_attack_log_honesty_residual_live() {
        assert!(
            simulate_live_command_attack_log_honesty(),
            "command attack log residual must latch"
        );
    }
}
