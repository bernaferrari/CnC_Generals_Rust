//! Actual Main group requests preserve selected-layer height without Core discovery.
use crate::game_logic::pathfinding::GridPos;
use crate::game_logic::{GameLogic, ObjectId, Team};
use glam::Vec3;

fn member_world() -> GameLogic {
    game_engine::common::ini::ini_locomotor::load_locomotors_from_str(
        "Locomotor HqGroupHeightLocomotor\n  Surfaces = GROUND\n  Speed = 30\n  Appearance = TREADS\nEnd\n",
    ).unwrap();
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(
        "Object HqGroupHeightMember\n  KindOf = VEHICLE SELECTABLE\n  Geometry = CYLINDER\n  GeometryMajorRadius = 1\n  Body = ActiveBody ModuleTag_Body\n    MaxHealth = 100\n  End\n  Behavior = AIUpdateInterface ModuleTag_AI\n  End\n  Locomotor = SET_NORMAL HqGroupHeightLocomotor\nEnd\n",
        "hq_group_height.ini",
    ).unwrap();
    let mut world = GameLogic::new();
    world.templates.insert(
        "HqGroupHeightMember".into(),
        GameLogic::build_template_from_object_definition(
            "HqGroupHeightMember",
            parser.get_definition("HqGroupHeightMember").unwrap(),
            None,
        ),
    );
    world
}

fn map_bytes(raw: u8, deck: Option<f32>, slope: bool) -> Vec<u8> {
    let mut out = game_engine::common::system::DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        out.write_int(value);
    }
    for _y in 0..21 {
        for x in 0..21 {
            out.write_byte(if slope { x * 8 } else { raw });
        }
    }
    out.close_data_chunk();
    if let Some(deck) = deck {
        out.open_data_chunk("ObjectsList", 3);
        for (x, flags) in [(20.0, 0x10), (80.0, 0x20)] {
            out.open_data_chunk("Object", 3);
            for value in [x, 50.0, deck, 0.0] {
                out.write_real(value);
            }
            out.write_int(flags);
            out.write_ascii_string("HqGroupHeightBridge");
            out.close_data_chunk();
        }
        out.close_data_chunk();
    }
    out.into_ckmp_bytes()
}

fn native_world(
    path: &std::path::Path,
    raw: u8,
    deck: Option<f32>,
    wall: f32,
    slope: bool,
) -> (GameLogic, ObjectId) {
    std::fs::write(path, map_bytes(raw, deck, slope)).unwrap();
    eprintln!("group height fixture constructing {}", path.display());
    let mut world = member_world();
    world.set_ai_definition_base(game_engine::common::ini::AIData {
        wall_height: wall,
        ..Default::default()
    });
    eprintln!("group height fixture admitting {}", path.display());
    assert!(world.load_map(path.to_str().unwrap()));
    eprintln!("group height fixture admitted {}", path.display());
    let raw_height = if slope { 75.0 } else { raw as f32 * 0.625 };
    let id = world
        .create_object(
            "HqGroupHeightMember",
            Team::USA,
            Vec3::new(140.0, raw_height, 140.0),
        )
        .unwrap();
    assert!(
        crate::game_logic::PathfindingGrid::is_doing_ground_movement(
            world.host_object(id).unwrap()
        )
    );
    world.pathfinding_system.grid.add_wall_piece(
        900,
        Vec3::new(50.0, wall, 120.0),
        0.0,
        30.0,
        20.0,
    );
    (world, id)
}

fn with_foreign_held(run: impl FnOnce()) {
    let foreign = gamelogic::system::engine_stores::new_for_world();
    let terrain = foreign.services().terrain().write().unwrap();
    let ai = foreign.ai().write().unwrap();
    gamelogic::system::engine_stores::with_active_stores(&foreign, run);
    drop((terrain, ai));
}

