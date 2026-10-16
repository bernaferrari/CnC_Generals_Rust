//! Actual logic owner capture/decode/admit, independent of Engine/GPU state.
use super::*;
use game_client::display::view::{FilterMode, FilterType, Point3, Vector2};

fn moving_filter() -> GameLogic {
    let mut world = GameLogic::new();
    world.session_view.target = Vec3::new(7., 8., 9.);
    world.request_path_camera_focus(Vec3::new(40., 50., 60.));
    world
        .session_view
        .filter
        .set_mode(FilterMode::MBInAndOutAlpha);
    world.session_view.filter.set_filter(FilterType::MotionBlur);
    world
        .session_view
        .filter
        .set_pos(Point3::new(10., 20., 30.));
    for frame in 1..7 {
        world
            .session_view
            .filter
            .post_render(frame, Vector2::zero());
    }
    world
}
#[test]
fn logic_camera_filter_owned_envelope_preserves_future_focus_and_peer() {
    let mut source = moving_filter();
    let mut peer = GameLogic::new();
    peer.session_view.filter.set_motion_blur_follow(17);
    let peer_before = codec().serialize(&peer.session_view.filter).unwrap();
    let bytes = source.capture_camera_continuation().unwrap();
    assert_eq!(&bytes[..4], b"\xffLCF");
    let decoded = GameLogic::decode_camera_continuation(&bytes).unwrap();
    let mut restored = GameLogic::new();
    restored.admit_camera_continuation(decoded);
    assert_eq!(
        codec().serialize(&peer.session_view.filter).unwrap(),
        peer_before,
        "admitting another world must not mutate the peer filter"
    );
    assert_eq!(
        restored.session_view.pending_focus_previous_target,
        Some(Vec3::new(7., 8., 9.))
    );
    assert_eq!(restored.pending_camera_focus, source.pending_camera_focus);
    for frame in 7..26 {
        let a = source
            .session_view
            .filter
            .post_render(frame, Vector2::zero());
        peer.session_view
            .filter
            .post_render(frame, Vector2::new(0.1, 0.));
        let b = restored
            .session_view
            .filter
            .post_render(frame, Vector2::zero());
        assert_eq!(a.motion_blur_count, b.motion_blur_count);
        assert_eq!(a.look_at, b.look_at);
        assert_eq!(a.continue_effect, b.continue_effect);
    }
    assert_ne!(
        codec().serialize(&peer.session_view.filter).unwrap(),
        peer_before
    );
    assert_eq!(peer.session_view.pending_focus_previous_target, None);
}
#[test]
fn logic_camera_filter_legacy_record_bytes_keep_original_layout_and_default_extras() {
    let source = moving_filter();
    let current =
        GameLogic::decode_camera_continuation(&source.capture_camera_continuation().unwrap())
            .unwrap();
    let legacy = codec().serialize(&current).unwrap();
    assert!(matches!(legacy.first(), Some(0 | 1)));
    let decoded = GameLogic::decode_camera_continuation(&legacy).unwrap();
    assert_eq!(codec().serialize(&decoded).unwrap(), legacy);
    let mut restored = GameLogic::new();
    restored
        .session_view
        .filter
        .set_filter(FilterType::BlackAndWhite);
    restored.admit_camera_continuation(decoded);
    assert_eq!(restored.session_view.filter.filter(), FilterType::Null);
    assert_eq!(restored.session_view.pending_focus_previous_target, None);
}
#[test]
fn logic_camera_filter_malformed_envelope_and_nonfinite_extras_reject_before_admission() {
    let source = moving_filter();
    let bytes = source.capture_camera_continuation().unwrap();
    let mut version = bytes.clone();
    version[4..8].copy_from_slice(&99u32.to_le_bytes());
    assert!(GameLogic::decode_camera_continuation(&version).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(GameLogic::decode_camera_continuation(&trailing).is_err());
    for len in 1..8 {
        assert!(GameLogic::decode_camera_continuation(&bytes[..len]).is_err());
    }
    let mut state = GameLogic::decode_camera_continuation(&bytes).unwrap();
    state
        .owned_extras
        .filter
        .set_pos(Point3::new(f32::NAN, 0., 0.));
    let invalid = owned_envelope::encode(&state).unwrap();
    assert!(GameLogic::decode_camera_continuation(&invalid).is_err());
    let mut state = GameLogic::decode_camera_continuation(&bytes).unwrap();
    state.owned_extras.pending_focus_previous_target = Some(Vec3::splat(f32::NAN));
    assert!(
        GameLogic::decode_camera_continuation(&owned_envelope::encode(&state).unwrap()).is_err()
    );
    let mut live = moving_filter();
    live.session_view
        .filter
        .set_pos(Point3::new(f32::NAN, 0., 0.));
    assert!(live.capture_camera_continuation().is_err());
}

// Deliberately mirrors the positional filter record to construct malformed
// input without exposing mutation-only APIs on the production owner.
#[derive(Serialize, Deserialize)]
struct FilterWire {
    filter: FilterType,
    mode: FilterMode,
    position: Option<Point3>,
    fade_total: i32,
    fade_progress: i32,
    fade_direction: i32,
    count: i32,
    decrement: bool,
    last_logic_frame: u32,
    skip_render: bool,
    prior_scroll: Vector2,
    pan_factor: i32,
}

fn filter_wire(world: &GameLogic) -> FilterWire {
    let bytes = codec().serialize(&world.session_view.filter).unwrap();
    let wire = codec().deserialize(&bytes).unwrap();
    assert_eq!(
        codec().serialize(&wire).unwrap(),
        bytes,
        "fixture wire layout"
    );
    wire
}

#[test]
fn logic_camera_filter_impossible_phase_rejects_before_receiving_owner_admission() {
    let source = moving_filter();
    let mut receiving = moving_filter();
    let peer = moving_filter();
    let before = receiving.capture_camera_continuation().unwrap();
    let peer_before = peer.capture_camera_continuation().unwrap();
    for case in 0..4 {
        let mut wire = filter_wire(&source);
        if case < 2 {
            wire.filter = FilterType::BlackAndWhite;
            wire.mode = FilterMode::BWBlackAndWhite;
            wire.fade_total = i32::MAX;
            wire.fade_progress = i32::MAX;
            wire.fade_direction = if case == 0 { 1 } else { -1 };
        } else {
            wire.pan_factor = i32::MAX;
            wire.count = if case == 2 { 61 } else { i32::MAX };
        }
        let mut state = GameLogic::decode_camera_continuation(&before).unwrap();
        state.owned_extras.filter = codec()
            .deserialize(&codec().serialize(&wire).unwrap())
            .unwrap();
        let bytes = owned_envelope::encode(&state).unwrap();
        let result = GameLogic::decode_camera_continuation(&bytes).map(|candidate| {
            receiving.admit_camera_continuation(candidate);
        });
        assert!(
            result.is_err(),
            "impossible filter phase {case} must not be admitted"
        );
        assert_eq!(receiving.capture_camera_continuation().unwrap(), before);
        assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    }
}

#[test]
fn logic_camera_filter_boundary_phases_resume_without_counter_overflow() {
    let source = moving_filter();
    let saved = source.capture_camera_continuation().unwrap();
    for case in 0..5 {
        let mut wire = filter_wire(&source);
        wire.filter = FilterType::BlackAndWhite;
        wire.mode = FilterMode::BWBlackAndWhite;
        wire.fade_total = if case < 2 { 0 } else { i32::MAX };
        wire.fade_progress = if case < 2 { 0 } else { i32::MAX - 1 };
        wire.fade_direction = if case % 2 == 0 { -1 } else { 1 };
        if case == 4 {
            wire.filter = FilterType::MotionBlur;
            wire.mode = FilterMode::MBEndPanAlpha;
            wire.fade_total = 0;
            wire.fade_progress = 0;
            wire.fade_direction = 0;
            wire.pan_factor = i32::MAX;
            wire.count = i32::MAX;
        }
        let mut state = GameLogic::decode_camera_continuation(&saved).unwrap();
        state.owned_extras.filter = codec()
            .deserialize(&codec().serialize(&wire).unwrap())
            .unwrap();
        let decoded =
            GameLogic::decode_camera_continuation(&owned_envelope::encode(&state).unwrap())
                .unwrap();
        let mut receiving = GameLogic::new();
        receiving.admit_camera_continuation(decoded);
        receiving.session_view.filter.tick_fade();
        let next = receiving
            .session_view
            .filter
            .post_render(7, Vector2::zero());
        if case == 4 {
            assert_eq!(next.motion_blur_count, i32::MAX - 1);
            assert!(next.pan);
        } else {
            assert_eq!(
                receiving.session_view.filter.filter(),
                if case % 2 == 0 {
                    FilterType::Null
                } else {
                    FilterType::BlackAndWhite
                }
            );
        }
        receiving.capture_camera_continuation().unwrap();
    }
}
