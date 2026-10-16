use super::*;
use crate::assets::AuthoredDrawAnimationMode as Mode;
fn state(mode: Mode, frame: f32, sync: u32) -> ObjectAnimationState {
    ObjectAnimationState {
        animation_binding_key: None,
        animation_identity: "oracle.clip".into(),
        current_frame: frame,
        previous_frame: -99.,
        frame_rate: 30.,
        num_frames: 12,
        direction: ObjectAnimationState::initial_direction(&mode),
        mode: mode.clone(),
        last_sync_ms: sync,
        frame_rate_multiplier: 1.,
        playback_mode: mode.clone(),
    }
}
#[test]
fn actual_animation_owner_matches_extracted_cpp_single_anim_progress_bits() {
    let modes = [
        Mode::Manual,
        Mode::Loop,
        Mode::Once,
        Mode::LoopPingPong,
        Mode::LoopBackwards,
        Mode::OnceBackwards,
    ];
    let mut count = 0;
    for row in include_str!("animation_sync_cpp.csv").lines().skip(1) {
        let c: Vec<_> = row.split(',').collect();
        let f = |i: usize| c[i].parse::<f32>().unwrap();
        let u = |i: usize| c[i].parse::<u32>().unwrap();
        let mut a = state(modes[u(0) as usize].clone(), f(3), u(5));
        a.frame_rate = f(1);
        a.set_frame_rate_multiplier(f(2));
        a.direction = f(4);
        a.num_frames = if count % 8 == 6 { 1 } else { 12 };
        a.admit_sync(u(6));
        assert_eq!(a.current_frame.to_bits(), u(7), "frame {row}");
        assert_eq!(a.previous_frame.to_bits(), u(8), "previous {row}");
        assert_eq!(a.direction.to_bits(), u(9), "direction {row}");
        assert_eq!(a.last_sync_ms, u(10));
        a.admit_sync(u(6));
        assert_eq!(a.current_frame.to_bits(), u(11), "repeat frame {row}");
        assert_eq!(a.previous_frame.to_bits(), u(12), "repeat previous {row}");
        assert_eq!(a.direction.to_bits(), u(13), "repeat direction {row}");
        count += 1;
    }
    assert_eq!(count, 48);
}
#[test]
fn pause_reads_stored_frame_and_resets_source_baseline_without_sampling() {
    let mut a = state(Mode::Loop, 3., 100);
    a.set_frame_rate_multiplier(2.);
    let mut owner = ObjectVisualState {
        animation: Some(a),
        ..Default::default()
    };
    owner.install_animation_pause(true, 200);
    let a = owner.animation.as_mut().unwrap();
    assert_eq!(a.current_frame, 3., "Peek returns stored frame");
    assert_eq!(a.frame_rate_multiplier, 1.);
    a.admit_sync(900);
    assert_eq!(a.current_frame, 3.);
    owner.install_animation_pause(false, 900);
    let a = owner.animation.as_mut().unwrap();
    a.admit_sync(933);
    assert_eq!(
        a.current_frame.to_bits(),
        (3.0f32 + 30. * 33. * 0.001).to_bits()
    );
    let mut queried = state(Mode::Loop, 3., 100);
    queried.admit_sync(200);
    let mut owner = ObjectVisualState {
        animation: Some(queried),
        ..Default::default()
    };
    owner.install_animation_pause(true, 200);
    let queried = owner.animation.as_ref().unwrap();
    assert_eq!(
        queried.current_frame, 6.,
        "prior bone query updates stored frame"
    );
}
#[test]
fn binding_install_resets_source_direction_and_multiplier_at_receiving_sync() {
    let mut a = state(Mode::LoopPingPong, 10., 100);
    a.direction = -1.;
    a.set_frame_rate_multiplier(7.);
    a.install_animation_clock(900);
    assert_eq!(a.current_frame, 10.);
    assert_eq!(a.last_sync_ms, 900);
    assert_eq!(a.direction, 1.);
    assert_eq!(a.frame_rate_multiplier, 1.);
    let peer = state(Mode::Loop, 5., 17);
    a.admit_sync(933);
    assert_eq!(peer.current_frame, 5.);
    assert_eq!(peer.last_sync_ms, 17);
}

#[test]
fn module_pause_latch_survives_binding_install_and_restores_cached_old_mode() {
    let mut owner = ObjectVisualState {
        animation: Some(state(Mode::LoopBackwards, 9., 100)),
        ..Default::default()
    };
    owner.install_animation_pause(true, 133);
    assert!(owner.animation_paused);
    assert!(matches!(
        owner.animation.as_ref().unwrap().playback_mode,
        Mode::Manual
    ));
    // dirty-condition setModelState installs a new animation, but does not clear
    // W3DModelDraw's pause flag or cached resume mode.
    owner.animation = Some(state(Mode::Once, 2., 200));
    owner.install_animation_pause(true, 233);
    assert!(matches!(
        owner.animation.as_ref().unwrap().playback_mode,
        Mode::Once
    ));
    owner.animation.as_mut().unwrap().admit_sync(233);
    assert_eq!(
        owner.animation.as_ref().unwrap().current_frame.to_bits(),
        (2.0f32 + 30. * 33. * 0.001).to_bits()
    );
    owner.install_animation_pause(false, 266);
    let a = owner.animation.as_ref().unwrap();
    assert!(matches!(a.playback_mode, Mode::LoopBackwards));
    assert_eq!(a.direction, -1.);
    assert_eq!(a.last_sync_ms, 266);
}
