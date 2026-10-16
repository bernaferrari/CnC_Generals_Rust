//! Versioned native camera observations; Native85 and envelope V1 stay readable.
use super::*;

// The second source float would be NaN, so no valid legacy CameraState can
// accidentally claim this envelope's tag. Fixed-integer version follows it.
const TAG: &[u8; 8] = b"NCAM\xff\xff\xff\x7f";

#[derive(Serialize, Deserialize)]
struct FilterCameraState {
    version: u32,
    camera: Vec<u8>,
    scroll: glam::Vec2,
    previous_target: Vec3,
}

/// A new envelope version, never an extension to a historical positional DTO.
/// The optional filter has one wire layout in GameClient and headless builds.
#[derive(Serialize, Deserialize)]
struct CompletedCameraState {
    version: u32,
    camera: Vec<u8>,
    completed_view: glam::Mat4,
    filter: Option<(glam::Vec2, Vec3)>,
}

pub(super) fn encode_native_camera(
    engine: &CnCGameEngine,
    state: &CameraState,
) -> Result<Vec<u8>, String> {
    let filter = {
        #[cfg(feature = "game_client")]
        {
            Some((
                engine.game_logic.session_view.camera.native_filter_scroll,
                engine
                    .game_logic
                    .session_view
                    .camera
                    .native_filter_previous_target,
            ))
        }
        #[cfg(not(feature = "game_client"))]
        {
            None
        }
    };
    encode_completed_camera(
        state,
        engine.game_logic.session_view.camera.view_matrix,
        filter,
    )
}

fn validate_filter(filter: Option<(glam::Vec2, Vec3)>) -> Result<(), String> {
    if filter.is_some_and(|(scroll, previous_target)| {
        !scroll.is_finite() || !previous_target.is_finite()
    }) {
        return Err("invalid camera filter scroll".into());
    }
    #[cfg(not(feature = "game_client"))]
    if filter.is_some() {
        return Err("saved native camera filter requires the GameClient feature".into());
    }
    Ok(())
}

fn tagged(payload: Vec<u8>) -> Result<Vec<u8>, String> {
    if payload.len() > MAX_BYTES - TAG.len() {
        return Err("camera continuation exceeds size limit".into());
    }
    let mut bytes = Vec::with_capacity(TAG.len() + payload.len());
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn encode_completed_camera(
    state: &CameraState,
    completed_view: glam::Mat4,
    filter: Option<(glam::Vec2, Vec3)>,
) -> Result<Vec<u8>, String> {
    state.validate_with_completed_view(Some(completed_view))?;
    validate_filter(filter)?;
    tagged(codec().serialize(&CompletedCameraState {
        version: 2,
        camera: codec().serialize(state)?,
        completed_view,
        filter,
    })?)
}

// Retained only to exercise the original version1 wire; live capture is V2.
#[cfg(all(test, feature = "game_client"))]
fn encode_filter_camera(
    camera: Vec<u8>,
    scroll: glam::Vec2,
    previous_target: Vec3,
) -> Result<Vec<u8>, String> {
    validate_filter(Some((scroll, previous_target)))?;
    tagged(codec().serialize(&FilterCameraState {
        version: 1,
        camera,
        scroll,
        previous_target,
    })?)
}

pub(super) fn decode_native_camera(bytes: &[u8]) -> Result<NativeCameraContinuation, String> {
    if let Some(payload) = bytes.strip_prefix(TAG) {
        // CameraCodec uses bincode legacy fixed little-endian integers. Read
        // only the discriminator, then require full consumption by its DTO.
        let version = payload
            .get(..4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .ok_or_else(|| "truncated native camera phase version".to_owned())?;
        let (camera_bytes, extras) = match version {
            1 => {
                let saved: FilterCameraState = codec().deserialize(payload)?;
                let filter = Some((saved.scroll, saved.previous_target));
                validate_filter(filter)?;
                (
                    saved.camera,
                    NativeCameraExtras {
                        filter,
                        completed_view: None,
                    },
                )
            }
            2 => {
                let saved: CompletedCameraState = codec().deserialize(payload)?;
                validate_filter(saved.filter)?;
                (
                    saved.camera,
                    NativeCameraExtras {
                        filter: saved.filter,
                        completed_view: Some(saved.completed_view),
                    },
                )
            }
            _ => return Err("unsupported native camera phase version".into()),
        };
        let camera: CameraState = codec().deserialize(&camera_bytes)?;
        camera.validate_with_completed_view(extras.completed_view)?;
        return Ok(NativeCameraContinuation(camera, extras));
    }
    let camera: CameraState = codec().deserialize(bytes)?;
    camera.validate()?;
    Ok(NativeCameraContinuation(
        camera,
        NativeCameraExtras::default(),
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
        let (scroll, previous_target) = decoded.1.filter.unwrap();
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
        assert!(decode_native_camera(&legacy).unwrap().1.filter.is_none());
        let bytes = encode_filter_camera(legacy.clone(), glam::Vec2::ZERO, Vec3::ZERO).unwrap();
        for len in [TAG.len(), bytes.len() - 1] {
            assert!(decode_native_camera(&bytes[..len]).is_err());
        }
        let mut unknown = bytes.clone();
        unknown[TAG.len()..TAG.len() + 4].copy_from_slice(&3u32.to_le_bytes());
        assert!(decode_native_camera(&unknown).is_err());
        assert!(encode_filter_camera(legacy, glam::Vec2::splat(f32::NAN), Vec3::ZERO).is_err());
    }
}

#[cfg(test)]
#[path = "completed_matrix_tests.rs"]
mod completed_matrix_tests;
