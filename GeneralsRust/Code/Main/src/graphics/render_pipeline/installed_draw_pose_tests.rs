use super::*;
use crate::assets::{W3dAnimChannel, W3dAnimation, W3dHierarchy, W3dPivot};

fn pivot(name: &str, parent_idx: u32, translation: [f32; 3], rotation: [f32; 4]) -> W3dPivot {
    W3dPivot {
        name: name.into(),
        parent_idx,
        translation,
        euler_angles: [0.; 3],
        rotation,
    }
}
fn model_with_camera(translation: [f32; 3], rotation: [f32; 4]) -> W3DModel {
    let mut model = W3DModel::new("CameraModel".into());
    model.hierarchy = Some(W3dHierarchy {
        name: "TREE".into(),
        pivot_fixups: Vec::new(),
        pivots: vec![
            pivot("ROOT", u32::MAX, [0.; 3], [0., 0., 0., 1.]),
            pivot("CAMERA", 0, translation, rotation),
        ],
    });
    model.hlods.push(crate::assets::W3dHlod {
        version: 0x0001_0000,
        name: "CAMERA_MODEL".into(),
        hierarchy_name: "TREE".into(),
        lods: vec![crate::assets::W3dHlodLod {
            max_screen_size: f32::MAX,
            subobjects: Vec::new(),
        }],
        aggregates: None,
        proxies: None,
        has_unrendered_aggregates: false,
        has_invalid_trailing_records: false,
    });
    model
}
fn state(
    model: W3DModel,
    binding: Option<crate::assets::W3dAnimationBinding>,
) -> ObjectVisualState {
    ObjectVisualState {
        installed_draw_pose: InstalledDrawObject::Ready(InstalledDrawPose::created(
            Arc::new(model),
            Mat4::IDENTITY,
            &crate::assets::AuthoredDrawModel::default(),
            binding,
        )),
        ..Default::default()
    }
}
#[test]
fn installed_camera_pose_matches_extracted_cpp_full_transform_projection() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("slave_basis_cpp.json")).unwrap();
    let swap = Mat4::from_cols_array(&[
        1., 0., 0., 0., 0., 0., 1., 0., 0., 1., 0., 0., 0., 0., 0., 1.,
    ]);
    let projection = Mat4::from_cols_array(&[
        1.,
        0.,
        0.,
        0.,
        0.,
        4. / 3.,
        0.,
        0.,
        0.,
        0.,
        -100. / 98.,
        -1.,
        0.,
        0.,
        -200. / 98.,
        0.,
    ]);
    let mut cases = 0;
    for row in data["records"].as_array().unwrap() {
        let values = |key: &str| {
            row[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap() as f32)
                .collect::<Vec<_>>()
        };
        let r = values("source_rotation_row_major");
        let t = values("source_translation");
        let rotation = glam::Quat::from_mat3(&glam::Mat3::from_cols(
            Vec3::new(r[0], r[3], r[6]),
            Vec3::new(r[1], r[4], r[7]),
            Vec3::new(r[2], r[5], r[8]),
        ));
        let mut owner = state(
            model_with_camera([t[0], t[1], t[2]], rotation.to_array()),
            None,
        );
        let camera = owner.installed_camera_pose(999, "camera").unwrap();
        assert!(
            camera.determinant() < 0.,
            "world basis reflection must survive camera commitment"
        );
        let source_point = values("source_world_point");
        let host = swap * glam::Vec4::new(source_point[0], source_point[1], source_point[2], 1.);
        let mut receiving_view = crate::presentation_frame::SessionView::default();
        assert!(receiving_view.commit_native_slave_world(camera));
        let camera_point = receiving_view.camera.view_matrix * host;
        let expected = values("cpp_projected_gl");
        if row["cpp_result"].as_u64().unwrap() == 2 {
            assert!(camera_point.z > -2.);
        } else {
            let clip = projection * camera_point;
            let ndc = clip.truncate() / clip.w;
            let want = Vec3::new(expected[0], expected[1], 0.5 * expected[2] + 0.5);
            assert!(
                (ndc - want).abs().max_element() < 3.e - 5,
                "{ndc:?} != {want:?}"
            );
        }
        cases += 1;
    }
    assert_eq!(cases, 24);
}
#[test]
fn missing_bone_is_full_source_identity_without_hierarchy_or_animation_advance() {
    let mut owner = state(model_with_camera([10., 20., 30.], [0., 0., 0., 1.]), None);
    let before = owner.clone();
    let swap = Mat4::from_cols_array(&[
        1., 0., 0., 0., 0., 0., 1., 0., 0., 1., 0., 0., 0., 0., 0., 1.,
    ]);
    for name in ["", "ROOT", "missing"] {
        assert_eq!(owner.installed_camera_pose(200, name).unwrap(), swap);
        assert_eq!(owner, before);
    }
    assert_eq!(
        ObjectVisualState::default().installed_camera_pose(0, "CAMERA"),
        Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)
    );
    let mut absent = ObjectVisualState {
        installed_draw_pose: InstalledDrawObject::NoRenderObject,
        ..Default::default()
    };
    assert_eq!(
        absent.installed_camera_pose(0, "CAMERA"),
        Err(NativeCameraPoseUnavailable::NoRenderObject)
    );
}
#[test]
fn installed_hierarchy_validity_controls_lazy_sync_not_refresh_or_sync_identity() {
    let mut model = model_with_camera([0.; 3], [0., 0., 0., 1.]);
    model.animations.push(W3dAnimation {
        name: "MOVE".into(),
        hierarchy_name: "TREE".into(),
        num_frames: 4,
        frame_rate: 1,
        source_is_compressed: false,
        raw_visibility_channels: Vec::new(),
        unsupported_visibility_pivots: Vec::new(),
        channels: vec![W3dAnimChannel {
            first_frame: 0,
            last_frame: 3,
            vector_len: 1,
            flags: 0,
            pivot: 1,
            data: vec![0., 10., 20., 30.],
        }],
    });
    let mut owner = state(model, Some(crate::assets::W3dAnimationBinding::local(0)));
    owner.animation = Some(ObjectAnimationState {
        animation_binding_key: Some(crate::assets::W3dAnimationBindingKey::Local(0)),
        animation_identity: "TREE.MOVE".into(),
        current_frame: 0.,
        frame_rate: 1.,
        num_frames: 4,
        mode: crate::assets::AuthoredDrawAnimationMode::Loop,
        playback_mode: crate::assets::AuthoredDrawAnimationMode::Loop,
        last_sync_ms: 0,
        previous_frame: 0.,
        frame_rate_multiplier: 1.,
        direction: 1.,
    });
    let first = owner.installed_camera_pose(500, "CAMERA").unwrap();
    assert!((first.w_axis.x - 5.).abs() < 1.0e-5);
    let observed = owner.clone();
    assert_eq!(owner.installed_camera_pose(1000, "CAMERA").unwrap(), first);
    assert_eq!(
        owner, observed,
        "new Sync alone does not invalidate a valid HTree"
    );
    let InstalledDrawObject::Ready(pose) = &mut owner.installed_draw_pose else {
        panic!()
    };
    pose.install_transform(Mat4::IDENTITY); // equal Set_Transform still invalidates
    let second = owner.installed_camera_pose(1000, "CAMERA").unwrap();
    assert!((second.w_axis.x - 10.).abs() < 1.0e-5);
    assert_eq!(owner.installed_camera_pose(1000, "CAMERA").unwrap(), second);
}

