//! Executable command and lifecycle regressions.

fn command_production_construction_log() -> bool {
    use crate::game_logic::Team;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ loads Object INI before NewGame: the building under construction, the
    // trained unit and a producer with a ProductionUpdate are all authored.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("ConstrBldg199")
            .kind_of("STRUCTURE")
            .health(400.0),
        crate::fixture_content::FixtureObject::new("CancelUnit199")
            .kind_of("INFANTRY")
            .health(100.0)
            .walker(),
        crate::fixture_content::FixtureObject::new("CancelFactory199")
            .kind_of("STRUCTURE FS_BARRACKS")
            .module(
                "Body = StructureBody FixtureBody\n MaxHealth = 400.0\n InitialHealth = 400.0\nEnd",
            )
            .module("Behavior = ProductionUpdate FixtureProduction\nEnd")
            .module(
                "Behavior = DefaultProductionExitUpdate FixtureExit\n UnitCreatePoint = X: 0.0 Y: 0.0 Z: 0.0\n NaturalRallyPoint = X: 55.0 Y: 0.0 Z: 0.0\nEnd",
            ),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("ProdConstr199"),
    );
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }

    let Ok(Some(bid)) = logic.create_object_under_construction(
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
    let Some(factory) =
        logic.create_object_for_player("CancelFactory199", player, Vec3::new(70.0, 0.0, 50.0))
    else {
        return false;
    };
    {
        use crate::game_logic::buildings::{
            BuildingData, BuildingType, ProductionItem, ProductionKind,
        };
        let object = logic.host_object_mut(factory).expect("created factory");
        let building = object
            .building_data
            .get_or_insert_with(|| BuildingData::new(BuildingType::Barracks));
        building.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
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

#[test]
fn command_production_construction_log_runs() {
    assert!(command_production_construction_log());
}
