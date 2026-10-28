use game_engine::common::system::DataChunkOutput;
use generals_main::command_system::SpecialPowerType;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use generals_main::save_load::SnapshotBuilder;

fn minimal_map_bytes() -> Vec<u8> {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        map.write_int(value);
    }
    for _ in 0..441 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    map.into_ckmp_bytes()
}

fn world(power: &str, enum_name: &str) -> (tempfile::TempDir, GameLogic, ObjectId) {
    let root = tempfile::tempdir().unwrap();
    let map_path = root.path().join("Owner.map");
    std::fs::write(&map_path, minimal_map_bytes()).unwrap();
    let definitions = format!(
            "SpecialPower {power}\n Enum = {enum_name}\n ReloadTime = 0\n ShortcutPower = Yes\nEnd\n\
             CommandButton BuildWithPower\n Command = SPECIAL_POWER_CONSTRUCT\n Object = ConstructTarget\n SpecialPower = {power}\nEnd\n\
             CommandButton ShortcutBuild\n Command = SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT\n Object = ConstructTarget\n SpecialPower = {power}\nEnd\n\
             CommandButton UnlistedBuild\n Command = SPECIAL_POWER_CONSTRUCT\n Object = ConstructTarget\n SpecialPower = {power}\nEnd\n\
             CommandSet ProducerSet\n 1 = BuildWithPower\nEnd\n\
             Object ConstructTarget\n KindOf = STRUCTURE SELECTABLE IMMOBILE\n Body = ActiveBody TargetBody\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\n\
             ObjectReskin ConstructTargetReskin ConstructTarget\nEnd\n\
             Object ConstructProducer\n KindOf = STRUCTURE SELECTABLE IMMOBILE\n CommandSet = ProducerSet\n Body = ActiveBody ProducerBody\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = ProductionUpdate ProducerProduction\n End\n Behavior = SpecialAbility ProducerShortcut\n SpecialPowerTemplate = {power}\n End\nEnd\n"
        );
    let (map_ini, object_ini) = definitions.split_once("Object ConstructTarget\n").unwrap();
    std::fs::write(root.path().join("Map.ini"), map_ini).unwrap();

    let mut logic = GameLogic::new();
    logic
        .admit_object_ini_sources([(
            "Object.ini".into(),
            format!("Object ConstructTarget\n{object_ini}"),
        )])
        .unwrap();
    logic.start_new_game(GameMode::SinglePlayer);
    assert!(logic.load_map(map_path.to_str().unwrap()));
    logic.add_player(Player::new(42, Team::USA, "ConstructOwner", true));
    let producer = logic
        .create_object_for_player("ConstructProducer", 42, glam::Vec3::new(50., 0., 50.))
        .unwrap();
    let power_type = match enum_name {
        "SPECIAL_SPY_DRONE" => SpecialPowerType::SpyDrone,
        _ => SpecialPowerType::SpySatellite,
    };
    logic
        .host_object_mut(producer)
        .unwrap()
        .set_special_power_ready_seconds(&power_type, 0.0);
    (root, logic, producer)
}

#[test]
fn public_special_power_construction_admission_is_owner_scoped_and_transient() {
    let (_owner_map, mut owner, owner_producer) =
        world("SpecialPowerSpySatellite", "SPECIAL_SPY_SATELLITE");
    let (_peer_map, mut peer, peer_producer) = world("SpecialPowerSpyDrone", "SPECIAL_SPY_DRONE");
    assert_eq!(owner_producer, peer_producer, "same map-local object ID");

    // The authored ordinary construct button is in this receiving producer's
    // CommandSet. An admitted but unlisted ordinary button cannot arm it.
    assert!(!owner.arm_special_power_construction(
        owner_producer,
        "UnlistedBuild",
        "SpecialPowerSpySatellite",
        0,
        "ConstructTarget",
    ));
    assert!(!owner.arm_special_power_construction(
        owner_producer,
        "NotAnAdmittedButton",
        "SpecialPowerSpySatellite",
        0,
        "ConstructTarget",
    ));
    // A different special power requested under an admitted button's name fails.
    assert!(!owner.arm_special_power_construction(
        owner_producer,
        "BuildWithPower",
        "SpecialPowerSpyDrone",
        0,
        "ConstructTarget",
    ));
    assert!(owner.arm_special_power_construction(
        owner_producer,
        "BuildWithPower",
        "SpecialPowerSpySatellite",
        0,
        "ConstructTarget",
    ));
    assert_eq!(
        owner.can_make_unit_for_place(owner_producer, "ConstructTarget"),
        game_engine::common::system::build_assistant::CanMakeType::Ok
    );
    let pending = owner
        .pending_special_power_construction_for_template(owner_producer, "ConstructTarget")
        .expect("exact authored target receives the bypass");
    assert_eq!(pending.special_power_name, "SpecialPowerSpySatellite");
    assert!(
        owner
            .pending_special_power_construction_for_template(
                owner_producer,
                "ConstructTargetReskin"
            )
            .is_none()
    );
    assert_eq!(
        owner.can_make_unit_for_place(owner_producer, "ConstructTargetReskin"),
        game_engine::common::system::build_assistant::CanMakeType::NoPrereq,
        "equivalent reskins do not satisfy the C++ ThingTemplate pointer check"
    );

    // ShortcutBuild is admitted as a receiving UI button but deliberately is
    // not present in ProducerSet. C++ resolves its ready producer from the
    // receiving shortcut button and then arms that producer's ProductionUpdate.
    assert!(owner.arm_special_power_construction(
        owner_producer,
        "ShortcutBuild",
        "SpecialPowerSpySatellite",
        0,
        "ConstructTarget",
    ));
    assert_eq!(
        owner
            .pending_special_power_construction(owner_producer)
            .unwrap()
            .command_button_name,
        "ShortcutBuild"
    );

    assert!(
        peer.pending_special_power_construction(peer_producer)
            .is_none()
    );
    assert_eq!(
        peer.can_make_unit_for_place(peer_producer, "ConstructTarget"),
        game_engine::common::system::build_assistant::CanMakeType::NoPrereq
    );
    owner.clear_special_power_construction(owner_producer);
    assert_eq!(
        owner.can_make_unit_for_place(owner_producer, "ConstructTarget"),
        game_engine::common::system::build_assistant::CanMakeType::NoPrereq
    );
    assert!(peer.arm_special_power_construction(
        peer_producer,
        "ShortcutBuild",
        "SpecialPowerSpyDrone",
        0,
        "ConstructTarget",
    ));
    assert_eq!(
        peer.pending_special_power_construction(peer_producer)
            .unwrap()
            .special_power_name,
        "SpecialPowerSpyDrone"
    );
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&peer).unwrap();
    let (_restored_map, mut restored, _) = world("SpecialPowerSpyDrone", "SPECIAL_SPY_DRONE");
    builder.restore_from_snapshot(&snapshot, &mut restored).unwrap();
    assert!(
        restored.pending_special_power_construction(peer_producer).is_none(),
        "the C++ ProductionUpdate construction button is not saved"
    );
    assert!(
        peer.pending_special_power_construction(peer_producer).is_some(),
        "restoring a different world cannot clear the live producer"
    );
    assert!(
        owner
            .pending_special_power_construction(owner_producer)
            .is_none()
    );
}
