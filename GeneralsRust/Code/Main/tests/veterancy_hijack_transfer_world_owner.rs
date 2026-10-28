//! Live hijack and car-bomb veterancy transfers use the receiving world's
//! GameData when separate sessions contain objects with matching IDs.
use game_engine::common::ini::ini_locomotor::LocomotorStore;
use game_engine::common::system::DataChunkOutput;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team, VeterancyLevel};
use generals_main::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use glam::Vec3;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

const PLAYER_GLA: u32 = 41;
const PLAYER_USA: u32 = 42;
const HIJACKER: &str = "GLAInfantryHijacker";
const TERRORIST: &str = "GLATerrorist";
const VEHICLE: &str = "ReceivingHijackVehicle";

fn map_file(root: &Path) -> std::path::PathBuf {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        map.write_int(value);
    }
    for _ in 0..1681 {
        map.write_byte(0);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    let path = root.join("HijackVeterancyOwner.map");
    std::fs::write(&path, map.into_ckmp_bytes()).expect("write receiving map");
    path
}

fn world(map: &Path, veteran_bonus: u32, elite_bonus: u32) -> GameLogic {
    let rank_marker = if veteran_bonus == 130 {
        "BOOBY_TRAPPED"
    } else {
        "CAN_STEALTH"
    };
    let mut locomotors = LocomotorStore::new_owned();
    locomotors
        .apply_source(
            "Locomotor HijackOwnerGround\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n\
             Locomotor HijackOwnerVehicle\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 120\n Appearance = FOUR_WHEELS\nEnd\n",
        )
        .expect("receiving locomotor definitions accepted");
    let mut world = GameLogic::new();
    world
        .admit_locomotor_definitions(locomotors)
        .expect("receiving locomotor store admitted");
    world
        .admit_combat_bonus_ini_sources([format!(
            "GameData\n HealthBonus_Veteran = {veteran_bonus}%\n HealthBonus_Elite = {elite_bonus}%\nEnd\n"
        )])
        .expect("receiving GameData accepted");
    let object_ini = format!(
        r#"Object {HIJACKER}
 IsTrainable = Yes
 KindOf = INFANTRY SELECTABLE
 Locomotor = SET_NORMAL HijackOwnerGround
 ExperienceRequired = 0 60 150 300
 Body = ActiveBody HijackerBody
 MaxHealth = 100
 InitialHealth = 100
 End
 Behavior = AIUpdateInterface HijackerAI
 End
 Behavior = HijackerUpdate HijackerRider
 End
 Behavior = VeterancyGainCreate HijackerRank
 StartingLevel = VETERAN
 End
 Behavior = ConvertToHijackedVehicleCrateCollide HijackerCollide
 End
End
Object {TERRORIST}
 IsTrainable = Yes
 KindOf = INFANTRY SELECTABLE
 Locomotor = SET_NORMAL HijackOwnerGround
 ExperienceRequired = 0 60 150 300
 Body = ActiveBody TerroristBody
 MaxHealth = 100
 InitialHealth = 100
 End
 Behavior = AIUpdateInterface TerroristAI
 End
 Behavior = VeterancyGainCreate TerroristRank
 StartingLevel = VETERAN
 End
 Behavior = ConvertToCarBombCrateCollide TerroristCollide
 End
End
Object {VEHICLE}
 KindOf = VEHICLE SELECTABLE SCORE
 Locomotor = SET_NORMAL HijackOwnerVehicle
 IsTrainable = Yes
 ExperienceRequired = 0 60 150 300
 Body = ActiveBody VehicleBody
 MaxHealth = 1000
 InitialHealth = 1000
 End
 Behavior = AIUpdateInterface VehicleAI
 End
 Behavior = StatusBitsUpgrade RankMarker
 TriggeredBy = Upgrade_Veterancy_VETERAN
 StatusToSet = {rank_marker}
 End
 Behavior = EjectPilotDie PilotEject
 End
 WeaponSet
 Conditions = NONE
 End
 WeaponSet
 Conditions = CARBOMB
 End
End
"#
    );
    world
        .admit_object_ini_sources([("VeterancyTransfers.ini".into(), object_ini)])
        .expect("receiving object definitions accepted");
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().expect("map path UTF-8")));
    let mut gla = Player::new(PLAYER_GLA, Team::GLA, "GLA", true);
    gla.alliance_team = 1;
    let mut usa = Player::new(PLAYER_USA, Team::USA, "USA", false);
    usa.alliance_team = 2;
    world.add_player(gla);
    world.add_player(usa);
    world
}

