use glam::{Mat4, Quat, Vec3};
use std::sync::Arc;
use ww3d_animation::{
    Animatable3DObjClass, AnimationMode, BitChannelClass, HAnimClass, HRawAnimClass, HTreeClass,
};

fn bit_anim(name: &str, hierarchy: &str, frames: u32, pivots: usize, bits: Vec<BitChannelClass>) -> HAnimClass {
    let mut raw = HRawAnimClass::new(name, hierarchy, frames, 30);
    bits.into_iter().for_each(|channel| raw.add_bit_channel(channel));
    HAnimClass::from_raw(raw.bind_to_hierarchy(pivots))
}

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

#[test]
fn single_and_blended_animation_apply_visibility_per_pivot() {
    let channel = |pivot, bits| BitChannelClass::try_new(pivot, 1, 0, 1, vec![bits]).unwrap();
    let hidden = Arc::new(bit_anim(
        "hidden", "tree", 2, 4,
        vec![channel(1, 0b10), channel(2, 0), channel(3, 0)],
    ));
    let other = Arc::new(bit_anim("other", "tree", 2, 3, vec![channel(1, 0b01), channel(2, 0)]));
    let mut tree = HTreeClass::default();
    for name in ["one", "two", "three", "outside"] {
        tree.add_pivot(name, 0, Vec3::ZERO, Quat::IDENTITY);
    }
    tree.pivots[4].is_visible = false;
    let mut object = Animatable3DObjClass::new(tree);
    object.capture_bone(2);
    object.set_animation(Some(hidden.clone()), 0.0, AnimationMode::Manual, 0.0);
    object.update_sub_object_transforms();
    assert!(object.htree.get_visibility(0));
    assert!(!object.htree.get_visibility(1));
    assert!(object.htree.get_visibility(2));
    assert!(!object.htree.get_visibility(3));
    assert!(!object.htree.get_visibility(4));

    object.set_animation(Some(hidden.clone()), 1.0, AnimationMode::Manual, 0.0);
    object.update_sub_object_transforms();
    assert!(object.htree.get_visibility(1));
    object.release_bone(2);
    object.set_animation_blend(Some(hidden.clone()), 0.0, Some(other.clone()), 0.0, 0.0);
    object.update_sub_object_transforms();
    assert!(object.htree.get_visibility(1)); // OR, even at weight zero
    assert!(!object.htree.get_visibility(2));
    assert!(!object.htree.get_visibility(3)); // outside shorter motion, unchanged
    assert!(!object.htree.get_visibility(4));

    object.set_animation_blend(Some(hidden), 0.0, Some(other), 1.0, 1.0);
    object.update_sub_object_transforms();
    assert!(!object.htree.get_visibility(1));
}

#[test]
fn unanimated_trailing_pivots_use_authored_hierarchy_size_in_single_and_blended_updates() {
    let hidden = Arc::new(bit_anim(
        "hidden", "large", 1, 5,
        vec![BitChannelClass::try_new(3, 1, 0, 0, vec![0]).unwrap()],
    ));
    let unanimated = Arc::new(bit_anim("idle", "large", 1, 5, vec![]));
    let shorter = Arc::new(bit_anim("short", "small", 1, 3, vec![]));
    let mut tree = HTreeClass::default();
    for name in ["one", "two", "three", "four", "outside"] {
        tree.add_pivot(name, 0, Vec3::ZERO, Quat::IDENTITY);
    }
    let mut object = Animatable3DObjClass::new(tree);
    object.htree.pivots[4].is_visible = false;
    object.htree.pivots[5].is_visible = false;
    object.set_animation(Some(hidden.clone()), 0.0, AnimationMode::Manual, 0.0);
    object.update_sub_object_transforms();
    assert!(!object.htree.get_visibility(3));
    assert!(object.htree.get_visibility(4));
    assert!(!object.htree.get_visibility(5));

    object.htree.pivots[4].is_visible = false;
    object.set_animation_blend(Some(hidden), 0.0, Some(shorter), 0.0, 0.5);
    object.update_sub_object_transforms();
    assert!(!object.htree.get_visibility(3));
    assert!(!object.htree.get_visibility(4));

    object.set_animation_blend(Some(unanimated.clone()), 0.0, Some(unanimated), 0.0, 0.5);
    object.update_sub_object_transforms();
    assert!(object.htree.get_visibility(3));
    assert!(object.htree.get_visibility(4));
    assert!(!object.htree.get_visibility(5));
}
