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
    let terrain = crate::terrain::TerrainLogic::new();
    let mut collision = crate::object::collide::collision_system::CollisionSystem::new();
    let position = Coord3D::new(40.0, 50.0, 60.0);
    object
        .set_snapshot_position_with_services(
            &position,
            &terrain,
            &mut collision,
            24,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
    let moved = object.get_transform_matrix();
    assert_eq!(moved.x_axis, matrix.x_axis);
    assert_eq!(moved.y_axis, matrix.y_axis);
    assert_eq!(moved.z_axis, matrix.z_axis);
    assert_eq!(moved.w_axis, position.extend(1.0));
    object
        .set_snapshot_orientation_with_services(
            0.5,
            &terrain,
            &mut collision,
            24,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
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
    let terrain = crate::terrain::TerrainLogic::new();
    let mut collision = crate::object::collide::collision_system::CollisionSystem::new();
    let position = Coord3D::new(10.0, 20.0, 0.0);
    object
        .set_snapshot_position_with_services(
            &position,
            &terrain,
            &mut collision,
            23,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(
        object.get_orientation().to_bits(),
        (-std::f32::consts::PI).to_bits()
    );
    assert_eq!(object.get_transform_matrix().x_axis, matrix.x_axis);
    // Thing.cpp174 uses raw angle for the basis, normalizeAngle for the cache.
    object
        .set_snapshot_orientation_with_services(
            -std::f32::consts::PI,
            &terrain,
            &mut collision,
            23,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
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
