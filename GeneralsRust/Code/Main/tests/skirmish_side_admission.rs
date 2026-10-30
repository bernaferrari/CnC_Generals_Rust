use game_engine::common::dict::Dict;
use game_engine::common::ini::INI;
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{key_player_faction, key_player_name};
use gamelogic::common::Relationship;
use generals_main::game_logic::GameLogic;
use generals_main::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

#[test]
fn skirmish_map_admits_generated_slot_names_before_relationships() {
    let mut ini = INI::new();
    ini.with_inline_source(
        "PlayerTemplate FactionAmerica\n Side = America\n BaseSide = America\n StartingBuilding = AmericaCommandCenter\nEnd\n\
         PlayerTemplate FactionGLA\n Side = GLA\n BaseSide = GLA\n StartingBuilding = GLACommandCenter\nEnd\n\
         PlayerTemplate FactionCivilian\n Side = Civilian\n BaseSide = Civilian\nEnd\n",
        |ini| ini.parse_current_file(),
    ).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("SlotRelationships.map");
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        out.write_int(value);
    }
    for _ in 0..441 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.close_data_chunk();
    out.open_data_chunk("SidesList", 2);
    out.write_int(2);
    for (name, faction) in [
        ("PlyrCivilian", "FactionCivilian"),
        ("PlyrGLA", "FactionGLA"),
    ] {
        let mut dict = Dict::new();
        dict.set_ascii_string(key_player_name(), name);
        dict.set_ascii_string(key_player_faction(), faction);
        out.write_dict(&dict);
        out.write_int(0);
    }
    out.write_int(0);
    out.close_data_chunk();
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    let map = path.to_str().unwrap();
    let mut config = golden_skirmish_config(map);
    for slot in &mut config.slots {
        slot.team = -1;
    }
    for (index, color) in [(2, (0, 200, 0)), (3, (200, 200, 0))] {
        let mut slot = config.slots[1].clone();
        slot.slot_index = index;
        slot.start_position = index as i32;
        slot.color_rgb = color;
        slot.team = 3;
        slot.player_name = format!("AI {index}");
        config.slots.push(slot);
    }
    let mut world = GameLogic::new();
    apply_skirmish_config(&mut world, &config).unwrap();
    let slot_state = |world: &GameLogic| {
        [0, 1, 2, 3].map(|id| {
            let player = world.get_player(id).unwrap();
            (
                player.name.clone(),
                player.is_human,
                player.team,
                player.color_rgb,
                player.resources.supplies,
            )
        })
    };
    let before = slot_state(&world);
    assert!(world.load_map(map));
    assert_eq!(
        slot_state(&world),
        before,
        "map admission must retain lobby slot settings"
    );
    assert_eq!(world.player_relationship(0, 1), Relationship::Enemies);
    assert_eq!(world.player_relationship(1, 0), Relationship::Enemies);
    assert_eq!(world.player_relationship(0, 2), Relationship::Enemies);
    assert_eq!(world.player_relationship(2, 0), Relationship::Enemies);
    assert_eq!(world.player_relationship(2, 3), Relationship::Allies);
    assert_eq!(world.player_relationship(3, 2), Relationship::Allies);
}
