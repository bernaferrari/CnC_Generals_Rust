//! Actual Main locomotor boundary controls; no additional process/test locks.
use super::*;
use crate::game_logic::{GameLogic, LocomotorAppearance, LocomotorBehaviorZ, ObjectId, Team};
use game_engine::common::ini::{INI, ini_ai_data::AIDataStore};
use std::sync::{Arc, RwLock};

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, || {
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::game_logic::GameWorldAuthority::DEFAULT_OFF,
            run,
        );
    });
}

fn world(wall: f32, raw_ground: f32) -> (GameLogic, ObjectId) {
    let draft = Arc::new(RwLock::new(AIDataStore::default()));
    let mut ini = INI::new();
    ini.set_ai_data_store_target(Arc::clone(&draft));
    ini.with_inline_source(&format!("AIData\n WallHeight = {wall}\n End\n"), |ini| {
        ini.parse_current_file()
    })
    .unwrap();
    drop(ini);
    let data = Arc::try_unwrap(draft)
        .unwrap()
        .into_inner()
        .unwrap()
        .get_active()
        .unwrap()
        .clone();
    game_engine::common::ini::ini_locomotor::load_locomotors_from_str(
        r#"
Locomotor HqFlightOwnerHover
  Surfaces = AIR
  Speed = 30
  Acceleration = 100
  Appearance = HOVER
  ZAxisBehavior = RELATIVE_TO_HIGHEST_LAYER
  PreferredHeight = 10
  PreferredHeightDamping = 1
  Lift = 90000
End
"#,
    )
    .unwrap();
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object AmericaVehicleComanche
  KindOf = VEHICLE AIRCRAFT PRODUCED_AT_HELIPAD
  Geometry = CYLINDER
  GeometryMajorRadius = 8
  GeometryHeight = 12
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 220
  End
  Behavior = AIUpdateInterface ModuleTag_AI
  End
  Locomotor = SET_NORMAL HqFlightOwnerHover
End
"#,
            "hq_flight_owner.ini",
        )
        .unwrap();
    let mut world = GameLogic::new();
    world.set_ai_definition_base(data);
    world.override_world_size(200.0, 200.0);
    world.templates.insert(
        "AmericaVehicleComanche".into(),
        GameLogic::build_template_from_object_definition(
            "AmericaVehicleComanche",
            parser.get_definition("AmericaVehicleComanche").unwrap(),
            None,
        ),
    );
    let width = world.pathfinding_system.grid.width() as u32;
    let height = world.pathfinding_system.grid.height() as u32;
    assert!(world.restore_terrain_heights_from_grid(
        width,
        height,
        &vec![raw_ground; (width * height) as usize]
    ));
    assert_eq!(
        world.pathfinding_system.wall_height(),
        wall,
        "explicit authored definition admission"
    );
    world
        .pathfinding_system
        .grid
        .add_wall_piece(9982, Vec3::ZERO, 0.0, 50.0, 50.0);
    let id = world
        .create_object(
            "AmericaVehicleComanche",
            Team::USA,
            Vec3::new(5.0, 60.0, 5.0),
        )
        .unwrap();
    assert_eq!(
        world.objects[&id].loco_behavior_z,
        LocomotorBehaviorZ::SmoothRelativeToHighestLayer
    );
    (world, id)
}

fn step(world: &mut GameLogic, id: ObjectId) {
    world.update_movement_locomotor_pass(&[id], 1.0 / 30.0);
}

#[test]
fn real_locomotor_uses_same_id_world_wall_rules_while_foreign_core_ai_is_held() {
    isolated(
        "real_locomotor_uses_same_id_world_wall_rules_while_foreign_core_ai_is_held",
        || {
            let (mut first, id) = world(21.0, 3.0);
            let (mut second, second_id) = world(55.0, 7.0);
            assert_eq!(id, second_id);
            let (mut reference, reference_id) = world(21.0, 3.0);
            assert_eq!(id, reference_id);
            step(&mut reference, id);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let foreign_ai = foreign.ai();
            let held = foreign_ai.write().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                step(&mut second, id);
                step(&mut first, id);
                assert_eq!(
                    first.objects[&id].get_position(),
                    reference.objects[&id].get_position()
                );
                assert_eq!(
                    first.objects[&id].movement.velocity,
                    reference.objects[&id].movement.velocity
                );
                assert!(
                    first.objects[&id].get_position().y < 60.0,
                    "21+10 is below the flyer"
                );
                assert!(
                    second.objects[&id].get_position().y > 60.0,
                    "55+10 is above the flyer"
                );
            });
            drop(held);
            second.reset();
            let unrelated = GameLogic::new();
            drop(unrelated);
            step(&mut first, id);
            step(&mut reference, id);
            assert_eq!(
                first.objects[&id].get_position(),
                reference.objects[&id].get_position()
            );
        },
    );
}

