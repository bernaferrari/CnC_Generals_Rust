//! Construction, factory publication, and per-object supplies.

use super::*;

#[test]
fn unit_completion_identifies_the_published_object() {
    use crate::game_logic::{BuildingData, BuildingType, Player, ProductionExitMetadata};
    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "CompletionOwner", true);
    player.resources.supplies = 10_000;
    player.power_produced = 1;
    logic.add_player(player);
    let mut factory = ThingTemplate::new("CompletionFactory");
    factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .add_kind_of(KindOf::MpCountForVictory)
        .set_health(500.0);
    factory.production_exit_metadata = Some(ProductionExitMetadata {
        style: crate::game_logic::thing::ProductionExitStyle::Default,
        unit_create_point: [0.0; 3],
        natural_rally_point: [0.0; 3],
        exit_delay_frames: 0,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert(factory.name.clone(), factory);
    let mut unit = ThingTemplate::new("CompletionInfantry");
    unit.add_kind_of(KindOf::Infantry).set_health(100.0);
    unit.build_time = 0.05;
    unit.build_cost.supplies = 0;
    unit.build_cost.power = 0;
    logic.templates.insert(unit.name.clone(), unit);
    let producer = logic
        .create_object_for_player("CompletionFactory", 0, Vec3::ZERO)
        .unwrap();
    logic.host_object_mut(producer).unwrap().building_data =
        Some(BuildingData::new(BuildingType::Barracks));
    let before: std::collections::HashSet<_> =
        logic.host_objects().registered_keys().copied().collect();
    assert!(logic.enqueue_production(producer, "CompletionInfantry".into()));
    let mut completed = None;
    for _ in 0..300 {
        logic.update_with_dt(1.0 / 30.0);
        logic.production_events.complete_boundary();
        for event in logic.production_events.take_completed() {
            let crate::game_logic::host_production_log::HostProductionEvent::Complete {
                producer: actual_producer,
                template_name,
                spawned,
            } = event;
            if template_name == "CompletionInfantry" {
                assert_eq!(actual_producer, producer);
                assert!(
                    completed.replace(spawned).is_none(),
                    "one queue entry completes once"
                );
            }
        }
        if completed.is_some() {
            break;
        }
    }
    let spawned =
        completed.expect("accepted production must complete within its bounded build time");
    assert!(!before.contains(&spawned));
    let object = logic
        .host_object(spawned)
        .expect("Complete names an actual live Object");
    assert_eq!(object.template_name, "CompletionInfantry");
    assert_eq!(object.owner_player_id, Some(0));
    assert_eq!(object.team, Team::USA);
    let position = object.get_position();
    let frame = publish(&mut logic);
    let released = actor(&frame, spawned);
    assert_eq!(released.position, position);
    assert_eq!(released.template_name, "CompletionInfantry");
    assert_eq!(released.owner_player_id, Some(0));
}

#[test]
fn enqueue_publishes_the_queue_before_any_completion_receipt() {
    use crate::game_logic::{BuildingData, BuildingType, Player, ProductionExitMetadata};
    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "QueueOwner", true);
    player.resources.supplies = 10_000;
    player.power_produced = 1;
    logic.add_player(player);
    let mut barracks = ThingTemplate::new("ReceiptQueueFactory");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(500.0);
    barracks.production_exit_metadata = Some(ProductionExitMetadata {
        style: crate::game_logic::thing::ProductionExitStyle::Default,
        unit_create_point: [0.0; 3],
        natural_rally_point: [0.0; 3],
        exit_delay_frames: 0,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert(barracks.name.clone(), barracks);
    let mut unit = ThingTemplate::new("ReceiptQueueInfantry");
    unit.add_kind_of(KindOf::Infantry).set_health(100.0);
    unit.build_time = 1.0;
    unit.build_cost.supplies = 100;
    logic.templates.insert(unit.name.clone(), unit);
    let producer = logic
        .create_object_for_player("ReceiptQueueFactory", 0, Vec3::ZERO)
        .unwrap();
    logic.host_object_mut(producer).unwrap().building_data =
        Some(BuildingData::new(BuildingType::Barracks));
    assert!(logic.enqueue_production(producer, "ReceiptQueueInfantry".into()));
    let queue = &logic
        .host_object(producer)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_queue;
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].template_name, "ReceiptQueueInfantry");
    assert_eq!(logic.get_player(0).unwrap().resources.supplies, 9_900);
    logic.production_events.complete_boundary();
    assert!(
        logic.production_events.take_completed().is_empty(),
        "an accepted queue is not a completed unit"
    );
    let frame = publish(&mut logic);
    let factory = actor(&frame, producer);
    assert_eq!(factory.production_queue.len(), 1);
    assert_eq!(
        factory.production_queue[0].template_name,
        "ReceiptQueueInfantry"
    );
}

