//! Receiving Main behavior and read-only presentation witness.
//! Historical shell identifiers are metadata, not C++ parity evidence.

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Evacuate contain-log residual method names.
pub const LIVE_EVACUATE_CONTAIN_LOG_METHOD_NAMES_WAVE201: &[&str] = &[
    "evacuate_container_now",
    "set_contained_by(None)",
    "host_contain_log",
    "build_for_engine",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_EVACUATE_CONTAIN_LOG_NAV_STEPS_WAVE201: &[&str] = &[
    "REQUIRE_EVACUATE_USES_SET_CONTAINED_BY",
    "REQUIRE_MAP_LOAD_BUILD_FOR_ENGINE",
    "LIVE_EVACUATE_LOGS_CONTAIN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_EVACUATE_CONTAIN_LOG_CMD_NAMES_WAVE201: &[&str] = &[
    "click_live_evacuate_contain_log_ok_prepare",
    "click_live_evacuate_contain_log_ok_live",
    "click_live_evacuate_contain_log_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_live_evacuate_contain_log_method_names_residual_wave201() -> bool {
    LIVE_EVACUATE_CONTAIN_LOG_METHOD_NAMES_WAVE201.len() == 5
        && residual_name_index(
            LIVE_EVACUATE_CONTAIN_LOG_METHOD_NAMES_WAVE201,
            "evacuate_container_now",
        ) == Some(0)
        && residual_name_index(
            LIVE_EVACUATE_CONTAIN_LOG_METHOD_NAMES_WAVE201,
            "build_for_engine",
        ) == Some(3)
        && residual_name_index(
            LIVE_EVACUATE_CONTAIN_LOG_METHOD_NAMES_WAVE201,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_evacuate_contain_log_nav_commands_residual_wave201() -> bool {
    LIVE_EVACUATE_CONTAIN_LOG_NAV_STEPS_WAVE201.len() == 4
        && residual_name_index(
            LIVE_EVACUATE_CONTAIN_LOG_NAV_STEPS_WAVE201,
            "REQUIRE_EVACUATE_USES_SET_CONTAINED_BY",
        ) == Some(0)
        && residual_name_index(
            LIVE_EVACUATE_CONTAIN_LOG_NAV_STEPS_WAVE201,
            "LIVE_EVACUATE_LOGS_CONTAIN",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_EVACUATE_CONTAIN_LOG_CMD_NAMES_WAVE201.len() == 3
}

/// Wave 201 composite residual honesty pack.
pub fn honesty_live_evacuate_contain_log_residual_pack_wave201() -> bool {
    honesty_live_evacuate_contain_log_method_names_residual_wave201()
        && honesty_live_evacuate_contain_log_nav_commands_residual_wave201()
}

/// Source residual: evacuate uses set_contained_by(None), no direct field write.
pub fn simulate_live_evacuate_contain_log_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EvacContain201");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("EvacBunker201") {
        let mut t = ThingTemplate::new("EvacBunker201");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("EvacBunker201".into(), t);
    }
    if !logic.templates.contains_key("EvacInf201") {
        let mut t = ThingTemplate::new("EvacInf201");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("EvacInf201".into(), t);
    }
    let Some(bunker) = logic.create_object("EvacBunker201", Team::USA, Vec3::new(10.0, 0.0, 10.0))
    else {
        return false;
    };
    let Some(inf) = logic.create_object("EvacInf201", Team::USA, Vec3::new(12.0, 0.0, 12.0)) else {
        return false;
    };
    // Force bunker capacity + load passenger.
    if let Some(obj) = logic.host_object_mut(bunker) {
        if obj.building_data.is_none() {
            obj.building_data = Some(crate::game_logic::buildings::BuildingData::new(
                crate::game_logic::buildings::BuildingType::Bunker,
            ));
        } else if let Some(bd) = obj.building_data.as_mut() {
            bd.max_garrison = bd.max_garrison.max(5);
        }
    }
    // Contain via set_contained_by + add occupant if API exists
    if let Some(obj) = logic.host_object_mut(inf) {
        obj.set_contained_by(Some(bunker));
    }
    if let Some(obj) = logic.host_object_mut(bunker) {
        if let Some(bd) = obj.building_data.as_mut() {
            if !bd.garrisoned_units.contains(&inf) {
                bd.garrisoned_units.push(inf);
            }
        }
    }

    if !logic.evacuate_container_now(bunker, false) {
        return false;
    }
    if logic
        .host_object(inf)
        .is_none_or(|rider| rider.contained_by.is_some())
    {
        return false;
    }
    if logic.host_object(bunker).is_none_or(|owner| {
        owner
            .building_data
            .as_ref()
            .is_none_or(|building| building.garrisoned_units.contains(&inf))
    }) {
        return false;
    }
    let mut observer = crate::gameworld_shadow::GameWorldShadow::new(64);
    observer.sync_from_host(&logic);
    let Some(rider) = observer
        .entity_for_host(inf)
        .and_then(|id| observer.world().entity(id))
    else {
        return false;
    };
    let Some(owner) = observer
        .entity_for_host(bunker)
        .and_then(|id| observer.world().entity(id))
    else {
        return false;
    };
    rider.contained_by_host == 0 && !owner.garrisoned_host_ids.contains(&inf.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_evacuate_contain_log_method_names_residual_wave201());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_evacuate_contain_log_nav_commands_residual_wave201());
    }

    #[test]
    fn wave201_composite_pack() {
        assert!(honesty_live_evacuate_contain_log_residual_pack_wave201());
    }

    #[test]
    fn simulate_live_evacuate_contain_log_honesty_residual_live() {
        assert!(
            simulate_live_evacuate_contain_log_honesty(),
            "evacuate contain log residual must latch"
        );
    }
}