#[test]
fn retained_hanim_survives_animationless_draw_and_is_saved_as_actual_motion() {
    let mut model = model_with_camera([0.; 3], [0., 0., 0., 1.]);
    model.animations.push(W3dAnimation {
        name: "MOVE".into(),
        hierarchy_name: "TREE".into(),
        num_frames: 4,
        frame_rate: 1,
        source_is_compressed: false,
        raw_visibility_channels: Vec::new(),
        unsupported_visibility_pivots: Vec::new(),
        channels: vec![W3dAnimChannel {
            first_frame: 0,
            last_frame: 3,
            vector_len: 1,
            flags: 0,
            pivot: 1,
            data: vec![0., 10., 20., 30.],
        }],
    });
    let mut owner = state(model, Some(crate::assets::W3dAnimationBinding::local(0)));
    owner.identity = Some(FrozenVisualDrawIdentity {
        source_template_name: "CameraObject".into(),
        model_key: "CameraModel".into(),
        selected_condition_state_index: 2,
        animation: None,
    });
    owner.animation = Some(ObjectAnimationState {
        animation_binding_key: Some(crate::assets::W3dAnimationBindingKey::Local(0)),
        animation_identity: "TREE.MOVE".into(),
        current_frame: 0.5,
        frame_rate: 1.,
        num_frames: 4,
        mode: crate::assets::AuthoredDrawAnimationMode::Loop,
        playback_mode: crate::assets::AuthoredDrawAnimationMode::Loop,
        last_sync_ms: 500,
        previous_frame: 0.,
        frame_rate_multiplier: 1.,
        direction: 1.,
    });
    let (binding, frame) = owner
        .advance_retained_motion_at_draw(1000, true)
        .expect("m_which=-1 does not remove installed HAnim");
    assert_eq!(
        binding.state_key(),
        crate::assets::W3dAnimationBindingKey::Local(0)
    );
    assert_eq!(frame, 0.5, "doDraw Peek does not advance the animation");
    assert_eq!(owner.admit_parent_render_hierarchy(1000).unwrap(), 1.);
    let saved =
        super::super::pipeline_drawable_state::snapshot_from_visual_state(42, 0, &owner).unwrap();
    let motion = saved.animation.as_ref().unwrap();
    assert_eq!(motion.hierarchy_animation, "TREE.MOVE");
    assert_eq!(motion.frame, 1.);
    assert_eq!(
        motion.mode,
        crate::save_load::snapshot::ClientDrawableAnimationMode::Loop
    );
    assert!(
        super::super::pipeline_drawable_state::visual_identity_matches_snapshot(
            owner.identity.as_ref().unwrap(),
            &saved
        )
    );
    let mut malformed = saved;
    malformed.animation.as_mut().unwrap().frame = f32::NAN;
    assert!(
        !super::super::pipeline_drawable_state::visual_identity_matches_snapshot(
            owner.identity.as_ref().unwrap(),
            &malformed
        )
    );
}

#[path = "installed_render_phase_tests.rs"]
mod render_phase_tests;
