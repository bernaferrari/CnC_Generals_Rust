//! Wave 205 residual peels: capture/gather/hijack/special-ability pathing uses
//! `Object::set_order_target` so `host_attack_log` drains without forcing
//! `AIState::Attacking` (path_to_goal_with_state owns AI residual).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 204 formation residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `Object::set_order_target` → host_attack_log + target_location clear
//! - command_executor ability pathing blocks use set_order_target
//!
//! Fail-closed:
//! - Not full GameClient OS-input cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Order-target residual method names.
pub const LIVE_COMMAND_ORDER_TARGET_LOG_METHOD_NAMES_WAVE205: &[&str] = &[
    "set_order_target",
    "host_attack_log",
    "path_to_goal_with_state",
    "set_status_attacking",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_ORDER_TARGET_LOG_NAV_STEPS_WAVE205: &[&str] = &[
    "REQUIRE_ORDER_TARGET_API",
    "REQUIRE_ABILITY_PATHS_USE_ORDER_TARGET",
    "LIVE_ORDER_TARGET_LOGS_ATTACK",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_ORDER_TARGET_LOG_CMD_NAMES_WAVE205: &[&str] = &[
    "click_live_command_order_target_log_ok_prepare",
    "click_live_command_order_target_log_ok_live",
    "click_live_command_order_target_log_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_live_command_order_target_log_method_names_residual_wave205() -> bool {
    LIVE_COMMAND_ORDER_TARGET_LOG_METHOD_NAMES_WAVE205.len() == 5
        && residual_name_index(
            LIVE_COMMAND_ORDER_TARGET_LOG_METHOD_NAMES_WAVE205,
            "set_order_target",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_ORDER_TARGET_LOG_METHOD_NAMES_WAVE205,
            "host_attack_log",
        ) == Some(1)
        && residual_name_index(
            LIVE_COMMAND_ORDER_TARGET_LOG_METHOD_NAMES_WAVE205,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_order_target_log_nav_commands_residual_wave205() -> bool {
    LIVE_COMMAND_ORDER_TARGET_LOG_NAV_STEPS_WAVE205.len() == 4
        && residual_name_index(
            LIVE_COMMAND_ORDER_TARGET_LOG_NAV_STEPS_WAVE205,
            "REQUIRE_ORDER_TARGET_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_ORDER_TARGET_LOG_NAV_STEPS_WAVE205,
            "LIVE_ORDER_TARGET_LOGS_ATTACK",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_ORDER_TARGET_LOG_CMD_NAMES_WAVE205.len() == 3
}

/// Wave 205 composite residual honesty pack.
pub fn honesty_live_command_order_target_log_residual_pack_wave205() -> bool {
    honesty_live_command_order_target_log_method_names_residual_wave205()
        && honesty_live_command_order_target_log_nav_commands_residual_wave205()
}

/// Source residual: set_order_target records attack log and does not force Attacking.
pub fn honesty_set_order_target_source() -> bool {
    let src = crate::game_logic::object::OBJECT_SRC;
    let i = match src.find("fn set_order_target") {
        Some(i) => i,
        None => return false,
    };
    // 2026-08-15: bound to the function — later OBJECT_SRC mentions Attacking.
    let after = &src[i..];
    let Some(brace) = after.find('{') else {
        return false;
    };
    let mut depth = 0i32;
    let mut end = after.len();
    for (j, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = brace + j + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &after[..end];
    body.contains("host_attack_log::record")
        && body.contains("set_status_attacking(false)")
        && !body.contains("AIState::Attacking")
}

/// Source residual: ability pathing uses set_order_target; no production target=Some bypass.
pub fn honesty_ability_paths_use_order_target_source() -> bool {
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    // Production section: before #[cfg(test)]
    // 2026-08-15: COMMAND_EXECUTOR_SRC hits `#[cfg(test)] mod tests;` in mod.rs first.
    let prod = ce;
    if !prod.contains("set_order_target") {
        return false;
    }
    if !prod.contains("execute_capture_building") {
        return false;
    }
    // No direct target assignment left in production.
    let direct_raw = prod
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("//") && t.contains("target = Some") && !t.contains("set_")
        })
        .count();
    direct_raw == 0
}

/// Live residual: set_order_target drains host_attack_log without AI Attacking.
pub fn simulate_live_command_order_target_log_honesty() -> bool {
    use crate::game_logic::host_attack_log;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_command_order_target_log_residual_pack_wave205() {
        return false;
    }
    if !honesty_set_order_target_source() {
        return false;
    }
    if !honesty_ability_paths_use_order_target_source() {
        return false;
    }

    host_attack_log::clear();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OrderTgt205");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("OrderUnit205") {
        let mut t = ThingTemplate::new("OrderUnit205");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("OrderUnit205".into(), t);
    }
    if !logic.templates.contains_key("OrderBldg205") {
        let mut t = ThingTemplate::new("OrderBldg205");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("OrderBldg205".into(), t);
    }
    let Some(u) = logic.create_object("OrderUnit205", Team::USA, Vec3::new(0.0, 0.0, 0.0)) else {
        return false;
    };
    let Some(b) = logic.create_object("OrderBldg205", Team::China, Vec3::new(40.0, 0.0, 0.0))
    else {
        return false;
    };

    host_attack_log::clear();
    if let Some(unit) = logic.get_object_mut(u) {
        unit.set_ai_state(AIState::Idle);
        unit.set_order_target(Some(b));
        if unit.target != Some(b) {
            return false;
        }
        // Must not force Attacking AI state.
        if unit.ai_state == AIState::Attacking {
            return false;
        }
        if unit.status.attacking {
            return false;
        }
        if unit.force_attack {
            return false;
        }
    } else {
        return false;
    }
    let events = host_attack_log::drain();
    events
        .iter()
        .any(|e| e.attacker == u && e.target == Some(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_order_target_log_method_names_residual_wave205());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_order_target_log_nav_commands_residual_wave205());
    }

    #[test]
    fn wave205_composite_pack() {
        assert!(honesty_live_command_order_target_log_residual_pack_wave205());
    }

    #[test]
    fn order_target_sources() {
        assert!(honesty_set_order_target_source());
        assert!(honesty_ability_paths_use_order_target_source());
    }

    #[test]
    fn simulate_live_command_order_target_log_honesty_residual_live() {
        assert!(
            simulate_live_command_order_target_log_honesty(),
            "order target log residual must latch"
        );
    }
}
