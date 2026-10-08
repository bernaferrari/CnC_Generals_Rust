use super::*;
use crate::object::draw::w3d_model_draw::{
    ModelConditionInfo, PristineBoneInfo, W3DModelDraw, W3DModelDrawModuleData,
};

fn install_module(
    drawable: &mut Drawable,
    name: &str,
    selected_condition: ModelConditionFlags,
    bones: &[(&str, Coord3D)],
) -> DrawableModuleHandle {
    let mut data = W3DModelDrawModuleData::new();
    data.default_state = 0;
    let mut default = ModelConditionInfo::new();
    default.conditions_yes.push(ModelConditionFlags::empty());
    default.model_name = AsciiString::from("DefaultCondition");
    let mut selected = ModelConditionInfo::new();
    selected.conditions_yes.push(selected_condition);
    selected.model_name = AsciiString::from("SelectedCondition");
    for (bone_name, position) in bones {
        selected.pristine_bones.insert(
            NameKeyGenerator::name_to_key(bone_name),
            PristineBoneInfo {
                transform: Matrix3D::from_translation(*position),
                bone_index: 1,
            },
        );
    }
    data.condition_states.extend([default, selected]);
    let model = W3DModelDraw::new(data.clone());
    drawable.add_module(
        ModuleInterfaceType::DRAW,
        "W3DModelDraw".into(),
        name.into(),
        Arc::new(data),
        Box::new(model),
    )
}

#[test]
fn pristine_bones_append_module_results_in_order_with_remaining_capacity() {
    let test_name = concat!(
        module_path!(),
        "::pristine_bones_append_module_results_in_order_with_remaining_capacity"
    )
    .strip_prefix("gamelogic::")
    .unwrap();
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(test_name, "GENERALS_BONE_AGGREGATION_CHILD")
    {
        return;
    }
    let _serial = crate::test_sync::lock();
    const PRONE: ModelConditionFlags = ModelConditionFlags::PRONE;
    let a1 = Coord3D::new(1.0, 0.0, 0.0);
    let b1 = Coord3D::new(2.0, 0.0, 0.0);
    let b2 = Coord3D::new(2.0, 1.0, 0.0);
    let c1 = Coord3D::new(3.0, 0.0, 0.0);
    let mut drawable = Drawable::new(
        0xA54_9001,
        0xA54_9001,
        "BoneOrder".into(),
        DrawableType::Static,
    );
    // Each module sees the same one-based start index. Module B has both
    // suffixes; its first result must be joint01, not joint02.
    install_module(&mut drawable, "BoneOrderA", PRONE, &[("joint01", a1)]);
    let active = install_module(
        &mut drawable,
        "BoneOrderB",
        PRONE,
        &[("joint01", b1), ("joint02", b2)],
    );
    install_module(&mut drawable, "BoneOrderC", PRONE, &[("joint01", c1)]);
    assert!(
        drawable
            .get_pristine_bone_positions("joint", 1, 4)
            .is_empty()
    );
    drawable.set_model_conditions(PRONE);

    // CPP Drawable.cpp:747-770: append in module order, same start_index,
    // advance output/capacity after each positive return, stop when full.
    assert_eq!(
        drawable.get_pristine_bone_positions("joint", 1, 2),
        vec![a1, b1]
    );
    assert_eq!(
        drawable.get_pristine_bone_positions("joint", 2, 4),
        vec![b2]
    );
    assert_eq!(
        drawable.get_pristine_bone_positions("joint", 1, 3),
        vec![a1, b1, b2]
    );
    assert_eq!(
        drawable.get_pristine_bone_positions("joint", 1, 4),
        vec![a1, b1, b2, c1]
    );
    let active_positions = active.with_module(|module| {
        let current = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DModelDraw>()
            .unwrap();
        drawable.get_pristine_bone_positions_for_active(
            Some((active.entry.as_ref(), current)),
            "joint",
            1,
            4,
        )
    });
    assert_eq!(active_positions, vec![a1, b1, b2, c1]);
    assert_eq!(
        drawable.get_pristine_bone_positions("joint", 1, 1),
        vec![a1]
    );
    assert!(
        drawable
            .get_pristine_bone_positions("joint", 1, 0)
            .is_empty()
    );
}

#[test]
fn pristine_bone_miss_does_not_substitute_drawable_skeleton() {
    let test_name = concat!(
        module_path!(),
        "::pristine_bone_miss_does_not_substitute_drawable_skeleton"
    )
    .strip_prefix("gamelogic::")
    .unwrap();
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(test_name, "GENERALS_BONE_AGGREGATION_CHILD")
    {
        return;
    }
    let _serial = crate::test_sync::lock();
    let mut drawable = Drawable::new(
        0xA54_9002,
        0xA54_9002,
        "BoneMiss".into(),
        DrawableType::Static,
    );
    let mut data = W3DModelDrawModuleData::new();
    data.default_state = 0;
    let mut state = ModelConditionInfo::new();
    state.conditions_yes.push(ModelConditionFlags::empty());
    state.model_name = AsciiString::from("NamedStateWithoutRequestedBone");
    data.condition_states.push(state);
    let model = W3DModelDraw::new(data.clone());
    drawable.add_module(
        ModuleInterfaceType::DRAW,
        "W3DModelDraw".into(),
        "BoneMiss".into(),
        Arc::new(data),
        Box::new(model),
    );
    drawable.skeleton.push(BoneData {
        name: "joint".to_owned(),
        parent_index: -1,
        transform: Matrix3D::from_translation(Coord3D::new(99.0, 98.0, 97.0)),
        inverse_bind_pose: Matrix3D::IDENTITY,
    });
    drawable
        .bone_transforms
        .push(Matrix3D::from_translation(Coord3D::new(99.0, 98.0, 97.0)));

    // C++ Drawable::getPristineBonePositions returns the accumulated module
    // count (zero here); it never falls back to Drawable.skeleton.
    assert!(
        drawable
            .get_pristine_bone_positions("joint", 0, 1)
            .is_empty()
    );
}

