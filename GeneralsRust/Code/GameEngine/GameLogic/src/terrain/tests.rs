//! Terrain behavior tests, kept adjacent to the implementation without inflating production modules.

use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn make_water_trigger(
    id: Int,
    name: &str,
    z: Int,
    min_x: Int,
    min_y: Int,
    max_x: Int,
    max_y: Int,
) -> PolygonTrigger {
    let mut trigger = PolygonTrigger::new(id, AsciiString::from(name), Vec::new());
    trigger.set_water_area(true);
    trigger.add_point(ICoord3D::new(min_x, min_y, z));
    trigger.add_point(ICoord3D::new(max_x, min_y, z));
    trigger.add_point(ICoord3D::new(max_x, max_y, z));
    trigger.add_point(ICoord3D::new(min_x, max_y, z));
    trigger
}

fn map_data_with_heightmap(
    width: u32,
    height: u32,
    heightmap: Vec<u8>,
) -> crate::system::map_loader::MapData {
    let mut map_data = crate::system::map_loader::MapData::new();
    map_data.width = width;
    map_data.height = height;
    map_data.heightmap = heightmap;
    map_data
}

#[test]
fn get_ground_height_adds_border_size_like_cpp() {
    // Playable 4x4, border 1 → full 6x6 sample buffer.
    let mut heightmap = vec![0u8; 6 * 6];
    heightmap[1 + 1 * 6] = 80;
    let mut map_data = map_data_with_heightmap(4, 4, heightmap);
    map_data.border_size = 1;
    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map_data);
    let height = terrain.get_ground_height(0.0, 0.0, None);
    let expected = 80.0 * MAP_HEIGHT_SCALE;
    assert!(
        (height - expected).abs() < 0.01,
        "world (0,0) must sample (border,border) like C++ BaseHeightMap, got {height} expected {expected}"
    );
}

#[test]
fn register_bridge_attach_cells_offset_like_cpp_surface() {
    let prod = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/terrain/bridge.rs"
    ));
    let i = prod
        .find("fn register_bridge_with_pathfinder")
        .expect("register_bridge");
    let w = &prod[i..prod.len().min(i + 2500)];
    assert!(
        w.contains("PATHFIND_CELL_SIZE_F * 0.7")
            && w.contains("bridge_info.from.x - bridge_dir.x")
            && w.contains("bridge_info.to.x + bridge_dir.x")
            && w.contains("add_bridge_ex"),
        "bridge m_startCell/m_endCell must offset 0.7 cell along bridgeDir like C++"
    );
}

#[test]
fn add_landmark_bridge_from_geometry_registers_deck() {
    let mut terrain = TerrainLogic::new();
    terrain.add_landmark_bridge_from_geometry(
        Coord3D::new(10.0, 20.0, 5.0),
        0.0,
        6.0,
        2.0,
        42,
        AsciiString::from("TsingMaLandmarkBridge"),
    );
    let bridge = terrain
        .find_bridge_at(&Coord3D::new(10.0, 20.0, 5.0))
        .expect("landmark deck must register");
    let info = bridge.get_bridge_info();
    assert_eq!(info.bridge_object_id, 42);
    assert_eq!(info.from_left, Coord3D::new(4.0, 22.0, 5.0));
    assert_eq!(info.from_right, Coord3D::new(4.0, 18.0, 5.0));
    assert_eq!(info.to_left, Coord3D::new(16.0, 22.0, 5.0));
    assert_eq!(info.to_right, Coord3D::new(16.0, 18.0, 5.0));
    assert_eq!(info.bridge_width, 4.0);
    assert!((bridge.get_bridge_height(&Coord3D::new(10.0, 20.0, 0.0), None) - 5.0).abs() < 0.01);
}

