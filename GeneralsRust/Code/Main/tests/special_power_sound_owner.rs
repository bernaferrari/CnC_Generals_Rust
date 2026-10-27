use game_engine::common::system::DataChunkOutput;
use generals_main::command_system::{
    CommandType, GameCommand, ModifierKeys, PowerTarget, SpecialPowerType,
};
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};

fn world(map: &std::path::Path) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("Empty.ini".into(), String::new())])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(42, Team::USA, "Owner", true));
    let id = world
        .create_object_for_player("SharedCaster", 42, glam::Vec3::new(50., 0., 50.))
        .unwrap();
    world.queued_audio_events.drain_requests();
    (world, id)
}

fn activate(world: &mut GameLogic, id: ObjectId) -> Vec<String> {
    world.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpySatellite,
            target: PowerTarget::Location(glam::Vec3::new(60., 0., 60.)),
        },
        player_id: 42,
        command_id: 1,
        timestamp: std::time::UNIX_EPOCH,
        selected_units: vec![id],
        modifier_keys: ModifierKeys::default(),
    });
    world.process_commands();
    world
        .queued_audio_events
        .drain_requests()
        .into_iter()
        .map(|event| event.event_type)
        .collect()
}

#[test]
fn selected_object_module_owns_special_power_initiate_sound() {
    let _ = env_logger::builder().is_test(true).try_init();
    let foreign_fields = std::collections::HashMap::from([(
        "InitiateSound".to_string(),
        "ForeignGlobalSound".to_string(),
    )]);
    game_engine::common::rts::special_power::get_special_power_store_mut()
        .parse_special_power_definition("SpecialPowerSpySatellite", &foreign_fields)
        .unwrap();
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
    let bytes = map.into_ckmp_bytes();
    let root = tempfile::tempdir().unwrap();
    let make_map = |name: &str, module_sound: Option<&str>| {
        let dir = root.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("Owner.map");
        std::fs::write(&path, &bytes).unwrap();
        let module_sound = module_sound
            .map(|sound| format!("InitiateSound = {sound}\n"))
            .unwrap_or_default();
        let definitions = format!(
            "SpecialPower SpecialPowerSpySatellite\n Enum = SPECIAL_SPY_SATELLITE\n ReloadTime = 0\n InitiateSound = WrongTemplateSound\nEnd\n\
             Object SharedCaster\n KindOf = STRUCTURE SELECTABLE\n\
             AddModule\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n End\n\
             AddModule\n Behavior = OCLSpecialPower PowerTag\n SpecialPowerTemplate = SpecialPowerSpySatellite\n {module_sound}End\n End\nEnd\n"
        );
        std::fs::write(dir.join("Map.ini"), definitions).unwrap();
        path
    };
    let first_map = make_map("first", Some("FirstModuleSound"));
    let peer_map = make_map("peer", Some("PeerModuleSound"));
    let silent_map = make_map("silent", None);
    let (mut first, first_id) = world(&first_map);
    let (mut peer, peer_id) = world(&peer_map);
    let (mut silent, silent_id) = world(&silent_map);
    assert_eq!((first_id, first_id), (peer_id, silent_id));
    assert_eq!(
        game_engine::common::rts::special_power::get_special_power_store()
            .find_template("SpecialPowerSpySatellite")
            .unwrap()
            .initiate_sound,
        "ForeignGlobalSound",
        "map action admission cannot mutate a foreign catalog"
    );
    let first_sounds = activate(&mut first, first_id);
    let peer_sounds = activate(&mut peer, peer_id);
    let silent_sounds = activate(&mut silent, silent_id);
    assert!(first.honesty_spy_satellite_activate_ok());
    assert!(peer.honesty_spy_satellite_activate_ok());
    assert!(silent.honesty_spy_satellite_activate_ok());
    assert!(
        first_sounds.iter().any(|sound| sound == "FirstModuleSound"),
        "{first_sounds:?}"
    );
    assert!(
        peer_sounds.iter().any(|sound| sound == "PeerModuleSound"),
        "{peer_sounds:?}"
    );
    assert!(
        !silent_sounds
            .iter()
            .any(|sound| sound == "WrongTemplateSound"),
        "{silent_sounds:?}"
    );
}
