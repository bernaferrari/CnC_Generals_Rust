//! Receiving projectile initialization and retarget use the same actual owner.
use super::*;

fn world(raw: u8) -> GameLogic {
    let mut world = GameLogic::new();
    world.admit_camera_application_movement(
        game_engine::common::ini::ini_game_data::CameraMovementRules {
            partition_cell_size: 40.0,
            ..Default::default()
        },
    );
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![raw; 1024];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(32, 32)];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .try_load_map_geometry(map)
        .unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object ReceivingGroundProjectile\n Behavior = DumbProjectileBehavior Flight\n FirstHeight = 10\n SecondHeight = 20\n FirstPercentIndent = 0.25\n SecondPercentIndent = 0.75\n FlightPathAdjustDistPerSecond = 60\n End\nEnd\n", "receiving-projectile.ini").unwrap();
    assert!(
        world
            .projectile_definitions
            .admit_definition(
                "ReceivingGroundProjectile",
                parser.get_definition("ReceivingGroundProjectile").unwrap(),
                0.,
            )
            .unwrap()
    );
    let mut target = ThingTemplate::new("LiveProjectileTarget");
    target.add_kind_of(KindOf::Vehicle).set_health(100.);
    let mut target = Object::new(target, ObjectId(42), Team::GLA);
    target.set_position(Vec3::new(120., 20., 30.));
    assert!(target.is_alive());
    world.objects.insert(target.id, target);
    world
}
fn initialize(world: &mut GameLogic) -> ObjectId {
    let mut shot = tests::lifecycle_test_pending_projectile(
        "ReceivingGroundProjectile",
        Some(ObjectId(42)),
        Vec3::new(120., 20., 30.),
    );
    shot.shooter_pos = Vec3::new(20., 20., 30.);
    shot.speed = 30.;
    // Actual synchronous accepted-shot admission, not a second path builder.
    world.materialize_owned_passive_shot(shot, None);
    assert_eq!(world.combat_system.projectiles.len(), 1);
    *world.combat_system.projectiles.keys().next().unwrap()
}
fn peak(world: &GameLogic, id: ObjectId) -> f32 {
    world.combat_system.projectiles[&id]
        .flight_runtime
        .path
        .iter()
        .map(|p| p.y)
        .fold(f32::MIN, f32::max)
}
fn move_target(world: &mut GameLogic) {
    world
        .objects
        .get_mut(&ObjectId(42))
        .unwrap()
        .set_position(Vec3::new(150., 20., 30.));
}

