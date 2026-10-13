//! Receiving Main formation state and immutable projection witness.
//! Historical navigation labels are diagnostic metadata, not C++ evidence.

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Formation owner and observation metadata.
pub const LIVE_COMMAND_FORMATION_LOG_METHOD_NAMES_WAVE204: &[&str] = &[
    "set_formation",
    "Object.formation_id",
    "Entity.formation_offset",
    "execute_create_formation",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_FORMATION_LOG_NAV_STEPS_WAVE204: &[&str] = &[
    "REQUIRE_CREATE_FORMATION_USES_SET_FORMATION",
    "REQUIRE_CANONICAL_FORMATION_VALUES",
    "LIVE_FROZEN_FORMATION_VALUES",
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
            "Entity.formation_offset",
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
            "LIVE_FROZEN_FORMATION_VALUES",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_FORMATION_LOG_CMD_NAMES_WAVE204.len() == 3
}

/// Wave 204 composite residual honesty pack.
pub fn honesty_live_command_formation_log_residual_pack_wave204() -> bool {
    honesty_live_command_formation_log_method_names_residual_wave204()
        && honesty_live_command_formation_log_nav_commands_residual_wave204()
}

/// Observe real formation setters through the receiving immutable projection.
pub fn simulate_live_command_formation_log_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::{Vec2, Vec3};

    if !honesty_live_command_formation_log_residual_pack_wave204() {
        return false;
    }
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

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    [(a, Vec2::new(1.0, -2.0)), (b, Vec2::new(-1.0, 2.0))]
        .into_iter()
        .all(|(id, offset)| {
            let Some(owner) = logic.get_object(id) else {
                return false;
            };
            let Some(entity_id) = shadow.entity_for_host(id) else {
                return false;
            };
            let Some(observed) = shadow.world().entity(entity_id) else {
                return false;
            };
            owner.formation_id == 7
                && owner.formation_offset == offset
                && observed.formation_id == 7
                && observed.formation_offset == [offset.x, offset.y]
        })
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
    fn simulate_live_command_formation_log_honesty_residual_live() {
        assert!(
            simulate_live_command_formation_log_honesty(),
            "receiving formation values must reach the immutable projection"
        );
    }
}
