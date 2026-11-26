//! Factory timing, construction and containment across checked frame publication.
use crate::game_logic::{BuildingData, BuildingType, GameLogic, KindOf, Team, ThingTemplate};
use crate::presentation_frame::PresentationFrame;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use glam::Vec3;

#[test]
fn production_queue_advances_once_per_update_and_publishes_progress() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("P4Fact");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object P4Fact\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("P4Fact");
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSBarracks);
        logic.templates.insert("P4Fact".into(), t);
    }
    let owner = logic.local_player_id().unwrap();
    let oid = logic
        .create_object_for_player("P4Fact", owner, Vec3::new(6.0, 0.0, 6.0))
        .expect("id");
    let player = logic.get_player_mut(owner).unwrap();
    player.power_produced = 100;
    player.power_consumed = 100;
    assert_eq!(logic.production_power_factor_for_player(owner), 1.0);
    {
        use crate::game_logic::{ProductionItem, ProductionKind, Resources};
        let mut data = BuildingData::new(BuildingType::Barracks);
        data.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
            template_name: "Ranger".into(),
            progress: 0.0,
            total_time: 1.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 100,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        logic.host_object_mut(oid).unwrap().building_data = Some(data);
    }
    let initial = PresentationFrame::try_build_from_logic(&mut logic, owner).unwrap();
    let producer = initial.objects.iter().find(|row| row.id == oid).unwrap();
    assert_eq!(producer.production_queue[0].progress, 0.0);
    const TICKS: u32 = 5;
    const STEP: f32 = 1.0 / 30.0;
    for tick in 1..=TICKS {
        logic.update_production(STEP);
        let host = logic.host_object(oid).unwrap();
        let queue = &host.building_data.as_ref().unwrap().production_queue;
        assert_eq!(queue[0].construction_frames, tick);
        let frame = PresentationFrame::try_build_from_logic(&mut logic, owner).unwrap();
        let producer = frame.objects.iter().find(|row| row.id == oid).unwrap();
        let head = &producer.production_queue[0];
        assert_eq!(head.template_name, "Ranger");
        assert!((head.progress_ratio - tick as f32 * STEP).abs() < 1e-6);
    }
    assert_eq!(producer.production_queue[0].progress, 0.0);
}

#[test]
fn production_door_advances_opening_to_waiting_open() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("P4Door");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("P4Door")
            .kind_of("STRUCTURE")
            .health(100.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("P4Door", Team::USA, Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.production_door_phase = 1;
        o.production_door_phase_end_frame = 10;
    }
    // C++ ProductionUpdate::updateDoors advances after the deadline.
    let producer = logic.host_object_mut(oid).unwrap();
    assert!(!producer.tick_production_door(10));
    assert_eq!(producer.production_door_phase, 1);
    assert!(!producer.tick_production_door(11));
    assert_eq!(producer.production_door_phase, 2);
}

#[test]
fn publishing_a_construction_site_does_not_advance_it() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("P4Ctor");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object P4Ctor\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("P4Ctor");
        t.set_health(400.0);
        t.build_time = 10.0;
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("P4Ctor".into(), t);
    }
    let oid = logic
        .create_object("P4Ctor", Team::USA, Vec3::new(7.0, 0.0, 7.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.status.under_construction = true;
        o.construction_percent = 0.99;
    }

    let frame = logic.get_frame();
    for _ in 0..3 {
        let presentation = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        let site = presentation
            .objects
            .iter()
            .find(|row| row.id == oid)
            .unwrap();
        assert!((site.construction_percent - 0.99).abs() < 1e-5);
        assert!(site.under_construction);
    }
    let site = logic.host_object(oid).unwrap();
    assert!((site.construction_percent - 0.99).abs() < 1e-5);
    assert!(site.status.under_construction);
    assert_eq!(logic.get_frame(), frame);
}

