//! The factory owns door-gated births, batches and the queued exit delay.
use crate::game_logic::game_logic::pose_owner_tests::isolated_at;
use crate::game_logic::{
    BuildingData, BuildingType, GameLogic, KindOf, ProductionItem, ProductionKind, Resources, Team,
    ThingTemplate,
};
use crate::presentation_frame::PresentationFrame;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

fn install_infantry_template(logic: &mut GameLogic, name: &str) {
    let mut template = ThingTemplate::new(name);
    template
        .set_health(100.0)
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    logic.templates.insert(name.into(), template);
}

fn ready_queue(unit: &str, quantity: u32) -> BuildingData {
    let mut building = BuildingData::new(BuildingType::Barracks);
    building.production_queue.push(ProductionItem {
        runtime_entry_identity: 0,
        template_name: unit.to_owned(),
        progress: 1.0,
        total_time: 1.0,
        construction_frames: 30,
        cost: Resources {
            supplies: 100,
            power: 0,
        },
        quantity_total: quantity,
        quantity_produced: 0,
        kind: ProductionKind::Unit,
    });
    building
}

#[test]
fn ready_unit_stays_queued_until_the_exit_door_opens() {
    isolated_at(
        module_path!(),
        "ready_unit_stays_queued_until_the_exit_door_opens",
        wait_for_open_door,
    );
}