#[test]
fn bridge_info_from_parts_matches_expected_rectangle() {
    let bridge_info = TerrainLogic::bridge_info_from_parts(
        Coord3D::new(10.0, 20.0, 3.0),
        0.0,
        6.0,
        2.0,
        crate::object::INVALID_ID,
    );

    assert_eq!(bridge_info.from_left, Coord3D::new(4.0, 22.0, 3.0));
    assert_eq!(bridge_info.from_right, Coord3D::new(4.0, 18.0, 3.0));
    assert_eq!(bridge_info.to_left, Coord3D::new(16.0, 22.0, 3.0));
    assert_eq!(bridge_info.to_right, Coord3D::new(16.0, 18.0, 3.0));
    assert_eq!(bridge_info.bridge_width, 4.0);
}

#[test]
fn delete_bridge_at_removes_bridge_from_list() {
    let bridge_info = TerrainLogic::bridge_info_from_parts(
        Coord3D::new(10.0, 20.0, 0.0),
        0.0,
        6.0,
        2.0,
        crate::object::INVALID_ID,
    );

    let mut terrain = TerrainLogic::new();
    let mut bridge = Box::new(Bridge::new(bridge_info, AsciiString::from("TestBridge")));
    bridge.set_layer(PathfindLayerEnum::Bridge1);
    terrain.bridge_list_head = Some(bridge);

    let hit_point = Coord3D::new(10.0, 20.0, 0.0);
    assert!(terrain.delete_bridge_at(&hit_point));
    assert!(terrain.find_bridge_at(&hit_point).is_none());
}

#[test]
fn bridge_point_test_rejects_bounds_only_false_positive() {
    let mut info = BridgeInfo::new();
    info.from_left = Coord3D::new(0.0, 0.0, 0.0);
    info.from_right = Coord3D::new(2.0, 2.0, 0.0);
    info.to_right = Coord3D::new(0.0, 4.0, 0.0);
    info.to_left = Coord3D::new(-2.0, 2.0, 0.0);

    let bridge = Bridge::new(info, AsciiString::from("TestBridge"));
    let false_positive = Coord3D::new(2.0, 0.0, 0.0); // inside AABB, outside rotated bridge quad
    let inside = Coord3D::new(0.0, 2.0, 0.0);

    assert!(!bridge.is_point_on_bridge(&false_positive));
    assert!(bridge.is_point_on_bridge(&inside));
}

#[test]
fn terrain_load_map_returns_false_for_invalid_map_data() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let map_path = std::env::temp_dir().join(format!(
        "generalsrust_terrain_invalid_{}_{}.map",
        std::process::id(),
        timestamp
    ));

    std::fs::write(&map_path, b"not-a-valid-map-datachunk").expect("write map fixture");

    let mut terrain = TerrainLogic::new();
    let loaded = terrain.load_map(
        AsciiString::from(map_path.to_string_lossy().as_ref()),
        false,
    );
    assert!(!loaded);

    let _ = std::fs::remove_file(&map_path);
}

#[test]
fn new_map_enables_water_grid_when_waveguide1_exists() {
    let mut terrain = TerrainLogic::new();
    terrain.add_waypoint_from_map(&MapWaypoint {
        id: 1,
        name: "WaveGuide1".to_string(),
        location: crate::system::map_loader::Coord3D::new(20.0, 20.0, 5.0),
        path_label1: String::new(),
        path_label2: String::new(),
        path_label3: String::new(),
        bi_directional: false,
    });
    assert!(!terrain.is_water_grid_enabled());
    terrain.new_map(false);
    assert!(
        terrain.is_water_grid_enabled(),
        "C++ TerrainLogic::newMap enables the water grid when WaveGuide1 is present"
    );
}

#[test]
fn find_named_waypoint_resolves_player_rally() {
    // C++ GameLogic.cpp:160 findNamedWaypoint("Player_%d_Rally").
    let mut terrain = TerrainLogic::new();
    terrain.add_waypoint_from_map(&MapWaypoint {
        id: 7,
        name: "Player_1_Rally".to_string(),
        location: crate::system::map_loader::Coord3D::new(321.0, 654.0, 12.0),
        path_label1: String::new(),
        path_label2: String::new(),
        path_label3: String::new(),
        bi_directional: false,
    });
    let name = AsciiString::from("Player_1_Rally");
    let loc = terrain
        .get_waypoint_by_name(&name)
        .expect("Player_1_Rally")
        .get_location();
    assert!(
        (loc.x - 321.0).abs() < 0.01 && (loc.y - 654.0).abs() < 0.01,
        "rally XY discarded: {loc:?}"
    );
}

