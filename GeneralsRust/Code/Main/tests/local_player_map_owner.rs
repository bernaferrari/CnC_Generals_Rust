//! Public map admission must retain the receiving PlayerList's local identity.
use game_engine::common::dict::Dict;
use game_engine::common::ini::INILoadType;
use game_engine::common::rts::player_template::PlayerTemplateStore;
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
};
use generals_main::game_logic::{GameLogic, GameMode};

fn map_bytes(sides: &[Dict]) -> Vec<u8> {
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
    map.write_int(sides.len() as i32);
    for side in sides {
        map.write_dict(side);
        map.write_int(0); // no authored build-list entries
    }
    map.write_int(0); // no authored team prototypes
    map.close_data_chunk();

    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    map.into_ckmp_bytes()
}

fn side(name: &str, faction: &str, human: bool) -> Dict {
    let mut side = Dict::new();
    side.set_ascii_string(key_player_name(), name);
    side.set_ascii_string(key_player_faction(), faction);
    side.set_unicode_string(key_player_display_name(), name);
    side.set_bool(key_player_is_human(), human);
    side
}

fn map_in(root: &std::path::Path, name: &str, sides: &[Dict]) -> std::path::PathBuf {
    let directory = root.join(name);
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("LocalIdentity.map");
    std::fs::write(&path, map_bytes(sides)).unwrap();
    path
}

fn world(map: &std::path::Path) -> GameLogic {
    let mut templates = PlayerTemplateStore::new_owned();
    templates
        .apply_source(
            "PlayerTemplate FactionCivilian\n Side = Civilian\nEnd\n\
             PlayerTemplate FactionAmerica\n Side = America\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    let mut world = GameLogic::new();
    world.admit_player_template_definitions(templates).unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(
        world.load_map(map.to_str().unwrap()),
        "loading {}",
        map.display()
    );
    world
}

#[test]
fn map_admission_preserves_local_identity_for_neutral_and_same_id_civilians() {
    let _ = env_logger::builder().is_test(true).try_init();
    let root = tempfile::tempdir().unwrap();
    let neutral_map = map_in(
        &root.path(),
        "neutral",
        &[side("", "FactionCivilian", false)],
    );
    let civilian_map = map_in(
        &root.path(),
        "civilian",
        &[
            side("", "FactionCivilian", false),
            side("PlyrCivilian", "FactionCivilian", false),
        ],
    );
    let peer_map = map_in(
        &root.path(),
        "peer",
        &[
            side("", "FactionCivilian", false),
            side("PlyrCivilian", "FactionCivilian", false),
            side("HumanOwner", "FactionAmerica", true),
        ],
    );

    let neutral = world(&neutral_map);
    let mut civilian = world(&civilian_map);
    let peer = world(&peer_map);
    assert!(neutral.get_player(0).unwrap().is_local);
    assert!(civilian.get_player(1).unwrap().is_local);
    assert_eq!(
        civilian.get_player(1).unwrap().map_side.map_player_name,
        "PlyrCivilian"
    );
    assert_eq!(
        peer.get_player(1).unwrap().map_side.map_player_name,
        "PlyrCivilian"
    );
    assert!(!peer.get_player(1).unwrap().is_local);
    assert!(peer.get_player(2).unwrap().is_local);

    // Re-admitting the same map must reinitialize the existing local slot in
    // place, as C++ PlayerList::newGame does, while preserving the peer.
    assert!(civilian.load_map(civilian_map.to_str().unwrap()));
    assert!(civilian.get_player(1).unwrap().is_local);
    assert!(!peer.get_player(1).unwrap().is_local);
    assert!(neutral.get_player(0).unwrap().is_local);

    // Save and decode through the real archive before restoring into an
    // independently admitted world with the same civilian side.
    let save_root = tempfile::tempdir().unwrap();
    let mut saves =
        generals_main::save_load::SaveFileManager::with_save_directory(save_root.path());
    saves.init().unwrap();
    saves.quick_save(&civilian).unwrap();
    let decoded = saves.load_game_snapshot("quicksave").unwrap();
    let mut restored = world(&civilian_map);
    saves
        .restore_game_snapshot(&decoded.snapshot, &mut restored)
        .unwrap();
    assert_eq!(restored.local_player_id(), Some(1));
    assert_eq!(
        restored.get_player(1).unwrap().map_side.map_player_name,
        "PlyrCivilian"
    );
    assert!(peer.get_player(2).unwrap().is_local);
}
