use super::*;
fn zoom() -> ViewFilterState {
    let mut state = ViewFilterState::new();
    state.set_mode(FilterMode::MBInAndOutAlpha);
    state.set_filter(FilterType::MotionBlur);
    state.set_pos(Point3::new(10., 20., 30.));
    state
}
#[test]
fn nonpan_uses_distinct_logic_frames_and_returns_peak_once_without_mutating_peer() {
    let mut a = zoom();
    let peer = zoom();
    assert_eq!(a.post_render(0, Vector2::zero()).motion_blur_count, 0);
    for frame in 1..12 {
        let step = a.post_render(frame, Vector2::zero());
        assert_eq!(step.motion_blur_count, frame as i32 * COUNT_STEP);
        assert!(step.look_at.is_none());
        let repeat = a.post_render(frame, Vector2::zero());
        assert_eq!(repeat.motion_blur_count, step.motion_blur_count);
        assert!(repeat.look_at.is_none());
    }
    let peak = a.post_render(12, Vector2::zero());
    assert_eq!(peak.look_at, Some(Point3::new(10., 20., 30.)));
    assert!(peak.continue_effect);
    assert!(!peak.skip_render);
    assert!(a.post_render(12, Vector2::zero()).look_at.is_none());
    for frame in 13..24 {
        assert!(a.post_render(frame, Vector2::zero()).continue_effect);
    }
    assert!(!a.post_render(24, Vector2::zero()).continue_effect);
    assert_eq!(a.filter(), FilterType::Null);
    assert_eq!(peer.count, 0);
}
#[test]
fn out_setup_and_no_zoom_peak_end_at_original_boundaries() {
    let mut out = ViewFilterState::new();
    out.set_mode(FilterMode::MBOutAlpha);
    out.set_filter(FilterType::MotionBlur);
    assert_eq!(out.count, MOTION_BLUR_MAX_COUNT);
    assert_eq!(out.post_render(1, Vector2::zero()).motion_blur_count, 55);
    let mut inward = ViewFilterState::new();
    inward.set_mode(FilterMode::MBInAlpha);
    inward.set_filter(FilterType::MotionBlur);
    for frame in 1..12 {
        assert!(inward.post_render(frame, Vector2::zero()).continue_effect);
    }
    let peak = inward.post_render(12, Vector2::zero());
    assert!(!peak.continue_effect);
    assert!(peak.look_at.is_none());
}
#[test]
fn pan_and_endpan_preserve_count_and_prior_scroll_without_frame_gate() {
    let mut state = ViewFilterState::new();
    state.set_motion_blur_follow(30);
    let scroll = Vector2::new(0.1, 0.);
    let pan = state.post_render(1, scroll);
    assert_eq!(pan.motion_blur_count, 20);
    assert!(pan.pan);
    state.set_mode(FilterMode::MBEndPanAlpha);
    let end = state.post_render(1, Vector2::zero());
    assert_eq!(end.motion_blur_count, 19);
    assert_eq!(end.scroll_delta, scroll);
    assert!(end.pan);
    for expected in (2..19).rev() {
        assert_eq!(
            state.post_render(1, Vector2::zero()).motion_blur_count,
            expected
        );
    }
    assert!(!state.post_render(1, Vector2::zero()).continue_effect);
}
#[test]
fn fade_zero_start_can_advance_after_resource_admission() {
    let mut state = ViewFilterState::new();
    state.set_mode(FilterMode::BWBlackAndWhite);
    state.set_filter(FilterType::BlackAndWhite);
    state.set_fade_parameters(4, 1);
    assert_eq!(state.composite(Vector2::zero()).fade, 0.);
    state.tick_fade();
    assert_eq!(state.post_render(0, Vector2::zero()).fade, 0.25);
    state.set_fade_parameters(4, -1);
    for _ in 0..4 {
        state.tick_fade();
    }
    assert_eq!(state.filter(), FilterType::Null);
}
#[test]
fn serialized_owner_preserves_future_without_render_resources() {
    let mut owner = zoom();
    for frame in 1..8 {
        owner.post_render(frame, Vector2::zero());
    }
    let bytes = bincode_legacy::serialize(&owner).unwrap();
    let mut restored: ViewFilterState = bincode_legacy::deserialize(&bytes).unwrap();
    restored.validate().unwrap();
    for frame in 8..26 {
        let a = owner.post_render(frame, Vector2::zero());
        let b = restored.post_render(frame, Vector2::zero());
        assert_eq!(a.motion_blur_count, b.motion_blur_count);
        assert_eq!(a.look_at, b.look_at);
        assert_eq!(a.continue_effect, b.continue_effect);
    }
}

#[test]
fn zero_duration_bw_fade_completes_on_first_admitted_render() {
    // ScreenBWFilter::set increments the frame before comparing fadeFrames,
    // so zero (and normalized negative) durations complete on that render.
    for frames in [0, -1] {
        let mut state = ViewFilterState::new();
        state.set_mode(FilterMode::BWBlackAndWhite);
        state.set_filter(FilterType::BlackAndWhite);
        state.set_fade_parameters(frames, 1);
        state.tick_fade();
        assert_eq!(state.filter(), FilterType::BlackAndWhite);
        assert_eq!(state.composite(Vector2::zero()).fade, 1.);
        state.set_fade_parameters(frames, -1);
        assert_eq!(state.filter(), FilterType::BlackAndWhite);
        state.tick_fade();
        assert_eq!(state.filter(), FilterType::Null);
        assert_eq!(state.mode(), FilterMode::Null);
        assert_eq!(state.composite(Vector2::zero()).fade, 0.);
        state.validate().unwrap();
    }
}