#[test]
fn terrain_query_load_skips_new_map_finalization() {
    let mut terrain = TerrainLogic::new();

    terrain.add_waypoint_from_map(&MapWaypoint {
        id: 1,
        name: "WaveGuide1".to_string(),
        location: crate::system::map_loader::Coord3D::new(20.0, 20.0, 5.0),
        path_label1: String::new(),
        path_label2: String::new(),
        path_label3: String::new(),
        bi_directional: false,
    });

    terrain.query_load_pending = true;
    terrain.new_map(false);

    assert!(
        !terrain.water_grid_enabled,
        "query-mode load should skip the follow-up new_map side effects"
    );
    assert!(terrain
        .get_waypoint_by_name(&AsciiString::from("WaveGuide1"))
        .is_some());
}

#[test]
fn w3d_extents_use_boundaries_border_and_height_range() {
    let mut terrain = TerrainLogic::new();
    let mut map_data = map_data_with_heightmap(8, 6, vec![4, 10, 1, 9, 8, 2, 7, 5]);
    map_data.boundaries = vec![ICoord2D::new(3, 4), ICoord2D::new(7, 5)];
    map_data.border_size = 2;
    terrain.load_map_data(map_data);

    let extent = terrain.get_extent();
    assert_eq!(extent.lo, Coord3D::new(0.0, 0.0, 1.0 * MAP_HEIGHT_SCALE));
    assert_eq!(
        extent.hi,
        Coord3D::new(
            3.0 * MAP_XY_FACTOR,
            4.0 * MAP_XY_FACTOR,
            10.0 * MAP_HEIGHT_SCALE
        )
    );

    let max_extent = terrain.get_maximum_pathfind_extent();
    assert_eq!(max_extent.lo.x, 0.0);
    assert_eq!(max_extent.lo.y, 0.0);
    assert_eq!(max_extent.hi.x, 7.0 * MAP_XY_FACTOR);
    assert_eq!(max_extent.hi.y, 5.0 * MAP_XY_FACTOR);
    assert_eq!(max_extent.lo.z, 1.0 * MAP_HEIGHT_SCALE);
    assert_eq!(max_extent.hi.z, 10.0 * MAP_HEIGHT_SCALE);

    let with_border = terrain.get_extent_including_border();
    assert_eq!(with_border.lo.x, -2.0 * MAP_XY_FACTOR);
    assert_eq!(with_border.lo.y, -2.0 * MAP_XY_FACTOR);
    assert_eq!(with_border.hi.x, 6.0 * MAP_XY_FACTOR);
    assert_eq!(with_border.hi.y, 4.0 * MAP_XY_FACTOR);
}

#[test]
fn w3d_reset_restores_empty_extent_state() {
    let mut terrain = TerrainLogic::new();
    let mut map_data = map_data_with_heightmap(3, 3, vec![2, 4, 6, 8]);
    map_data.boundaries = vec![ICoord2D::new(3, 3)];
    map_data.border_size = 1;
    terrain.load_map_data(map_data);

    terrain.reset();

    assert!(terrain.map_data.is_empty());
    assert_eq!(terrain.map_dx, 0);
    assert_eq!(terrain.map_dy, 0);
    assert_eq!(terrain.map_min_z, 0.0);
    assert_eq!(terrain.map_max_z, 1.0);
    assert!(terrain.boundaries.is_empty());
    assert!(terrain.terrain_data.is_none());
    assert_eq!(terrain.get_extent().hi, Coord3D::new(0.0, 0.0, 1.0));
}