fn assert_member_heights(world: &mut GameLogic, id: ObjectId, raw: f32, deck: f32, wall: f32) {
    for (destination, expected_layer, expected_height) in [
        (Vec3::new(140.0, raw, 140.0), 1, raw),
        (Vec3::new(50.0, deck, 50.0), 2, deck),
        (Vec3::new(50.0, wall, 120.0), 15, wall),
    ] {
        assert_eq!(
            world
                .pathfinding_system
                .grid
                .layer_for_destination(destination) as u8,
            expected_layer
        );
        eprintln!("group height request layer{expected_layer} height{expected_height}");
        let adjusted = world.adjust_group_member_goal(id, destination, destination);
        eprintln!("group height resolved layer{expected_layer}: {adjusted:?}");
        assert_eq!(
            adjusted.y, expected_height,
            "actual owned layer {expected_layer}"
        );
        let grid = &world.pathfinding_system.grid;
        assert_eq!(
            world.host_object(id).unwrap().pathfind_goal_cell,
            {
                let c = grid.world_to_grid(adjusted);
                (c.x, c.y)
            },
            "group goal receipt uses the actual adjusted destination"
        );
    }
}

#[test]
fn main_group_goals_use_driving_ground_bridge_and_wall_with_foreign_ai_held() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "main_group_goals_use_driving_ground_bridge_and_wall_with_foreign_ai_held",
        || {
            let temp = tempfile::tempdir().unwrap();
            let (mut first, id) =
                native_world(&temp.path().join("A.map"), 5, Some(21.0), 21.0, false);
            let (mut other, other_id) =
                native_world(&temp.path().join("B.map"), 12, Some(55.0), 55.0, false);
            assert_eq!(id, other_id);
            with_foreign_held(|| {
                assert_member_heights(&mut first, id, 3.125, 21.0, 21.0);
                assert_member_heights(&mut other, other_id, 7.5, 55.0, 55.0);
                eprintln!("group height resetting B");
                other.reset();
                eprintln!("group height reset B; constructing inert owner");
                let _inert = GameLogic::new();
                eprintln!("group height inert owner constructed; rereading A");
                assert_member_heights(&mut first, id, 3.125, 21.0, 21.0);
            });
        },
    );
}

#[test]
fn group_selected_layer_clips_footprints_and_excludes_unlinked_or_buried_decks() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "group_selected_layer_clips_footprints_and_excludes_unlinked_or_buried_decks",
        || {
            let temp = tempfile::tempdir().unwrap();
            let (mut world, _) =
                native_world(&temp.path().join("Clip.map"), 5, Some(21.0), 55.0, false);
            let grid = &mut world.pathfinding_system.grid;
            let bridge = Vec3::new(50.0, 99.0, 50.0);
            let wall = Vec3::new(50.0, 99.0, 120.0);
            let outside = Vec3::new(150.0, 99.0, 150.0);
            with_foreign_held(|| {
                assert_eq!(grid.layer_height_clipped(bridge, 2, 3.125), 21.0);
                assert_eq!(grid.layer_height_clipped(wall, 15, 3.125), 55.0);
                assert_eq!(grid.layer_height_clipped(outside, 2, 3.125), 3.125);
                assert_eq!(grid.layer_height_clipped(outside, 15, 3.125), 3.125);
                assert_eq!(
                    grid.layer_height_clipped(bridge, 2, 40.0),
                    40.0,
                    "buried deck does not replace raw terrain"
                );
                assert_eq!(grid.layer_height_clipped(outside, 1, 0.0), 0.0);
                assert_eq!(grid.layer_height_clipped(outside, 1, -7.0), -7.0);
                grid.deactivate_bridge_layer(2);
                assert_eq!(
                    grid.layer_height_clipped(bridge, 2, 3.125),
                    3.125,
                    "unlink removes selected layer's geometry"
                );
            });
        },
    );
}