#[test]
fn authored_bw_blur_bw_preserves_inactive_bw_phase_and_full_blur() {
    // ScreenBWFilter::set alone advances its fade. ScreenMotionBlurFilter
    // uses its own count and never consumes BW's pending fade clock.
    let mut state = ViewFilterState::new();
    state.set_mode(FilterMode::BWBlackAndWhite);
    state.set_filter(FilterType::BlackAndWhite);
    state.set_fade_parameters(4, 1);
    state.tick_fade();
    assert_eq!(state.composite(Vector2::zero()).fade, 0.25);
    state.set_filter(FilterType::MotionBlur);
    state.set_mode(FilterMode::MBInAndOutAlpha);
    state.set_pos(Point3::new(1., 2., 3.));
    for frame in 1..=3 {
        state.tick_fade();
        assert_eq!(state.composite(Vector2::zero()).fade, 1.);
        assert_eq!(state.post_render(frame, Vector2::zero()).fade, 1.);
    }
    assert_eq!(state.fade_progress, 1);
    state.set_mode(FilterMode::BWBlackAndWhite);
    state.set_filter(FilterType::BlackAndWhite);
    assert_eq!(state.composite(Vector2::zero()).fade, 0.25);
    state.tick_fade();
    assert_eq!(state.composite(Vector2::zero()).fade, 0.5);
}

#[test]
fn existing_crossfade_adapter_still_consumes_its_active_fade() {
    let mut state = ViewFilterState::new();
    state.set_mode(FilterMode::CrossfadeFbMask);
    state.set_filter(FilterType::Crossfade);
    state.set_fade_parameters(4, 1);
    state.tick_fade();
    assert_eq!(state.composite(Vector2::zero()).fade, 0.25);
    state.set_filter(FilterType::Null);
    state.tick_fade();
    assert_eq!(state.fade_progress, 1);
    assert_eq!(state.composite(Vector2::zero()).fade, 0.);
}

#[test]
fn saved_filter_phase_rejects_completed_fade_but_accepts_last_frame_and_zero_duration() {
    for filter in [
        FilterType::BlackAndWhite,
        FilterType::Crossfade,
        FilterType::MotionBlur,
    ] {
        for direction in [-1, 1] {
            let mut state = zoom();
            state.filter = filter;
            state.fade_total = i32::MAX;
            state.fade_progress = i32::MAX;
            state.fade_direction = direction;
            assert!(state.validate().is_err(), "completed {filter:?} fade");
            state.fade_progress -= 1;
            state.validate().unwrap();
            if state.uses_fade() {
                state.tick_fade();
                assert_eq!(state.fade_progress, 0);
                assert_eq!(state.fade_direction, 0);
                state.validate().unwrap();
            }
            state.set_fade_parameters(0, direction);
            state.validate().unwrap();
        }
    }
}

#[test]
fn saved_filter_phase_separates_nonpan_count_limit_from_retained_pan_limit() {
    let mut state = zoom();
    state.pan_factor = i32::MAX;
    for mode in [
        FilterMode::Null,
        FilterMode::MBInAndOutAlpha,
        FilterMode::MBInAndOutSaturate,
        FilterMode::MBInAlpha,
        FilterMode::MBOutAlpha,
        FilterMode::MBInSaturate,
        FilterMode::MBOutSaturate,
    ] {
        state.mode = mode;
        for count in [MOTION_BLUR_MAX_COUNT + 1, i32::MAX] {
            state.count = count;
            assert!(state.validate().is_err(), "{mode:?} count {count}");
        }
        state.count = MOTION_BLUR_MAX_COUNT;
        state.validate().unwrap();
    }
    for (mode, pan_factor) in [
        (FilterMode::MBPanAlpha, i32::MAX),
        (FilterMode::MBPanAlpha1, 1),
        (FilterMode::MBPanAlpha2, 2),
        (FilterMode::MBPanAlpha3, 3),
        (FilterMode::MBEndPanAlpha, i32::MAX),
    ] {
        state.mode = mode;
        state.pan_factor = pan_factor;
        state.count = pan_factor;
        state.validate().unwrap();
        let step = state.post_render(1, Vector2::zero());
        assert!(step.pan);
        state.validate().unwrap();
    }
    state.filter = FilterType::BlackAndWhite;
    state.mode = FilterMode::BWBlackAndWhite;
    state.count = i32::MAX;
    state.validate().unwrap();
    state.set_filter(FilterType::MotionBlur);
    state.set_mode(FilterMode::MBInAndOutAlpha);
    assert_eq!(
        state.count, 0,
        "new nonpan setup discards retained PAN count"
    );
    state.validate().unwrap();
}