#[test]
fn stored_supplies_follow_the_object_and_freeze_between_frames() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("StoreSupCh");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object SsU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SsU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SsU".into(), t);
    }
    let id = logic
        .create_object("SsU", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_stored_supplies(900);
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, id);

    assert_eq!(row.stored_supplies, 900);
    assert_eq!(
        logic.host_object(id).unwrap().stored_resources.supplies,
        900
    );
    logic.host_object_mut(id).unwrap().set_stored_supplies(125);
    assert_eq!(actor(&publish(&mut logic), id).stored_supplies, 125);
    assert_eq!(actor(&frame, id).stored_supplies, 900);
}

#[test]
fn cancel_production_updates_only_subsequent_frames() {
    use crate::game_logic::{
        BuildingData, BuildingType, ProductionItem, ProductionKind, Resources,
    };
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("ProdAuthWB");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object ProdAuthBarracks\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("ProdAuthBarracks");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ProdAuthBarracks".into(), t);
    }
    let oid = logic
        .create_object(
            "ProdAuthBarracks",
            Team::USA,
            glam::Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("barracks");
    {
        let o = logic.host_object_mut(oid).expect("b");
        let mut bd = BuildingData::new(BuildingType::Barracks);
        bd.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
            template_name: "ProdAuthRanger".into(),
            progress: 2.0,
            total_time: 10.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 150,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        o.building_data = Some(bd);
    }
    let queued = publish(&mut logic);
    assert_eq!(actor(&queued, oid).production_queue.len(), 1);
    assert_eq!(
        actor(&queued, oid).production_queue[0].template_name,
        "ProdAuthRanger"
    );
    assert_eq!(
        logic
            .host_object(oid)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .len(),
        1
    );
    assert!(logic.cancel_production_at_index(oid, 0));
    assert!(
        logic
            .host_object(oid)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    assert!(actor(&publish(&mut logic), oid).production_queue.is_empty());
    assert_eq!(actor(&queued, oid).production_queue.len(), 1);
}

#[test]
fn radar_extension_status_stays_with_the_receiving_structure() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RadarEx");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object RadarB\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("RadarB");
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("RadarB".into(), t);
    }
    let oid = logic
        .create_object("RadarB", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.radar_extend_done_frame = 120;
        o.radar_extend_complete = false;
        o.radar_active = true;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert!(row.radar_active);
    assert!(!row.radar_extend_complete);
    assert_eq!(logic.host_object(oid).unwrap().radar_extend_done_frame, 120);
    logic.host_object_mut(oid).unwrap().radar_extend_complete = true;
    assert!(actor(&publish(&mut logic), oid).radar_extend_complete);
    assert!(!actor(&frame, oid).radar_extend_complete);
}

#[test]
fn construction_receipt_publishes_the_live_structure_once() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("USA_Barracks");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("USA_Barracks".into(), t);
    let id = logic
        .create_object("USA_Barracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    let partial = publish(&mut logic);
    assert_eq!(actor(&partial, id).template_name, "USA_Barracks");
    assert!(
        !partial
            .events
            .iter()
            .any(|event| matches!(event, PresentationEvent::ConstructionComplete { .. }))
    );
    logic.construction_events.record(id, "USA_Barracks");
    assert_eq!(logic.construction_events.pending().len(), 1);
    let complete = publish(&mut logic);
    assert_eq!(actor(&complete, id).template_name, "USA_Barracks");
    assert!(complete.events.iter().any(|event| matches!(event,
        PresentationEvent::ConstructionComplete { id: object, template }
            if *object == id && template == "USA_Barracks"
    )));
    assert!(logic.construction_events.pending().is_empty());
    assert!(
        !publish(&mut logic)
            .events
            .iter()
            .any(|event| matches!(event, PresentationEvent::ConstructionComplete { .. }))
    );
}

#[test]
fn production_door_phase_publishes_without_consuming_its_deadline() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ProdDoor");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("DoorFact")
            .kind_of("STRUCTURE FS_BARRACKS")
            .health(100.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("DoorFact", Team::USA, glam::Vec3::new(60.0, 0.0, 60.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.production_door_phase = 2;
        o.production_door_phase_end_frame = 500;
        o.production_door_hold_open = true;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.production_door_phase, 2);
    let o = logic.host_object(oid).unwrap();

    assert_eq!(o.production_door_phase, 2);
    assert_eq!(o.production_door_phase_end_frame, 500);
    assert!(o.production_door_hold_open);
}