#[test]
fn layer_height_ignores_bridge_below_ground() {
    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map_data_with_heightmap(2, 2, vec![20, 20, 20, 20]));

    let ground_height = 20.0 * MAP_HEIGHT_SCALE;
    let bridge_info = TerrainLogic::bridge_info_from_parts(
        Coord3D::new(5.0, 5.0, ground_height - 5.0),
        0.0,
        5.0,
        5.0,
        crate::object::INVALID_ID,
    );
    let mut bridge = Box::new(Bridge::new(bridge_info, AsciiString::from("BuriedBridge")));
    bridge.set_layer(PathfindLayerEnum::Bridge1);
    terrain.bridge_list_head = Some(bridge);

    let height = terrain.get_layer_height(5.0, 5.0, PathfindLayerEnum::Bridge1, None, true);
    assert_eq!(height, ground_height);
}

#[test]
fn layer_height_uses_wall_height_only_when_unclipped_or_on_wall() {
    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map_data_with_heightmap(2, 2, vec![10, 10, 10, 10]));

    let ground_height = 10.0 * MAP_HEIGHT_SCALE;
    assert_eq!(
        terrain.get_layer_height(5.0, 5.0, PathfindLayerEnum::Wall, None, true),
        ground_height
    );
    assert_eq!(
        terrain.get_layer_height(5.0, 5.0, PathfindLayerEnum::Wall, None, false),
        terrain.get_wall_height()
    );
}

#[test]
fn water_handle_lookup_by_name_prefers_trigger_identity_over_name_cache() {
    let mut terrain = TerrainLogic::new();
    terrain.add_trigger_area(make_water_trigger(11, "SharedWater", 12, 0, 0, 40, 40));
    terrain.add_trigger_area(make_water_trigger(
        22,
        "SharedWater",
        28,
        100,
        100,
        140,
        140,
    ));

    terrain.water_handles.insert(
        AsciiString::from("SharedWater"),
        WaterHandle::new(
            AsciiString::from("SharedWater"),
            999.0,
            Region3D::new(
                Coord3D::new(-1.0, -1.0, -1.0),
                Coord3D::new(-1.0, -1.0, -1.0),
            ),
        ),
    );

    let by_name = terrain
        .get_water_handle_by_name(&AsciiString::from("SharedWater"))
        .expect("expected first matching water trigger");
    assert_eq!(by_name.get_current_height(), 12.0);
    assert_eq!(by_name.get_bounds().lo.z, 12.0);

    let first_location = terrain
        .get_water_handle(10.0, 10.0)
        .expect("expected first trigger to resolve by location");
    assert_eq!(first_location.get_current_height(), 12.0);

    let second_location = terrain
        .get_water_handle(110.0, 110.0)
        .expect("expected second trigger to resolve by location");
    assert_eq!(second_location.get_current_height(), 28.0);
}

#[test]
fn water_handle_lookup_by_name_ignores_orphaned_name_cache_entries() {
    let mut terrain = TerrainLogic::new();
    terrain.water_handles.insert(
        AsciiString::from("OrphanedWater"),
        WaterHandle::new(
            AsciiString::from("OrphanedWater"),
            42.0,
            Region3D::new(Coord3D::new(1.0, 1.0, 1.0), Coord3D::new(2.0, 2.0, 2.0)),
        ),
    );

    assert!(
        terrain
            .get_water_handle_by_name(&AsciiString::from("OrphanedWater"))
            .is_none(),
        "C++ TerrainLogic::getWaterHandleByName only resolves polygon-trigger water handles"
    );
}

#[test]
fn ground_height_returns_zero_for_empty_terrain() {
    let terrain = TerrainLogic::new();
    let h = terrain.get_ground_height(50.0, 50.0, None);
    assert_eq!(h, 0.0, "Empty terrain should return 0.0 height");
}

