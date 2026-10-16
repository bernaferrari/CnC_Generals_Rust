use super::*;
use crate::assets::AuthoredDrawAnimationMode as Mode;
fn animated_owner(
    mode: Mode,
    start: f32,
    last_sync: u32,
    count: u32,
    rate: f32,
    mult: f32,
) -> ObjectVisualState {
    let mut model = model_with_camera([0.; 3], [0., 0., 0., 1.]);
    model.animations.push(W3dAnimation {
        name: "MOVE".into(),
        hierarchy_name: "TREE".into(),
        num_frames: count,
        frame_rate: 30,
        source_is_compressed: false,
        raw_visibility_channels: Vec::new(),
        unsupported_visibility_pivots: Vec::new(),
        channels: Vec::new(),
    });
    let mut owner = state(model, Some(crate::assets::W3dAnimationBinding::local(0)));
    owner.animation = Some(ObjectAnimationState {
        animation_binding_key: Some(crate::assets::W3dAnimationBindingKey::Local(0)),
        animation_identity: "TREE.MOVE".into(),
        current_frame: start,
        frame_rate: rate,
        num_frames: count,
        mode: mode.clone(),
        playback_mode: mode.clone(),
        last_sync_ms: last_sync,
        previous_frame: -99.,
        frame_rate_multiplier: mult,
        direction: ObjectAnimationState::initial_direction(&mode),
    });
    owner
}
#[test]
fn actual_parent_render_matches_original_render_then_hierarchy_double_progress() {
    let modes = [
        Mode::Manual,
        Mode::Loop,
        Mode::Once,
        Mode::LoopPingPong,
        Mode::LoopBackwards,
        Mode::OnceBackwards,
    ];
    let mut count = 0;
    for row in include_str!("installed_render_cpp.csv").lines().skip(1) {
        let c: Vec<_> = row.split(',').collect();
        let u = |i: usize| c[i].parse::<u32>().unwrap();
        let f = |i: usize| c[i].parse::<f32>().unwrap();
        let mode = modes[u(0) as usize].clone();
        let mut owner = animated_owner(
            mode.clone(),
            f(3),
            u(4),
            if count % 8 == 6 { 1 } else { 12 },
            f(1),
            f(2),
        );
        let peer = owner.clone();
        let frame = owner.admit_parent_render_hierarchy(u(5)).unwrap();
        let animation = owner.animation.as_ref().unwrap();
        assert_eq!(frame.to_bits(), u(6), "Render+Update frame {row}");
        assert_eq!(animation.direction.to_bits(), u(8), "direction {row}");
        if !matches!(mode, Mode::Manual) {
            assert_eq!(animation.previous_frame.to_bits(), u(7), "previous {row}");
            assert_eq!(animation.last_sync_ms, u(9));
        } else {
            assert_eq!(
                animation.last_sync_ms,
                u(4),
                "manual hierarchy skips SingleAnimProgress"
            );
        }
        let frozen = owner.clone();
        let _ = owner
            .installed_camera_pose(u(5).wrapping_add(33), "CAMERA")
            .unwrap();
        assert_eq!(
            owner, frozen,
            "valid hierarchy cameraquery cannot recompute or advance"
        );
        assert_eq!(peer.animation.as_ref().unwrap().current_frame, f(3));
        count += 1;
    }
    assert_eq!(count, 48);
}
#[test]
fn camera_before_draw_and_render_are_distinct_owner_observation_phases() {
    let mut owner = animated_owner(Mode::LoopPingPong, 0., 100, 12, 30., 1.);
    owner.installed_camera_pose(1100, "CAMERA").unwrap();
    assert_eq!(owner.animation.as_ref().unwrap().current_frame, -8.);
    let camera_observed = owner.clone();
    // A cached camera observation leaves the original current-sync result.
    owner.installed_camera_pose(1100, "CAMERA").unwrap();
    assert_eq!(owner, camera_observed);
    // Actual Render executes even when the camera validated this same Sync.
    let frozen_frame = owner.admit_parent_render_hierarchy(1100).unwrap();
    assert_eq!(frozen_frame, 8.);
    assert_eq!(owner.animation.as_ref().unwrap().direction, 1.);
    assert_ne!(owner, camera_observed);
    let rendered = owner.clone();
    owner.installed_camera_pose(1133, "CAMERA").unwrap();
    assert_eq!(
        owner, rendered,
        "new Sync alone does not dirty installed HTree"
    );
}
