use super::*;
use crate::presentation_frame::camera::ScriptCameraShaker;
#[test]
fn actual_set_transform_builds_twice_only_when_invalid_and_force_recalc_builds_zero() {
    let mut view = SessionView::default();
    view.camera.rebuild_projection();
    view.target = Vec3::new(400., 0., 400.);
    view.camera
        .script_camera_shakers
        .push(ScriptCameraShaker::new(Vec3::ZERO, 10000., 30., 0.1));
    let bounds = (Vec3::ZERO, Vec3::new(2000., 0., 2000.));
    let mut samples = 0;
    view.set_camera_transform(bounds, &mut |_| {
        samples += 1;
        SlaveCameraSample::NoInterface
    })
    .unwrap();
    assert_eq!(samples, 0, "no slave mode performs no backend query");
    assert_eq!(
        view.camera.script_camera_shakers[0].elapsed_seconds,
        2. * (1. / 30.)
    );
    let first = view.camera.script_camera_shakers[0].elapsed_seconds;
    view.set_camera_transform(bounds, &mut |_| unreachable!())
        .unwrap();
    assert_eq!(
        view.camera.script_camera_shakers[0].elapsed_seconds,
        first + 1. / 30.
    );
    let before = view.camera.script_camera_shakers[0].elapsed_seconds;
    view.camera.force_recalculate_constraints(bounds).unwrap();
    assert_eq!(before, view.camera.script_camera_shakers[0].elapsed_seconds);
    for _ in 0..120 {
        view.commit_orbit_pose();
    }
    assert_eq!(before, view.camera.script_camera_shakers[0].elapsed_seconds);
}
#[test]
fn invalid_constraint_build_queries_receiving_slave_at_both_original_points_and_never_clamps_final_slam()
 {
    use crate::game_logic::mission_scripts::CameraSlaveModeRequest;
    let mut a = SessionView::default();
    let peer = SessionView::default();
    a.camera.rebuild_projection();
    a.camera.camera_slave_mode = Some(CameraSlaveModeRequest {
        thing_template_name: "OwnedUnit".into(),
        bone_name: "Camera".into(),
    });
    let first = Mat4::from_translation(Vec3::new(300., 400., 300.));
    let final_pose = Mat4::from_translation(Vec3::new(4000., 500., -300.));
    let mut calls = 0;
    a.set_camera_transform((Vec3::ZERO, Vec3::new(1000., 0., 1000.)), &mut |mode| {
        assert_eq!(mode.thing_template_name, "OwnedUnit");
        calls += 1;
        SlaveCameraSample::Pose(if calls == 1 { first } else { final_pose })
    })
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(a.target, final_pose.w_axis.truncate());
    assert_eq!(a.camera.camera_position, a.target);
    assert_eq!(a.camera.view_matrix, final_pose.inverse());
    assert_eq!(peer.target, Vec3::ZERO);
    let cache = a.camera.constraints;
    calls = 0;
    a.set_camera_transform((Vec3::ZERO, Vec3::new(1000., 0., 1000.)), &mut |_| {
        calls += 1;
        SlaveCameraSample::Pose(final_pose)
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(a.camera.constraints, cache);
}

#[test]
fn provisional_slave_position_is_logically_constrained_before_final_ordinary_build() {
    use crate::game_logic::mission_scripts::CameraSlaveModeRequest;
    let mut view = SessionView::default();
    view.camera.rebuild_projection();
    view.camera.camera_slave_mode = Some(CameraSlaveModeRequest {
        thing_template_name: "Owned".into(),
        bone_name: "Cam".into(),
    });
    let first = Mat4::look_at_rh(
        Vec3::new(2000., 300., -500.),
        Vec3::new(2000., 0., -400.),
        Vec3::Y,
    )
    .inverse();
    let mut calls = 0;
    view.set_camera_transform((Vec3::ZERO, Vec3::new(1000., 0., 1000.)), &mut |_| {
        calls += 1;
        if calls == 1 {
            SlaveCameraSample::Pose(first)
        } else {
            SlaveCameraSample::NoInterface
        }
    })
    .unwrap();
    assert_eq!(calls, 2);
    let bounds = view.camera.constraint_bounds().unwrap();
    assert_eq!(view.target, bounds.constrain(first.w_axis.truncate()));
    let (eye, look) = view.camera.native_camera.basis().pose(
        view.target,
        view.camera.camera_zoom,
        view.camera.camera_pitch_radians,
        view.camera.camera_yaw_radians,
    );
    assert_eq!(view.camera.camera_position, eye);
    assert_eq!(
        view.camera.view_matrix,
        Mat4::look_at_rh(eye, look, Vec3::Y)
    );
}

#[test]
fn rejected_nonfinite_transform_inputs_do_not_advance_owner() {
    let mut view = SessionView::default();
    view.camera
        .script_camera_shakers
        .push(ScriptCameraShaker::new(Vec3::ZERO, 10000., 30., 1.));
    let before = (
        view.target,
        view.camera.view_matrix,
        view.camera.constraints,
        view.camera.script_camera_shakers[0].elapsed_seconds,
    );
    assert!(
        view.set_camera_transform((Vec3::splat(f32::NAN), Vec3::ONE), &mut |_| unreachable!())
            .is_err()
    );
    assert_eq!(
        (
            view.target,
            view.camera.view_matrix,
            view.camera.constraints,
            view.camera.script_camera_shakers[0].elapsed_seconds
        ),
        before
    );
}