#[test]
fn evacuation_clears_the_published_roster_before_bunker_retirement() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("P5Cont");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("P5Bunker")
                .kind_of("SELECTABLE STRUCTURE")
                .health(200.0),
            crate::fixture_content::FixtureObject::new("P5Inf")
                .kind_of("SELECTABLE STRUCTURE")
                .health(200.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["P5Bunker", "P5Inf"] {
        {
            let mut t = ThingTemplate::new(name);
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            if name == "P5Bunker" {
                t.add_kind_of(KindOf::Structure);
            }
            logic.templates.insert(name.into(), t);
        }
    }
    let bunker = logic
        .create_object("P5Bunker", Team::USA, Vec3::ZERO)
        .expect("b");
    let inf = logic
        .create_object("P5Inf", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("i");
    {
        let o = logic.host_object_mut(bunker).expect("b");
        o.building_data = Some(BuildingData::new(BuildingType::Bunker));
        assert!(o.add_occupant(inf));
    }
    {
        let (o, fixture_partition) = logic.host_object_and_partition_mut(inf).expect("i");
        o.set_contained_by_with_partition_owner(Some(bunker), true, fixture_partition);
    }
    let occupied = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let container = occupied
        .objects
        .iter()
        .find(|row| row.id == bunker)
        .unwrap();
    assert_eq!(container.garrisoned_units, [inf]);
    assert_eq!(logic.host_object(inf).unwrap().contained_by, Some(bunker));

    let frame = logic.get_frame();
    assert!(logic.evacuate_container_now(bunker, false).unwrap());
    assert_eq!(logic.host_object(inf).unwrap().contained_by, None);
    let evacuated = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let empty = evacuated
        .objects
        .iter()
        .find(|row| row.id == bunker)
        .unwrap();
    assert!(empty.garrisoned_units.is_empty());
    assert_eq!(container.garrisoned_units, [inf]);
    assert_eq!(logic.get_frame(), frame);

    logic.destroy_object(bunker).unwrap();
    assert!(logic.host_object(bunker).unwrap().status.destroyed);
    logic.process_destroy_list();
    let retired = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert!(retired.objects.iter().all(|row| row.id != bunker));
    assert!(retired.objects.iter().any(|row| row.id == inf));
    assert_eq!(logic.host_object(inf).unwrap().contained_by, None);
}

#[test]
fn factory_frame_keeps_the_owned_queue_rally_point_and_occupants() {
    use crate::fixture_content::{FixtureObject, admit_fixture_objects};
    use crate::game_logic::{ProductionItem, ProductionKind, Resources};
    use crate::presentation_frame::PresentationBuildingType;

    let mut logic = GameLogic::new();
    admit_fixture_objects(
        &mut logic,
        &[
            FixtureObject::new("VisibleFactory")
                .kind_of("STRUCTURE FS_BARRACKS SELECTABLE")
                .health(500.0)
                .module(
                    "Behavior = DefaultProductionExitUpdate FixtureExit
 UnitCreatePoint = X:0 Y:0 Z:0
 NaturalRallyPoint = X:20 Y:20 Z:0
End",
                ),
            FixtureObject::new("VisibleOccupant")
                .kind_of("INFANTRY")
                .health(100.0),
        ],
    );
    let factory = logic
        .create_object("VisibleFactory", Team::USA, Vec3::ZERO)
        .unwrap();
    let rider = logic
        .create_object("VisibleOccupant", Team::USA, Vec3::X)
        .unwrap();
    let actor = logic.host_object_mut(factory).unwrap();
    let mut data = BuildingData::new(BuildingType::Barracks);
    data.production_queue.push(ProductionItem {
        runtime_entry_identity: 0,
        template_name: "VisibleOccupant".into(),
        progress: 0.35,
        total_time: 10.0,
        construction_frames: 0,
        cost: Resources {
            supplies: 225,
            power: 0,
        },
        quantity_total: 1,
        quantity_produced: 0,
        kind: ProductionKind::Unit,
    });
    data.max_garrison = 5;
    data.garrisoned_units.push(rider);
    actor.building_data = Some(data);
    let first_rally = Vec3::new(20.0, 0.0, 20.0);
    actor.set_rally_point_for_exit(first_rally);
    let (rider_actor, partition) = logic.host_object_and_partition_mut(rider).unwrap();
    rider_actor.set_contained_by_with_partition_owner(Some(factory), true, partition);

    let before = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let first = before.objects.iter().find(|row| row.id == factory).unwrap();
    assert_eq!(
        first.building_type,
        Some(PresentationBuildingType::Barracks)
    );
    assert_eq!(first.rally_point, Some(first_rally));
    assert_eq!(first.garrisoned_units, [rider]);
    assert_eq!(first.occupant_count, 1);
    assert_eq!(first.max_garrison, 5);
    assert_eq!(first.production_queue.len(), 1);
    assert_eq!(first.production_queue[0].template_name, "VisibleOccupant");
    assert_eq!(first.production_queue[0].cost_supplies, 225);
    assert_eq!(first.production_queue[0].progress, 0.35);

    let next_rally = Vec3::new(9.0, 0.0, 8.0);
    let actor = logic.host_object_mut(factory).unwrap();
    actor.set_rally_point_for_exit(next_rally);
    actor.building_data.as_mut().unwrap().production_queue[0].progress = 0.75;
    let after = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let next = after.objects.iter().find(|row| row.id == factory).unwrap();
    assert_eq!(next.rally_point, Some(next_rally));
    assert_eq!(next.production_queue[0].progress, 0.75);
    assert_eq!(first.rally_point, Some(first_rally));
    assert_eq!(first.production_queue[0].progress, 0.35);
}

#[test]
fn construction_frames_use_their_own_actor_state_with_equal_ids() {
    use crate::fixture_content::{FixtureObject, admit_fixture_objects};
    use crate::game_logic::ObjectId;

    fn site() -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        admit_fixture_objects(
            &mut world,
            &[FixtureObject::new("VisibleSite")
                .kind_of("STRUCTURE SELECTABLE")
                .health(500.0)],
        );
        let id = world
            .create_object("VisibleSite", Team::USA, Vec3::ZERO)
            .unwrap();
        let actor = world.host_object_mut(id).unwrap();
        actor.construction_percent = 0.0;
        actor.set_status_under_construction(true);
        (world, id)
    }
    let (mut owner, id) = site();
    let (mut peer, peer_id) = site();
    assert_eq!(id, peer_id);
    let zero = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    let initial = zero.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(initial.construction_percent, 0.0);
    assert!(initial.under_construction);

    owner.host_object_mut(id).unwrap().construction_percent = 0.25;
    let foreign_actor = peer.host_object_mut(peer_id).unwrap();
    foreign_actor.construction_percent = 0.9;
    foreign_actor.status.sold = true;
    foreign_actor.status.reconstructing = true;
    foreign_actor.status.unselectable = true;
    foreign_actor.status.stealthed = true;
    let foreign = PresentationFrame::try_build_from_logic(&mut peer, 0).unwrap();
    let other = foreign.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(other.construction_percent, 0.9);
    assert!(other.sold && other.unselectable);

    for _ in 0..2 {
        let frame = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
        let own = frame.objects.iter().find(|row| row.id == id).unwrap();
        assert_eq!(own.construction_percent, 0.25);
        assert!(own.under_construction);
        assert!(!own.sold && !own.unselectable);
        let actor = owner.host_object(id).unwrap();
        assert!(!actor.status.reconstructing);
        assert!(!actor.status.stealthed && !actor.status.selected);
    }
    assert_eq!(initial.construction_percent, 0.0);
    assert_eq!(owner.get_frame(), 0);
    assert_eq!(peer.get_frame(), 0);
}

