//! Wave 204 residual peels: CreateFormation uses `Object::set_formation` so
//! `host_formation_log` drains `WorldMutation::SetFormation` last-writer.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 203 deploy-status residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `Object::set_formation` → host_formation_log
//! - `execute_create_formation` → set_formation
//! - shadow `apply_host_formation_events` → SetFormation
//!
//! Fail-closed:
//! - Not full GameClient OS-input cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Formation log residual method names.
pub const LIVE_COMMAND_FORMATION_LOG_METHOD_NAMES_WAVE204: &[&str] = &[
    "set_formation",
    "host_formation_log",
    "SetFormation",
    "execute_create_formation",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_FORMATION_LOG_NAV_STEPS_WAVE204: &[&str] = &[
    "REQUIRE_CREATE_FORMATION_USES_SET_FORMATION",
    "REQUIRE_SET_FORMATION_LOGS",
    "LIVE_FORMATION_LOGS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_FORMATION_LOG_CMD_NAMES_WAVE204: &[&str] = &[
    "click_live_command_formation_log_ok_prepare",
    "click_live_command_formation_log_ok_live",
    "click_live_command_formation_log_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_formation_log_method_names_residual_wave204() -> bool {
    LIVE_COMMAND_FORMATION_LOG_METHOD_NAMES_WAVE204.len() == 5
        && residual_name_index(
            LIVE_COMMAND_FORMATION_LOG_METHOD_NAMES_WAVE204,
            "set_formation",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_FORMATION_LOG_METHOD_NAMES_WAVE204,
            "SetFormation",
        ) == Some(2)
        && residual_name_index(
            LIVE_COMMAND_FORMATION_LOG_METHOD_NAMES_WAVE204,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_formation_log_nav_commands_residual_wave204() -> bool {
    LIVE_COMMAND_FORMATION_LOG_NAV_STEPS_WAVE204.len() == 4
        && residual_name_index(
            LIVE_COMMAND_FORMATION_LOG_NAV_STEPS_WAVE204,
            "REQUIRE_CREATE_FORMATION_USES_SET_FORMATION",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_FORMATION_LOG_NAV_STEPS_WAVE204,
            "LIVE_FORMATION_LOGS",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_FORMATION_LOG_CMD_NAMES_WAVE204.len() == 3
}

/// Wave 204 composite residual honesty pack.
pub fn honesty_live_command_formation_log_residual_pack_wave204() -> bool {
    honesty_live_command_formation_log_method_names_residual_wave204()
        && honesty_live_command_formation_log_nav_commands_residual_wave204()
}

/// Source residual: execute_create_formation uses set_formation (no direct field writes).
pub fn honesty_create_formation_uses_set_formation_source() -> bool {
    let ce = include_str!("../command_executor.rs");
    let i = match ce.find("fn execute_create_formation") {
        Some(i) => i,
        None => return false,
    };
    let body = &ce[i..ce.len().min(i + 2500)];
    body.contains("set_formation")
        && !body.contains("formation_id = new_id")
        && !body.contains("formation_offset = glam")
}

/// Source residual: set_formation records host_formation_log.
pub fn honesty_set_formation_logs_source() -> bool {
    let src = include_str!("object.rs");
    let i = match src.find("fn set_formation") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 500)];
    body.contains("host_formation_log::record")
}

/// Source residual: WorldMutation::SetFormation exists.
pub fn honesty_set_formation_mutation_source() -> bool {
    let wm = include_str!("../../../GameEngine/GameLogic/src/world/mod.rs");
    wm.contains("SetFormation") && wm.contains("formation_offset")
}

/// Live residual: set_formation drains host_formation_log.
pub fn simulate_live_command_formation_log_honesty() -> bool {
    use crate::game_logic::host_formation_log;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::{Vec2, Vec3};

    if !honesty_live_command_formation_log_residual_pack_wave204() {
        return false;
    }
    if !honesty_create_formation_uses_set_formation_source() {
        return false;
    }
    if !honesty_set_formation_logs_source() {
        return false;
    }
    if !honesty_set_formation_mutation_source() {
        return false;
    }

    host_formation_log::clear();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Form204");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("FormUnit204") {
        let mut t = ThingTemplate::new("FormUnit204");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("FormUnit204".into(), t);
    }
    let Some(a) = logic.create_object("FormUnit204", Team::USA, Vec3::new(0.0, 0.0, 0.0)) else {
        return false;
    };
    let Some(b) = logic.create_object("FormUnit204", Team::USA, Vec3::new(10.0, 0.0, 0.0)) else {
        return false;
    };

    host_formation_log::clear();
    if let Some(unit) = logic.get_object_mut(a) {
        unit.set_formation(7, Vec2::new(1.0, -2.0));
        if unit.formation_id != 7 {
            return false;
        }
    } else {
        return false;
    }
    if let Some(unit) = logic.get_object_mut(b) {
        unit.set_formation(7, Vec2::new(-1.0, 2.0));
    }

    let events = host_formation_log::drain();
    events.iter().any(|e| e.object == a && e.formation_id == 7)
        && events.iter().any(|e| e.object == b && e.formation_id == 7)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_formation_log_method_names_residual_wave204());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_formation_log_nav_commands_residual_wave204());
    }

    #[test]
    fn wave204_composite_pack() {
        assert!(honesty_live_command_formation_log_residual_pack_wave204());
    }

    #[test]
    fn formation_sources() {
        assert!(honesty_create_formation_uses_set_formation_source());
        assert!(honesty_set_formation_logs_source());
        assert!(honesty_set_formation_mutation_source());
    }

    #[test]
    fn simulate_live_command_formation_log_honesty_residual_live() {
        assert!(
            simulate_live_command_formation_log_honesty(),
            "formation log residual must latch"
        );
    }
}