#[test]
fn ground_height_triangle_interpolation_lower() {
    let mut terrain = TerrainLogic::new();
    let map_data = crate::system::map_loader::MapData {
        heightmap: vec![0, 128, 0, 255],
        width: 2,
        height: 2,
        border_size: 0,
        boundaries: vec![],
        bridges: vec![],
        water_height: None,
        waypoints: vec![],
        waypoint_links: vec![],
        polygon_triggers: vec![],
        texture_tiles: vec![],
    };
    terrain.load_map_data(map_data);

    let h00 = terrain.get_ground_height(0.0, 0.0, None);
    let h10 = terrain.get_ground_height(10.0, 0.0, None);
    let h01 = terrain.get_ground_height(0.0, 10.0, None);
    let h11 = terrain.get_ground_height(10.0, 10.0, None);

    assert!(
        h00 < h11,
        "Corner heights should reflect heightmap: h00={}, h11={}",
        h00,
        h11
    );

    let h_center = terrain.get_ground_height(5.0, 5.0, None);
    let expected = 127.5 * MAP_HEIGHT_SCALE;
    assert!(
        (h_center - expected).abs() < 0.1,
        "Center height should match C++ triangle interpolation: got {}, expected {}",
        h_center,
        expected
    );
}

#[test]
fn ground_height_triangle_interpolation_upper() {
    let mut terrain = TerrainLogic::new();
    let map_data = crate::system::map_loader::MapData {
        heightmap: vec![0, 255, 255, 0],
        width: 2,
        height: 2,
        border_size: 0,
        boundaries: vec![],
        bridges: vec![],
        water_height: None,
        waypoints: vec![],
        waypoint_links: vec![],
        polygon_triggers: vec![],
        texture_tiles: vec![],
    };
    terrain.load_map_data(map_data);

    let h = terrain.get_ground_height(2.0, 8.0, None);
    assert!(h > 0.0, "Upper triangle height should be > 0, got {}", h);
}

#[test]
fn ground_height_matches_cpp_triangle_split() {
    let mut terrain = TerrainLogic::new();
    let map_data = crate::system::map_loader::MapData {
        heightmap: vec![0, 100, 200, 255],
        width: 2,
        height: 2,
        border_size: 0,
        boundaries: vec![],
        bridges: vec![],
        water_height: None,
        waypoints: vec![],
        waypoint_links: vec![],
        polygon_triggers: vec![],
        texture_tiles: vec![],
    };
    terrain.load_map_data(map_data);

    let h = terrain.get_ground_height(5.0, 5.0, None);
    let p0 = 0.0;
    let p1 = 100.0;
    let p2 = 255.0;
    let p3 = 200.0;
    let fx = 0.5;
    let fy = 0.5;
    let expected = if fy > fx {
        p3 + (1.0 - fy) * (p0 - p3) + fx * (p2 - p3)
    } else {
        p1 + fy * (p2 - p1) + (1.0 - fx) * (p0 - p1)
    } * MAP_HEIGHT_SCALE;

    assert!(
        (h - expected).abs() < 0.01,
        "Triangle interpolation mismatch: got {}, expected {}",
        h,
        expected
    );
}

#[test]
fn ground_height_with_border_offset() {
    let mut terrain = TerrainLogic::new();
    let mut heightmap = vec![0u8; 49];
    for i in 0..7 {
        for j in 0..7 {
            heightmap[i * 7 + j] = 128;
        }
    }
    let map_data = crate::system::map_loader::MapData {
        heightmap,
        width: 7,
        height: 7,
        border_size: 1,
        boundaries: vec![],
        bridges: vec![],
        water_height: None,
        waypoints: vec![],
        waypoint_links: vec![],
        polygon_triggers: vec![],
        texture_tiles: vec![],
    };
    terrain.load_map_data(map_data);

    let h = terrain.get_ground_height(10.0, 10.0, None);
    let expected = 128.0 * MAP_HEIGHT_SCALE;
    assert!(
        (h - expected).abs() < 0.1,
        "Height with border offset: got {}, expected {}",
        h,
        expected
    );
}