#[test]
fn pristine_bone_transforms_append_the_same_ordered_module_results() {
    let test_name = concat!(
        module_path!(),
        "::pristine_bone_transforms_append_the_same_ordered_module_results"
    )
    .strip_prefix("gamelogic::")
    .unwrap();
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(test_name, "GENERALS_BONE_AGGREGATION_CHILD")
    {
        return;
    }
    let _serial = crate::test_sync::lock();
    let first = Coord3D::new(4.0, 5.0, 6.0);
    let second = Coord3D::new(7.0, 8.0, 9.0);
    let mut drawable = Drawable::new(
        0xA54_9003,
        0xA54_9003,
        "BoneTransforms".into(),
        DrawableType::Static,
    );
    install_module(
        &mut drawable,
        "BoneTransformsA",
        ModelConditionFlags::PRONE,
        &[("joint01", first)],
    );
    install_module(
        &mut drawable,
        "BoneTransformsB",
        ModelConditionFlags::PRONE,
        &[("joint01", second)],
    );
    drawable.set_model_conditions(ModelConditionFlags::PRONE);
    assert_eq!(
        drawable.get_pristine_bone_transforms("joint", 1, 2),
        vec![
            Matrix3D::from_translation(first),
            Matrix3D::from_translation(second)
        ]
    );
    assert_eq!(
        drawable.get_pristine_bone_transforms("joint", 1, 1),
        vec![Matrix3D::from_translation(first)]
    );
    assert!(
        drawable
            .get_pristine_bone_transforms("joint", 1, 0)
            .is_empty()
    );
    drawable.set_model_conditions(ModelConditionFlags::empty());
    assert!(
        drawable
            .get_pristine_bone_transforms("joint", 1, 2)
            .is_empty()
    );
}

#[test]
fn pristine_bone_transform_miss_does_not_substitute_drawable_skeleton() {
    let test_name = concat!(
        module_path!(),
        "::pristine_bone_transform_miss_does_not_substitute_drawable_skeleton"
    )
    .strip_prefix("gamelogic::")
    .unwrap();
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(test_name, "GENERALS_BONE_AGGREGATION_CHILD")
    {
        return;
    }
    let _serial = crate::test_sync::lock();
    let mut drawable = Drawable::new(
        0xA54_9004,
        0xA54_9004,
        "BoneTransformMiss".into(),
        DrawableType::Static,
    );
    install_module(
        &mut drawable,
        "BoneTransformMiss",
        ModelConditionFlags::PRONE,
        &[],
    );
    drawable.skeleton.push(BoneData {
        name: "joint".to_owned(),
        parent_index: -1,
        transform: Matrix3D::IDENTITY,
        inverse_bind_pose: Matrix3D::IDENTITY,
    });
    drawable
        .bone_transforms
        .push(Matrix3D::from_translation(Coord3D::new(99.0, 98.0, 97.0)));
    assert!(
        drawable
            .get_pristine_bone_transforms("joint", 0, 1)
            .is_empty()
    );
}

#[test]
fn exhausted_bone_capacity_does_not_lock_an_installed_entry_again() {
    let test_name = concat!(
        module_path!(),
        "::exhausted_bone_capacity_does_not_lock_an_installed_entry_again"
    )
    .strip_prefix("gamelogic::")
    .unwrap();
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(test_name, "GENERALS_BONE_AGGREGATION_CHILD")
    {
        return;
    }
    let _serial = crate::test_sync::lock();
    let position = Coord3D::new(11.0, 12.0, 13.0);
    let mut drawable = Drawable::new(
        0xA54_9005,
        0xA54_9005,
        "BoneCapacity".into(),
        DrawableType::Static,
    );
    let first = install_module(
        &mut drawable,
        "BoneCapacityA",
        ModelConditionFlags::PRONE,
        &[("joint01", position)],
    );
    let after = install_module(
        &mut drawable,
        "BoneCapacityB",
        ModelConditionFlags::PRONE,
        &[("joint01", Coord3D::ZERO)],
    );
    drawable.set_model_conditions(ModelConditionFlags::PRONE);
    // Once the first module fills capacity, neither wrapper may acquire the
    // following installed entry. Holding that entry makes an extra query fail
    // under the bounded child's reentry deadline rather than pass unnoticed.
    after.with_module(|_| {
        assert_eq!(
            drawable.get_pristine_bone_positions("joint", 1, 1),
            vec![position]
        );
        assert_eq!(
            drawable.get_pristine_bone_transforms("joint", 1, 1),
            vec![Matrix3D::from_translation(position)]
        );
    });
    // C++ checks zero remaining capacity before even querying the first entry.
    first.with_module(|_| {
        assert!(
            drawable
                .get_pristine_bone_positions("joint", 1, 0)
                .is_empty()
        );
        assert!(
            drawable
                .get_pristine_bone_transforms("joint", 1, 0)
                .is_empty()
        );
    });
}
