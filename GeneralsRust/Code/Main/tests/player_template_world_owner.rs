use game_engine::common::dict::Dict;
use game_engine::common::ini::INILoadType;
use game_engine::common::rts::player_template::{
    PlayerTemplateStore, get_player_template_store_mut,
};
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
};
use generals_main::game_logic::{GameLogic, GameMode, PlayerTemplateIdentity};

fn definitions(money: u32) -> PlayerTemplateStore {
    let mut store = PlayerTemplateStore::new_owned();
    store
        .apply_source(
            &format!(
                "PlayerTemplate FactionAmerica\n Side = America\n StartMoney = {money}\n StartingBuilding = AmericaCommandCenter\nEnd\n"
            ),
            INILoadType::Overwrite,
        )
        .unwrap();
    store
}

fn world(start_money: u32, map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("Empty.ini".into(), String::new())])
        .unwrap();
    world
        .admit_player_template_definitions(definitions(start_money))
        .unwrap();
    let identity = PlayerTemplateIdentity {
        template_name: "FactionAmerica".into(),
        template_index: Some(0),
    };
    assert!(world.start_new_game_with_player_template(GameMode::SinglePlayer, 0, identity));
    assert_eq!(world.get_player(0).unwrap().resources.supplies, start_money);
    assert!(world.load_map(map.to_str().unwrap()));
    world
}