#[test]
fn set_raw_map_height_applies_border_offset_like_cpp() {
    let _test_lock = crate::test_sync::lock();
    // C++ W3DTerrainVisual::setRawMapHeight (W3DTerrainVisual.cpp:918-923)
    // adds getBorderSizeInline() before indexing the full height buffer.
    let mut heightmap = vec![200u8; 6 * 6];
    let mut map_data = map_data_with_heightmap(4, 4, heightmap);
    map_data.border_size = 1;
    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map_data);

    terrain.set_raw_map_height(0, 0, 50);
    assert_eq!(
        terrain.get_raw_map_height(0, 0),
        50,
        "playable (0,0) must read the border-offset cell"
    );
    assert_eq!(
        terrain.map_data[1 + 1 * 6],
        50,
        "set_raw_map_height(0,0) must write buffer (border,border)"
    );
    assert_eq!(
        terrain.map_data[0], 200,
        "border cell (0,0) must stay untouched when writing playable (0,0)"
    );
}

#[test]
fn set_raw_map_height_notifies_visual_static_lighting_like_cpp() {
    let _test_lock = crate::test_sync::lock();
    // C++ W3DTerrainVisual::setRawMapHeight (W3DTerrainVisual.cpp:923-924)
    // writes the golden logic map then calls staticLightingChanged().
    use std::sync::atomic::{AtomicU32, Ordering};
    static CALLS: AtomicU32 = AtomicU32::new(0);
    CALLS.store(0, Ordering::SeqCst);
    crate::helpers::register_terrain_visual_raw_height_hook(Some(|x, y, height| {
        CALLS.fetch_add(1, Ordering::SeqCst);
        assert_eq!((x, y, height), (0, 0, 50));
    }));

    let heightmap = vec![200u8; 6 * 6];
    let mut map_data = map_data_with_heightmap(4, 4, heightmap);
    map_data.border_size = 1;
    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map_data);
    terrain.set_raw_map_height(0, 0, 50);
    crate::helpers::register_terrain_visual_raw_height_hook(None);
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn load_post_process_deletes_bridges_whose_object_id_is_gone() {
    // C++ TerrainLogic::loadPostProcess (TerrainLogic.cpp:2994-3009)
    let bridge_info =
        TerrainLogic::bridge_info_from_parts(Coord3D::new(10.0, 20.0, 0.0), 0.0, 6.0, 2.0, 42);
    let mut terrain = TerrainLogic::new();
    let mut bridge = Box::new(Bridge::new(bridge_info, AsciiString::from("OrphanBridge")));
    bridge.set_layer(PathfindLayerEnum::Bridge1);
    terrain.bridge_list_head = Some(bridge);

    assert!(terrain.get_first_bridge().is_some());
    terrain.load_post_process();
    assert!(
        terrain.get_first_bridge().is_none(),
        "orphan bridgeObjectID must be pruned after loadPostProcess"
    );
}

#[test]
fn get_ground_height_normal_uses_cpp_12_neighbor_smoothing() {
    // C++ BaseHeightMap.cpp:914-970. A left-neighbor step changes deltaZ_X
    // under the 12-point filter; the old 2-sample (p1-p0) path stays flat.
    let mut heightmap = vec![100u8; 5 * 5];
    heightmap[0 + 1 * 5] = 50; // d11 at (ix-1, iy) when sampling cell (1,1)
    let map_data = map_data_with_heightmap(5, 5, heightmap);
    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map_data);

    let mut normal = Coord3D::new(0.0, 0.0, 0.0);
    let _h = terrain.get_ground_height(MAP_XY_FACTOR, MAP_XY_FACTOR, Some(&mut normal));
    assert!(
        normal.x.abs() > 0.05,
        "12-neighbor deltaZ_X must tilt the normal, got {normal:?}"
    );
    assert!(normal.z > 0.0, "smoothed normal should still point up");
    let len = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();
    assert!(
        (len - 1.0).abs() < 0.01,
        "normal must be unit length, len={len}"
    );
}

