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

#[derive(Serialize, Deserialize)]
struct FollowCameraState {
    base: CompletedCameraState,
    follow_snap_immediate: bool,
}

#[derive(Serialize, Deserialize)]
struct ConstrainedCameraState {
    base: FollowCameraState,
    constraints: crate::presentation_frame::camera::constraints::CameraConstraints,
}

#[derive(Serialize, Deserialize)]
struct OwnedCameraState {
    base: ConstrainedCameraState,
    camera_settings: crate::presentation_frame::camera::settings::CameraSettings,
}

#[derive(Serialize, Deserialize)]
struct DefinedCameraState {
    base: OwnedCameraState,
    clip_policy: crate::presentation_frame::camera::clip::CameraClipPolicy,
    shake_policy: crate::presentation_frame::camera::shake_rules::CameraShakePolicy,
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
    encode_defined_camera(
        state,
        engine.game_logic.session_view.camera.view_matrix,
        filter,
        engine
            .game_logic
            .session_view
            .camera
            .camera_follow_snap_immediate,
        engine.game_logic.session_view.camera.constraints,
        engine.game_logic.session_view.camera.settings,
        engine.game_logic.session_view.camera.clip_policy,
        engine.game_logic.session_view.camera.shake_policy,
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

fn encode_follow_camera(
    state: &CameraState,
    completed_view: glam::Mat4,
    filter: Option<(glam::Vec2, Vec3)>,
    follow_snap_immediate: bool,
) -> Result<Vec<u8>, String> {
    state.validate_with_completed_view(Some(completed_view))?;
    validate_filter(filter)?;
    tagged(codec().serialize(&FollowCameraState {
        base: CompletedCameraState {
            version: 3,
            camera: codec().serialize(state)?,
            completed_view,
            filter,
        },
        follow_snap_immediate,
    })?)
}

fn encode_constrained_camera(
    state: &CameraState,
    completed_view: glam::Mat4,
    filter: Option<(glam::Vec2, Vec3)>,
    snap: bool,
    constraints: crate::presentation_frame::camera::constraints::CameraConstraints,
) -> Result<Vec<u8>, String> {
    state.validate_with_completed_view(Some(completed_view))?;
    validate_filter(filter)?;
    constraints.validate()?;
    tagged(codec().serialize(&ConstrainedCameraState {
        base: FollowCameraState {
            base: CompletedCameraState {
                version: 4,
                camera: codec().serialize(state)?,
                completed_view,
                filter,
            },
            follow_snap_immediate: snap,
        },
        constraints,
    })?)
}

fn encode_owned_camera(
    state: &CameraState,
    completed_view: glam::Mat4,
    filter: Option<(glam::Vec2, Vec3)>,
    snap: bool,
    constraints: crate::presentation_frame::camera::constraints::CameraConstraints,
    settings: crate::presentation_frame::camera::settings::CameraSettings,
) -> Result<Vec<u8>, String> {
    state.validate_with_completed_view(Some(completed_view))?;
    validate_filter(filter)?;
    constraints.validate()?;
    settings.validate()?;
    tagged(codec().serialize(&OwnedCameraState {
        base: ConstrainedCameraState {
            base: FollowCameraState {
                base: CompletedCameraState {
                    version: 5,
                    camera: codec().serialize(state)?,
                    completed_view,
                    filter,
                },
                follow_snap_immediate: snap,
            },
            constraints,
        },
        camera_settings: settings,
    })?)
}

fn encode_defined_camera(
    state: &CameraState,
    completed_view: glam::Mat4,
    filter: Option<(glam::Vec2, Vec3)>,
    snap: bool,
    constraints: crate::presentation_frame::camera::constraints::CameraConstraints,
    settings: crate::presentation_frame::camera::settings::CameraSettings,
    clip_policy: crate::presentation_frame::camera::clip::CameraClipPolicy,
    shake_policy: crate::presentation_frame::camera::shake_rules::CameraShakePolicy,
) -> Result<Vec<u8>, String> {
    shake_policy.validate()?;
    // Reuse the historical V5 validation/schema, then wrap it without changing it.
    let old = encode_owned_camera(state, completed_view, filter, snap, constraints, settings)?;
    let mut base: OwnedCameraState = codec().deserialize(&old[TAG.len()..])?;
    base.base.base.base.version = 6;
    tagged(codec().serialize(&DefinedCameraState {
        base,
        clip_policy,
        shake_policy,
    })?)
}

// Retained only to exercise the original version1 wire; live capture is V3.
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
                        clip_policy: None,
                        shake_policy: None,
                        camera_settings: None,
                        filter,
                        completed_view: None,
                        follow_snap_immediate: false,
                        constraints: Default::default(),
                    },
                )
            }
            2 => {
                let saved: CompletedCameraState = codec().deserialize(payload)?;
                validate_filter(saved.filter)?;
                (
                    saved.camera,
                    NativeCameraExtras {
                        clip_policy: None,
                        shake_policy: None,
                        camera_settings: None,
                        filter: saved.filter,
                        completed_view: Some(saved.completed_view),
                        follow_snap_immediate: false,
                        constraints: Default::default(),
                    },
                )
            }
            3 => {
                let saved: FollowCameraState = codec().deserialize(payload)?;
                validate_filter(saved.base.filter)?;
                (
                    saved.base.camera,
                    NativeCameraExtras {
                        clip_policy: None,
                        shake_policy: None,
                        camera_settings: None,
                        filter: saved.base.filter,
                        completed_view: Some(saved.base.completed_view),
                        follow_snap_immediate: saved.follow_snap_immediate,
                        constraints: Default::default(),
                    },
                )
            }
            4 => {
                let saved: ConstrainedCameraState = codec().deserialize(payload)?;
                validate_filter(saved.base.base.filter)?;
                saved.constraints.validate()?;
                (
                    saved.base.base.camera,
                    NativeCameraExtras {
                        clip_policy: None,
                        shake_policy: None,
                        camera_settings: None,
                        filter: saved.base.base.filter,
                        completed_view: Some(saved.base.base.completed_view),
                        follow_snap_immediate: saved.base.follow_snap_immediate,
                        constraints: saved.constraints,
                    },
                )
            }
            5 => {
                let saved: OwnedCameraState = codec().deserialize(payload)?;
                saved.camera_settings.validate()?;
                validate_filter(saved.base.base.base.filter)?;
                saved.base.constraints.validate()?;
                (
                    saved.base.base.base.camera,
                    NativeCameraExtras {
                        clip_policy: None,
                        shake_policy: None,
                        camera_settings: Some(saved.camera_settings),
                        filter: saved.base.base.base.filter,
                        completed_view: Some(saved.base.base.base.completed_view),
                        follow_snap_immediate: saved.base.base.follow_snap_immediate,
                        constraints: saved.base.constraints,
                    },
                )
            }
            6 => {
                let saved: DefinedCameraState = codec().deserialize(payload)?;
                saved.shake_policy.validate()?;
                saved.base.camera_settings.validate()?;
                validate_filter(saved.base.base.base.base.filter)?;
                saved.base.base.constraints.validate()?;
                (
                    saved.base.base.base.base.camera,
                    NativeCameraExtras {
                        clip_policy: Some(saved.clip_policy),
                        shake_policy: Some(saved.shake_policy),
                        camera_settings: Some(saved.base.camera_settings),
                        filter: saved.base.base.base.base.filter,
                        completed_view: Some(saved.base.base.base.base.completed_view),
                        follow_snap_immediate: saved.base.base.base.follow_snap_immediate,
                        constraints: saved.base.base.constraints,
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
    fn native_v6_clip_policy_roundtrip_rejects_peer_and_keeps_v5_wire_input() {
        use crate::presentation_frame::camera::clip::{CameraClipPolicy, CameraClipRules};
        let state = super::super::tests::fixture();
        let clip = CameraClipPolicy::for_application(CameraClipRules {
            draw_entire_terrain: true,
        });
        let bytes = encode_defined_camera(
            &state,
            state.completed_view(),
            None,
            false,
            Default::default(),
            Default::default(),
            clip,
            Default::default(),
        )
        .unwrap();
        let decoded = decode_native_camera(&bytes).unwrap();
        assert_eq!(decoded.1.clip_policy, Some(clip));
        let old = encode_owned_camera(
            &state,
            state.completed_view(),
            None,
            false,
            Default::default(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(decode_native_camera(&old).unwrap().1.clip_policy, None);
        let world = crate::game_logic::GameLogic::new();
        let before = world.capture_camera_continuation().unwrap();
        let logic = crate::game_logic::GameLogic::decode_camera_continuation(&before).unwrap();
        assert!(
            decoded
                .validate_logic_continuation(&logic)
                .unwrap_err()
                .contains("clip policy")
        );
        let mut bad = bytes;
        let shake_len = codec()
            .serialize(&decoded.1.shake_policy.unwrap())
            .unwrap()
            .len();
        let clip_end = bad.len() - shake_len;
        bad[clip_end - 1] = 2;
        assert!(decode_native_camera(&bad).is_err());
        assert_eq!(before, world.capture_camera_continuation().unwrap());
    }

    #[test]
    fn native_v6_shake_policy_rejects_peer_and_nonfinite_detached_definition() {
        use crate::presentation_frame::camera::shake_rules::CameraShakePolicy;
        let mut state = super::super::tests::fixture();
        state.screen_shake_intensity = 1.75;
        state.screen_shake_angle_cos = 0.6;
        state.screen_shake_angle_sin = -0.8;
        state.camera_shake_offset = Vec3::new(0.3, 0., -0.4);
        let shake = CameraShakePolicy::for_application(
            game_engine::common::ini::ini_game_data::CameraShakeRules {
                max_shake_range: 543.,
                ..Default::default()
            },
        )
        .unwrap();
        let bytes = encode_defined_camera(
            &state,
            state.completed_view(),
            None,
            false,
            Default::default(),
            Default::default(),
            Default::default(),
            shake,
        )
        .unwrap();
        let decoded = decode_native_camera(&bytes).unwrap();
        assert_eq!(decoded.1.shake_policy, Some(shake));
        assert_eq!(
            decoded.0.screen_shake_intensity,
            state.screen_shake_intensity
        );
        assert_eq!(
            decoded.0.screen_shake_angle_cos,
            state.screen_shake_angle_cos
        );
        assert_eq!(
            decoded.0.screen_shake_angle_sin,
            state.screen_shake_angle_sin
        );
        assert_eq!(decoded.0.camera_shake_offset, state.camera_shake_offset);

        let world = crate::game_logic::GameLogic::new();
        let before = world.capture_camera_continuation().unwrap();
        let logic = crate::game_logic::GameLogic::decode_camera_continuation(&before).unwrap();
        assert!(
            decoded
                .validate_logic_continuation(&logic)
                .unwrap_err()
                .contains("shake policy")
        );
        let mut bad = bytes;
        let tail = bad.len();
        bad[tail - 4..].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(decode_native_camera(&bad).is_err());
        assert_eq!(before, world.capture_camera_continuation().unwrap());
    }

    #[test]
    fn native_v6_rejects_matching_pose_logic_v5_without_incoming_rules_precommit() {
        use crate::presentation_frame::camera::clip::{CameraClipPolicy, CameraClipRules};
        let mut world = crate::game_logic::GameLogic::new();
        let peer = crate::game_logic::GameLogic::new();
        let peer_before = peer.capture_camera_continuation().unwrap();
        let clip = CameraClipPolicy::for_application(CameraClipRules {
            draw_entire_terrain: true,
        });
        world.session_view.camera.clip_policy = clip;
        let mut state = super::super::tests::fixture();
        super::super::tests::fixture_on_owner(&mut state, &mut world);
        let saved = world.capture_camera_continuation().unwrap();
        let logic = crate::game_logic::GameLogic::decode_camera_continuation(&saved).unwrap();
        let native_bytes = encode_defined_camera(
            &state,
            state.completed_view(),
            None,
            false,
            world.session_view.camera.constraints,
            world.session_view.camera.settings,
            clip,
            world.session_view.camera.shake_policy,
        )
        .unwrap();
        let native = decode_native_camera(&native_bytes).unwrap();
        native.validate_logic_continuation(&logic).unwrap();
        // V6 is exactly frozen V5 followed by both policy records. Produce a
        // genuine old V5 input, preserving the complete matching camera pose.
        let tail = codec().serialize(&clip).unwrap().len()
            + codec()
                .serialize(&world.session_view.camera.shake_policy)
                .unwrap()
                .len();
        let mut old_logic_bytes = saved.clone();
        old_logic_bytes.truncate(old_logic_bytes.len() - tail);
        old_logic_bytes[4..8].copy_from_slice(&5u32.to_le_bytes());
        let old_logic =
            crate::game_logic::GameLogic::decode_camera_continuation(&old_logic_bytes).unwrap();
        assert!(
            native
                .validate_logic_continuation(&old_logic)
                .unwrap_err()
                .contains("shake policy")
        );
        // Independently exercise clip admission's absence check: this detached
        // probe omits native shake, so it cannot mask the clip mismatch.
        let mut clip_probe = decode_native_camera(&native_bytes).unwrap();
        clip_probe.1.shake_policy = None;
        assert!(
            clip_probe
                .validate_logic_continuation(&old_logic)
                .unwrap_err()
                .contains("clip policy")
        );
        let old_native = decode_native_camera(
            &encode_owned_camera(
                &state,
                state.completed_view(),
                None,
                false,
                world.session_view.camera.constraints,
                world.session_view.camera.settings,
            )
            .unwrap(),
        )
        .unwrap();
        old_native.validate_logic_continuation(&logic).unwrap();
        assert_eq!(world.capture_camera_continuation().unwrap(), saved);
        assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    }

    #[test]
    fn native_v6_keeps_logic_activity_clock_and_receiving_client_click_duration() {
        use crate::cnc_game_engine::mouse::{CameraInputLoan, LookAtHostModes};
        use game_client::core::game_client::GameClient;
        let mut client = GameClient::new().unwrap();
        let mut peer_client = GameClient::new().unwrap();
        client.set_frame(37);
        peer_client.set_frame(900);
        let mut world = crate::game_logic::GameLogic::new();
        world.set_current_frame(713);
        let peer = crate::game_logic::GameLogic::new();
        let peer_before = peer.capture_camera_continuation().unwrap();
        let (mut modes, mut rotating, mut anchor) = (LookAtHostModes::default(), false, None);
        {
            let (frame, _, _) = client.split_frame_snow_and_drawable_presence();
            CameraInputLoan::new(&mut modes, &mut rotating, &mut anchor, frame)
                .begin_mmb((100., 100.), &world);
        }
        let mut state = super::super::tests::fixture();
        state.frame_counter = 777;
        state.look_at = modes.capture_continuation();
        state.is_mmb_rotating = rotating;
        state.mmb_anchor = anchor;
        let bytes = encode_defined_camera(
            &state,
            state.completed_view(),
            None,
            false,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let decoded = decode_native_camera(&bytes).unwrap();
        assert_eq!(decoded.0.frame_counter, 777);
        let mut restored = LookAtHostModes::default();
        restored.admit_continuation(decoded.0.look_at);
        let (mut restored_rotating, mut restored_anchor) =
            (decoded.0.is_mmb_rotating, decoded.0.mmb_anchor);
        let mut recent = |world: &crate::game_logic::GameLogic| {
            let (frame, _, _) = client.split_frame_snow_and_drawable_presence();
            CameraInputLoan::new(
                &mut restored,
                &mut restored_rotating,
                &mut restored_anchor,
                frame,
            )
            .observe_recent_activity(world)
        };
        assert!(recent(&world));
        world.set_current_frame(743);
        assert!(recent(&world));
        world.set_current_frame(744);
        assert!(!recent(&world));
        drop(recent);
        world.set_current_frame(713);
        let owner_before = world.capture_camera_continuation().unwrap();
        client.set_frame(42);
        peer_client.set_frame(38);
        let (frame, _, _) = client.split_frame_snow_and_drawable_presence();
        assert!(
            !CameraInputLoan::new(
                &mut restored,
                &mut restored_rotating,
                &mut restored_anchor,
                frame
            )
            .end_mmb((100., 101.), &mut world, &mut Default::default())
            .unwrap(),
            "five receiving Client frames are not a short click despite paused Logic713 and Engine777"
        );
        assert!(!restored_rotating);
        assert_eq!(restored_anchor, None);
        assert_eq!(world.capture_camera_continuation().unwrap(), owner_before);
        assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    }

    #[test]
    fn native_v5_captured_rules_roundtrip_and_reject_malformed_detached_capsule() {
        use crate::presentation_frame::camera::settings::{CameraContentRules, CameraSettings};
        let state = super::super::tests::fixture();
        let view = state.completed_view();
        let mut settings = CameraSettings::for_application(CameraContentRules {
            min_camera_height: 100.,
            max_camera_height: 300.,
            ..Default::default()
        })
        .unwrap();
        settings
            .admit_map(CameraContentRules {
                min_camera_height: 200.,
                max_camera_height: 600.,
                ..Default::default()
            })
            .unwrap();
        settings.capture_default(0.75).unwrap();
        let bytes =
            encode_owned_camera(&state, view, None, false, Default::default(), settings).unwrap();
        let decoded = decode_native_camera(&bytes).unwrap();
        assert_eq!(decoded.1.camera_settings, Some(settings));
        assert_eq!(decoded.1.camera_settings.unwrap().bounds(), (100., 450.));
        let mut malformed = bytes.clone();
        let len = malformed.len();
        malformed[len - 4..].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(decode_native_camera(&malformed).is_err());
        let mut impossible_min = bytes.clone();
        let len = impossible_min.len();
        impossible_min[len - 8..len - 4].copy_from_slice(&99f32.to_le_bytes());
        assert!(decode_native_camera(&impossible_min).is_err());

        let old = encode_constrained_camera(&state, view, None, false, Default::default()).unwrap();
        assert_eq!(decode_native_camera(&old).unwrap().1.camera_settings, None);
        let world = crate::game_logic::GameLogic::new();
        let before = world.capture_camera_continuation().unwrap();
        let logic = crate::game_logic::GameLogic::decode_camera_continuation(&before).unwrap();
        assert!(
            decoded
                .validate_logic_continuation(&logic)
                .unwrap_err()
                .contains("captured camera settings")
        );
        assert_eq!(before, world.capture_camera_continuation().unwrap());
    }

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
        restored
            .admit_camera_continuation(
                crate::game_logic::GameLogic::decode_camera_continuation(&logic).unwrap(),
            )
            .unwrap();
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

#[cfg(test)]
#[path = "follow_snap_v3_tests.rs"]
mod follow_snap_v3_tests;

#[cfg(test)]
#[path = "constraint_v4_tests.rs"]
mod constraint_v4_tests;

#[cfg(all(test, feature = "game_client"))]
#[path = "screen_shake_phase_save_tests.rs"]
mod screen_shake_phase_save_tests;