#[test]
fn authored_wall_height_does_not_require_the_duplicate_core_ai_projection() {
    isolated(
        "authored_wall_height_does_not_require_the_duplicate_core_ai_projection",
        || {
            let (mut first, id) = world(21.0, 3.0);
            let (mut second, second_id) = world(55.0, 7.0);
            assert_eq!(id, second_id);
            let empty = gamelogic::system::engine_stores::new_for_world();
            assert_eq!(empty.ai().read().unwrap().get_ai_data().wall_height, 0.0);
            gamelogic::system::engine_stores::with_active_stores(&empty, || {
                step(&mut first, id);
                step(&mut second, id);
            });
            assert!(first.objects[&id].get_position().y < 60.0);
            assert!(
                second.objects[&id].get_position().y > 60.0,
                "authored 55+10 cruise height must not become Core's default zero wall"
            );
        },
    );
}

#[test]
fn live_z_resolves_post_motion_position_and_retains_current_layer_unclipped() {
    isolated(
        "live_z_resolves_post_motion_position_and_retains_current_layer_unclipped",
        || {
            let (mut world, id) = world(21.0, 3.0);
            let corners = [
                Vec3::new(30.0, 40.0, -20.0),
                Vec3::new(70.0, 40.0, -20.0),
                Vec3::new(30.0, 60.0, 20.0),
                Vec3::new(70.0, 60.0, 20.0),
            ];
            world
                .pathfinding_system
                .grid
                .stamp_bridge_deck(corners[0], corners[1], corners[2], corners[3], false);
            let grid = &world.pathfinding_system.grid;
            let view = FlightTerrainView {
                grid,
                terrain: world.terrain.as_ref(),
                samples: world.pathfinding_height_samples.as_ref(),
            };
            let object = world.objects.get_mut(&id).unwrap();
            object.set_position(Vec3::new(5.0, 80.0, 5.0));
            let before = view.highest_surface(object);
            assert_eq!(before, 21.0);
            // Actual Wings appearance movement precedes the exact live Z routine.
            object.loco_appearance = LocomotorAppearance::Wings;
            object.min_speed = 45.0;
            object.movement.velocity = Vec3::new(45.0, 0.0, 0.0);
            object.movement.turn_rate = 0.0;
            object.status.airborne_target = true;
            object.apply_motive_force(Vec3::ZERO);
            object.loco_maintain_appearance(1.0);
            let moved = object.get_position();
            assert!(
                moved.x >= 30.0 && moved.x <= 70.0,
                "Wings crossed the deck edge: {moved:?}"
            );
            let after = view.highest_surface(object);
            assert!(after > before);
            let mut expected = object.clone();
            // Independent resolved observation: call Object's force algorithm
            // and Physics Euler explicitly, without the live query adapter.
            expected.handle_behavior_z_with_highest_surface(before, None, after);
            expected.apply_gravitational_forces();
            expected.movement.velocity.y += expected.physics_accel.y;
            expected.physics_accel.y = 0.0;
            let mut expected_pos = expected.get_position();
            expected_pos.y += expected.movement.velocity.y;
            expected.set_position(expected_pos);
            GameLogic::apply_live_handle_behavior_z(object, before, None, Some(&view));
            assert_eq!(object.get_position(), expected.get_position());
            assert_eq!(object.movement.velocity, expected.movement.velocity);
            object.pathfind_layer = 2;
            object.set_position(Vec3::new(90.0, 100.0, 40.0));
            assert_eq!(
                view.highest_surface(object),
                70.0,
                "current bridge plane survives outside bounds"
            );
            object.pathfind_layer = 15;
            assert_eq!(
                view.highest_surface(object),
                21.0,
                "current wall survives outside footprint"
            );
        },
    );
}

