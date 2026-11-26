//! Executable command and lifecycle regressions.

#[test]
fn cancellation_refunds_once_and_preserves_the_other_building_site() {
    use crate::game_logic::Team;
    use crate::presentation_frame::PresentationFrame;
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
    apply_skirmish_config(&mut logic, &cfg).unwrap();

    let bid = logic
        .create_object_under_construction("ConstrBldg199", Team::USA, Vec3::new(50.0, 0.0, 50.0))
        .unwrap()
        .unwrap();
    let site = logic.host_object(bid).unwrap();
    assert!(site.status.under_construction);
    assert_eq!(site.construction_percent, 0.0);
    let player = logic
        .get_players()
        .values()
        .find(|player| player.team == Team::USA && player.is_alive)
        .map(|player| player.id)
        .unwrap();
    logic
        .get_player_mut(player)
        .expect("selected player")
        .resources
        .supplies = 1000;
    let factory = logic
        .create_object_for_player("CancelFactory199", player, Vec3::new(70.0, 0.0, 50.0))
        .unwrap();
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
    let queued = PresentationFrame::try_build_from_logic(&mut logic, player).unwrap();
    let producer = queued.objects.iter().find(|row| row.id == factory).unwrap();
    assert_eq!(producer.production_queue.len(), 1);
    assert!(logic.cancel_production(factory, "CancelUnit199".into()));
    assert!(!logic.cancel_production(factory, "CancelUnit199".into()));
    assert_eq!(logic.get_player(player).unwrap().resources.supplies, 1250);
    let building = logic
        .host_object(factory)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap();
    assert!(building.production_queue.is_empty());

    let presentation = PresentationFrame::try_build_from_logic(&mut logic, player).unwrap();
    let site = presentation
        .objects
        .iter()
        .find(|row| row.id == bid)
        .unwrap();
    assert!(site.under_construction);
    assert_eq!(site.construction_percent, 0.0);
    let producer = presentation
        .objects
        .iter()
        .find(|row| row.id == factory)
        .unwrap();
    assert!(producer.production_queue.is_empty());
}
