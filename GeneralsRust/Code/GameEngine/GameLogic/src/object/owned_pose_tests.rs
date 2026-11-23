//! Actual Thing.cpp pose/caches, without a registry or a shared clock.
use super::*;

fn inert_object() -> Object {
    let mut object = Object::new_raw(
        Arc::new(DefaultThingTemplate::new("OwnedPose".into())),
        7,
        ObjectStatusMaskType::none(),
        None,
    );
    object.ambient_lifecycle_admitted = false;
    object
}

fn transform(object: &mut Object, matrix: &Matrix3D) {
    object
        .set_snapshot_transform_with_services(
            matrix,
            &crate::terrain::TerrainLogic::new(),
            &mut crate::object::collide::collision_system::CollisionSystem::new(),
            23,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
}

#[test]
fn owned_pose_off_map_uses_receiving_terrain_and_cpp_movement_threshold() {
    fn terrain(size: i32) -> crate::terrain::TerrainLogic {
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 10;
        map.height = 10;
        map.heightmap = vec![0; 100];
        map.boundaries = vec![
            crate::common::ICoord2D::new(10, 10),
            crate::common::ICoord2D::new(size, size),
        ];
        let mut terrain = crate::terrain::TerrainLogic::new();
        terrain.load_map_data(map);
        terrain.restore_active_boundary_from_snapshot(1);
        terrain
    }
    fn move_to(object: &mut Object, terrain: &crate::terrain::TerrainLogic, x: f32) {
        object
            .set_snapshot_transform_with_services(
                &Matrix3D::from_translation(Coord3D::new(x, 25.0, 0.0)),
                terrain,
                &mut crate::object::collide::collision_system::CollisionSystem::new(),
                0,
                &mut |_| {},
                &mut |_| {},
            )
            .unwrap();
    }
    fn off_map(object: &Object) -> bool {
        object.private_status & (ObjectPrivateStatusBits::OffMap as u8) != 0
    }

    let large = terrain(10);
    let small = terrain(5);
    let mut a = inert_object();
    let mut b = inert_object(); // Both objects intentionally have the same ID.
    move_to(&mut a, &large, 99.996);
    move_to(&mut b, &small, 99.996);
    assert!(!off_map(&a));
    assert!(off_map(&b));

    // Object.cpp:1683 and 1752 only recompute OFF_MAP if a component
    // changes by more than 0.01, even when this tiny move crosses the edge.
    move_to(&mut a, &large, 100.004);
    assert!(!off_map(&a));
    move_to(&mut a, &large, 100.02);
    assert!(off_map(&a));
    move_to(&mut b, &small, 25.0);
    assert!(!off_map(&b));
    assert!(off_map(&a));
}

#[test]
fn compatibility_position_rejects_owned_terrain_contention_before_mutating_pose() {
    let stores = crate::system::engine_stores::new_for_world();
    let terrain = Arc::clone(stores.services().terrain());
    {
        let mut terrain = terrain.write().unwrap();
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 10;
        map.height = 10;
        map.heightmap = vec![0; 100];
        map.boundaries = vec![crate::common::ICoord2D::new(10, 10)];
        terrain.load_map_data(map);
    }

    let mut object = inert_object();
    object.bind_owner_world_for_test(stores.services());
    object
        .set_position(&Coord3D::new(99.98, 25.0, 0.0))
        .unwrap();
    assert!(!object.is_off_map());

    let terrain_write = terrain.write().unwrap();
    let result = object.set_position(&Coord3D::new(100.02, 25.0, 0.0));
    assert!(
        result.is_err(),
        "owned terrain contention must reject the move"
    );
    assert_eq!(*object.get_position(), Coord3D::new(99.98, 25.0, 0.0));
    assert!(!object.is_off_map());
    drop(terrain_write);

    object
        .set_position(&Coord3D::new(100.02, 25.0, 0.0))
        .unwrap();
    assert_eq!(*object.get_position(), Coord3D::new(100.02, 25.0, 0.0));
    assert!(object.is_off_map());
}

#[test]
fn slope_position_rejects_owned_terrain_contention_without_reentering_terrain() {
    let stores = crate::system::engine_stores::new_for_world();
    let terrain = Arc::clone(stores.services().terrain());
    {
        let mut terrain = terrain.write().unwrap();
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 10;
        map.height = 10;
        map.heightmap = vec![0; 100];
        map.boundaries = vec![crate::common::ICoord2D::new(10, 10)];
        terrain.load_map_data(map);
    }

    let mut template = DefaultThingTemplate::new("OwnedSlopePose".into());
    template.add_kind_of(KindOf::StickToTerrainSlope);
    let mut object = Object::new_raw(Arc::new(template), 8, ObjectStatusMaskType::none(), None);
    object.ambient_lifecycle_admitted = false;
    object.bind_owner_world_for_test(stores.services());
    object
        .set_position(&Coord3D::new(99.98, 25.0, 0.0))
        .unwrap();
    assert!(!object.is_off_map());

    let terrain_write = terrain.write().unwrap();
    let result = object.set_position(&Coord3D::new(100.02, 25.0, 0.0));
    assert!(
        result.is_err(),
        "slope alignment must reject terrain contention"
    );
    assert!((object.get_position().x - 99.98).abs() <= 0.01);
    assert!(!object.is_off_map());
    drop(terrain_write);

    object
        .set_position(&Coord3D::new(100.02, 25.0, 0.0))
        .unwrap();
    assert!(object.is_off_map());
}

#[test]
fn actual_object_retains_finite_singular_and_sheared_cpp_matrix_rows() {
    let mut object = inert_object();
    // C++ copies the 3x4 rows even if a rotation/scale decomposition is absent.
    let matrix = Matrix3D::from_cols_array(&[
        0.0, 0.0, 0.0, 0.0, 2.0, 3.0, 4.0, 0.0, -1.0, 5.0, 6.0, 0.0, 19.0, -7.0, 11.0, 1.0,
    ]);
    transform(&mut object, &matrix);
    assert_eq!(object.get_transform_matrix(), matrix);
    assert_eq!(*object.get_position(), Coord3D::new(19.0, -7.0, 11.0));
    assert_eq!(object.get_orientation(), 0.0);
    assert_eq!(object.convert_bone_pos_to_world_pos(None, None), matrix);
}

#[test]
fn cpp_position_retains_basis_then_orientation_makes_it_upright() {
    let mut object = inert_object();
    let matrix = Matrix3D::from_cols_array(&[
        2.0, 1.0, 3.0, 0.0, -1.0, 4.0, 5.0, 0.0, 6.0, 7.0, 8.0, 0.0, 10.0, 20.0, 30.0, 1.0,
    ]);
    transform(&mut object, &matrix);
    let position = Coord3D::new(40.0, 50.0, 60.0);
    object.set_position(&position).unwrap();
    let moved = object.get_transform_matrix();
    assert_eq!(moved.x_axis, matrix.x_axis);
    assert_eq!(moved.y_axis, matrix.y_axis);
    assert_eq!(moved.z_axis, matrix.z_axis);
    assert_eq!(moved.w_axis, position.extend(1.0));
    object.set_orientation(0.5).unwrap();
    assert_eq!(
        object.get_transform_matrix(),
        Matrix3D::from_translation(position) * Matrix3D::from_rotation_z(0.5)
    );
    assert_eq!(*object.get_position(), position);
    assert_eq!(object.get_orientation(), 0.5);
}

#[test]
fn cpp_geometry_replacement_keeps_pose_and_full_matrix() {
    let mut object = inert_object();
    object.partition_data = None;
    let matrix = Matrix3D::from_translation(Coord3D::new(7.0, 8.0, 9.0))
        * Matrix3D::from_rotation_x(0.7)
        * Matrix3D::from_rotation_z(0.4);
    transform(&mut object, &matrix);
    let old_angle = object.get_orientation();
    let mut geometry = GeometryInfo::default();
    geometry.shape.set(
        game_engine::common::system::GeometryType::Box,
        geometry.get_is_small(),
        14.0,
        12.0,
        13.0,
    );
    object.set_geometry_info(geometry);
    assert_eq!(object.get_transform_matrix(), matrix);
    assert_eq!(*object.get_position(), Coord3D::new(7.0, 8.0, 9.0));
    assert_eq!(object.get_orientation(), old_angle);
    assert_eq!(
        object.get_geometry_info().bounds().max,
        Coord3D::new(12.0, 13.0, 14.0)
    );
}

#[test]
fn cpp_matrix_angle_keeps_signed_boundary_while_orientation_normalizes_only_cache() {
    let mut object = inert_object();
    let matrix = Matrix3D::from_cols_array(&[
        -1.0, -0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);
    // Matrix3D.cpp226 WWMath::Atan2(Row1[0],Row0[0]) returns -PI.
    transform(&mut object, &matrix);
    assert_eq!(
        object.get_orientation().to_bits(),
        (-std::f32::consts::PI).to_bits()
    );
    let position = Coord3D::new(10.0, 20.0, 0.0);
    object.set_position(&position).unwrap();
    assert_eq!(
        object.get_orientation().to_bits(),
        (-std::f32::consts::PI).to_bits()
    );
    assert_eq!(object.get_transform_matrix().x_axis, matrix.x_axis);
    // Thing.cpp174 uses raw angle for the basis, normalizeAngle for the cache.
    object.set_orientation(-std::f32::consts::PI).unwrap();
    assert_eq!(
        object.get_orientation().to_bits(),
        std::f32::consts::PI.to_bits()
    );
    assert_eq!(
        object.get_transform_matrix(),
        Matrix3D::from_translation(position) * Matrix3D::from_rotation_z(-std::f32::consts::PI)
    );
    assert_eq!(*object.get_position(), position);
}

#[test]
fn cpp_bone_point_and_matrix_are_independent_without_double_translation() {
    let mut object = inert_object();
    let world = Matrix3D::from_rotation_translation(
        glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        Coord3D::new(50.0, 60.0, 0.0),
    );
    transform(&mut object, &world);
    let local = Matrix3D::from_rotation_translation(
        glam::Quat::from_rotation_z(0.3),
        Coord3D::new(-20.0, 0.0, 0.0),
    );
    let point = Coord3D::new(-20.0, 0.0, 0.0);
    assert_eq!(
        object.convert_bone_pos_to_world_pos(Some(&point), Some(&local)),
        world * local
    );
    assert_eq!(
        object.convert_bone_pos_to_world_pos(None, Some(&local)),
        world * local
    );
    let position_only = object.convert_bone_pos_to_world_pos(Some(&point), None);
    assert!((position_only.w_axis.truncate() - world.transform_point3(point)).length() < 1.0e-5);
    assert_eq!(position_only.x_axis, world.x_axis);
    assert_eq!(position_only.y_axis, world.y_axis);
    assert_eq!(position_only.z_axis, world.z_axis);
}

#[test]
fn compatibility_position_caches_height_from_the_receiving_map() {
    let _lock = crate::test_sync::lock();
    fn world_with_height(raw_height: u8) -> Arc<crate::system::engine_stores::EngineStores> {
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 10;
        map.height = 10;
        map.heightmap = vec![raw_height; 100];
        map.boundaries = vec![crate::common::ICoord2D::new(10, 10)];
        let world = crate::system::engine_stores::new_for_world();
        world
            .services()
            .terrain()
            .write()
            .unwrap()
            .load_map_data(map);
        world
    }

    let owner = world_with_height(32);
    let foreign = world_with_height(0);
    let destination = Coord3D::new(25.0, 25.0, 100.0);
    let ground = owner
        .services()
        .terrain()
        .read()
        .unwrap()
        .get_ground_height(destination.x, destination.y, None);
    assert!(ground > 0.0);

    for slope in [false, true] {
        let mut template = DefaultThingTemplate::new("ReceivingPoseHeight".into());
        if slope {
            template.add_kind_of(KindOf::StickToTerrainSlope);
        }
        let mut object = Object::new_raw(Arc::new(template), 7, ObjectStatusMaskType::none(), None);
        object.ambient_lifecycle_admitted = false;
        object.bind_owner_world_for_test(owner.services());

        crate::system::engine_stores::with_active_stores(&foreign, || {
            object.set_position(&destination).unwrap();
            let position = object.get_position();
            if slope {
                assert_eq!(
                    position.z, ground,
                    "the slope branch snaps to its own ground"
                );
            } else {
                assert_eq!(*position, destination);
            }
            assert_eq!(object.get_height_above_terrain(), position.z - ground);
        });
    }
}

fn flat_pose_world(height: u8, active_cells: i32) -> Arc<crate::system::engine_stores::EngineStores> {
    let stores = crate::system::engine_stores::new_for_world();
    let mut map = crate::system::map_loader::MapData::new();
    map.width = 10;
    map.height = 10;
    map.heightmap = vec![height; 100];
    map.boundaries = vec![
        crate::common::ICoord2D::new(10, 10),
        crate::common::ICoord2D::new(active_cells, active_cells),
    ];
    let mut terrain = stores.services().terrain().write().unwrap();
    terrain.load_map_data(map);
    terrain.restore_active_boundary_from_snapshot(1);
    drop(terrain);
    stores
}

#[test]
fn compatibility_matrix_observes_owner_height_and_rejects_borrow_conflicts() {
    use crate::system::engine_stores::with_active_stores;

    let owner = flat_pose_world(40, 5);
    let peer = flat_pose_world(0, 10);
    let mut object = inert_object();
    object.bind_owner_world_for_test(owner.services());
    let inside = Coord3D::new(30.0, 25.0, 100.0);
    let outside = Coord3D::new(60.0, 25.0, 100.0);
    with_active_stores(&peer, || {
        let initial = Matrix3D::from_translation(inside);
        object.set_transform_matrix(&initial);
        assert_eq!(object.get_height_above_terrain(), 75.0);
        assert!(!object.is_off_map());

        let next = Matrix3D::from_translation(outside);
        let owner_write = owner.services().terrain().write().unwrap();
        let result = object.set_transform_matrix_with_trigger_context(
            &next,
            Some(30),
            &mut |_| panic!("terrain rejection precedes trigger notifications"),
        );
        assert!(result.is_err());
        assert_eq!(object.get_transform_matrix(), initial);
        assert_eq!(*object.get_position(), inside);
        assert_eq!(object.get_height_above_terrain(), 75.0);
        assert!(!object.is_off_map());
        drop(owner_write);

        object.set_transform_matrix(&next);
        assert_eq!(*object.get_position(), outside);
        assert_eq!(object.get_height_above_terrain(), 75.0);
        assert!(object.is_off_map());
    });
}

#[test]
fn live_altitude_reads_its_world_and_uses_accepted_height_during_a_terrain_callback() {
    use crate::system::engine_stores::with_active_stores;

    let owner = flat_pose_world(40, 10);
    let peer = flat_pose_world(0, 10);
    let mut grounded = inert_object();
    let mut flying = inert_object();
    grounded.bind_owner_world_for_test(owner.services());
    flying.bind_owner_world_for_test(peer.services());
    let pos = Coord3D::new(30.0, 25.0, 25.0);

    with_active_stores(&peer, || {
        grounded.set_position(&pos).unwrap();
        flying.set_position(&pos).unwrap();
        assert_eq!(grounded.calculate_height_above_terrain(), 0.0);
        assert!(!grounded.is_above_terrain());
        assert!(flying.is_above_terrain());

        let blocked = owner.services().terrain().write().unwrap();
        assert_eq!(grounded.calculate_height_above_terrain(), 0.0);
        assert!(!grounded.is_above_terrain());
        drop(blocked);

        grounded.set_position(&Coord3D::new(30.0, 25.0, 26.0)).unwrap();
        assert_eq!(grounded.calculate_height_above_terrain(), 1.0);
        assert!(grounded.is_above_terrain());
    });
}
