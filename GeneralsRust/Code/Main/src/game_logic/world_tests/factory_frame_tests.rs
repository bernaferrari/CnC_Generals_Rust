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
