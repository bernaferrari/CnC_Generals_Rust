use game_engine::common::dict::Dict;
use game_engine::common::ini::INILoadType;
use game_engine::common::rts::player_template::PlayerTemplateStore;
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
};
use generals_main::command_executor::{
    CommandExecutor, host_beacon_caption, host_local_player_can_place_beacon, host_object_is_beacon,
};
use generals_main::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, PlayerTemplateIdentity};
use std::time::SystemTime;

fn minimal_map(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    let mut output = DataChunkOutput::new();
    output.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        output.write_int(value);
    }
    for _ in 0..441 {
        output.write_byte(0);
    }
    output.close_data_chunk();
    // Map admission creates the receiving player's actual default Team.
    output.open_data_chunk("SidesList", 2);
    output.write_int(2);
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "BeaconOwner");
    let mut player = Dict::new();
    player.set_ascii_string(key_player_name(), "RustPlayer1");
    player.set_ascii_string(key_player_faction(), "BeaconOwner");
    player.set_unicode_string(key_player_display_name(), "Beacon owner");
    player.set_bool(key_player_is_human(), true);
    for row in [neutral, player] {
        output.write_dict(&row);
        output.write_int(0);
    }
    output.write_int(0);
    output.close_data_chunk();
    output.open_data_chunk("ObjectsList", 3);
    output.close_data_chunk();
    let path = root.join(name);
    std::fs::write(&path, output.into_ckmp_bytes()).unwrap();
    path
}

fn world(root: &std::path::Path, map_name: &str, beacon_name: &str, cap: i32) -> GameLogic {
    let map = minimal_map(root, map_name);
    let mut logic = GameLogic::new();
    logic.admit_max_beacons_per_player(cap);
    logic
        .admit_object_ini_sources([(
            "CustomBeacon.ini".into(),
            format!(
                "Object {beacon_name}\n KindOf = SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 1\n InitialHealth = 1\nEnd\nEnd\n"
            ),
        )])
        .unwrap();
    let mut templates = PlayerTemplateStore::new_owned();
    templates
        .apply_source(
            &format!(
                "PlayerTemplate BeaconOwner\n Side = America\n BeaconName = {beacon_name}\nEnd\n"
            ),
            INILoadType::Overwrite,
        )
        .unwrap();
    logic.admit_player_template_definitions(templates).unwrap();
    assert!(logic.start_new_game_with_player_template(
        GameMode::SinglePlayer,
        1,
        PlayerTemplateIdentity {
            template_name: "BeaconOwner".into(),
            template_index: None,
        },
    ));
    assert!(logic.load_map(map.to_str().unwrap()));
    let owner = logic.get_player(1).expect("map admits the placing player");
    assert!(owner.is_alive, "placing player must be active");
    assert!(
        logic.player_template_identity(1).is_some(),
        "authored PlayerTemplate remains bound"
    );
    assert!(
        host_local_player_can_place_beacon(&logic, 1),
        "authored cap permits the first beacon"
    );
    logic
}

fn place(logic: &mut GameLogic, player: u32, x: f32, text: &str) -> CommandResult {
    CommandExecutor::new(logic, player)
        .execute_command(GameCommand {
            command_type: CommandType::PlaceBeacon {
                location: glam::Vec3::new(x, 0.0, 12.0),
                text: text.to_owned(),
            },
            player_id: player,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: vec![],
            modifier_keys: ModifierKeys::default(),
        })
        .unwrap()
}

fn remove(logic: &mut GameLogic, player: u32, id: ObjectId) -> CommandResult {
    CommandExecutor::new(logic, player)
        .execute_command(GameCommand {
            command_type: CommandType::RemoveBeacon,
            player_id: player,
            command_id: 2,
            timestamp: SystemTime::now(),
            selected_units: vec![id],
            modifier_keys: ModifierKeys::default(),
        })
        .unwrap()
}

#[test]
fn authored_beacon_cap_and_template_are_session_owned_through_admission_and_reset() {
    let one_root = tempfile::tempdir().unwrap();
    let two_root = tempfile::tempdir().unwrap();
    let mut one = world(one_root.path(), "one.map", "SignalGlyph", 1);
    let mut two = world(two_root.path(), "two.map", "RallyMarker", 2);

    assert_eq!(place(&mut one, 1, 11.0, "one"), CommandResult::Success);
    assert_eq!(place(&mut two, 1, 11.0, "two"), CommandResult::Success);
    let id = *one.host_objects().registered_keys().next().unwrap();
    assert_eq!(*two.host_objects().registered_keys().next().unwrap(), id);
    assert_eq!(one.host_object(id).unwrap().template_name, "SignalGlyph");
    assert_eq!(two.host_object(id).unwrap().template_name, "RallyMarker");
    assert!(host_object_is_beacon(&one, id));
    assert!(host_object_is_beacon(&two, id));
    assert_eq!(host_beacon_caption(&one, id).as_deref(), Some("one"));
    assert_eq!(host_beacon_caption(&two, id).as_deref(), Some("two"));
    assert!(!host_local_player_can_place_beacon(&one, 1));
    assert!(host_local_player_can_place_beacon(&two, 1));

    assert_eq!(
        place(&mut one, 1, 21.0, "overflow"),
        CommandResult::InvalidCommand
    );
    // C++ countObjectsByThingTemplate is called with ignoreDead=false. A
    // beacon retained as effectively dead and awaiting destruction still
    // consumes its authored slot until the owning store retires it.
    {
        let beacon = two.host_object_mut(id).unwrap();
        beacon.status.effectively_dead = true;
        beacon.status.on_die_started = true;
    }
    two.destroy_object(id).unwrap();
    assert!(two.host_object(id).is_some(), "destruction remains pending");
    assert!(host_local_player_can_place_beacon(&two, 1));
    assert_eq!(place(&mut two, 1, 21.0, "second"), CommandResult::Success);
    assert!(!host_local_player_can_place_beacon(&two, 1));
    assert_eq!(
        place(&mut two, 1, 26.0, "third"),
        CommandResult::InvalidCommand
    );
    assert_eq!(remove(&mut one, 1, id), CommandResult::Success);
    assert!(
        one.host_object(id).unwrap().status.destroyed,
        "C++ destroyObject marks the beacon before deferred physical removal"
    );
    assert!(two.host_object(id).is_some());
    assert_eq!(host_beacon_caption(&two, id).as_deref(), Some("two"));

    // Restart preserves this receiving session's admitted application rule.
    assert!(one.start_new_game_with_player_template(
        GameMode::SinglePlayer,
        1,
        PlayerTemplateIdentity {
            template_name: "BeaconOwner".into(),
            template_index: None,
        },
    ));
    assert!(one.load_map(one_root.path().join("one.map").to_str().unwrap()));
    assert_eq!(
        place(&mut one, 1, 31.0, "after reset"),
        CommandResult::Success
    );
    assert_eq!(
        place(&mut one, 1, 41.0, "still capped"),
        CommandResult::InvalidCommand
    );
}
