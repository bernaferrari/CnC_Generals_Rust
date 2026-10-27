use game_engine::common::system::DataChunkOutput;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};

fn world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("Empty.ini".into(), String::new())])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    let mut player = Player::new(42, Team::USA, "Owner", true);
    player.unlocked_sciences.insert("SCIENCE_Root".into());
    player.science_purchase_points = 2;
    world.add_player(player);
    world
}

fn purchase(world: &mut GameLogic) {
    world.queue_command(GameCommand {
        command_type: CommandType::PurchaseScience {
            science_name: "SCIENCE_Shared".into(),
        },
        player_id: 42,
        command_id: 1,
        timestamp: std::time::UNIX_EPOCH,
        selected_units: Vec::new(),
        modifier_keys: ModifierKeys::default(),
    });
    world.process_commands();
}

#[test]
fn science_purchase_uses_the_receiving_match_not_foreign_catalog_or_player() {
    use game_engine::common::rts::science::{
        ScienceDefinition, get_science_store, get_science_store_mut, init_science_store,
    };
    use gamelogic::player::{PlayerList, ScienceAvailabilityType};
    use std::sync::{Arc, RwLock};

    init_science_store();
    let foreign_id = {
        let mut store = get_science_store_mut().unwrap();
        store.ingest_definition(ScienceDefinition {
            name: "SCIENCE_Shared".into(),
            cost: Some(99),
            ..Default::default()
        });
        store.get_science_from_internal_name("SCIENCE_Shared")
    };
    let foreign_player = Arc::new(RwLock::new(gamelogic::player::Player::new(42)));
    foreign_player
        .write()
        .unwrap()
        .set_science_availability(foreign_id, ScienceAvailabilityType::Hidden);
    let mut roster = PlayerList::new();
    roster.add_player(Arc::clone(&foreign_player));
    let previous = std::mem::replace(
        &mut *gamelogic::player::player_list().write().unwrap(),
        roster,
    );

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
    let make_map = |name: &str, cost: i32| {
        let dir = root.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("Owner.map");
        std::fs::write(&path, &bytes).unwrap();
        std::fs::write(dir.join("Map.ini"), format!(
            "Science SCIENCE_Root\n IsGrantable = Yes\nEnd\nScience SCIENCE_Shared\n PrerequisiteSciences = SCIENCE_Root\n SciencePurchasePointCost = {cost}\nEnd\n"
        )).unwrap();
        path
    };
    let first_map = make_map("first", 1);
    let peer_map = make_map("peer", 3);
    let mut first = world(&first_map);
    let mut peer = world(&peer_map);
    assert_eq!(
        get_science_store()
            .unwrap()
            .get_science_purchase_cost(foreign_id),
        99
    );
    assert!(first.player_can_purchase_science(42, "SCIENCE_Shared"));
    assert!(!peer.player_can_purchase_science(42, "SCIENCE_Shared"));

    purchase(&mut first);
    purchase(&mut peer);
    assert!(
        first
            .get_player(42)
            .unwrap()
            .has_unlocked_science("SCIENCE_Shared")
    );
    assert_eq!(first.get_player(42).unwrap().science_purchase_points, 1);
    assert!(
        !peer
            .get_player(42)
            .unwrap()
            .has_unlocked_science("SCIENCE_Shared")
    );
    assert_eq!(peer.get_player(42).unwrap().science_purchase_points, 2);
    assert!(foreign_player.read().unwrap().is_science_hidden(foreign_id));
    assert!(!foreign_player.read().unwrap().has_science(foreign_id));
    peer.reset();
    assert_eq!(first.get_player(42).unwrap().science_purchase_points, 1);

    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    let mut restored = world(&first_map);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(
        restored
            .get_player(42)
            .unwrap()
            .has_unlocked_science("SCIENCE_Shared")
    );
    assert_eq!(restored.get_player(42).unwrap().science_purchase_points, 1);
    *gamelogic::player::player_list().write().unwrap() = previous;
}
