//! Versioned filter owner envelope; the previous native camera record is intact.
use super::*;

// The second source float would be NaN, so no valid legacy CameraState can
// accidentally claim this envelope's tag.
const TAG: &[u8; 8] = b"NCAM\xff\xff\xff\x7f";

#[cfg(feature = "game_client")]
#[derive(Serialize, Deserialize)]
struct FilterCameraState {
    version: u32,
    camera: Vec<u8>,
    scroll: glam::Vec2,
    previous_target: Vec3,
}

pub(super) fn encode_native_camera(
    engine: &CnCGameEngine,
    state: &CameraState,
) -> Result<Vec<u8>, String> {
    let camera = codec().serialize(state)?;
    #[cfg(feature = "game_client")]
    {
        encode_filter_camera(
            camera,
            engine.game_logic.session_view.camera.native_filter_scroll,
            engine
                .game_logic
                .session_view
                .camera
                .native_filter_previous_target,
        )
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = engine;
        Ok(camera)
    }
}

#[cfg(feature = "game_client")]
fn encode_filter_camera(
    camera: Vec<u8>,
    scroll: glam::Vec2,
    previous_target: Vec3,
) -> Result<Vec<u8>, String> {
    if !scroll.is_finite() || !previous_target.is_finite() {
        return Err("invalid camera filter scroll".into());
    }
    let payload = codec().serialize(&FilterCameraState {
        version: 1,
        camera,
        scroll,
        previous_target,
    })?;
    if payload.len() > MAX_BYTES - TAG.len() {
        return Err("camera continuation exceeds size limit".into());
    }
    let mut bytes = Vec::with_capacity(TAG.len() + payload.len());
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

pub(super) fn decode_native_camera(bytes: &[u8]) -> Result<NativeCameraContinuation, String> {
    #[cfg(feature = "game_client")]
    if let Some(payload) = bytes.strip_prefix(TAG) {
        let saved: FilterCameraState = codec().deserialize(payload)?;
        if saved.version != 1 {
            return Err("unsupported native camera phase version".into());
        }
        if !saved.scroll.is_finite() || !saved.previous_target.is_finite() {
            return Err("invalid camera filter scroll".into());
        }
        let camera: CameraState = codec().deserialize(&saved.camera)?;
        camera.validate()?;
        return Ok(NativeCameraContinuation(
            camera,
            Some((saved.scroll, saved.previous_target)),
        ));
    }
    let camera: CameraState = codec().deserialize(bytes)?;
    camera.validate()?;
    Ok(NativeCameraContinuation(
        camera,
        #[cfg(feature = "game_client")]
        None,
    ))
}

#[cfg(all(test, feature = "game_client"))]
mod tests {
    use super::*;
    use game_client::display::view::{FilterMode, FilterType, Point3, Vector2};

    #[test]
    fn filter_and_hag_resume_together_without_peer_or_extra_present_advancement() {
        let camera = super::super::tests::fixture();
        let legacy = codec().serialize(&camera).unwrap();
        let mut world = crate::game_logic::GameLogic::new();
        world.session_view.filter.set_filter(FilterType::MotionBlur);
        world
            .session_view
            .filter
            .set_mode(FilterMode::MBInAndOutAlpha);
        world
            .session_view
            .filter
            .set_pos(Point3::new(100., 200., 30.));
        for frame in 1..=5 {
            world
                .session_view
                .filter
                .post_render(frame, Vector2::zero());
        }
        let logic = world.capture_camera_continuation().unwrap();
        let bytes = encode_filter_camera(
            legacy.clone(),
            glam::Vec2::new(0.2, -0.3),
            Vec3::new(13., 0., 14.),
        )
        .unwrap();
        let decoded = decode_native_camera(&bytes).unwrap();
        assert_eq!(
            codec().serialize(&decoded.0).unwrap(),
            legacy,
            "HAG basis/goal and camera movement records are unchanged"
        );
        let (scroll, previous_target) = decoded.1.unwrap();
        assert_eq!(previous_target, Vec3::new(13., 0., 14.));
        assert_eq!(scroll, glam::Vec2::new(0.2, -0.3));
        let mut restored = crate::game_logic::GameLogic::new();
        restored.admit_camera_continuation(
            crate::game_logic::GameLogic::decode_camera_continuation(&logic).unwrap(),
        );
        let mut peer = crate::game_logic::GameLogic::new();
        let mut peaks = Vec::new();
        for frame in 6..=30 {
            peer.session_view.filter.reset();
            peer.session_view
                .filter
                .set_filter(FilterType::BlackAndWhite);
            peer.session_view.filter.set_fade_parameters(2, 1);
            peer.session_view.filter.tick_fade();
            let actual = world
                .session_view
                .filter
                .post_render(frame, Vector2::zero());
            let loaded = restored
                .session_view
                .filter
                .post_render(frame, Vector2::zero());
            assert_eq!(actual.motion_blur_count, loaded.motion_blur_count);
            assert_eq!(actual.look_at, loaded.look_at);
            assert_eq!(actual.continue_effect, loaded.continue_effect);
            if actual.look_at.is_some() {
                peaks.push(frame);
            }
            for _ in 0..3 {
                let repeat = restored
                    .session_view
                    .filter
                    .post_render(frame, Vector2::zero());
                assert!(repeat.look_at.is_none());
                assert_eq!(repeat.motion_blur_count, loaded.motion_blur_count);
            }
        }
        assert_eq!(peaks, vec![12]);
        assert_eq!(world.session_view.filter.filter(), FilterType::Null);
        assert_eq!(restored.session_view.filter.filter(), FilterType::Null);
    }

    #[test]
    fn legacy_hag_record_stays_readable_and_malformed_filter_is_not_a_fallback() {
        let camera = super::super::tests::fixture();
        let legacy = codec().serialize(&camera).unwrap();
        assert!(decode_native_camera(&legacy).unwrap().1.is_none());
        let bytes = encode_filter_camera(legacy.clone(), glam::Vec2::ZERO, Vec3::ZERO).unwrap();
        for len in [TAG.len(), bytes.len() - 1] {
            assert!(decode_native_camera(&bytes[..len]).is_err());
        }
        let mut unknown = bytes.clone();
        unknown[TAG.len()..TAG.len() + 4].copy_from_slice(&2u32.to_le_bytes());
        assert!(decode_native_camera(&unknown).is_err());
        assert!(encode_filter_camera(legacy, glam::Vec2::splat(f32::NAN), Vec3::ZERO).is_err());
    }
}