fn save_info() -> SaveGameInfo {
    SaveGameInfo {
        pristine_map_name: None,
        filename: "converted-car-bomb".into(),
        display_name: "Converted car bomb ownership fixture".into(),
        description: String::new(),
        map_name: "HijackVeterancyOwner".into(),
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH + Duration::from_secs(1),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

fn start_hijack(world: &mut GameLogic) -> (ObjectId, ObjectId) {
    let hijacker = world
        .create_object_for_player(HIJACKER, PLAYER_GLA, Vec3::new(100.0, 0.0, 100.0))
        .expect("admitted veteran hijacker");
    let vehicle = world
        .create_object_for_player(VEHICLE, PLAYER_USA, Vec3::new(102.0, 0.0, 100.0))
        .expect("admitted vehicle");
    assert!(world.host_object(hijacker).unwrap().health.current > 0.0);
    assert!(world.host_object(hijacker).unwrap().health.maximum > 0.0);
    assert!(world.host_object(vehicle).unwrap().health.current > 0.0);
    assert!(world.host_object(vehicle).unwrap().health.maximum > 0.0);
    assert_eq!(
        world.host_object(vehicle).unwrap().owner_player_id,
        Some(PLAYER_USA)
    );
    assert_eq!(world.host_object(vehicle).unwrap().team, Team::USA);
    let status = world.host_object(vehicle).unwrap().object_status_bits;
    assert!(
        !generals_main::game_logic::host_status_bits_upgrade::status_bits_has(
            status,
            "BOOBY_TRAPPED"
        )
    );
    assert!(
        !generals_main::game_logic::host_status_bits_upgrade::status_bits_has(
            status,
            "CAN_STEALTH"
        )
    );
    assert_eq!(
        world.host_object(hijacker).unwrap().experience.level,
        VeterancyLevel::Veteran
    );
    world.queue_command(GameCommand {
        command_type: CommandType::Hijack { target_id: vehicle },
        player_id: PLAYER_GLA,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![hijacker],
        modifier_keys: ModifierKeys::default(),
    });
    world.process_commands();
    assert!(
        !world.has_pending_commands(),
        "queued hijack command processed"
    );
    let ordered = world.host_object(hijacker).expect("hijacker after command");
    assert_eq!(
        ordered.ai_state(),
        generals_main::game_logic::AIState::SpecialAbility,
        "GameCommand::Hijack must pass unit/team/path gates and enqueue pending Hijack"
    );
    assert_eq!(ordered.target, Some(vehicle));
    assert_eq!(
        world.host_object(vehicle).unwrap().owner_player_id,
        Some(PLAYER_USA)
    );
    assert_eq!(world.host_object(vehicle).unwrap().team, Team::USA);
    (hijacker, vehicle)
}

fn start_car_bomb(world: &mut GameLogic) -> (ObjectId, ObjectId) {
    let terrorist = world
        .create_object_for_player(TERRORIST, PLAYER_GLA, Vec3::new(100.0, 0.0, 100.0))
        .expect("admitted veteran terrorist");
    // ConvertToCarBomb accepts a neutral vehicle; leave it unowned just as a
    // map vehicle would be before the player converts it.
    let vehicle = world
        .create_object(VEHICLE, Team::Neutral, Vec3::new(102.0, 0.0, 100.0))
        .expect("admitted neutral vehicle");
    let status = world.host_object(vehicle).unwrap().object_status_bits;
    assert!(
        !generals_main::game_logic::host_status_bits_upgrade::status_bits_has(
            status,
            "BOOBY_TRAPPED"
        )
    );
    assert!(
        !generals_main::game_logic::host_status_bits_upgrade::status_bits_has(
            status,
            "CAN_STEALTH"
        )
    );
    assert_eq!(
        world.host_object(terrorist).unwrap().experience.level,
        VeterancyLevel::Veteran
    );
    world.queue_command(GameCommand {
        command_type: CommandType::ConvertToCarbomb { target_id: vehicle },
        player_id: PLAYER_GLA,
        command_id: 2,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![terrorist],
        modifier_keys: ModifierKeys::default(),
    });
    world.process_commands();
    assert!(
        !world.has_pending_commands(),
        "queued car-bomb command processed"
    );
    let ordered = world
        .host_object(terrorist)
        .expect("terrorist after command");
    assert_eq!(
        ordered.ai_state(),
        generals_main::game_logic::AIState::SpecialAbility,
        "GameCommand::ConvertToCarbomb must pass unit/path gates and enqueue pending CarBomb"
    );
    assert_eq!(ordered.target, Some(vehicle));
    (terrorist, vehicle)
}

fn update_until_veteran(scenario: &str, world: &mut GameLogic, actor: ObjectId, vehicle: ObjectId) {
    let describe = |world: &GameLogic, id: ObjectId| {
        world.host_object(id).map_or_else(
            || format!("{id:?}=<not registered>"),
            |object| {
                format!(
                    "{id:?}={{health:{:.1}/{:.1}, alive:{}, ai:{:?}, owner:{:?}, team:{:?}, pos:{:?}, destroyed:{}, effectively_dead:{}, carbomb:{}, booby:{}}}",
                    object.health.current,
                    object.health.maximum,
                    object.is_alive(),
                    object.ai_state(),
                    object.owner_player_id,
                    object.team,
                    object.get_position(),
                    object.status.destroyed,
                    object.status.effectively_dead,
                    object.status.is_carbomb,
                    object.status.booby_trapped,
                )
            },
        )
    };
    let mut history = Vec::new();
    for frame in 0..90 {
        assert_eq!(world.game_mode(), GameMode::SinglePlayer);
        let before = format!(
            "before {frame}: {} {}",
            describe(world, actor),
            describe(world, vehicle)
        );
        world.update_with_dt(1.0 / 30.0);
        history.push(format!(
            "{before}; after {frame} (mode={:?}, pending={:?}): {} {}",
            world.game_mode(),
            world.pending_special_ability(actor),
            describe(world, actor),
            describe(world, vehicle),
        ));
        if world
            .host_object(vehicle)
            .is_some_and(|object| object.experience.level == VeterancyLevel::Veteran)
        {
            return;
        }
        if world.host_object(vehicle).is_none() {
            panic!(
                "{scenario} target {vehicle:?} removed at update {frame}: bounds={:?}, actor {actor:?}={:?}, registered_ids={:?}, history={history:#?}",
                world.world_bounds(),
                world
                    .host_object(actor)
                    .map(|object| { (object.ai_state(), object.target, object.get_position()) }),
                world
                    .host_objects()
                    .registered_keys()
                    .map(|id| id.0)
                    .collect::<Vec<_>>()
            );
        }
    }
    let source = world.host_object(actor);
    let target = world.host_object(vehicle);
    panic!(
        "{scenario} command/update path did not transfer Veteran rank: actor {actor:?}=({:?}, {:?}, {:?}, {}, {}), target {vehicle:?}=({:?}, {:?}, {:?}, {}, {})",
        source.map(|object| object.ai_state()),
        source.map(|object| object.target),
        source.map(|object| object.get_position()),
        source.is_some_and(|object| object.can_move()),
        source.map_or(0, |object| object.movement.path.len()),
        target.map(|object| object.experience.level),
        target.map(|object| object.get_position()),
        target.map(|object| (object.is_hijacked(), object.is_car_bomb())),
        target.map_or(0.0, |object| object.health.maximum),
        target.is_some(),
    );
}

#[test]
fn live_hijack_and_car_bomb_transfers_use_each_receiving_world() {
    let map_root = tempfile::tempdir().expect("map tempdir");
    let map = map_file(map_root.path());
    let mut first_hijack = world(&map, 130, 160);
    let mut peer_hijack = world(&map, 175, 220);
    let (first_actor, first_vehicle) = start_hijack(&mut first_hijack);
    assert_eq!(
        first_hijack
            .host_object(first_vehicle)
            .unwrap()
            .owner_player_id,
        Some(PLAYER_USA),
        "first world owns target before peer command"
    );
    let (peer_actor, peer_vehicle) = start_hijack(&mut peer_hijack);
    assert_eq!(first_vehicle, peer_vehicle, "world object IDs should match");
    assert_eq!(
        first_hijack
            .host_object(first_vehicle)
            .unwrap()
            .owner_player_id,
        Some(PLAYER_USA),
        "peer command cannot alter first world same-ID target owner"
    );
    assert_eq!(
        first_hijack.host_object(first_vehicle).unwrap().team,
        Team::USA,
        "peer command cannot alter first world same-ID target team"
    );
    update_until_veteran(
        "first hijack",
        &mut first_hijack,
        first_actor,
        first_vehicle,
    );
    update_until_veteran("peer hijack", &mut peer_hijack, peer_actor, peer_vehicle);
    for (owner, actor) in [(&first_hijack, first_actor), (&peer_hijack, peer_actor)] {
        let rider = owner
            .host_object(actor)
            .expect("EjectPilotDie retains hijacker");
        assert!(rider.hijacker_in_vehicle);
        assert_eq!(rider.experience.level, VeterancyLevel::Veteran);
    }
    for (owner, vehicle, expected_health, expected_marker) in [
        (&first_hijack, first_vehicle, 1300.0, "BOOBY_TRAPPED"),
        (&peer_hijack, peer_vehicle, 1750.0, "CAN_STEALTH"),
    ] {
        let object = owner.host_object(vehicle).expect("hijacked vehicle");
        assert_eq!(object.experience.level, VeterancyLevel::Veteran);
        assert!((object.health.maximum - expected_health).abs() < 0.001);
        assert!(
            generals_main::game_logic::host_status_bits_upgrade::status_bits_has(
                object.object_status_bits,
                expected_marker
            ),
            "receiving-world StatusBitsUpgrade must execute its distinct authored effect on Veteran rank"
        );
        let peer_marker = if expected_marker == "BOOBY_TRAPPED" {
            "CAN_STEALTH"
        } else {
            "BOOBY_TRAPPED"
        };
        assert!(
            !generals_main::game_logic::host_status_bits_upgrade::status_bits_has(
                object.object_status_bits,
                peer_marker
            )
        );
    }

    let mut first_car_bomb = world(&map, 130, 160);
    let mut peer_car_bomb = world(&map, 175, 220);
    let (first_actor, first_vehicle) = start_car_bomb(&mut first_car_bomb);
    let (peer_actor, peer_vehicle) = start_car_bomb(&mut peer_car_bomb);
    assert_eq!(first_vehicle, peer_vehicle, "world object IDs should match");
    update_until_veteran(
        "first car bomb",
        &mut first_car_bomb,
        first_actor,
        first_vehicle,
    );
    update_until_veteran(
        "peer car bomb",
        &mut peer_car_bomb,
        peer_actor,
        peer_vehicle,
    );
    for (owner, vehicle, expected_health, expected_marker) in [
        (&first_car_bomb, first_vehicle, 1300.0, "BOOBY_TRAPPED"),
        (&peer_car_bomb, peer_vehicle, 1750.0, "CAN_STEALTH"),
    ] {
        let object = owner.host_object(vehicle).expect("converted car bomb");
        assert_eq!(object.experience.level, VeterancyLevel::Veteran);
        assert!((object.health.maximum - expected_health).abs() < 0.001);
        assert!(
            generals_main::game_logic::host_status_bits_upgrade::status_bits_has(
                object.object_status_bits,
                expected_marker
            ),
            "receiving-world StatusBitsUpgrade must execute its distinct authored effect on Veteran rank"
        );
        let peer_marker = if expected_marker == "BOOBY_TRAPPED" {
            "CAN_STEALTH"
        } else {
            "BOOBY_TRAPPED"
        };
        assert!(
            !generals_main::game_logic::host_status_bits_upgrade::status_bits_has(
                object.object_status_bits,
                peer_marker
            )
        );
        let expected_current = expected_health - 700.0;
        assert!(
            (object.health.current - expected_current).abs() < 0.001,
            "the consumed terrorist's death weapon applies 700 damage: current={}, maximum={}",
            object.health.current,
            object.health.maximum
        );
        assert!(object.is_car_bomb());
    }

    let save_directory = tempfile::tempdir().expect("save tempdir");
    let mut save_manager = SaveFileManager::with_save_directory(save_directory.path());
    save_manager.init().expect("initialize save manager");
    save_manager
        .save_game("converted-car-bomb", &first_car_bomb, &save_info())
        .expect("encode converted vehicle world");
    let mut restored = world(&map, 130, 160);
    save_manager
        .load_game("converted-car-bomb", &mut restored)
        .expect("restore converted vehicle world");
    let before_continuation = restored
        .host_object(first_vehicle)
        .expect("restored converted vehicle");
    assert_eq!(
        before_continuation.experience.level,
        VeterancyLevel::Veteran
    );
    assert!((before_continuation.health.maximum - 1300.0).abs() < 0.001);
    assert!((before_continuation.health.current - 600.0).abs() < 0.001);
    assert!(before_continuation.is_car_bomb());

    first_car_bomb.update_with_dt(1.0 / 30.0);
    restored.update_with_dt(1.0 / 30.0);
    let source_continuation = first_car_bomb
        .host_object(first_vehicle)
        .expect("source converted vehicle continues");
    let restored_continuation = restored
        .host_object(first_vehicle)
        .expect("restored converted vehicle continues");
    assert_eq!(
        (
            source_continuation.experience.level,
            source_continuation.health.current,
            source_continuation.health.maximum,
            source_continuation.is_car_bomb(),
        ),
        (
            restored_continuation.experience.level,
            restored_continuation.health.current,
            restored_continuation.health.maximum,
            restored_continuation.is_car_bomb(),
        ),
        "encoded restore must preserve the converted vehicle's continuation"
    );
    assert_eq!(
        peer_car_bomb
            .host_object(peer_vehicle)
            .unwrap()
            .health
            .current,
        1050.0,
        "restoring first world leaves peer's same-ID converted vehicle unchanged"
    );
}
