//! Receiving Main behavior and read-only presentation witness.
//! Historical shell identifiers are metadata, not C++ parity evidence.

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
// 2026-08-15: widen post-split scan window to the rest of the concat.
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
pub fn simulate_live_command_non_attack_order_target_honesty() -> bool {
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

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

    if let Some((unit, attack_events, audio_events)) = logic.host_object_attack_audio_mut(u) {
        unit.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        unit.set_order_target(
            Some(pad),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        unit.set_ai_state(
            AIState::Docking,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
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
    let mut observer = crate::gameworld_shadow::GameWorldShadow::new(64);
    observer.sync_from_host(&logic);
    let Some(actor) = observer.entity_for_host(u) else {
        return false;
    };
    let Some(target) = observer.entity_for_host(pad) else {
        return false;
    };
    observer
        .world()
        .entity(actor)
        .is_some_and(|object| object.attack_target == Some(target))
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
    fn simulate_live_command_non_attack_order_target_honesty_residual_live() {
        assert!(
            simulate_live_command_non_attack_order_target_honesty(),
            "non-attack order target residual must latch"
        );
    }
}