fn wait_for_open_door() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DoorGateProduction");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object AmericaBarracks\nEnd\nObject DoorGateRanger\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Isolate authored door behavior from the genuine low-power build penalty.
    for player in logic.get_players_mut().values_mut() {
        player.power_produced = 1;
        player.power_consumed = 0;
    }
    install_infantry_template(&mut logic, "DoorGateRanger");
    // Retail producers author ProductionUpdate NumDoorAnimations (C++
    // ProductionUpdate door cycle gates spawn on WAITING_OPEN); the host
    // door table resolves retail names (AmericaBarracks: 1 door). The
    // invented name resolved 0 doors and bypassed the door gate entirely.
    let mut producer = ThingTemplate::new("AmericaBarracks");
    producer.set_health(500.0);
    producer.add_kind_of(KindOf::Structure);
    producer.add_kind_of(KindOf::FSBarracks);
    logic
        .templates
        .insert("AmericaBarracks".to_string(), producer);
    let producer_id = logic
        .create_object("AmericaBarracks", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("producer");
    {
        let producer = logic.host_object_mut(producer_id).expect("producer object");
        let building = ready_queue("DoorGateRanger", 1);
        // CLOSED: C++ ProductionUpdate must start its door cycle before spawn.
        producer.production_door_phase = 0;
        producer.building_data = Some(building);
    }

    let before = logic.host_objects().registered_len();
    for _ in 0..2 {
        logic.update_production(1.0 / 30.0);
    }
    assert_eq!(logic.host_objects().registered_len(), before);
    let closed = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(closed.objects.len(), before);
    let queued = closed
        .objects
        .iter()
        .find(|row| row.id == producer_id)
        .unwrap();
    assert_eq!(queued.production_queue.len(), 1);
    assert_eq!(queued.production_queue[0].template_name, "DoorGateRanger");

    // WAITING_OPEN permits one admission and removes the queue head.
    logic
        .host_object_mut(producer_id)
        .expect("producer object")
        .production_door_phase = 2;
    logic
        .host_object_mut(producer_id)
        .unwrap()
        .production_door_phases[0] = 2;
    logic.update_production(1.0 / 30.0);
    assert_eq!(logic.host_objects().registered_len(), before + 1);
    let opened = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(opened.objects.len(), before + 1);
    let submitted = opened
        .objects
        .iter()
        .find(|row| row.id == producer_id)
        .unwrap();
    assert!(submitted.production_queue.is_empty());
    assert_eq!(
        opened
            .objects
            .iter()
            .filter(|row| row.template_name == "DoorGateRanger")
            .count(),
        1
    );
    assert_eq!(queued.production_queue.len(), 1);
    assert_eq!(
        logic
            .host_objects()
            .registered_values()
            .filter(|object| object.template_name == "DoorGateRanger")
            .count(),
        1
    );
    assert!(
        logic
            .host_object(producer_id)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
}

#[test]
fn open_default_exit_admits_the_full_completed_batch() {
    isolated_at(
        module_path!(),
        "open_default_exit_admits_the_full_completed_batch",
        admit_full_batch,
    );
}

fn admit_full_batch() {
    use crate::game_logic::{ProductionExitMetadata, ProductionExitStyle};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DoorBatchProduction");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object DoorBatchBarracks\nEnd\nObject DoorBatchRanger\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Isolate authored door behavior from the genuine low-power build penalty.
    for player in logic.get_players_mut().values_mut() {
        player.power_produced = 1;
        player.power_consumed = 0;
    }
    install_infantry_template(&mut logic, "DoorBatchRanger");
    let mut producer = ThingTemplate::new("DoorBatchBarracks");
    producer.set_health(500.0);
    producer.add_kind_of(KindOf::Structure);
    producer.add_kind_of(KindOf::FSBarracks);
    // This is specifically DefaultProductionExitUpdate semantics: C++ has no
    // Queue delay/burst busy state, so a completed QuantityModifier batch can
    // reserve every member in one terminal update.
    producer.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Default,
        unit_create_point: [0.0, 0.0, 0.0],
        natural_rally_point: [0.0, 0.0, 0.0],
        exit_delay_frames: 0,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic
        .templates
        .insert("DoorBatchBarracks".to_string(), producer);
    let producer_id = logic
        .create_object(
            "DoorBatchBarracks",
            Team::USA,
            glam::Vec3::new(8.0, 0.0, 8.0),
        )
        .expect("producer");
    let hold_open_until = logic.get_frame().saturating_add(100);
    {
        let producer = logic.host_object_mut(producer_id).expect("producer object");
        let building = ready_queue("DoorBatchRanger", 2);
        // C++ QuantityModifier loop is only reached after an exit is available.
        producer.production_door_phase = 2;
        producer.production_door_phases[0] = 2;
        producer.production_door_phase_end_frame = hold_open_until;
        producer.production_door_phase_end_frames[0] = hold_open_until;
        producer.building_data = Some(building);
    }

    let before = logic.host_objects().registered_len();
    logic.update_production(1.0 / 30.0);
    let spawned: Vec<_> = logic
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name == "DoorBatchRanger")
        .collect();
    assert_eq!(
        spawned.len(),
        2,
        "one terminal QuantityModifier update must create every member"
    );
    assert!(
        spawned
            .iter()
            .all(|object| object.team == logic.host_object(producer_id).unwrap().team)
    );
    assert!(
        logic
            .host_object(producer_id)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    let published = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(published.objects.len(), before + 2);
    assert_eq!(
        published
            .objects
            .iter()
            .filter(|row| row.template_name == "DoorBatchRanger")
            .count(),
        2
    );
}

#[test]
fn queue_exit_releases_one_then_waits_exactly_nine_frames() {
    isolated_at(
        module_path!(),
        "queue_exit_releases_one_then_waits_exactly_nine_frames",
        queue_exit_delay,
    );
}

fn queue_exit_delay() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("QueueSoleTickProduction");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            // Retail ChinaBarracks exit: QueueProductionExitUpdate, 300 ms = 9 frames.
            crate::fixture_content::FixtureObject::new("QueueSoleProducer")
                .kind_of("STRUCTURE FS_BARRACKS")
                .health(500.0)
                .module(
                    "Behavior = QueueProductionExitUpdate FixtureExit\n UnitCreatePoint = X:0.0 Y:-25.0 Z:0.0\n NaturalRallyPoint = X:36.0 Y:-25.0 Z:0.0\n ExitDelay = 300\n InitialBurst = 0\nEnd",
                ),
            crate::fixture_content::FixtureObject::new("QueueSoleRanger").kind_of("SELECTABLE").health(100.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let producer_id = logic
        .create_object("QueueSoleProducer", Team::China, glam::Vec3::ZERO)
        .expect("producer");
    let hold_open_until = logic.get_frame().saturating_add(100);
    {
        let producer = logic.host_object_mut(producer_id).expect("producer object");
        let building = ready_queue("QueueSoleRanger", 2);
        producer.production_door_phase = 2;
        producer.production_door_phases[0] = 2;
        producer.production_door_phase_end_frame = hold_open_until;
        producer.production_door_phase_end_frames[0] = hold_open_until;
        producer.building_data = Some(building);
    }

    logic.update_production(1.0 / 30.0);
    let spawned = |logic: &GameLogic| {
        logic
            .host_objects()
            .registered_values()
            .filter(|object| object.template_name == "QueueSoleRanger")
            .count()
    };
    assert_eq!(
        spawned(&logic),
        1,
        "fresh Queue InitialBurst=0 releases one member"
    );
    let runtime = logic
        .host_object(producer_id)
        .unwrap()
        .production_exit_state;
    assert_eq!(runtime.delay_frames, 9);
    let first_release = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(
        first_release
            .objects
            .iter()
            .filter(|row| row.template_name == "QueueSoleRanger")
            .count(),
        1
    );

    for _ in 0..8 {
        logic.update();
    }
    assert_eq!(
        spawned(&logic),
        1,
        "second member remains held after eight logic frames"
    );
    logic.update();
    assert_eq!(
        spawned(&logic),
        2,
        "ninth Queue frame releases exactly one member"
    );
    let second_release = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(
        second_release
            .objects
            .iter()
            .filter(|row| row.template_name == "QueueSoleRanger")
            .count(),
        2
    );
    assert_eq!(
        first_release
            .objects
            .iter()
            .filter(|row| row.template_name == "QueueSoleRanger")
            .count(),
        1
    );

    assert!(
        logic
            .host_object(producer_id)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
}