#[test]
fn construction_completion_and_body_heal_publish_separate_changes() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("ConstHp");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("PadHp")
                .kind_of("STRUCTURE")
                .health(500.0),
            crate::fixture_content::FixtureObject::new("HealthVictoryKeepAlive")
                .kind_of("STRUCTURE MP_COUNT_FOR_VICTORY")
                .health(100.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("PadHp", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.status.under_construction = true;
        o.construction_percent = 0.99;
        o.health.current = 50.0;
    }
    // CPP VictoryConditions87–95/168–196: skirmish requires a live
    // victory-counting structure. Otherwise the session defeats this player
    // and kills its army while this control is checking health writeback.
    logic
        .create_object("HealthVictoryKeepAlive", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let unfinished = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let prior = unfinished.objects.iter().find(|row| row.id == oid).unwrap();
    assert_eq!(prior.health_current, 50.0);
    assert_eq!(prior.construction_percent, 0.99);
    assert!(prior.under_construction);

    // The Dozer construction tick supplies body health separately from completion.
    logic.notify_structure_construction_complete(oid).unwrap();
    let complete = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let built = complete.objects.iter().find(|row| row.id == oid).unwrap();
    assert_eq!(built.construction_percent, 1.0);
    assert!(!built.under_construction);
    assert_eq!(built.health_current, 50.0);

    logic.health_events.clear_heal();
    let frame = logic.get_frame();
    let (body, events) = logic.host_object_and_health_events_mut(oid).unwrap();
    body.heal(450.0, events, frame);
    assert_eq!(body.health.current, 500.0);
    let heals = logic.health_events.drain_heal();
    assert_eq!(heals.len(), 1);
    assert_eq!(heals[0].target, oid);
    assert_eq!(heals[0].health, 500.0);
    let healed = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(
        healed
            .objects
            .iter()
            .find(|row| row.id == oid)
            .unwrap()
            .health_current,
        500.0
    );
    assert_eq!(prior.construction_percent, 0.99);
    assert_eq!(built.health_current, 50.0);
}

#[test]
fn rebuild_frame_keeps_receiving_actor_links_and_deadline() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RebuildP");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object HoleA\nEnd\nObject BldA\nEnd\nObject WorkerA\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["HoleA", "BldA", "WorkerA"] {
        {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert(name.into(), t);
        }
    }
    let hole = logic
        .create_object("HoleA", Team::USA, glam::Vec3::new(20.0, 0.0, 20.0))
        .expect("hole");
    let bld = logic
        .create_object("BldA", Team::USA, glam::Vec3::new(22.0, 0.0, 20.0))
        .expect("bld");
    let worker = logic
        .create_object("WorkerA", Team::USA, glam::Vec3::new(24.0, 0.0, 20.0))
        .expect("worker");
    {
        let o = logic.host_object_mut(hole).expect("o");
        o.is_rebuild_hole = true;
        o.rebuild_template_name = Some("BldA".into());
        o.rebuild_ready_frame = 100;
        o.rebuild_spawner_id = Some(bld);
        o.rebuild_worker_id = Some(worker);
        o.rebuild_reconstructing_id = Some(bld);
        o.producer_id = Some(hole);
        o.construction_complete_clear_frame = 250;
    }

    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let rebuild = frame.objects.iter().find(|row| row.id == hole).unwrap();
    assert!(rebuild.is_rebuild_hole);
    assert_eq!(rebuild.rebuild_template_name, "BldA");
    assert_eq!(rebuild.rebuild_ready_frame, 100);
    assert_eq!(rebuild.rebuild_spawner_id, Some(bld));
    assert_eq!(rebuild.rebuild_worker_id, Some(worker));
    assert_eq!(rebuild.rebuild_reconstructing_id, Some(bld));
    assert_eq!(rebuild.producer_id, Some(hole));
    assert_eq!(
        logic
            .host_object(hole)
            .unwrap()
            .construction_complete_clear_frame,
        250
    );

    let actor = logic.host_object_mut(hole).unwrap();
    actor.rebuild_ready_frame = 101;
    let refreshed = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let next = refreshed.objects.iter().find(|row| row.id == hole).unwrap();
    assert_eq!(next.rebuild_ready_frame, 101);
    assert_eq!(rebuild.rebuild_ready_frame, 100);
}
