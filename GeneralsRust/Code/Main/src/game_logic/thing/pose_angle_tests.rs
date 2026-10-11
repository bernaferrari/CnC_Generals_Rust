//! CPP Thing145–215 keeps caller-normalized yaw across position updates.
use super::*;
use std::f32::consts::{PI, TAU};

fn thing() -> Thing {
    Thing::new(ThingTemplate::new("OwnedPoseAngle"))
}

#[test]
fn thing_orientation_keeps_exact_cpp_cached_angle_at_pi_boundaries() {
    let below = f32::from_bits(PI.to_bits() - 1);
    let above = f32::from_bits(PI.to_bits() + 1);
    for angle in [
        0.0,
        -0.0,
        0.345,
        -0.789,
        PI,
        -PI,
        below,
        above,
        -below,
        -above,
        TAU + 0.25,
    ] {
        let expected = game_engine::common::system::game_common::normalize_angle(angle);
        let mut thing = thing();
        thing.set_orientation(angle);
        assert_eq!(thing.get_orientation().to_bits(), expected.to_bits());
        assert_eq!(thing.geometry.rotation.to_bits(), expected.to_bits());
        assert_eq!(
            thing.get_direction_vector(),
            Vec3::new(expected.cos(), 0.0, -expected.sin())
        );
        thing.set_position(Vec3::new(7.0, 3.0, -4.0));
        assert_eq!(thing.get_orientation().to_bits(), expected.to_bits());
        assert_eq!(thing.geometry.rotation.to_bits(), expected.to_bits());
        assert_eq!(thing.geometry.position, Vec3::new(7.0, 3.0, -4.0));
        assert_eq!(
            thing.get_direction_vector(),
            Vec3::new(expected.cos(), 0.0, -expected.sin())
        );
        assert_eq!(
            thing.clone().get_orientation().to_bits(),
            expected.to_bits()
        );
    }
}

#[test]
fn thing_position_preserves_matrix_basis_and_transform_setter_derives_yaw() {
    let mut thing = thing();
    let matrix = Mat4::from_scale_rotation_translation(
        Vec3::new(2.0, 3.0, 4.0),
        glam::Quat::from_euler(glam::EulerRot::YXZ, 0.7, 0.2, -0.3),
        Vec3::new(1.0, 2.0, 3.0),
    );
    let derived = (-matrix.x_axis.z).atan2(matrix.x_axis.x);
    thing.set_transform_matrix(matrix);
    assert_eq!(thing.get_orientation().to_bits(), derived.to_bits());
    thing.set_position(Vec3::splat(8.0));
    assert_eq!(thing.get_orientation().to_bits(), derived.to_bits());
    assert_eq!(thing.get_transform_matrix().x_axis, matrix.x_axis);
    assert_eq!(thing.get_transform_matrix().y_axis, matrix.y_axis);
    assert_eq!(thing.get_transform_matrix().z_axis, matrix.z_axis);
    assert_eq!(thing.geometry.position, Vec3::splat(8.0));
}

#[test]
fn thing_orientation_extreme_and_nonfinite_inputs_are_bounded() {
    for angle in [
        f32::MAX,
        -f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ] {
        let mut thing = thing();
        thing.set_orientation(angle);
        assert!(thing.get_orientation().is_finite());
        assert!(thing.get_orientation() > -PI && thing.get_orientation() <= PI);
        assert!(thing.get_transform_matrix().is_finite());
        assert!(thing.get_direction_vector().is_finite());
    }
}

#[test]
fn thing_raw_pose_decode_still_derives_cache_from_matrix() {
    let mut thing = thing();
    thing.set_orientation(PI);
    let matrix = thing.get_transform_matrix();
    // Existing raw seven-field contract deliberately repairs stale caches.
    thing.cached_angle = 0.456;
    thing.cached_position = Vec3::splat(500.0);
    thing.cached_dir_vector = Vec3::Z;
    thing.cache_valid = false;
    let bytes = bincode_legacy::serialize(&thing).unwrap();
    let restored: Thing = bincode_legacy::deserialize(&bytes).unwrap();
    let derived = (-matrix.x_axis.z).atan2(matrix.x_axis.x);
    assert_eq!(restored.get_transform_matrix(), matrix);
    assert_eq!(restored.get_orientation().to_bits(), derived.to_bits());
    assert_eq!(restored.get_position(), matrix.w_axis.truncate());
    assert_eq!(
        restored.get_direction_vector(),
        Vec3::new(derived.cos(), 0.0, -derived.sin())
    );
    assert_eq!(restored.geometry.rotation.to_bits(), derived.to_bits());
    assert!(restored.cache_valid);
}
