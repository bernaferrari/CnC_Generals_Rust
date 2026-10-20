// Mechanical extract from game_logic.rs
// `mod skirmish_starting_unit_residual_tests` (L134760-L134824).
// Child module of game_logic.rs via `#[path]`.

use super::*;

#[test]
fn spawn_skirmish_starting_units_spawns_missing_builder() {
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    logic.clear_all_players();
    // Human USA with a structure but no dozer.
    let mut p0 = Player::new(0, Team::USA, "Human", true);
    p0.is_alive = true;
    logic.add_player(p0);
    let mut p1 = Player::new(1, Team::China, "AI", false);
    p1.is_alive = true;
    logic.add_player(p1);
    // Structures present for both teams (retail map supplies bases).
    // Wave 734: free invent of starting building when no base is fail-closed.
    let _ = logic.create_object(
        "AmericaCommandCenter",
        Team::USA,
        glam::Vec3::new(100.0, 0.0, 100.0),
    );
    let _ = logic.create_object(
        "ChinaCommandCenter",
        Team::China,
        glam::Vec3::new(500.0, 0.0, 500.0),
    );
    let before = logic
        .host_objects()
        .values()
        .filter(|o| o.team == Team::USA && o.is_mobile())
        .count();
    logic.spawn_skirmish_starting_units();
    let after = logic
        .host_objects()
        .values()
        .filter(|o| o.team == Team::USA && o.is_mobile())
        .count();
    assert!(
        after > before,
        "USA should gain a starting dozer/worker residual (before={before} after={after})"
    );
    let china_mobile = logic
        .host_objects()
        .values()
        .filter(|o| o.team == Team::China && o.is_mobile())
        .count();
    assert!(
        china_mobile >= 1,
        "China AI should gain starting builder residual"
    );
}

#[test]
fn america_vehicle_dozer_is_retail_starting_builder() {
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    logic.clear_all_players();
    let mut p0 = Player::new(0, Team::USA, "Human", true);
    p0.is_alive = true;
    logic.add_player(p0);
    logic.ensure_ai_faction_templates(Team::USA);
    {
        let shroud = logic.world_services.shroud();
        let mut shroud = shroud.lock().expect("receiving fixture shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(512.0, 512.0);
    }
    let _ = logic.create_object(
        "AmericaCommandCenter",
        Team::USA,
        glam::Vec3::new(100.0, 0.0, 100.0),
    );
    // Lone Eagle-style maps keep buildings but omit the starting dozer.
    // spawn_skirmish_starting_units is the map/skirmish start path (not spawn_dozer=1).
    assert!(
        !logic.host_objects().values().any(|o| {
            o.team == Team::USA
                && o.is_alive()
                && (o.template_name.eq_ignore_ascii_case("AmericaVehicleDozer")
                    || o.template_name.eq_ignore_ascii_case("USA_Dozer"))
        }),
        "fixture starts without a dozer (map-style)"
    );
    logic.spawn_skirmish_starting_units();
    let has_builder = logic
        .host_objects()
        .values()
        .any(|o| o.team == Team::USA && o.is_alive() && o.is_mobile() && o.can_construct());
    assert!(
        has_builder,
        "USA skirmish start must spawn retail AmericaVehicleDozer/USA_Dozer"
    );
    let builder = logic
        .host_objects()
        .values()
        .find(|o| o.team == Team::USA && o.can_construct())
        .unwrap()
        .id;
    logic.get_player_mut(0).unwrap().resources.supplies = 1000;
    let mut scaffold = ThingTemplate::new("SpawnedBuilderScaffold");
    scaffold
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Immobile)
        .set_health(1000.0)
        .set_cost(300, 0);
    scaffold.always_visible = true;
    logic.templates.insert(scaffold.name.clone(), scaffold);
    // Ordinary construction admission includes source SHROUD_REVEALED and
    // CLEAR_PATH. Drive the receiving vision phase before selecting an actual
    // legal pad; this fixture verifies constructor admission, not an arbitrary
    // unrevealed (250,250) placement. No fog disable or legality bypass.
    logic.update_main_crate_vision();
    let location = [180.0, 220.0, 260.0, 300.0]
        .into_iter()
        .flat_map(|x| {
            [60.0, 100.0, 140.0, 180.0, 220.0]
                .into_iter()
                .map(move |z| glam::Vec3::new(x, 0.0, z))
        })
        .find(|location| {
            logic.is_location_legal_to_build_for_builder(
                Team::USA,
                *location,
                "SpawnedBuilderScaffold",
                Some(builder),
            )
        })
        .expect("receiving spawned constructor must have a revealed legal build pad");
    assert!(logic.is_build_location_shroud_clear(0, location));
    assert!(logic.is_location_legal_to_build_for_builder(
        Team::USA,
        location,
        "SpawnedBuilderScaffold",
        Some(builder)
    ));
    let result = crate::command_executor::CommandExecutor::new(&mut logic, 0)
        .execute_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::DozerConstruct {
                template_name: "SpawnedBuilderScaffold".into(),
                location,
                orientation: 0.0,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::UNIX_EPOCH,
            selected_units: vec![builder],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        })
        .unwrap();
    assert_eq!(
        result,
        crate::command_system::CommandResult::Success,
        "actual spawned constructor command"
    );
    assert_eq!(logic.get_player(0).unwrap().effective_supplies(), 700);
    let site = logic
        .host_objects()
        .values()
        .find(|o| o.template_name == "SpawnedBuilderScaffold")
        .unwrap();
    assert!(site.status.under_construction);
    assert_eq!(site.builder_id, Some(builder));
}

