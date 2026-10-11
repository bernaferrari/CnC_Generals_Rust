//! Construction ownership audit. Historical CLI receipt names remain
//! navigation metadata; source checks require one canonical host writer and
//! immutable shadow comparisons. Runtime regressions exercise the host phases.

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live construction writeback residual method names.
pub const LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_METHOD_NAMES_WAVE181: &[&str] = &[
    "observe_host_construction",
    "update_construction",
    "compare_construction_to_host",
    "host_owns_construction",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_NAV_STEPS_WAVE181: &[&str] = &[
    "REQUIRE_CONSTRUCTION_PROGRESS_CHANNEL",
    "REQUIRE_TICK_AND_WRITEBACK",
    "REQUIRE_CONSTRUCTION_SOLE_TICK",
    "LIVE_HOST_TO_SHADOW_PERCENT",
    "LIVE_SOLE_TICK_ARMS",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_CMD_NAMES_WAVE181: &[&str] = &[
    "click_live_gameworld_construction_writeback_ok_progress",
    "click_live_gameworld_construction_writeback_ok_sole",
    "click_live_gameworld_construction_writeback_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_gameworld_construction_writeback_method_names_residual_wave181() -> bool {
    LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_METHOD_NAMES_WAVE181.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_METHOD_NAMES_WAVE181,
            "observe_host_construction",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_METHOD_NAMES_WAVE181,
            "host_owns_construction",
        ) == Some(3)
        && residual_name_index(
            LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_METHOD_NAMES_WAVE181,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_gameworld_construction_writeback_nav_commands_residual_wave181() -> bool {
    LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_NAV_STEPS_WAVE181.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_NAV_STEPS_WAVE181,
            "REQUIRE_CONSTRUCTION_PROGRESS_CHANNEL",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_NAV_STEPS_WAVE181,
            "LIVE_SOLE_TICK_ARMS",
        ) == Some(4)
        && RUNTIME_HOST_LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_CMD_NAMES_WAVE181.len() == 3
}

/// Wave 181 composite residual honesty pack.
pub fn honesty_live_gameworld_construction_writeback_residual_pack_wave181() -> bool {
    honesty_live_gameworld_construction_writeback_method_names_residual_wave181()
        && honesty_live_gameworld_construction_writeback_nav_commands_residual_wave181()
}

/// Source residual: construction progress channel + tick/writeback APIs.
pub fn honesty_construction_progress_channel_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    ok
}

/// Source residual: construction sole-tick requires coupling + auth + shadow.
pub fn honesty_construction_sole_tick_requires_coupling_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    ok
}

/// Live residual: host construction percent → GameWorld → sole-tick arms → writeback.
pub fn simulate_live_gameworld_construction_writeback_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "OwnedConstruction", true));
    let mut template = ThingTemplate::new("LiveConstr181");
    template.set_health(500.0).add_kind_of(KindOf::Structure);
    logic.templates.insert(template.name.clone(), template);
    let Some(id) = logic.create_object_for_player("LiveConstr181", 0, glam::Vec3::ZERO) else {
        return false;
    };
    let object = logic.host_object_mut(id).unwrap();
    object.construction_percent = 0.25;
    object.set_status_under_construction(true);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(entity) = shadow.entity_for_host(id) else {
        return false;
    };
    let Some(stale) = shadow.world_mut().world_mut().entity_mut(entity) else {
        return false;
    };
    stale.construction_percent = 1.0;
    stale.under_construction = false;
    stale.sold = true;
    stale.reconstructing = true;
    stale.unselectable = true;
    let differences = shadow.compare_construction_to_host(&logic);
    logic.host_object(id).is_some_and(|object| {
        differences == 1
            && object.construction_percent == 0.25
            && object.status.under_construction
            && !object.status.sold
            && !object.status.reconstructing
            && !object.status.unselectable
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_gameworld_construction_writeback_method_names_residual_wave181());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_gameworld_construction_writeback_nav_commands_residual_wave181());
    }

    #[test]
    fn wave181_composite_pack() {
        assert!(honesty_live_gameworld_construction_writeback_residual_pack_wave181());
    }

    #[test]
    fn construction_writeback_sources() {
        assert!(honesty_construction_progress_channel_source());
        assert!(honesty_construction_sole_tick_requires_coupling_source());
    }

    #[test]
    fn simulate_live_gameworld_construction_writeback_honesty_residual_live() {
        assert!(
            simulate_live_gameworld_construction_writeback_honesty(),
            "live GameWorld construction progress→tick→writeback residual must latch"
        );
    }
}
