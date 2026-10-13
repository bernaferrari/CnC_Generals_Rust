//! Receiving Main behavior and read-only presentation witness.
//! Historical shell identifiers are metadata, not C++ parity evidence.

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Production/construction log residual method names.
pub const LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_METHOD_NAMES_WAVE199: &[&str] = &[
    "create_object_under_construction",
    "host_owns_construction",
    "host_production_log::record_cancel",
    "HostProductionEvent::Cancel",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_NAV_STEPS_WAVE199: &[&str] = &[
    "REQUIRE_CONSTRUCTION_START_PROGRESS_LOG",
    "REQUIRE_PRODUCTION_CANCEL_LOG",
    "REQUIRE_SHADOW_HANDLES_CANCEL",
    "LIVE_LOGS_LATCH",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_CMD_NAMES_WAVE199: &[&str] = &[
    "click_live_command_production_construction_log_ok_prepare",
    "click_live_command_production_construction_log_ok_live",
    "click_live_command_production_construction_log_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_live_command_production_construction_log_method_names_residual_wave199() -> bool {
    LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_METHOD_NAMES_WAVE199.len() == 5
        && residual_name_index(
            LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_METHOD_NAMES_WAVE199,
            "create_object_under_construction",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_METHOD_NAMES_WAVE199,
            "HostProductionEvent::Cancel",
        ) == Some(3)
        && residual_name_index(
            LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_METHOD_NAMES_WAVE199,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_production_construction_log_nav_commands_residual_wave199() -> bool {
    LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_NAV_STEPS_WAVE199.len() == 5
        && residual_name_index(
            LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_NAV_STEPS_WAVE199,
            "REQUIRE_CONSTRUCTION_START_PROGRESS_LOG",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_NAV_STEPS_WAVE199,
            "LIVE_LOGS_LATCH",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_CMD_NAMES_WAVE199.len() == 3
}

/// Wave 199 composite residual honesty pack.
pub fn honesty_live_command_production_construction_log_residual_pack_wave199() -> bool {
    honesty_live_command_production_construction_log_method_names_residual_wave199()
        && honesty_live_command_production_construction_log_nav_commands_residual_wave199()
}

/// Source residual: under-construction create records progress log at 0%.
pub fn simulate_live_command_production_construction_log_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ProdConstr199");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("ConstrBldg199") {
        let mut t = ThingTemplate::new("ConstrBldg199");
        t.set_health(400.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("ConstrBldg199".into(), t);
    }

    let Some(bid) = logic.create_object_under_construction(
        "ConstrBldg199",
        Team::USA,
        Vec3::new(50.0, 0.0, 50.0),
    ) else {
        return false;
    };
    if !logic.host_object(bid).is_some_and(|object| {
        object.status.under_construction && object.construction_percent == 0.0
    }) {
        return false;
    }
    let Some(player) = logic
        .get_players()
        .values()
        .find(|player| player.team == Team::USA && player.is_alive)
        .map(|player| player.id)
    else {
        return false;
    };
    logic
        .get_player_mut(player)
        .expect("selected player")
        .resources
        .supplies = 1000;
    let mut unit_template = ThingTemplate::new("CancelUnit199");
    unit_template.set_health(100.0);
    unit_template.add_kind_of(KindOf::Infantry);
    logic
        .templates
        .insert("CancelUnit199".into(), unit_template);
    let mut factory_template = ThingTemplate::new("CancelFactory199");
    factory_template.add_kind_of(KindOf::Structure);
    factory_template.add_kind_of(KindOf::FSBarracks);
    logic
        .templates
        .insert("CancelFactory199".into(), factory_template);
    let Some(factory) =
        logic.create_object("CancelFactory199", Team::USA, Vec3::new(70.0, 0.0, 50.0))
    else {
        return false;
    };
    {
        use crate::game_logic::buildings::{
            BuildingData, BuildingType, ProductionItem, ProductionKind,
        };
        let (object, ownership_events) = logic
            .host_object_and_ownership_events_mut(factory)
            .expect("created factory");
        object.set_team_and_owner(Team::USA, Some(player), ownership_events);
        let building = object
            .building_data
            .get_or_insert_with(|| BuildingData::new(BuildingType::Barracks));
        building.production_queue.push(ProductionItem {
            template_name: "CancelUnit199".into(),
            progress: 0.0,
            total_time: 10.0,
            construction_frames: 0,
            cost: crate::game_logic::Resources {
                supplies: 250,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
    }
    if !logic.cancel_production(factory, "CancelUnit199".into()) {
        return false;
    }
    if logic
        .get_player(player)
        .is_none_or(|player| player.resources.supplies != 1250)
        || logic
            .host_object(factory)
            .and_then(|factory| factory.building_data.as_ref())
            .is_none_or(|building| !building.production_queue.is_empty())
        || logic.cancel_production(factory, "CancelUnit199".into())
    {
        return false;
    }
    let mut observer = crate::gameworld_shadow::GameWorldShadow::new(64);
    observer.sync_from_host(&logic);
    let Some(entity) = observer
        .entity_for_host(bid)
        .and_then(|id| observer.world().entity(id))
    else {
        return false;
    };
    entity.under_construction
        && entity.construction_percent == 0.0
        && logic
            .get_player(player)
            .is_some_and(|player| player.resources.supplies == 1250)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_production_construction_log_method_names_residual_wave199());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_production_construction_log_nav_commands_residual_wave199());
    }

    #[test]
    fn wave199_composite_pack() {
        assert!(honesty_live_command_production_construction_log_residual_pack_wave199());
    }

    #[test]
    fn simulate_live_command_production_construction_log_honesty_residual_live() {
        assert!(
            simulate_live_command_production_construction_log_honesty(),
            "production/construction log residual must latch"
        );
    }
}