#[test]
fn dumb_projectile_aims_and_retargets_to_box_geometry_center() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "dumb_projectile_aims_and_retargets_to_box_geometry_center",
        || {
            let mut world = world(16);
            let mut template = ThingTemplate::new("TallTarget");
            template.add_kind_of(KindOf::Vehicle).set_health(100.);
            template.geometry_info.geom_type = crate::game_logic::HostGeometryType::Box;
            template.geometry_info.height = 40.;
            template.geometry_info.authored = true;
            let mut target = Object::new(template, ObjectId(42), Team::GLA);
            target.set_position(Vec3::new(120., 20., 30.));
            world.objects.insert(target.id, target);
            let id = initialize(&mut world);
            assert_eq!(world.combat_system.projectiles[&id].flight_runtime.path_end,
                Vec3::new(120., 40., 30.));
            world.objects.get_mut(&ObjectId(42)).unwrap()
                .set_position(Vec3::new(150., 20., 30.));
            world.update_owned_projectile_impacts(1. / 30.);
            assert_eq!(world.combat_system.projectiles[&id].flight_runtime.path_end,
                Vec3::new(122., 40., 30.));
        },
    );
}
#[test]
fn actual_receiving_projectile_initialization_and_retarget_ignore_locked_foreign_terrain() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_receiving_projectile_initialization_and_retarget_ignore_locked_foreign_terrain",
        || {
            let mut a = world(16);
            let mut b = world(80);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let mut foreign_terrain = foreign.services().terrain().write().unwrap();
            let mut map = gamelogic::system::map_loader::MapData::new();
            map.width = 32;
            map.height = 32;
            map.heightmap = vec![240; 1024];
            foreign_terrain.try_load_map_geometry(map).unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                let aid = initialize(&mut a);
                let bid = initialize(&mut b);
                assert_eq!(aid, bid);
                assert!(peak(&b, bid) > peak(&a, aid) + 10.);
                let a_segments = a.combat_system.projectiles[&aid].flight_runtime.path_segments;
                let b_segments = b.combat_system.projectiles[&bid].flight_runtime.path_segments;
                let a_before = a.combat_system.projectiles[&aid]
                    .flight_runtime
                    .path
                    .clone();
                let b_before = b.combat_system.projectiles[&bid]
                    .flight_runtime
                    .path
                    .clone();
                move_target(&mut a);
                move_target(&mut b);
                a.update_owned_projectile_impacts(1. / 30.);
                b.update_owned_projectile_impacts(1. / 30.);
                assert_eq!(a.combat_system.projectiles[&aid].flight_runtime.path_segments, a_segments);
                assert_eq!(b.combat_system.projectiles[&bid].flight_runtime.path_segments, b_segments);
                assert_ne!(
                    a.combat_system.projectiles[&aid].flight_runtime.path,
                    a_before
                );
                assert_ne!(
                    b.combat_system.projectiles[&bid].flight_runtime.path,
                    b_before
                );
                assert!(peak(&b, bid) > peak(&a, aid) + 10.);
                assert_eq!(
                    a.combat_system.projectiles[&aid].flight_runtime.path_end.x,
                    122.
                );
                assert_eq!(
                    b.combat_system.projectiles[&bid].flight_runtime.path_end.x,
                    122.
                );
                // Real encoded arena continuation restores state, not launch callbacks.
                let encoded =
                    serde_json::to_vec(&a.combat_system.live_projectile_arena_snapshot()).unwrap();
                let saved: LiveProjectileArenaSnapshot = serde_json::from_slice(&encoded).unwrap();
                let mut restored = world(16);
                move_target(&mut restored);
                restored
                    .combat_system
                    .restore_live_projectile_arena(saved.validate().unwrap());
                b.reset();
                foreign_terrain
                    .try_apply_logic_height_map_bytes_with_effects(&[8; 1024], &mut || {})
                    .unwrap();
                a.update_owned_projectile_impacts(1. / 30.);
                restored.update_owned_projectile_impacts(1. / 30.);
                assert_eq!(
                    a.combat_system.projectiles[&aid].flight_runtime.path,
                    restored.combat_system.projectiles[&aid].flight_runtime.path,
                );
                assert_eq!(
                    a.combat_system.projectiles[&aid].position,
                    restored.combat_system.projectiles[&aid].position
                );
            });
        },
    );
}

#[test]
fn authored_shot_uses_off_chord_peak_in_visited_partition_cell() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_shot_uses_off_chord_peak_in_visited_partition_cell",
        || {
            let mut flat = world(16);
            let mut peaked = world(16);
            {
                let owner = peaked.world_services.terrain();
                let mut terrain = owner.write().unwrap();
                let mut heights = vec![16; 1024];
                heights[4 * 32 + 4] = 160;
                terrain.try_apply_logic_height_map_bytes_with_effects(&heights, &mut || {}).unwrap();
            }
            peaked.initialize_receiving_partition_grid().unwrap();
            let flat_id = initialize(&mut flat);
            let peaked_id = initialize(&mut peaked);
            assert!(peak(&peaked, peaked_id) > peak(&flat, flat_id) + 10.);
        },
    );
}

