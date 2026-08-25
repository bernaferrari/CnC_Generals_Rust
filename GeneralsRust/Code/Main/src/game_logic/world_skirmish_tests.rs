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
    let has_builder = logic.host_objects().values().any(|o| {
        o.team == Team::USA
            && o.is_alive()
            && o.is_mobile()
            && (o.can_construct()
                || o.template_name.to_ascii_lowercase().contains("dozer")
                || o.template_name.to_ascii_lowercase().contains("worker"))
    });
    assert!(
        has_builder,
        "USA skirmish start must spawn retail AmericaVehicleDozer/USA_Dozer"
    );
}

#[test]
fn dozer_template_name_counts_as_mobile() {
    let mut logic = GameLogic::new();
    let id = logic
        .create_object("USA_Dozer", Team::USA, glam::Vec3::ZERO)
        .expect("dozer");
    let o = logic.host_object(id).expect("obj");
    assert!(
        o.is_mobile(),
        "USA_Dozer must be mobile for host select/count"
    );
    assert!(o.is_worker() || o.can_construct(), "dozer should construct");
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
    let mut map_data = gamelogic::system::map_loader::MapData::new();
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
    {
        let mut terrain = gamelogic::terrain::get_terrain_logic()
            .write()
            .expect("THE_TERRAIN_LOGIC");
        terrain.reset();
        terrain.load_map_data(map_data);
    }

    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    logic.clear_all_players();
    let mut p0 = Player::new(0, Team::USA, "Human", true);
    p0.is_alive = true;
    p0.start_position = 0;
    logic.add_player(p0);
    let _ = logic.create_object(
        "AmericaCommandCenter",
        Team::USA,
        glam::Vec3::new(100.0, 0.0, 100.0),
    );
    logic.spawn_skirmish_starting_units();
    let dozer = logic
        .host_objects()
        .values()
        .find(|o| {
            o.team == Team::USA
                && o.is_alive()
                && o.is_mobile()
                && (o.template_name.to_ascii_lowercase().contains("dozer")
                    || o.template_name.to_ascii_lowercase().contains("worker"))
        })
        .expect("starting builder");
    let pos = dozer.get_position();
    assert!(
        (pos.x - 321.0).abs() < 0.5 && (pos.z - 654.0).abs() < 0.5,
        "starting unit must spawn at Player_1_Rally, got {pos:?}"
    );

    if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
        terrain.reset();
    }
}
