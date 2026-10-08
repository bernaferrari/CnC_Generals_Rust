use super::*;

fn data() -> W3DModelDrawModuleData {
    let mut data = W3DModelDrawModuleData::new();
    data.attach_to_drawable_bone = "attach".into();
    let mut state = ModelConditionInfo::new();
    state.conditions_yes.push(ModelConditionFlags::empty());
    state.weapon_barrels[0].push(WeaponBarrelInfo {
        projectile_offset_mtx: Matrix3D::from_translation(Coord3D::new(10.0, 2.0, 4.0)),
        ..WeaponBarrelInfo::new()
    });
    state.turrets.push(TurretInfo::new());
    data.condition_states.push(state);
    data
}

#[test]
fn frozen_plan_does_not_resume_mutated_module_state_or_cache_a_different_name() {
    let mut draw = W3DModelDraw::new(data());
    let plan = draw
        .prepare_projectile_launch_offset(&ModelConditionFlags::empty(), 0, 0, TurretType::Primary)
        .unwrap();
    draw.data.attach_to_drawable_bone = "other".into();
    draw.data.condition_states[0].weapon_barrels[0][0].projectile_offset_mtx = Matrix3D::IDENTITY;
    let offset = draw.cache_projectile_attachment(
        plan.attachment_bone.as_ref().unwrap().as_str(),
        Coord3D::new(7.0, 8.0, 9.0),
    );
    assert!(draw.attach_offset_cache.get_mut().unwrap().is_none());
    let mut launch = Matrix3D::IDENTITY;
    let mut rotation = Coord3D::origin();
    let mut pitch = Coord3D::origin();
    assert!(plan.finish(
        Some(offset),
        [None; 2],
        &mut launch,
        &mut rotation,
        &mut pitch
    ));
    assert_eq!(launch.w_axis.truncate(), Coord3D::new(10.0, 2.0, 4.0));
    assert_eq!(rotation, offset);
    assert_eq!(pitch, offset);
    assert_eq!(
        draw.cache_projectile_attachment("other", Coord3D::new(20.0, 21.0, 22.0)),
        Coord3D::new(20.0, 21.0, 22.0)
    );
    assert_eq!(
        draw.cache_projectile_attachment("other", Coord3D::origin()),
        Coord3D::new(20.0, 21.0, 22.0)
    );
}

#[test]
fn launch_rotation_preserves_cpp_pre_z_then_pre_negative_y_order() {
    let mut data = data();
    data.condition_states[0].turrets[0].turret_art_angle = std::f32::consts::FRAC_PI_2;
    data.condition_states[0].turrets[0].turret_art_pitch = std::f32::consts::FRAC_PI_2;
    let draw = W3DModelDraw::new(data);
    let plan = draw
        .prepare_projectile_launch_offset(&ModelConditionFlags::empty(), 0, 0, TurretType::Primary)
        .unwrap();
    let mut launch = Matrix3D::IDENTITY;
    let mut rotation = Coord3D::origin();
    let mut pitch = Coord3D::origin();
    assert!(plan.finish(
        Some(Coord3D::new(100.0, 200.0, 300.0)),
        [None; 2],
        &mut launch,
        &mut rotation,
        &mut pitch
    ));
    assert!((launch.w_axis.truncate() - Coord3D::new(-4.0, 10.0, -2.0)).length() < 1e-4);
    // Attachment is applied to turret pivots, never the projectile launch.
    assert_eq!(rotation, Coord3D::new(100.0, 200.0, 300.0));
}

#[test]
fn absent_state_and_invalid_slot_do_not_prepare_a_query() {
    let draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
    assert!(
        draw.prepare_projectile_launch_offset(
            &ModelConditionFlags::empty(),
            0,
            0,
            TurretType::Primary
        )
        .is_none()
    );
    let draw = W3DModelDraw::new(data());
    assert!(
        draw.prepare_projectile_launch_offset(
            &ModelConditionFlags::empty(),
            WEAPONSLOT_COUNT,
            0,
            TurretType::Primary
        )
        .is_none()
    );
    assert!(draw.attach_offset_cache.lock().unwrap().is_none());
}

#[test]
fn poisoned_private_cache_still_returns_query_without_storing_it() {
    let mut draw = W3DModelDraw::new(data());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _cache = draw.attach_offset_cache.lock().unwrap();
            panic!("controlled private cache poison");
        }))
        .is_err()
    );
    let plan = draw
        .prepare_projectile_launch_offset(&ModelConditionFlags::empty(), 0, 0, TurretType::Primary)
        .unwrap();
    assert_eq!(plan.attachment_bone.as_ref().unwrap().as_str(), "attach");
    let offset = Coord3D::new(5.0, 6.0, 7.0);
    assert_eq!(draw.cache_projectile_attachment("attach", offset), offset);
    assert!(
        draw.attach_offset_cache
            .get_mut()
            .unwrap_err()
            .into_inner()
            .is_none()
    );
}
