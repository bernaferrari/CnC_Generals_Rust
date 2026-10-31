//! AISkirmishPlayer::acquireEnemy counts only skirmish AI peer targets.
use game_engine::common::{
    dict::Dict,
    ini::INILoadType,
    rts::{ProductionPrerequisite, player_template::PlayerTemplateStore},
    system::DataChunkOutput,
    well_known_keys::{
        key_player_faction, key_player_is_human, key_player_is_skirmish, key_player_name,
    },
};
use generals_main::ai::{AIDifficulty, AIManager};
use generals_main::game_logic::{GameLogic, GameMode, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

fn selected_enemy(peer_is_skirmish: bool) -> Option<u32> {
    let mut world = GameLogic::new();
    let mut templates = PlayerTemplateStore::new_owned();
    templates.apply_source("PlayerTemplate FactionCivilian\n Side = Civilian\nEnd\nPlayerTemplate FactionAmerica\n Side = America\nEnd\n", INILoadType::Overwrite).unwrap();
    world.admit_player_template_definitions(templates).unwrap();
    world.start_new_game(GameMode::Skirmish);
    world.clear_all_players();
    let root = tempfile::tempdir().unwrap();
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
    let mut peer = Dict::new();
    peer.set_ascii_string(key_player_name(), "player2");
    peer.set_ascii_string(key_player_faction(), "FactionAmerica");
    peer.set_bool(key_player_is_human(), !peer_is_skirmish);
    peer.set_bool(key_player_is_skirmish(), peer_is_skirmish);
    for side in [neutral, peer] {
        map.write_dict(&side);
        map.write_int(0);
    }
    map.write_int(0);
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    let map_path = root.path().join("peer.map");
    std::fs::write(&map_path, map.into_ckmp_bytes()).unwrap();
    let mut barracks = ThingTemplate::new("PeerBarracks");
    barracks.add_kind_of(KindOf::Structure);
    barracks.add_kind_of(KindOf::FSBarracks);
    let mut infantry = ThingTemplate::new("PeerInfantry");
    infantry.add_kind_of(KindOf::Infantry);
    let mut prerequisite = ProductionPrerequisite::new();
    prerequisite.add_unit_prereq("PeerBarracks".into(), false);
    infantry.production_prerequisites.push(prerequisite);
    world.templates.insert("PeerBarracks".into(), barracks);
    world.templates.insert("PeerInfantry".into(), infantry);

    for (id, team, _position) in [
        (1, Team::USA, Vec3::ZERO),
        (2, Team::USA, Vec3::ZERO),
        (3, Team::China, Vec3::new(100.0, 0.0, 100.0)),
        (4, Team::GLA, Vec3::new(200.0, 0.0, 100.0)),
    ] {
        let mut player = Player::new(id, team, "Peer", id == 2 && !peer_is_skirmish);
        player.alliance_team = if id <= 2 { 1 } else { id as i32 };
        player = player.with_script_identity(&format!("player{id}"));
        world.add_player(player);
    }
    assert!(world.load_map(map_path.to_str().unwrap()));
    for (id, position) in [
        (1, Vec3::ZERO),
        (2, Vec3::ZERO),
        (3, Vec3::new(100.0, 0.0, 100.0)),
        (4, Vec3::new(200.0, 0.0, 100.0)),
    ] {
        world
            .create_object_for_player("PeerBarracks", id, position)
            .unwrap();
        world
            .create_object_for_player("PeerInfantry", id, position)
            .unwrap();
    }

    let mut manager = AIManager::new();
    manager.add_ai_player(1, Team::USA, AIDifficulty::Medium);
    manager.add_ai_player(2, Team::USA, AIDifficulty::Medium);
    let owner = manager.ai_players.get_mut(&1).unwrap();
    owner.base_center = Vec3::ZERO;
    owner.enemy_check_time = -5.0;
    owner.next_building_time = f32::MAX;
    owner.next_team_time = f32::MAX;
    owner.next_team_queue_time = f32::MAX;
    let peer = manager.ai_players.get_mut(&2).unwrap();
    peer.is_active = false;
    // Retain a target even for the ordinary-controller case to exercise the
    // class guard independently of how a real controller acquires that value.
    peer.enemy_player_id = Some(3);
    manager.update(&mut world, 0.0);
    manager.ai_players[&1].enemy_player_id
}

#[test]
fn only_skirmish_peer_targets_affect_enemy_scoring() {
    assert_eq!(selected_enemy(false), Some(3));
    assert_eq!(selected_enemy(true), Some(4));
}