#[test]
fn dozer_template_name_counts_as_mobile() {
    let mut logic = GameLogic::new();
    logic.ensure_ai_faction_templates(Team::USA);
    let id = logic
        .create_object("USA_Dozer", Team::USA, glam::Vec3::ZERO)
        .expect("dozer");
    let o = logic.host_object(id).expect("obj");
    assert!(
        o.is_mobile(),
        "USA_Dozer must be mobile for host select/count"
    );
    assert!(o.is_kind_of(KindOf::Dozer), "dozer must author DOZER");
    assert!(o.can_construct(), "actual starting dozer should construct");
    assert!(o.can_repair(), "actual starting dozer should repair");
}

#[test]
fn cached_player_start_waypoints_pairs_rally() {
    let mut logic = GameLogic::new();
    let mut meta = super::script_loader::MapMetadata::default();
    meta.start_waypoints = vec![
        (
            "Player_1_Start".into(),
            gamelogic::scripting::core::Coord3D::new(10.0, 20.0, 1.0),
        ),
        (
            "Player_1_Rally".into(),
            gamelogic::scripting::core::Coord3D::new(321.0, 654.0, 12.0),
        ),
        (
            "Player_2_Start".into(),
            gamelogic::scripting::core::Coord3D::new(30.0, 40.0, 2.0),
        ),
    ];
    logic.last_map_settings = Some(meta);
    let starts = logic
        .cached_player_start_waypoints()
        .expect("cached starts");
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[0].0, 0);
    let rally = starts[0].2.expect("Player_1_Rally");
    assert!(
        (rally.x - 321.0).abs() < 0.01 && (rally.y - 654.0).abs() < 0.01,
        "cached rally discarded: {rally:?}"
    );
    assert!(starts[1].2.is_none(), "Player_2 has no rally");
}

#[test]
fn spawn_skirmish_starting_units_uses_player_rally() {
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    logic.clear_all_players();
    let mut p0 = Player::new(0, Team::USA, "Human", true);
    p0.is_alive = true;
    p0.start_position = 0;
    logic.add_player(p0);
    logic.ensure_ai_faction_templates(Team::USA);

    // Terrain and parsed metadata are admitted to this receiving world after
    // new-game reset. Constructing a second world never publishes this map.
    let mut map_data = gamelogic::system::map_loader::MapData::new();
    map_data.width = 128;
    map_data.height = 128;
    map_data.heightmap = vec![0; 128 * 128];
    map_data
        .waypoints
        .push(gamelogic::system::map_loader::MapWaypoint {
            id: 1,
            name: "Player_1_Rally".to_string(),
            location: gamelogic::system::map_loader::Coord3D::new(321.0, 654.0, 12.0),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    logic
        .world_services
        .terrain()
        .write()
        .expect("receiving terrain")
        .load_map_data(map_data);
    let mut metadata = super::script_loader::MapMetadata::default();
    metadata.start_waypoints = vec![
        (
            "Player_1_Start".into(),
            gamelogic::scripting::core::Coord3D::new(100.0, 100.0, 0.0),
        ),
        (
            "Player_1_Rally".into(),
            gamelogic::scripting::core::Coord3D::new(321.0, 654.0, 12.0),
        ),
    ];
    logic.last_map_settings = Some(metadata);
    assert_eq!(
        logic.player_rally_spawn_pos(0),
        Some(glam::Vec3::new(321.0, 0.0, 654.0))
    );

    // Identical waypoint name on a peer is independent of the driving owner.
    let peer = GameLogic::new();
    let mut peer_map = gamelogic::system::map_loader::MapData::new();
    peer_map.width = 128;
    peer_map.height = 128;
    peer_map.heightmap = vec![0; 128 * 128];
    peer_map
        .waypoints
        .push(gamelogic::system::map_loader::MapWaypoint {
            id: 1,
            name: "Player_1_Rally".to_string(),
            location: gamelogic::system::map_loader::Coord3D::new(777.0, 888.0, 12.0),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    peer.world_services
        .terrain()
        .write()
        .expect("peer terrain")
        .load_map_data(peer_map);
    assert_eq!(
        peer.player_rally_spawn_pos(0),
        Some(glam::Vec3::new(777.0, 0.0, 888.0))
    );
    let _ = logic.create_object(
        "AmericaCommandCenter",
        Team::USA,
        glam::Vec3::new(100.0, 0.0, 100.0),
    );
    logic.spawn_skirmish_starting_units();
    let dozer = logic
        .host_objects()
        .values()
        .find(|o| o.team == Team::USA && o.is_alive() && o.is_mobile() && o.can_construct())
        .expect("starting builder");
    let pos = dozer.get_position();
    assert!(
        (pos.x - 321.0).abs() < 0.5 && (pos.z - 654.0).abs() < 0.5,
        "starting unit must spawn at Player_1_Rally, got {pos:?}"
    );
    assert_eq!(
        peer.player_rally_spawn_pos(0),
        Some(glam::Vec3::new(777.0, 0.0, 888.0))
    );
}
