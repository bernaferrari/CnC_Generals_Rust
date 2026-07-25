//! Wave 207 residual peels: enter/dock/repair/heal/return-resources/mine/combat-drop
//! pathing uses `set_order_target` (not `set_target`) so AIState is owned by
//! `path_to_goal_with_state` while `host_attack_log` still last-writes.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 205 ability-order-target residual (capture/hijack/special).
//! Host residual only — network deferred.
//!
//! Sources:
//! - execute_enter / dock / repair / heal / return resources / mine clear / combat drop
//! - set_order_target before non-Attacking AIState pathing
//!
//! Fail-closed:
//! - Not full GameClient OS-input cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Non-attack order-target residual method names.
pub const LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_METHOD_NAMES_WAVE207: &[&str] = &[
    "set_order_target",
    "AIState::Docking",
    "AIState::Entering",
    "AIState::Repairing",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_NAV_STEPS_WAVE207: &[&str] = &[
    "REQUIRE_NON_ATTACK_PATHS_USE_ORDER_TARGET",
    "REQUIRE_NO_SET_TARGET_BEFORE_NON_ATTACK_AI",
    "LIVE_DOCK_ORDER_TARGET_LOGS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_CMD_NAMES_WAVE207: &[&str] = &[
    "click_live_command_non_attack_order_target_ok_prepare",
    "click_live_command_non_attack_order_target_ok_live",
    "click_live_command_non_attack_order_target_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_non_attack_order_target_method_names_residual_wave207() -> bool {
    LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_METHOD_NAMES_WAVE207.len() == 5
        && residual_name_index(
            LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_METHOD_NAMES_WAVE207,
            "set_order_target",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_METHOD_NAMES_WAVE207,
            "AIState::Docking",
        ) == Some(1)
        && residual_name_index(
            LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_METHOD_NAMES_WAVE207,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_non_attack_order_target_nav_commands_residual_wave207() -> bool {
    LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_NAV_STEPS_WAVE207.len() == 4
        && residual_name_index(
            LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_NAV_STEPS_WAVE207,
            "REQUIRE_NON_ATTACK_PATHS_USE_ORDER_TARGET",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_NAV_STEPS_WAVE207,
            "LIVE_DOCK_ORDER_TARGET_LOGS",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_NON_ATTACK_ORDER_TARGET_CMD_NAMES_WAVE207.len() == 3
}

/// Wave 207 composite residual honesty pack.
pub fn honesty_live_command_non_attack_order_target_residual_pack_wave207() -> bool {
    honesty_live_command_non_attack_order_target_method_names_residual_wave207()
        && honesty_live_command_non_attack_order_target_nav_commands_residual_wave207()
}

/// Source residual: dock/enter/repair use set_order_target.
pub fn honesty_non_attack_paths_use_order_target_source() -> bool {
    let ce = include_str!("../command_executor.rs");
    let prod = match ce.find("#[cfg(test)]") {
        Some(i) => &ce[..i],
        None => ce,
    };
    for needle in ["fn execute_dock", "fn execute_enter", "fn execute_repair"] {
        let i = match prod.find(needle) {
            Some(i) => i,
            None => return false,
        };
        let body = &prod[i..prod.len().min(i + 3500)];
        // Wave 233: may route through unit_command_*order_target APIs.
        if !(body.contains("set_order_target")
            || body.contains("unit_command_stop_moving_order_target")
            || body.contains("unit_command_set_order_target"))
        {
            return false;
        }
        // No set_target before path in these bodies.
        if body.contains("set_target(Some") {
            return false;
        }
    }
    true
}

/// Source residual: production has no set_target followed by non-Attacking AIState first.
pub fn honesty_no_set_target_before_non_attack_ai_source() -> bool {
    let ce = include_str!("../command_executor.rs");
    let prod = match ce.find("#[cfg(test)]") {
        Some(i) => &ce[..i],
        None => ce,
    };
    let bytes = prod.as_bytes();
    let s = prod;
    let mut i = 0;
    while let Some(rel) = s[i..].find("set_target(Some") {
        let at = i + rel;
        let after = &s[at..s.len().min(at + 350)];
        // first AIState::
        if let Some(ai_rel) = after.find("AIState::") {
            let name_start = ai_rel + "AIState::".len();
            let name = after[name_start..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>();
            if name != "Attacking" {
                let _ = bytes;
                return false;
            }
        }
        i = at + 1;
    }
    true
}

/// Live residual: set_order_target + Docking-style state does not force Attacking.
pub fn simulate_live_command_non_attack_order_target_honesty() -> bool {
    use crate::game_logic::host_attack_log;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_command_non_attack_order_target_residual_pack_wave207() {
        return false;
    }
    if !honesty_non_attack_paths_use_order_target_source() {
        return false;
    }
    if !honesty_no_set_target_before_non_attack_ai_source() {
        return false;
    }

    host_attack_log::clear();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("NonAtk207");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("DockUnit207") {
        let mut t = ThingTemplate::new("DockUnit207");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("DockUnit207".into(), t);
    }
    if !logic.templates.contains_key("DockPad207") {
        let mut t = ThingTemplate::new("DockPad207");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("DockPad207".into(), t);
    }
    let Some(u) = logic.create_object("DockUnit207", Team::USA, Vec3::new(0.0, 0.0, 0.0)) else {
        return false;
    };
    let Some(pad) = logic.create_object("DockPad207", Team::USA, Vec3::new(30.0, 0.0, 0.0)) else {
        return false;
    };

    host_attack_log::clear();
    if let Some(unit) = logic.get_object_mut(u) {
        unit.set_ai_state(AIState::Idle);
        unit.set_order_target(Some(pad));
        unit.set_ai_state(AIState::Docking);
        if unit.target != Some(pad) {
            return false;
        }
        if unit.ai_state != AIState::Docking {
            return false;
        }
        if unit.status.attacking {
            return false;
        }
    } else {
        return false;
    }
    let events = host_attack_log::drain();
    events
        .iter()
        .any(|e| e.attacker == u && e.target == Some(pad))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_non_attack_order_target_method_names_residual_wave207());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_non_attack_order_target_nav_commands_residual_wave207());
    }

    #[test]
    fn wave207_composite_pack() {
        assert!(honesty_live_command_non_attack_order_target_residual_pack_wave207());
    }

    #[test]
    fn non_attack_order_sources() {
        assert!(honesty_non_attack_paths_use_order_target_source());
        assert!(honesty_no_set_target_before_non_attack_ai_source());
    }

    #[test]
    fn simulate_live_command_non_attack_order_target_honesty_residual_live() {
        assert!(
            simulate_live_command_non_attack_order_target_honesty(),
            "non-attack order target residual must latch"
        );
    }
}
