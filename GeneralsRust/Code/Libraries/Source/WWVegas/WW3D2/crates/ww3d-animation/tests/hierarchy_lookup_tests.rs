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