#[test]
fn native_save_restores_authored_smooth_flight_and_continues_owned_wall_lift() {
    isolated(
        "native_save_restores_authored_smooth_flight_and_continues_owned_wall_lift",
        || {
            let (mut source, id) = world(55.0, 7.0);
            step(&mut source, id);
            let dir = tempfile::TempDir::new().unwrap();
            let mut manager = crate::save_load::SaveFileManager::with_save_directory(dir.path());
            manager.init().unwrap();
            manager.quick_save(&source).unwrap();
            let (decoded, _) = manager.load_game_snapshot("quicksave").unwrap();
            // Native runtime loads into the already authored map candidate.
            let (mut restored, restored_id) = world(55.0, 7.0);
            assert_eq!(restored_id, id);
            manager
                .restore_game_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(
                restored.objects[&id].loco_behavior_z,
                source.objects[&id].loco_behavior_z
            );
            assert_eq!(restored.pathfinding_system.wall_height(), 55.0);
            for _ in 0..4 {
                step(&mut source, id);
                step(&mut restored, id);
                assert_eq!(
                    restored.objects[&id].get_position(),
                    source.objects[&id].get_position()
                );
                assert_eq!(
                    restored.objects[&id].movement.velocity,
                    source.objects[&id].movement.velocity
                );
            }
        },
    );
}

#[test]
fn authored_map_raw_height_is_owned_and_uses_the_cpp_triangle_on_both_backends() {
    isolated(
        "authored_map_raw_height_is_owned_and_uses_the_cpp_triangle_on_both_backends",
        || {
            let dir = tempfile::TempDir::new().unwrap();
            let mut worlds = Vec::new();
            for index in 0..2 {
                // CPP requires an interior cell: ix/iy >= 1 and <= extent-3.
                // Border 1 places world (2.5,7.5) in the authored cell (1,1).
                let factor = index as u8 + 1;
                let mut heights = [0u8; 25];
                heights[7] = 16 * factor;
                heights[11] = 32 * factor;
                heights[12] = 64 * factor;
                // Distinct edge samples prove the original single-sample
                // clipping branch rather than interpolation at the boundary.
                heights[8] = 80 * factor;
                heights[9] = 112 * factor;
                heights[5] = 48 * factor;
                let folder = dir.path().join(index.to_string());
                std::fs::create_dir(&folder).unwrap();
                let path = folder.join("Flight.map");
                let mut output = game_engine::common::system::DataChunkOutput::new();
                output.open_data_chunk("HeightMapData", 3);
                output.write_int(5);
                output.write_int(5);
                output.write_int(1);
                output.write_int(25);
                for h in heights {
                    output.write_byte(h);
                }
                output.close_data_chunk();
                std::fs::write(&path, output.into_ckmp_bytes()).unwrap();
                let (mut world, _) = world(21.0, 3.0);
                assert!(world.load_map(path.to_str().unwrap()));
                assert!(
                    world.terrain.is_some(),
                    "actual map admission retains owned raw heights"
                );
                let pos = Vec3::new(2.5, 60.0, 7.5);
                let view = FlightTerrainView {
                    grid: &world.pathfinding_system.grid,
                    terrain: world.terrain.as_ref(),
                    samples: world.pathfinding_height_samples.as_ref(),
                };
                assert!(
                    (view.raw_ground(pos, 999.0) - 20.0 * (index as f32 + 1.0)).abs() < 2.0e-5,
                    "raw map triangle, not bilinear or object deck fallback"
                );
                assert!(
                    (view.raw_ground(Vec3::new(21.0, 60.0, 2.5), 999.0)
                        - 50.0 * (index as f32 + 1.0))
                        .abs()
                        < 2.0e-5,
                    "CPP clips the outer cell to its raw sample"
                );
                assert!(
                    (view.raw_ground(Vec3::new(-21.0, 60.0, 2.5), 999.0)
                        - 30.0 * (index as f32 + 1.0))
                        .abs()
                        < 2.0e-5,
                    "CPP GetClipHeight clamps a negative index to the edge sample"
                );
                worlds.push((world, pos));
            }
            worlds[1].0.reset();
            let (first, pos) = &worlds[0];
            let view = FlightTerrainView {
                grid: &first.pathfinding_system.grid,
                terrain: first.terrain.as_ref(),
                samples: first.pathfinding_height_samples.as_ref(),
            };
            assert!((view.raw_ground(*pos, 999.0) - 20.0).abs() < 2.0e-5);
        },
    );
}

