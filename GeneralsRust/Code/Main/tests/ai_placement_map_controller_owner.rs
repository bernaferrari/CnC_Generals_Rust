//! Placement spacing reads the controller admitted from the receiving map.
use game_engine::common::{
    dict::Dict,
    ini::INILoadType,
    rts::player_template::PlayerTemplateStore,
    system::DataChunkOutput,
    well_known_keys::{
        key_player_display_name, key_player_faction, key_player_is_human, key_player_is_skirmish,
        key_player_name,
    },
};
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};
use glam::Vec3;

fn placement_map(root: &std::path::Path, skirmish_ai: bool) -> std::path::PathBuf {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [201, 201, 1, 40_401] {
        map.write_int(value);
    }
    for _ in 0..40_401 {
        map.write_byte(0);
    }
    map.close_data_chunk();

    map.open_data_chunk("SidesList", 2);
    map.write_int(2);
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "FactionCivilian");
    let mut player = Dict::new();
    player.set_ascii_string(key_player_name(), "player1");
    player.set_ascii_string(key_player_faction(), "FactionAmerica");
    player.set_unicode_string(key_player_display_name(), "Placement owner");
    player.set_bool(key_player_is_human(), !skirmish_ai);
    player.set_bool(key_player_is_skirmish(), skirmish_ai);
    for side in [neutral, player] {
        map.write_dict(&side);
        map.write_int(0);
    }
    map.write_int(0);
    map.close_data_chunk();

    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();

    let path = root.join(if skirmish_ai {
        "SkirmishController.map"
    } else {
        "OrdinaryController.map"
    });
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    path
}

fn world(root: &std::path::Path, skirmish_ai: bool) -> GameLogic {
    let mut templates = PlayerTemplateStore::new_owned();
    templates
        .apply_source(
            "PlayerTemplate FactionCivilian\n Side = Civilian\nEnd\n\
             PlayerTemplate FactionAmerica\n Side = America\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();

    let mut game = GameLogic::new();
    game.admit_player_template_definitions(templates).unwrap();
    game.admit_object_ini_sources([(
        "ai-placement.ini".into(),
        "Object PlacementCandidate\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 10\n GeometryMinorRadius = 10\n GeometryHeight = 10\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n\
         Object PlacementBlocker\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 5\n GeometryMinorRadius = 5\n GeometryHeight = 10\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
            .into(),
    )])
    .unwrap();
    game.start_new_game(GameMode::Skirmish);
    game.clear_all_players();
    game.add_player(
        Player::new(1, Team::USA, "Placement owner", !skirmish_ai).with_script_identity("player1"),
    );
    let map = placement_map(root, skirmish_ai);
    assert!(game.load_map(map.to_str().unwrap()));
    game.set_skirmish_rules(false, true, false, true, 1.0);
    game
}

#[test]
fn ai_spacing_uses_receiving_map_controller_and_ordinary_query_stays_unexpanded() {
    let root = tempfile::tempdir().unwrap();
    let mut ai_world = world(root.path(), true);
    let mut ordinary_world = world(root.path(), false);

    let blocker_pos = Vec3::new(525.0, 0.0, 500.0);
    let ai_blocker = ai_world
        .create_object_for_player("PlacementBlocker", 1, blocker_pos)
        .unwrap();
    let ordinary_blocker = ordinary_world
        .create_object_for_player("PlacementBlocker", 1, blocker_pos)
        .unwrap();
    assert_eq!(
        ai_blocker, ordinary_blocker,
        "worlds should reuse ObjectIds"
    );

    let candidate = Vec3::new(500.0, 0.0, 500.0);
    assert!(
        ai_world.is_location_legal_to_build(Team::USA, candidate, "PlacementCandidate"),
        "the same AI world is clear when no player is supplied"
    );
    assert!(
        !ai_world.is_location_legal_to_build_for_ai(1, candidate, "PlacementCandidate", None,),
        "C++ skirmish AI candidate bounds include the 30-unit spacing allowance"
    );
    assert!(ordinary_world.is_location_legal_to_build_for_ai(
        1,
        candidate,
        "PlacementCandidate",
        None,
    ));
    assert!(ordinary_world.is_location_legal_to_build(Team::USA, candidate, "PlacementCandidate",));
}