#[test]
fn ground_height_clamped_at_edges() {
    let mut terrain = TerrainLogic::new();
    let map_data = crate::system::map_loader::MapData {
        heightmap: vec![128; 4],
        width: 2,
        height: 2,
        border_size: 0,
        boundaries: vec![],
        bridges: vec![],
        water_height: None,
        waypoints: vec![],
        waypoint_links: vec![],
        polygon_triggers: vec![],
        texture_tiles: vec![],
    };
    terrain.load_map_data(map_data);

    let h_outside = terrain.get_ground_height(-10.0, -10.0, None);
    assert!(
        h_outside >= 0.0,
        "Out-of-bounds height should be non-negative"
    );
}

#[test]
fn ground_height_normal_computed() {
    let mut terrain = TerrainLogic::new();
    let map_data = crate::system::map_loader::MapData {
        heightmap: vec![0, 0, 0, 255],
        width: 2,
        height: 2,
        border_size: 0,
        boundaries: vec![],
        bridges: vec![],
        water_height: None,
        waypoints: vec![],
        waypoint_links: vec![],
        polygon_triggers: vec![],
        texture_tiles: vec![],
    };
    terrain.load_map_data(map_data);

    let mut normal = Coord3D::new(0.0, 0.0, 0.0);
    let _h = terrain.get_ground_height(5.0, 5.0, Some(&mut normal));
    let len = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();
    assert!(
        (len - 1.0).abs() < 0.01,
        "Normal should be unit length, got len={}",
        len
    );
    assert!(normal.z > 0.0, "Normal should point upward");
}

#[test]
fn load_map_data_registers_water_polygons_and_bridges() {
    // C++ TerrainLogic.cpp:2160 getWaterHandle walks water polygon triggers.
    // C++ TerrainLogic.cpp:1514 / W3DBridgeBuffer.cpp:1059 addBridgeToLogic.
    let mut map_data = map_data_with_heightmap(2, 2, vec![10, 10, 10, 10]);
    map_data.water_height = Some(7.5);
    map_data
        .polygon_triggers
        .push(make_water_trigger(3, "Lake", 12, 0, 0, 40, 40));
    map_data
        .bridges
        .push(crate::system::map_loader::BridgeData::new(
            crate::system::map_loader::Coord3D::new(0.0, 0.0, 20.0),
            crate::system::map_loader::Coord3D::new(40.0, 0.0, 20.0),
            10.0,
            "TestBridge".to_string(),
        ));

    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map_data);

    let handle = terrain
        .get_water_handle(10.0, 10.0)
        .expect("water polygon must enter TerrainLogic");
    assert_eq!(handle.get_current_height(), 12.0);

    let grid = terrain
        .get_water_handle_by_name(&AsciiString::from("Water Grid"))
        .expect("grid water height must come from map data");
    assert_eq!(grid.get_current_height(), 7.5);

    let bridge = terrain
        .get_first_bridge()
        .expect("add_bridge_to_logic must run for map bridges");
    assert_eq!(bridge.get_bridge_template_name().as_str(), "TestBridge");
    assert_eq!(bridge.get_bridge_info().bridge_width, 10.0);
}

#[test]
fn crater_subtracts_in_float_then_truncates_like_cpp() {
    // C++ TerrainLogic.cpp:2875: MAX(1, rawHeight - displacementAmount)
    // raw 10, displacement 2.9 → 7.1 truncates to 7, not 10-2=8.
    assert_eq!(TerrainLogic::crater_raw_target_height(10, 2.9), 7);
    assert_eq!(TerrainLogic::crater_raw_target_height(10, 2.0), 8);
    assert_eq!(TerrainLogic::crater_raw_target_height(1, 2.9), 1);
}