#[test]
fn group_goal_samples_final_adjusted_xy_on_authored_cpp_height_plane() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "group_goal_samples_final_adjusted_xy_on_authored_cpp_height_plane",
        || {
            let temp = tempfile::tempdir().unwrap();
            let (mut world, id) = native_world(&temp.path().join("Slope.map"), 0, None, 0.0, true);
            let requested = Vec3::new(83.0, 0.0, 80.0);
            with_foreign_held(|| {
                let adjusted = world.adjust_group_member_goal(id, requested, requested);
                assert_ne!(
                    adjusted.x, requested.x,
                    "fixture exercises actual cell adjustment"
                );
                // Bytes rise8/sample, source scale10, height0.625, border1.
                // Thus the independently authored world plane is y=0.5*x+5.
                // The admitted Rust heightmap stores byte/255 before the original
                // lower-triangle arithmetic and rescales by255*0.625. Pin
                // that exact f32 path independently of the production sampler.
                let x = adjusted.x / 10.0;
                let fx = x - x.floor();
                let p0 = ((x.floor() + 1.0) * 8.0) / 255.0;
                let p1 = ((x.floor() + 2.0) * 8.0) / 255.0;
                let encoded_height = (p1 + (1.0 - fx) * (p0 - p1)) * (255.0 * 0.625);
                assert_eq!(adjusted.y, encoded_height);
                // On this exact plane/cell, normalizing the integer samples
                // introduces one f32 ULP versus original byte arithmetic.
                let analytic_height = 0.5 * adjusted.x + 5.0;
                assert!(adjusted.y.to_bits().abs_diff(analytic_height.to_bits()) <= 1);
                assert_ne!(
                    adjusted.y,
                    0.5 * requested.x + 5.0,
                    "pre-adjustment sampling must fail"
                );
            });
        },
    );
}

#[test]
fn group_goal_accepts_owned_zero_and_negative_cache_without_ambient_fallback() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "group_goal_accepts_owned_zero_and_negative_cache_without_ambient_fallback",
        || {
            let mut world = member_world();
            let id = world
                .create_object("HqGroupHeightMember", Team::USA, Vec3::ZERO)
                .unwrap();
            let width = world.pathfinding_system.grid.width();
            let height = world.pathfinding_system.grid.height();
            with_foreign_held(|| {
                for raw in [-7.0, 0.0] {
                    world.pathfinding_system.set_terrain_height_samples(
                        width,
                        height,
                        vec![raw; (width * height) as usize],
                    );
                    let requested = Vec3::new(80.0, raw, 80.0);
                    let adjusted = world.adjust_group_member_goal(id, requested, requested);
                    assert_eq!(
                        adjusted.y, raw,
                        "zero and negative samples are actual content"
                    );
                }
            });
        },
    );
}

#[test]
fn logical_extent_queue_admission_uses_driving_active_boundary_and_constructor_is_inert() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "logical_extent_queue_admission_uses_driving_active_boundary_and_constructor_is_inert",
        || {
            fn admit_boundary(world: &mut GameLogic, cells: i32) {
                let mut data = gamelogic::system::map_loader::MapData::new();
                data.width = 32;
                data.height = 32;
                data.heightmap = vec![0; 32 * 32];
                data.boundaries = vec![gamelogic::common::ICoord2D::new(cells, cells)];
                world
                    .world_services
                    .terrain()
                    .write()
                    .unwrap()
                    .load_map_geometry(data);
            }
            let mut first = GameLogic::new();
            let mut other = GameLogic::new();
            first.override_world_size(320.0, 320.0);
            other.override_world_size(320.0, 320.0);
            admit_boundary(&mut first, 12);
            admit_boundary(&mut other, 25);
            with_foreign_held(|| {
                let inert = GameLogic::new();
                let grid = &inert.pathfinding_system.grid;
                assert!(grid.in_logical_extent(GridPos::new(0, 0)));
                assert!(grid.in_logical_extent(GridPos::new(grid.width() - 1, grid.height() - 1)));
                first.process_pathfind_queue();
                other.process_pathfind_queue();
                assert!(
                    first
                        .pathfinding_system
                        .grid
                        .in_logical_extent(GridPos::new(11, 11))
                );
                assert!(
                    !first
                        .pathfinding_system
                        .grid
                        .in_logical_extent(GridPos::new(12, 12))
                );
                assert!(
                    other
                        .pathfinding_system
                        .grid
                        .in_logical_extent(GridPos::new(24, 24))
                );
                admit_boundary(&mut first, 6);
                first.process_pathfind_queue();
                assert!(
                    first
                        .pathfinding_system
                        .grid
                        .in_logical_extent(GridPos::new(5, 5))
                );
                assert!(
                    !first
                        .pathfinding_system
                        .grid
                        .in_logical_extent(GridPos::new(6, 6))
                );
                assert!(
                    other
                        .pathfinding_system
                        .grid
                        .in_logical_extent(GridPos::new(24, 24))
                );
            });
        },
    );
}
