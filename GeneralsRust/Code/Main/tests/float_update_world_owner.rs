use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

const FLOATING_VEHICLE: &str = "CivilianVehicleFerry";

fn water_map(root: &std::path::Path, name: &str, water_y: i32) -> std::path::PathBuf {
    let directory = root.join(name);
    std::fs::create_dir(&directory).expect("create fixture map directory");

    let mut map = DataChunkOutput::new();
    // Deliberately omit HeightMapData. Main's TerrainData cache stays absent,
    // while PolygonTriggers still populate the receiving Core TerrainLogic.
    map.open_data_chunk("PolygonTriggers", 4);
    map.write_int(1);
    map.write_ascii_string("FloatOwnerWater");
    map.write_ascii_string("FloatOwnerLayer");
    map.write_int(1);
    map.write_byte(1); // is water
    map.write_byte(0); // is not a river
    map.write_int(0); // river start
    map.write_int(4);
    for (x, z) in [(0, 0), (200, 0), (200, 200), (0, 200)] {
        map.write_int(x);
        map.write_int(z);
        map.write_int(water_y);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();

    let map_path = directory.join("FloatOwner.map");
    std::fs::write(&map_path, map.into_ckmp_bytes()).expect("write fixture map");
    map_path
}

fn world(map_path: &std::path::Path) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::SinglePlayer);
    logic.add_player(Player::new(0, Team::USA, "FloatOwner", true));
    assert!(logic.load_map(map_path.to_str().expect("map path is UTF-8")));
    let mut ferry = ThingTemplate::new(FLOATING_VEHICLE);
    ferry.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert(FLOATING_VEHICLE.into(), ferry);

    let object = logic
        .create_object(FLOATING_VEHICLE, Team::USA, Vec3::new(100.0, 0.0, 100.0))
        .expect("create ferry");
    let float = logic
        .host_object_mut(object)
        .expect("created ferry")
        .float_update
        .as_mut()
        .expect("ferry installs FloatUpdate");
    // Retail defaults to sway-only. Enable snapping to exercise C++'s
    // waterZ output while retaining ordinary Main update ordering.
    float.set_enabled(true);
    (logic, object)
}

fn y(logic: &GameLogic, object: generals_main::game_logic::ObjectId) -> f32 {
    logic
        .host_object(object)
        .expect("live ferry")
        .get_position()
        .y
}

fn assert_water_height(
    logic: &GameLogic,
    object: generals_main::game_logic::ObjectId,
    expected: f32,
) {
    let actual = y(logic, object);
    assert!(
        (actual - expected).abs() < 1.0,
        "FloatUpdate should snap to receiving water {expected}, got {actual}"
    );
}

#[test]
fn actual_float_update_samples_the_receiving_water_plane_for_same_id_worlds() {
    let temp = tempfile::tempdir().unwrap();
    let high_map = water_map(temp.path(), "high", 80);
    let low_map = water_map(temp.path(), "low", 30);
    let (mut high, high_object) = world(&high_map);
    let (mut low, low_object) = world(&low_map);
    assert_eq!(
        high_object, low_object,
        "fixture must reuse the same ObjectId"
    );

    // The public fixed-step path must preserve each admitted water surface.
    // A separate internal test perturbs the active Core owner inside the
    // FloatUpdate boundary to distinguish the explicit borrow from ambient.
    high.update_with_dt(1.0 / 30.0);
    assert_water_height(&high, high_object, 80.0);
    assert_eq!(y(&low, low_object), 0.0);

    low.update_with_dt(1.0 / 30.0);
    assert_water_height(&low, low_object, 30.0);
    high.update_with_dt(1.0 / 30.0);
    assert_water_height(&high, high_object, 80.0);

    // Resetting the peer cannot replace the still-live receiver's terrain.
    low.reset();
    high.update_with_dt(1.0 / 30.0);
    assert_water_height(&high, high_object, 80.0);
}

#[cfg(feature = "integration-tests")]
#[test]
fn float_phase_ignores_foreign_published_terrain() {
    let temp = tempfile::tempdir().unwrap();
    let high_map = water_map(temp.path(), "high-direct", 80);
    let low_map = water_map(temp.path(), "low-direct", 30);
    let (mut high, high_object) = world(&high_map);
    let (mut low, low_object) = world(&low_map);
    assert_eq!(high_object, low_object);

    // Deliberately publish a different service owner around the FloatUpdate
    // phase itself. Calling the usual full tick would republish the receiver
    // and would not distinguish this from the old ambient implementation.
    let foreign = gamelogic::system::engine_stores::new_world_services();
    {
        let mut terrain = foreign.terrain().write().unwrap();
        let mut water = gamelogic::polygon_trigger::PolygonTrigger::new(
            99,
            "ForeignFloatWater".into(),
            vec![
                gamelogic::common::ICoord3D::new(0, 0, 120),
                gamelogic::common::ICoord3D::new(200, 0, 120),
                gamelogic::common::ICoord3D::new(200, 200, 120),
                gamelogic::common::ICoord3D::new(0, 200, 120),
            ],
        );
        water.set_water_area(true);
        terrain.add_trigger_area(water);
        assert_eq!(
            generals_main::game_logic::host_float_update::receiving_water_surface_y(
                &terrain, 100.0, 100.0,
            ),
            Some(120.0),
            "the published control must contain a distinct water plane"
        );
    }
    gamelogic::system::engine_stores::with_world_services(&foreign, || {
        assert!(std::sync::Arc::ptr_eq(
            gamelogic::system::engine_stores::active_services().terrain(),
            foreign.terrain(),
        ));
        high.update_float_for_owner_test();
        assert_water_height(&high, high_object, 80.0);
        assert_eq!(y(&low, low_object), 0.0);
        low.update_float_for_owner_test();
        assert_water_height(&low, low_object, 30.0);
        low.reset();
        high.update_float_for_owner_test();
        assert_water_height(&high, high_object, 80.0);
    });
}
