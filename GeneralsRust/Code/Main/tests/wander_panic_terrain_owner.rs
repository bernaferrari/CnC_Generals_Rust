use game_engine::common::{
    dict::Dict, ini::ini_locomotor::LocomotorStore, name_key_generator::NameKeyGenerator,
    system::DataChunkOutput,
};
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};

fn waypoint_map(
    root: &std::path::Path,
    name: &str,
    points: [(i32, i32); 2],
    ids: [i32; 2],
    terminal_label: &str,
) -> std::path::PathBuf {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [101, 101, 1, 10201] {
        map.write_int(value);
    }
    for _ in 0..10201 {
        map.write_byte(0);
    }
    map.close_data_chunk();

    map.open_data_chunk("ObjectsList", 3);
    for (index, ((x, y), id)) in points.into_iter().zip(ids).enumerate() {
        map.open_data_chunk("Object", 3);
        for value in [x as f32, y as f32, 0.0, 0.0] {
            map.write_real(value);
        }
        map.write_int(0);
        map.write_ascii_string("Waypoint");
        let mut dict = Dict::new();
        dict.set_int(NameKeyGenerator::name_to_key("waypointID"), id);
        dict.set_ascii_string(
            NameKeyGenerator::name_to_key("waypointName"),
            if index == 0 {
                "SharedStart"
            } else {
                "SharedEnd"
            },
        );
        dict.set_ascii_string(
            NameKeyGenerator::name_to_key("waypointPathLabel1"),
            if index == 0 {
                "SharedPath"
            } else {
                terminal_label
            },
        );
        map.write_dict(&dict);
        map.close_data_chunk();
    }
    map.close_data_chunk();

    map.open_data_chunk("WaypointsList", 1);
    map.write_int(1);
    map.write_int(ids[0]);
    map.write_int(ids[1]);
    map.close_data_chunk();

    let directory = root.join(name);
    std::fs::create_dir(&directory).expect("create map fixture directory");
    let path = directory.join("WaypointOwner.map");
    std::fs::write(&path, map.into_ckmp_bytes()).expect("write map fixture");
    path
}

fn world(path: &std::path::Path) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut locomotors = LocomotorStore::new_owned();
    locomotors
        .apply_source(
            "Locomotor OwnedNormal\nSurfaces = GROUND\nSpeed = 60\nAcceleration = 100\nTurnRate = 360\nAppearance = TWO_LEGS\nWanderWidthFactor = 0\nEnd\n\
             Locomotor OwnedWander\nSurfaces = GROUND\nSpeed = 60\nAcceleration = 100\nTurnRate = 360\nAppearance = TWO_LEGS\nWanderWidthFactor = 0\nEnd\n\
             Locomotor OwnedPanic\nSurfaces = GROUND\nSpeed = 60\nAcceleration = 100\nTurnRate = 360\nAppearance = TWO_LEGS\nWanderWidthFactor = 0\nEnd\n",
        )
        .unwrap();
    let mut logic = GameLogic::new();
    logic.admit_locomotor_definitions(locomotors).unwrap();
    logic
        .admit_object_ini_sources([(
            "WaypointOwner.ini".into(),
            "Object WaypointOwnerInfantry\nKindOf = SELECTABLE INFANTRY\nLocomotor = SET_NORMAL OwnedNormal\nLocomotor = SET_WANDER OwnedWander\nLocomotor = SET_PANIC OwnedPanic\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface Brain\nEnd\nEnd\n".into(),
        )])
        .unwrap();
    logic.start_new_game(GameMode::SinglePlayer);
    assert!(logic.load_map(path.to_str().expect("UTF-8 map path")));
    logic.add_player(Player::new(1, Team::USA, "Owner", true).with_script_identity("Owner"));
    let object = logic
        .create_object_for_player("WaypointOwnerInfantry", 1, glam::Vec3::new(5.0, 0.0, 10.0))
        .expect("create waypoint unit");
    let terrain = logic
        .host_object(object)
        .expect("created object")
        .get_position();
    assert!(terrain.is_finite());
    (logic, object)
}

fn team_path_script(kind: ScriptActionType, path: &str) -> ScriptEngine {
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamOwner".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            path.into(),
        ))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "OwnedWaypointMovement".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().expect("script engine");
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    engine
}

#[test]
fn live_team_wander_and_panic_ignore_foreign_same_label_terrain() {
    let temp = tempfile::tempdir().unwrap();
    let owner_map = waypoint_map(
        temp.path(),
        "owner",
        [(100, 100), (140, 100)],
        [701, 702],
        "OwnerTerminal",
    );
    let peer_map = waypoint_map(
        temp.path(),
        "peer",
        [(300, 300), (340, 300)],
        [801, 802],
        "PeerTerminal",
    );
    let (mut owner, owner_id) = world(&owner_map);
    let (mut peer, peer_id) = world(&peer_map);
    assert_eq!(owner_id, peer_id, "same-ID worlds are required");

    let foreign_services = gamelogic::system::engine_stores::new_world_services();
    {
        let mut terrain = foreign_services.terrain().write().unwrap();
        let mut map = gamelogic::system::map_loader::MapData::new();
        map.width = 101;
        map.height = 101;
        map.heightmap = vec![0; 10201];
        for (id, (x, y), label) in [
            (901, (700.0, 700.0), "SharedPath"),
            (902, (900.0, 700.0), "ForeignTerminal"),
        ] {
            map.waypoints
                .push(gamelogic::system::map_loader::MapWaypoint {
                    id,
                    name: format!("Foreign{id}"),
                    location: gamelogic::common::Coord3D::new(x, y, 0.0),
                    path_label1: label.into(),
                    path_label2: String::new(),
                    path_label3: String::new(),
                    bi_directional: false,
                });
        }
        map.waypoint_links.push((901, 902));
        terrain.load_map_data(map);
    }

    let wander = team_path_script(ScriptActionType::TeamWander, "SharedPath");
    gamelogic::system::engine_stores::with_world_services(&foreign_services, || {
        owner
            .update_local_script_engine_with_owner(&wander)
            .unwrap();
    });
    let owner_unit = owner.host_object(owner_id).unwrap();
    let destination = owner_unit
        .requested_destination
        .expect("wander destination");
    assert_eq!((destination.x, destination.z), (140.0, 100.0));
    assert!(
        owner_unit.waiting_for_path,
        "the admitted map queues pathfinding"
    );
    assert!(
        peer.host_object(peer_id)
            .unwrap()
            .requested_destination
            .is_none()
    );

    let panic = team_path_script(ScriptActionType::TeamPanic, "SharedPath");
    gamelogic::system::engine_stores::with_world_services(&foreign_services, || {
        peer.update_local_script_engine_with_owner(&panic).unwrap();
    });
    let peer_unit = peer.host_object(peer_id).unwrap();
    assert_eq!(
        peer_unit.ai_state(),
        generals_main::game_logic::AIState::Panic
    );
    assert!(peer_unit.is_panicking);
    assert_eq!(
        peer_unit.requested_destination.unwrap(),
        glam::Vec3::new(300.0, 0.0, 300.0)
    );
    assert!((peer_unit.path_extra_distance - 41.0).abs() < 0.01);
    assert!(!owner.host_object(owner_id).unwrap().is_panicking);

    let after_foreign = owner
        .host_object(owner_id)
        .unwrap()
        .requested_destination
        .unwrap();
    drop(foreign_services);
    assert_eq!(
        owner.host_object(owner_id).unwrap().requested_destination,
        Some(after_foreign)
    );
}