#[test]
fn edge_points_use_active_boundary_extent_not_max_pathfind() {
    // C++ findClosestEdgePoint/findFarthestEdgePoint call getExtent()
    // (W3DTerrainLogic.cpp:176-193 active boundary), not the largest.
    let mut terrain = TerrainLogic::new();
    let mut map_data = map_data_with_heightmap(8, 6, vec![4, 10, 1, 9, 8, 2, 7, 5]);
    map_data.boundaries = vec![ICoord2D::new(3, 4), ICoord2D::new(7, 5)];
    terrain.load_map_data(map_data);

    let near_origin = Coord3D::new(MAP_XY_FACTOR, MAP_XY_FACTOR, 0.0);
    let farthest = terrain.find_farthest_edge_point(&near_origin);
    assert_eq!(farthest.x, 3.0 * MAP_XY_FACTOR);
    assert_eq!(farthest.y, 4.0 * MAP_XY_FACTOR);

    let max_extent = terrain.get_maximum_pathfind_extent();
    assert!(
        (farthest.x - max_extent.hi.x).abs() > 1.0,
        "farthest edge must not use the largest boundary"
    );

    let closest = terrain.find_closest_edge_point(&near_origin);
    let active = terrain.get_extent();
    assert!(
        (closest.x - active.lo.x).abs() < f32::EPSILON
            || (closest.y - active.lo.y).abs() < f32::EPSILON
            || (closest.x - active.hi.x).abs() < f32::EPSILON
            || (closest.y - active.hi.y).abs() < f32::EPSILON
    );
}

#[test]
fn bridge_attack_fallback_looks_up_object_position_surface() {
    let prod = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/terrain/bridges.rs"
    ));
    let i = prod
        .find("pub fn get_bridge_attack_points")
        .expect("get_bridge_attack_points");
    let w = &prod[i..];
    assert!(
        w.contains("find_object_by_id(bridge_id)")
            && w.contains("attack_info.attack_point1 = pos")
            && w.contains("attack_info.attack_point2 = pos"),
        "fallback must use the bridge object position like TerrainLogic.cpp:1936-1937"
    );
    assert!(
            !w.contains("attack_info.attack_point1 = Coord3D::origin();\n        attack_info.attack_point2 = Coord3D::origin();\n        return;"),
            "origin must not be the first fallback"
        );
}

#[test]
fn set_active_boundary_runs_fog_and_ghost_dance_surface() {
    let prod = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/terrain/terrain_ops.rs"
    ));
    let i = prod
        .find("pub fn set_active_boundary")
        .expect("set_active_boundary");
    let w = &prod[i..];
    assert!(
        w.contains("store_fogged_cells(player, true)")
            && w.contains("store_fogged_cells(player, false)")
            && w.contains("restore_fogged_cells(player, false)")
            && w.contains("restore_fogged_cells(player, true)")
            && w.contains("release_partition_data")
            && w.contains("lock_ghost_objects(true)")
            && w.contains("restore_partition_data")
            && w.contains("lock_ghost_objects(false)")
            && w.contains("set_lock_ghost_objects(true)")
            && w.contains("set_lock_ghost_objects(false)")
            && w.contains("add_looker")
            && w.contains("remove_looker"),
        "setActiveBoundary must store/restore fog and lock/release ghosts like TerrainLogic.cpp:2545-2615"
    );
}

#[test]
fn flatten_terrain_box_at_covers_long_axis_corners() {
    // hq-6smw3: C++ GEOMETRY_BOX two-triangle flatten, not a cylinder disk.
    let mut heightmap = vec![200u8; 24 * 24];
    // Raise a ridge along +X so a cylinder of r=15 leaves it, a 40x10 box hits it.
    for x in 16..22 {
        for y in 10..14 {
            heightmap[x + y * 24] = 240;
        }
    }
    let map_data = map_data_with_heightmap(24, 24, heightmap);
    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map_data);
    let before = terrain.get_ground_height(180.0, 120.0, None);
    terrain.flatten_terrain_box_at(120.0, 120.0, 0.0, 70.0, 15.0);
    let after = terrain.get_ground_height(180.0, 120.0, None);
    assert!(
        after + 0.01 < before,
        "box flatten must lower a long-axis cell a cylinder of r=15 would miss: before={before} after={after}"
    );
}
