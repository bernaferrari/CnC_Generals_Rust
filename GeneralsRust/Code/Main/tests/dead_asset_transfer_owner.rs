use game_engine::common::dict::Dict;
use game_engine::common::ini::INILoadType;
use game_engine::common::rts::player_template::PlayerTemplateStore;
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_allies, key_player_display_name, key_player_faction, key_player_is_human,
    key_player_name,
};
use generals_main::command_executor::CommandExecutor;
use generals_main::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{GameLogic, GameMode};
use std::time::SystemTime;

fn map_with_mutual_allies(root: &std::path::Path) -> std::path::PathBuf {
    let mut output = DataChunkOutput::new();
    output.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        output.write_int(value);
    }
    for _ in 0..441 {
        output.write_byte(0);
    }
    output.close_data_chunk();
    output.open_data_chunk("SidesList", 2);
    output.write_int(3);
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "SourceFaction");
    let mut source = Dict::new();
    source.set_ascii_string(key_player_name(), "Source");
    source.set_ascii_string(key_player_faction(), "SourceFaction");
    source.set_unicode_string(key_player_display_name(), "Source");
    source.set_bool(key_player_is_human(), true);
    source.set_ascii_string(key_player_allies(), "Ally");
    let mut ally = Dict::new();
    ally.set_ascii_string(key_player_name(), "Ally");
    ally.set_ascii_string(key_player_faction(), "AllyFaction");
    ally.set_unicode_string(key_player_display_name(), "Ally");
    ally.set_ascii_string(key_player_allies(), "Source");
    for row in [neutral, source, ally] {
        output.write_dict(&row);
        output.write_int(0);
    }
    output.write_int(0);
    output.close_data_chunk();
    output.open_data_chunk("ObjectsList", 3);
    output.close_data_chunk();
    let path = root.join("TransferAssets.map");
    std::fs::write(&path, output.into_ckmp_bytes()).unwrap();
    path
}

#[test]
fn self_destruct_transfers_dead_and_live_assets_before_source_death() {
    let map_root = tempfile::tempdir().unwrap();
    let map = map_with_mutual_allies(map_root.path());
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "TransferAssets.ini".into(),
            "Object TransferWitness\n KindOf = SELECTABLE VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\nEnd\nEnd\n"
                .into(),
        )])
        .unwrap();
    let mut player_templates = PlayerTemplateStore::new_owned();
    player_templates
        .apply_source(
            "PlayerTemplate SourceFaction\n Side = America\n BeaconName = TransferBeacon\nEnd\n\
             PlayerTemplate AllyFaction\n Side = GLA\n BeaconName = TransferBeacon\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    world
        .admit_player_template_definitions(player_templates)
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    assert_eq!(
        world.player_relationship(1, 2),
        gamelogic::common::Relationship::Allies
    );
    assert_eq!(
        world.player_relationship(2, 1),
        gamelogic::common::Relationship::Allies
    );

    let live = world
        .create_object_for_player("TransferWitness", 1, glam::Vec3::ZERO)
        .expect("live source asset");
    let dead = world
        .create_object_for_player("TransferWitness", 1, glam::Vec3::X)
        .expect("dead source asset");
    assert!(
        !world
            .host_object(live)
            .unwrap()
            .team_instance_name
            .is_empty()
    );
    assert!(
        !world
            .host_object(dead)
            .unwrap()
            .team_instance_name
            .is_empty()
    );
    world.host_object_mut(dead).unwrap().status.effectively_dead = true;
    assert!(!world.host_object(dead).unwrap().is_alive());

    let result = CommandExecutor::new(&mut world, 1)
        .execute_command(GameCommand {
            command_type: CommandType::SelfDestruct {
                transfer_to_ally: true,
            },
            player_id: 1,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: vec![],
            modifier_keys: ModifierKeys::default(),
        })
        .unwrap();

    assert_eq!(result, CommandResult::Success);
    assert!(!world.get_player(1).unwrap().is_alive);
    assert_eq!(world.host_object(live).unwrap().owner_player_id, Some(2));
    assert_eq!(world.host_object(dead).unwrap().owner_player_id, Some(2));
}