#[test]
fn accepted_shot_requires_a_visited_receiving_cell() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "accepted_shot_requires_a_visited_receiving_cell",
        || {
            let mut world = world(16);
            let mut crossing = tests::lifecycle_test_pending_projectile(
                "ReceivingGroundProjectile", None, Vec3::new(20., 20., 30.),
            );
            crossing.shooter_pos = Vec3::new(-60., 20., 30.);
            crossing.speed = 30.;
            world.materialize_owned_passive_shot(crossing, None);
            assert_eq!(world.combat_system.projectiles.len(), 1);

            let mut outside = tests::lifecycle_test_pending_projectile(
                "ReceivingGroundProjectile", None, Vec3::new(-40., 20., -30.),
            );
            outside.shooter_pos = Vec3::new(-80., 20., -30.);
            outside.speed = 30.;
            world.materialize_owned_passive_shot(outside, None);
            assert_eq!(world.combat_system.projectiles.len(), 1);
        },
    );
}

#[test]
fn retarget_failure_detonates_before_advancing_when_grid_is_replaced() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "retarget_failure_detonates_before_advancing_when_grid_is_replaced",
        || {
            let mut world = world(16);
            let id = initialize(&mut world);
            {
                let mut shroud = world.world_services.shroud().lock().unwrap();
                shroud.init_shroud_grid_at_origin_with_cell_size([400., 400.], 80., 80., 40.).unwrap();
                world.partition_manager.configure_grid_from_shroud(&shroud);
            }
            world.rebuild_receiving_partition_terrain();
            move_target(&mut world);
            let before = world.combat_system.projectiles[&id].position;
            let (_, retired) = world.combat_system.prepare_projectile_impact_with_runtime(
                id, 1. / 30., &mut world.objects, None, world.frame, None,
                Some(&world.partition_manager), &mut world.ownership_events, 0, None, &mut (),
            );
            assert_eq!(retired, vec![id]);
            assert_eq!(world.combat_system.projectiles[&id].position, before);
        },
    );
}

#[test]
fn authored_shot_uses_receiving_grid_with_nonzero_origin() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_shot_uses_receiving_grid_with_nonzero_origin",
        || {
            let mut a = world(16);
            let mut b = world(16);
            let mut heights = vec![16; 1024];
            heights[4 * 32 + 4] = 160;
            b.world_services.terrain().write().unwrap()
                .try_apply_logic_height_map_bytes_with_effects(&heights, &mut || {}).unwrap();
            for world in [&mut a, &mut b] {
                {
                    let mut shroud = world.world_services.shroud().lock().unwrap();
                    shroud.init_shroud_grid_at_origin_with_cell_size([20., 20.], 120., 120., 40.).unwrap();
                    world.partition_manager.configure_grid_from_shroud(&shroud);
                }
                world.rebuild_receiving_partition_terrain();
            }
            let aid = initialize(&mut a);
            let bid = initialize(&mut b);
            assert!(peak(&b, bid) > peak(&a, aid) + 10.);
        },
    );
}

#[test]
fn fractional_partition_cell_uses_source_float_step_loop() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "fractional_partition_cell_uses_source_float_step_loop",
        || {
            let mut flat = world(0);
            let mut peaked = world(0);
            let mut heights = vec![0; 1024];
            heights[7 * 32 + 7] = 255;
            peaked.world_services.terrain().write().unwrap()
                .try_apply_logic_height_map_bytes_with_effects(&heights, &mut || {}).unwrap();
            for world in [&mut flat, &mut peaked] {
                world.admit_camera_application_movement(
                    game_engine::common::ini::ini_game_data::CameraMovementRules {
                        partition_cell_size: 61.0,
                        ..Default::default()
                    },
                );
                world.initialize_receiving_partition_grid().unwrap();
                let mut shot = tests::lifecycle_test_pending_projectile(
                    "ReceivingGroundProjectile", None, Vec3::new(50., 0., 10.),
                );
                shot.shooter_pos = Vec3::new(10., 0., 10.);
                shot.speed = 30.;
                world.materialize_owned_passive_shot(shot, None);
            }
            let flat = flat.combat_system.projectiles.values().next().unwrap();
            let peaked = peaked.combat_system.projectiles.values().next().unwrap();
            // Seven float additions of 61/7 overshoot 61. The source loop
            // stops at the sixth sample; forcing the endpoint at (61,61)
            // would interpolate the height at (70,70) into this cell.
            assert_eq!(flat.flight_runtime.path, peaked.flight_runtime.path);
        },
    );
}