#[test]
fn real_same_id_bridge_lift_is_isolated_from_other_worlds_and_held_core_ai() {
    isolated(
        "real_same_id_bridge_lift_is_isolated_from_other_worlds_and_held_core_ai",
        || {
            fn bridge(world: &mut GameLogic, height: f32) {
                world.pathfinding_system.grid.stamp_bridge_deck(
                    Vec3::new(-40.0, height, -40.0),
                    Vec3::new(40.0, height, -40.0),
                    Vec3::new(-40.0, height, 40.0),
                    Vec3::new(40.0, height, 40.0),
                    false,
                );
            }
            let (mut first, id) = world(0.0, 3.0);
            let (mut second, second_id) = world(0.0, 7.0);
            let (mut reference, reference_id) = world(0.0, 3.0);
            assert_eq!(id, second_id);
            assert_eq!(id, reference_id);
            bridge(&mut first, 21.0);
            bridge(&mut second, 55.0);
            bridge(&mut reference, 21.0);
            for world in [&first, &second, &reference] {
                assert_eq!(world.objects[&id].pathfind_layer, 1);
                assert_eq!(world.pathfinding_system.wall_height(), 0.0);
            }
            step(&mut reference, id);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let held = foreign.ai().write().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                step(&mut second, id);
                step(&mut first, id);
                assert_eq!(
                    first.objects[&id].get_position(),
                    reference.objects[&id].get_position()
                );
                assert_eq!(
                    first.objects[&id].movement.velocity,
                    reference.objects[&id].movement.velocity
                );
                assert!(
                    first.objects[&id].get_position().y < 60.0,
                    "owned deck21+10 descends"
                );
                assert!(
                    second.objects[&id].get_position().y > 60.0,
                    "owned deck55+10 rises"
                );
                let candidate = GameLogic::new();
                assert!(Arc::ptr_eq(
                    &gamelogic::system::engine_stores::active(),
                    &foreign
                ));
                drop(candidate);
            });
            drop(held);
            second.reset();
            step(&mut first, id);
            step(&mut reference, id);
            assert_eq!(
                first.objects[&id].get_position(),
                reference.objects[&id].get_position()
            );
            assert_eq!(
                first.objects[&id].movement.velocity,
                reference.objects[&id].movement.velocity
            );
        },
    );
}

#[test]
fn real_wall_factory_preserves_zero_authored_height_and_nonzero_rules() {
    isolated(
        "real_wall_factory_preserves_zero_authored_height_and_nonzero_rules",
        || {
            let mut parser = crate::assets::IniParser::new();
            parser
                .parse_ini_content(
                    r#"
Object HqFlightOwnerWall
  KindOf = STRUCTURE IMMOBILE WALK_ON_TOP_OF_WALL
  Geometry = BOX
  GeometryMajorRadius = 50
  GeometryMinorRadius = 50
  GeometryHeight = 80
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 220
  End
End
"#,
                    "hq_flight_owner_wall.ini",
                )
                .unwrap();
            let template = GameLogic::build_template_from_object_definition(
                "HqFlightOwnerWall",
                parser.get_definition("HqFlightOwnerWall").unwrap(),
                None,
            );
            for height in [0.0, 21.0] {
                let (mut world, _) = world(height, 3.0);
                world
                    .templates
                    .insert(template.name.clone(), template.clone());
                let wall = world
                    .create_object("HqFlightOwnerWall", Team::USA, Vec3::ZERO)
                    .unwrap();
                assert!(
                    world.objects[&wall].is_kind_of(crate::game_logic::KindOf::WalkOnTopOfWall)
                );
                assert_eq!(
                    world.objects[&wall].thing().template.geometry_info.height,
                    80.0
                );
                assert_eq!(
                    world.pathfinding_system.wall_height(),
                    height,
                    "real factory must not replace authored zero by geometry80"
                );
            }
        },
    );
}
