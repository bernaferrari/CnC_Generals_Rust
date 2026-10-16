use super::*;
use glam::{Mat4, Vec3, Vec4};
#[test]
fn clip_planes_replay_150_extracted_original_cpp_threshold_bit_records() {
    let mut count = 0;
    for row in include_str!("clip_cpp.csv").lines() {
        let c: Vec<_> = row.split(',').collect();
        let bits = |i| u32::from_str_radix(c[i], 16).unwrap();
        let (near, far) = source_clip_planes(
            c[1] == "2",
            f32::from_bits(bits(2)),
            f32::from_bits(bits(3)),
            c[0] == "1",
        )
        .unwrap();
        assert_eq!(near.to_bits(), bits(4), "{row}");
        assert_eq!(far.to_bits(), bits(5), "{row}");
        count += 1;
    }
    assert_eq!(count, 150);
    assert!(source_clip_planes(false, 0., 1., true).is_err());
    assert!(source_clip_planes(false, -1., 1., true).is_err());
}
fn depth(projection: Mat4, distance: f32) -> f32 {
    let p = projection * Vec4::new(0., 0., -distance, 1.);
    p.z / p.w
}
#[test]
fn actual_shared_projection_clips_depth_and_pick_far_endpoint_at_current_policy() {
    let mut view = crate::presentation_frame::SessionView::default();
    view.target = Vec3::new(1000., 0., 1000.);
    view.camera.native_camera.initialize_map(0., 300., 37.5, 0.);
    // Inert camera starts without a lens; real admission must install it before rays.
    assert_eq!(view.camera.projection_matrix, Mat4::IDENTITY);
    view.commit_orbit_pose();
    let expected_inset = view.camera.viewport.constraint_inset(
        view.camera.view_matrix,
        view.camera.projection_for_clip_planes(10., 1200.),
        view.camera.native_camera.basis().ground_level(),
    );
    assert!(expected_inset > 0.);

    view.set_camera_transform(
        (Vec3::ZERO, Vec3::new(4000., 0., 4000.)),
        &mut |_| unreachable!(),
    )
    .unwrap();
    let admitted = view.camera.constraint_bounds().unwrap();
    assert!((admitted.x_min - expected_inset).abs() < 0.001);
    assert!((admitted.z_min - expected_inset).abs() < 0.001);
    let projection = view.camera.projection_matrix;
    assert!(depth(projection, 1201.) > 1.);
    assert!((depth(projection, 10.)).abs() < 0.00001);
    let (a, b) = view
        .camera
        .viewport
        .pick_ray(view.camera.view_matrix, projection, 320., 240.)
        .unwrap();
    assert!((view.camera.view_matrix.transform_point3(a).z + 10.).abs() < 0.01);
    assert!((view.camera.view_matrix.transform_point3(b).z + 1200.).abs() < 1.);
    let cached = view.camera.constraints;
    view.camera
        .clip_policy
        .admit_map(super::super::clip::CameraClipRules {
            draw_entire_terrain: true,
        });
    assert_eq!(
        view.camera.projection_matrix, projection,
        "definition admission alone is not Set_Clip_Planes"
    );
    view.set_camera_transform(
        (Vec3::ZERO, Vec3::new(4000., 0., 4000.)),
        &mut |_| unreachable!(),
    )
    .unwrap();
    assert!(depth(view.camera.projection_matrix, 2000.) < 1.);
    assert!(depth(projection, 2000.) > 1.);
    assert_eq!(
        view.camera.constraints, cached,
        "changing clip policy alone does not invalidate original cached bounds"
    );
    let installed = view.camera.projection_matrix;
    view.camera.viewport.resize_display(1600, 900).unwrap();
    view.camera.rebuild_projection();
    assert_eq!(
        view.camera
            .projection_matrix
            .z_axis
            .to_array()
            .map(f32::to_bits),
        installed.z_axis.to_array().map(f32::to_bits)
    );
    assert_eq!(
        view.camera
            .projection_matrix
            .w_axis
            .to_array()
            .map(f32::to_bits),
        installed.w_axis.to_array().map(f32::to_bits)
    );
    assert_ne!(view.camera.projection_matrix.y_axis, installed.y_axis);
}
#[test]
fn invalid_clip_input_rejects_before_projection_target_cache_or_shaker_mutation() {
    let mut view = crate::presentation_frame::SessionView::default();
    view.target = Vec3::new(1000., 0., 1000.);
    view.camera.camera_fx_pitch = f32::NAN;
    let projection = view.camera.projection_matrix;
    let target = view.target;
    let cache = view.camera.constraints;
    assert!(
        view.set_camera_transform(
            (Vec3::ZERO, Vec3::new(4000., 0., 4000.)),
            &mut |_| unreachable!()
        )
        .is_err()
    );
    assert_eq!(view.camera.projection_matrix, projection);
    assert_eq!(view.target, target);
    assert_eq!(view.camera.constraints, cache);
}
