use super::*;
use crate::assets::{
    AuthoredDrawAnimation, AuthoredDrawAnimationMode as Mode, W3DModel, W3dAnimation, W3dHierarchy,
    W3dPivot,
};
fn fixture() -> (
    ObjectVisualState,
    W3DModel,
    crate::assets::AuthoredDrawModel,
    ClientDrawableStateSnapshot,
) {
    let mut model = W3DModel::new("RESTORE_MODEL".into());
    model.hierarchy = Some(W3dHierarchy {
        name: "TREE".into(),
        pivot_fixups: Vec::new(),
        pivots: vec![
            W3dPivot {
                name: "ROOT".into(),
                parent_idx: u32::MAX,
                translation: [0.; 3],
                euler_angles: [0.; 3],
                rotation: [0., 0., 0., 1.],
            },
            W3dPivot {
                name: "CAMERA".into(),
                parent_idx: 0,
                translation: [10., 0., 0.],
                euler_angles: [0.; 3],
                rotation: [0., 0., 0., 1.],
            },
        ],
    });
    model.hlods.push(crate::assets::W3dHlod {
        version: 0x0001_0000,
        name: "RESTORE_MODEL".into(),
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
    model.animations.push(W3dAnimation {
        name: "MOVE".into(),
        hierarchy_name: "TREE".into(),
        num_frames: 4,
        frame_rate: 30,
        source_is_compressed: false,
        channels: Vec::new(),
        raw_visibility_channels: Vec::new(),
        unsupported_visibility_pivots: Vec::new(),
    });
    let selected = crate::assets::AuthoredDrawModel {
        model_key: "RESTORE_MODEL".into(),
        animation_mode: Mode::LoopBackwards,
        animations: vec![AuthoredDrawAnimation {
            name: "TREE.MOVE".into(),
            is_idle: false,
            distance_covered_token: None,
        }],
        ..Default::default()
    };
    let identity = frozen_visual_identity_for_draw_model("RestoreReceiver", &selected).unwrap();
    let binding = crate::assets::W3dAnimationBinding::local(0);
    let owner = ObjectVisualState {
        identity: Some(identity),
        animation_paused: true,
        animation_resume_mode: Some(Mode::Once),
        animation: Some(ObjectAnimationState {
            animation_binding_key: Some(binding.state_key()),
            animation_identity: "TREE.MOVE".into(),
            current_frame: 3.,
            frame_rate: 30.,
            num_frames: 4,
            mode: Mode::LoopBackwards,
            playback_mode: Mode::Manual,
            last_sync_ms: 7,
            previous_frame: 1.,
            frame_rate_multiplier: 9.,
            direction: 1.,
        }),
        installed_draw_pose: super::super::super::installed_draw_pose::InstalledDrawObject::Ready(
            super::super::super::installed_draw_pose::InstalledDrawPose::created(
                Arc::new(model.clone()),
                Mat4::IDENTITY,
                &selected,
                Some(binding),
            ),
        ),
        ..Default::default()
    };
    let saved = ClientDrawableStateSnapshot {
        object_id: 42,
        draw_module_index: 0,
        source_template_name: "RestoreReceiver".into(),
        model_key: "RESTORE_MODEL".into(),
        selected_condition_state_index: 0,
        animation: Some(ClientDrawableAnimationSnapshot {
            hierarchy_animation: "TREE.MOVE".into(),
            frame: 2.,
            mode: ClientDrawableAnimationMode::LoopBackwards,
        }),
        last_seen_weapon_discharge_sequence: 71,
        ..Default::default()
    };
    (owner, model, selected, saved)
}
#[test]
fn narrow_restore_rebases_receiving_animation_without_draw_side_effects() {
    let (mut owner, model, selected, saved) = fixture();
    let peer = owner.clone();
    let identity = owner.identity.clone().unwrap();
    let restored =
        restore_client_drawable_state(&mut owner, &identity, &model, &selected, &saved, 5000)
            .unwrap();
    assert_eq!(restored.frame, 2.);
    let a = owner.animation.as_ref().unwrap();
    assert_eq!(a.current_frame, 2.);
    assert_eq!(a.previous_frame, 3.);
    assert_eq!(a.last_sync_ms, 5000);
    assert_eq!(a.frame_rate_multiplier, 1.);
    assert_eq!(a.direction, 1.);
    assert!(matches!(a.mode, Mode::LoopBackwards));
    assert!(matches!(a.playback_mode, Mode::Manual));
    assert!(
        owner.animation_paused,
        "Xfer does not run setPauseAnimation"
    );
    assert_eq!(owner.animation_resume_mode, Some(Mode::Once));
    assert_eq!(owner.last_seen_weapon_discharge_sequence, 71);
    assert_eq!(owner.recoil_slots, [Vec::new(), Vec::new(), Vec::new()]);
    assert_eq!(peer.animation.as_ref().unwrap().current_frame, 3.);
    let super::super::super::installed_draw_pose::InstalledDrawObject::Ready(pose) =
        &owner.installed_draw_pose
    else {
        panic!()
    };
    assert!(!pose.hierarchy_valid);
    assert!(
        pose.controls.is_empty(),
        "recoil/turret controls remain later Draw work"
    );
    // The real HAnim clock cannot progress this receiving Manual playback,
    // even if sampled later; native bone query also skips Manual progression.
    owner.animation.as_mut().unwrap().admit_sync(6000);
    assert_eq!(owner.animation.as_ref().unwrap().current_frame, 2.);
    assert_eq!(owner.animation.as_ref().unwrap().previous_frame, 3.);
    assert!(matches!(
        owner.animation.as_ref().unwrap().playback_mode,
        Mode::Manual
    ));
    assert!(owner.animation_paused);
    assert_eq!(peer.animation.as_ref().unwrap().current_frame, 3.);
}
#[test]
fn malformed_or_foreign_restore_is_transactional_and_retained_motion_remains_supported() {
    let (mut owner, model, mut selected, saved) = fixture();
    let identity = owner.identity.clone().unwrap();
    let before = owner.clone();
    let mut foreign = saved.clone();
    foreign.model_key = "OTHER_MODEL".into();
    assert!(
        restore_client_drawable_state(&mut owner, &identity, &model, &selected, &foreign, 5000)
            .is_none()
    );
    assert_eq!(owner, before);
    let mut malformed = saved.clone();
    malformed.recoil_slots[0].push(ClientDrawableRecoilSnapshot::default());
    assert!(
        restore_client_drawable_state(&mut owner, &identity, &model, &selected, &malformed, 5000)
            .is_none()
    );
    assert_eq!(owner, before);
    selected.animations.clear();
    let retained = frozen_visual_identity_for_draw_model("RestoreReceiver", &selected).unwrap();
    assert!(
        restore_client_drawable_state(&mut owner, &retained, &model, &selected, &saved, 6000)
            .is_some()
    );
    assert!(owner.identity.as_ref().unwrap().animation.is_none());
    assert_eq!(owner.animation.as_ref().unwrap().current_frame, 2.);
    assert_eq!(owner.animation.as_ref().unwrap().last_sync_ms, 6000);
}

#[test]
fn pre_camera_candidate_consumption_preserves_other_objects_and_never_replays() {
    let (_, _, _, saved) = fixture();
    let mut peer = saved.clone();
    peer.object_id = 71;
    let mut pending = Some(ClientDrawableWorldSnapshot {
        drawables: vec![saved.clone(), peer.clone()],
    });
    let mut imports = HashMap::new();
    assert_eq!(
        take_native_restore_candidates(&mut pending, &mut imports, 42),
        vec![saved]
    );
    assert_eq!(pending.as_ref().unwrap().drawables, vec![peer.clone()]);
    assert!(take_native_restore_candidates(&mut pending, &mut imports, 42).is_empty());
    assert_eq!(
        take_native_restore_candidates(&mut pending, &mut imports, 71),
        vec![peer]
    );
    assert!(pending.is_none());
    assert!(imports.is_empty());
}
