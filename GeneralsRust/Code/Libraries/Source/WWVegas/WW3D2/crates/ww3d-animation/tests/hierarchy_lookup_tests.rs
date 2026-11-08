use glam::{Mat4, Quat, Vec3};
use ww3d_animation::{Animatable3DObjClass, HTreeClass};

#[test]
fn hierarchy_name_lookups_are_case_insensitive_and_use_first_matching_pivot() {
    let mut tree = HTreeClass::default();
    tree.add_pivot("Turret", 0, Vec3::ZERO, Quat::IDENTITY);
    tree.add_pivot("TURRET", 0, Vec3::ZERO, Quat::IDENTITY);

    for name in ["turret", "TURRET", "Turret"] {
        assert_eq!(tree.get_pivot_index(name), Some(1));
        assert_eq!(tree.find_pivot_index(name), Some(1));
        assert_eq!(tree.get_pivot_by_name(name).map(|pivot| pivot.index), Some(1));
        assert_eq!(tree.get_bone_index(name), 1);
    }
    assert_eq!(tree.get_pivot_index("absent"), None);
    assert!(tree.get_pivot_by_name("absent").is_none());
    assert_eq!(tree.find_pivot_index("absent"), None);
    assert_eq!(tree.get_bone_index("absent"), 0);
    assert_eq!(HTreeClass::new().get_bone_index("absent"), 0);
}

#[test]
fn animatable_object_resolves_named_bones_and_missing_bones_to_root() {
    let mut tree = HTreeClass::default();
    tree.add_pivot("Turret", 0, Vec3::new(2.0, 0.0, 0.0), Quat::IDENTITY);
    let mut object = Animatable3DObjClass::new(tree);
    object.set_transform(Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)));

    assert_eq!(object.get_bone_index("tUrReT"), 1);
    assert_eq!(object.get_bone_index("missing"), 0);
    assert_eq!(object.get_bone_transform_by_name("TURRET").unwrap().w_axis.truncate(), Vec3::new(12.0, 0.0, 0.0));
    assert_eq!(object.get_bone_transform_by_name("missing").unwrap().w_axis.truncate(), Vec3::new(10.0, 0.0, 0.0));
}

#[test]
fn hierarchy_scale_accumulates_on_bind_and_animated_translations() {
    let mut tree = HTreeClass::new();
    let root = Mat4::from_rotation_translation(
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        Vec3::new(1.0, 2.0, 3.0),
    );
    tree.add_pivot_from_base("Root", -1, root);
    tree.add_pivot("Turret", 0, Vec3::new(2.0, 0.0, 0.0), Quat::IDENTITY);

    tree.scale(2.0);
    tree.scale(3.0);
    tree.scale(1.0);

    let scaled_root = tree.get_pivot(0).unwrap().base_transform;
    assert_eq!(scaled_root.x_axis, root.x_axis);
    assert_eq!(scaled_root.y_axis, root.y_axis);
    assert_eq!(scaled_root.z_axis, root.z_axis);
    assert_eq!(scaled_root.w_axis, Vec3::new(6.0, 12.0, 18.0).extend(1.0));
    assert_eq!(tree.get_scale_factor(), 6.0);

    tree.base_update(Mat4::IDENTITY);
    assert_eq!(tree.get_transform(1).unwrap().w_axis.truncate(), Vec3::new(12.0, 0.0, 0.0));
    tree.anim_update(
        Mat4::IDENTITY,
        &[Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)],
        &[Quat::IDENTITY, Quat::IDENTITY],
    );
    assert_eq!(tree.get_transform(1).unwrap().w_axis.truncate(), Vec3::new(18.0, 0.0, 0.0));
}

#[test]
fn captured_bone_control_applies_local_or_world_translation_after_base_pose() {
    let mut tree = HTreeClass::default();
    tree.add_pivot("Turret", 0, Vec3::new(2.0, 0.0, 0.0), Quat::IDENTITY);
    let mut object = Animatable3DObjClass::new(tree);
    object.set_transform(Mat4::from_rotation_translation(
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        Vec3::new(10.0, 20.0, 0.0),
    ));
    object.capture_bone(1);
    let control = Mat4::from_rotation_translation(
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        Vec3::new(3.0, 0.0, 0.0),
    );

    object.control_bone(1, control, false);
    let local = object.get_bone_transform(1).unwrap();
    assert!(local.w_axis.truncate().abs_diff_eq(Vec3::new(10.0, 25.0, 0.0), 0.00001));
    assert!(local.x_axis.truncate().abs_diff_eq(Vec3::NEG_X, 0.00001));

    object.control_bone(1, control, true);
    let world = object.get_bone_transform(1).unwrap();
    assert!(world.w_axis.truncate().abs_diff_eq(Vec3::new(13.0, 22.0, 0.0), 0.00001));
    assert!(world.x_axis.truncate().abs_diff_eq(Vec3::NEG_X, 0.00001));

    object.release_bone(1);
    assert!(!object.is_bone_captured(1));
    object.update_sub_object_transforms();
    let released = object.get_bone_transform(1).unwrap();
    assert!(released.w_axis.truncate().abs_diff_eq(Vec3::new(10.0, 22.0, 0.0), 0.00001));
    object.capture_bone(1);
    object.update_sub_object_transforms();
    let recaptured = object.get_bone_transform(1).unwrap();
    assert!(recaptured.w_axis.truncate().abs_diff_eq(Vec3::new(10.0, 22.0, 0.0), 0.00001));
}