#[test]
fn player_templates_follow_the_receiving_world_through_map_reset_and_save() {
    let mut foreign = PlayerTemplateStore::new_owned();
    foreign.apply_source(
        "PlayerTemplate Other\n Side = China\nEnd\nPlayerTemplate FactionAmerica\n Side = America\n StartMoney = 9999\n StartingBuilding = AmericaCommandCenter\nEnd\n",
        INILoadType::Overwrite,
    ).unwrap();
    let previous = std::mem::replace(&mut *get_player_template_store_mut(), foreign);
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        map.write_int(value);
    }
    for _ in 0..441 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("SidesList", 2);
    map.write_int(2);
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "FactionCivilian");
    let mut human = Dict::new();
    human.set_ascii_string(key_player_name(), "Owner");
    human.set_ascii_string(key_player_faction(), "FactionAmerica");
    human.set_unicode_string(key_player_display_name(), "Owner");
    human.set_bool(key_player_is_human(), true);
    for side in [neutral, human] {
        map.write_dict(&side);
        map.write_int(0);
    }
    map.write_int(0);
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    let bytes = map.into_ckmp_bytes();
    let root = tempfile::tempdir().unwrap();
    let create_map = |name: &str, money: u32| {
        let dir = root.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("Owner.map");
        std::fs::write(&path, &bytes).unwrap();
        std::fs::write(
            dir.join("Map.ini"),
            format!("PlayerTemplate FactionAmerica\n StartMoney = {money}\nEnd\n"),
        )
        .unwrap();
        path
    };
    let first_map = create_map("first", 1300);
    let peer_map = create_map("peer", 2400);
    let first = world(1200, &first_map);
    let mut peer = world(2200, &peer_map);
    assert_eq!(first.get_player(1).unwrap().resources.supplies, 1300);
    assert_eq!(peer.get_player(1).unwrap().resources.supplies, 2400);
    assert_eq!(
        get_player_template_store_mut()
            .find_template("FactionAmerica")
            .unwrap()
            .get_money()
            .count_money(),
        9999
    );
    // A lobby carries only indices. Pair each index with the name from its
    // receiving application, even while another catalog is installed globally.
    let mut info = game_engine::common::system::skirmish_game_info::SkirmishGameInfo::default();
    info.slots[0].state = game_client::SlotState::Player as i32;
    info.slots[0].player_template = 0;
    info.slots[0].name = "Owner".into();
    let launch = game_engine::common::message_stream::ClientSkirmishLaunch {
        map: first_map.to_string_lossy().into_owned(),
        game_info: info,
        local_slot: 0,
        old_factions_only: false,
        fog_of_war: true,
        game_mode_code: 2,
        max_fps: 30,
    };
    let chosen =
        generals_main::skirmish_config::config_from_skirmish_launch_in(&launch, &definitions(3700))
            .unwrap();
    assert!(matches!(&chosen.slots[0].player_template,
        generals_main::skirmish_config::SkirmishPlayerTemplateSelection::Exact { template_name, template_index: 0 } if template_name == "FactionAmerica"));
    let foreign = get_player_template_store_mut();
    let other =
        generals_main::skirmish_config::config_from_skirmish_launch_in(&launch, &foreign).unwrap();
    assert!(matches!(&other.slots[0].player_template,
        generals_main::skirmish_config::SkirmishPlayerTemplateSelection::Exact { template_name, template_index: 0 } if template_name == "Other"));
    drop(foreign);
    peer.reset();
    assert_eq!(first.get_player(1).unwrap().resources.supplies, 1300);
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    let mut restored = world(1200, &first_map);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(restored.get_player(1).unwrap().resources.supplies, 1300);
    assert_eq!(
        restored.player_template_identity(1).unwrap().template_index,
        Some(0)
    );
    assert_eq!(first.get_player(1).unwrap().resources.supplies, 1300);
    // The legacy catalog has this same faction at index 1. Real Skirmish
    // admission must validate index 0 against the receiving world instead.
    use generals_main::skirmish_config::{
        GameRulesSnapshot, SkirmishMatchConfig, SkirmishPlayerTemplateSelection,
        SkirmishSlotConfig, apply_skirmish_config,
    };
    let config = SkirmishMatchConfig {
        source_game_info: None,
        map: first_map.to_string_lossy().into_owned(),
        random_seed: 31,
        old_factions_only: false,
        rules: GameRulesSnapshot::default_rules(),
        slots: vec![SkirmishSlotConfig {
            slot_index: 0,
            is_human: true,
            is_active: true,
            player_template: SkirmishPlayerTemplateSelection::Exact {
                template_name: "FactionAmerica".into(),
                template_index: 0,
            },
            faction: "USA".into(),
            color_rgb: (0, 0, 200),
            team: 0,
            start_position: 0,
            player_name: "Owner".into(),
            ai_difficulty: None,
        }],
    };
    let mut skirmish = GameLogic::new();
    skirmish
        .admit_object_ini_sources([("Empty.ini".into(), String::new())])
        .unwrap();
    skirmish
        .admit_player_template_definitions(definitions(3700))
        .unwrap();
    apply_skirmish_config(&mut skirmish, &config).unwrap();
    assert_eq!(skirmish.get_player(0).unwrap().resources.supplies, 3700);
    assert_eq!(
        skirmish.player_template_identity(0).unwrap().template_index,
        Some(0)
    );
    assert!(skirmish.load_map(first_map.to_str().unwrap()));
    let skirmish_cash = skirmish.get_player(0).unwrap().resources.supplies;
    let skirmish_snapshot = builder.create_world_snapshot(&skirmish).unwrap();
    let mut resumed_skirmish = GameLogic::new();
    resumed_skirmish
        .admit_object_ini_sources([("Empty.ini".into(), String::new())])
        .unwrap();
    resumed_skirmish
        .admit_player_template_definitions(definitions(3700))
        .unwrap();
    apply_skirmish_config(&mut resumed_skirmish, &config).unwrap();
    assert!(resumed_skirmish.load_map(first_map.to_str().unwrap()));
    builder
        .restore_from_snapshot(&skirmish_snapshot, &mut resumed_skirmish)
        .unwrap();
    assert_eq!(
        resumed_skirmish.get_player(0).unwrap().resources.supplies,
        skirmish_cash
    );
    assert_eq!(
        resumed_skirmish
            .player_template_identity(0)
            .unwrap()
            .template_index,
        Some(0)
    );
    *get_player_template_store_mut() = previous;
}
