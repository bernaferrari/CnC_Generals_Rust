//! Versioned owned state around the unchanged historical positional record.
use super::*;

// A historical record starts with Option<Vec3>'s 0/1 discriminant. FF cannot
// prefix a valid old record, so malformed envelopes never fall back to legacy.
const MAGIC: &[u8; 4] = b"\xffLCF";
const VERSION: u32 = 7;

/// The first owned envelope is retained byte for byte as an input-only schema.
#[derive(Serialize, Deserialize)]
struct EnvelopeV1 {
    legacy_record: Vec<u8>,
    filter: Option<Vec<u8>>,
    pending_focus_previous_target: Option<Vec3>,
}

#[derive(Serialize, Deserialize)]
struct EnvelopeV2 {
    legacy_record: Vec<u8>,
    // Opaque bounded filter bytes retain one shape with or without GameClient.
    filter: Option<Vec<u8>>,
    pending_focus_previous_target: Option<Vec3>,
    // This new typed state deliberately uses bincode's legacy usize=u64 wire;
    // it is never interpreted as either historical path DTO.
    motion: WaypointMotion,
    camera: SessionCameraState,
    target: Vec3,
    path_arrived: bool,
    object_tracking: Option<TrackingV2>,
}

// V2's positional payload stays byte-for-byte frozen. V3 appends the source
// snap latch as an extra, never as a 34th SessionCameraState field.
#[derive(Serialize, Deserialize)]
struct EnvelopeV3 {
    base: EnvelopeV2,
    follow_snap_immediate: bool,
}

// V4 retains cache validity and bounds. V3 snap and every historical DTO stay frozen.
#[derive(Serialize, Deserialize)]
struct EnvelopeV4 {
    base: EnvelopeV3,
    constraints: crate::presentation_frame::camera::constraints::CameraConstraints,
}

// V5 adds captures/rule inputs without changing any older positional DTO.
#[derive(Serialize, Deserialize)]
struct EnvelopeV5 {
    base: EnvelopeV4,
    camera_settings: crate::presentation_frame::camera::settings::CameraSettings,
}

// V5 remains input-only and byte frozen. V6 appends receiving clip/shake definitions.
#[derive(Serialize, Deserialize)]
struct EnvelopeV6 {
    base: EnvelopeV5,
    clip_policy: crate::presentation_frame::camera::clip::CameraClipPolicy,
    shake_policy: crate::presentation_frame::camera::shake_rules::CameraShakePolicy,
}

// V6 and every nested positional DTO remain input compatible.
#[derive(Serialize, Deserialize)]
struct EnvelopeV7 {
    base: EnvelopeV6,
    rotation_timing: crate::presentation_frame::camera::rotation::RotationTiming,
}

/// Input/output schema of the original V2 tracking payload, retained exactly.
/// The two time fields project the one SessionView rotation-time owner.
#[derive(Serialize, Deserialize)]
struct TrackingV2 {
    duration_ms: i32,
    turn_frames: u32,
    hold_frames: u32,
    last_position: Vec3,
    start_time_multiplier: i32,
    end_time_multiplier: i32,
}
impl TrackingV2 {
    fn capture(
        tracking: &CameraObjectTracking,
        timing: crate::presentation_frame::camera::rotation::RotationTiming,
    ) -> Self {
        Self {
            duration_ms: timing.duration_ms(),
            turn_frames: timing.turn_frames,
            hold_frames: tracking.hold_frames,
            last_position: tracking.last_position,
            start_time_multiplier: timing.time.start,
            end_time_multiplier: timing.time.end,
        }
    }
    fn timing(
        &self,
        camera: &SessionCameraState,
    ) -> Result<crate::presentation_frame::camera::rotation::RotationTiming, String> {
        if self.duration_ms < 1
            || self.turn_frames != (self.duration_ms / 33).max(1) as u32
            || self.turn_frames > i32::MAX as u32 / 33
            || self.turn_frames.checked_add(self.hold_frames).is_none()
        {
            return Err("invalid historical tracking timing".into());
        }
        Ok(
            crate::presentation_frame::camera::rotation::RotationTiming::milliseconds(
                self.duration_ms,
                camera.historical_rotation(Default::default()).ease_in(),
                camera.historical_rotation(Default::default()).ease_out(),
                crate::presentation_frame::camera::rotation::RotationTime {
                    start: self.start_time_multiplier,
                    end: self.end_time_multiplier,
                },
            ),
        )
    }
    fn into_runtime(self) -> CameraObjectTracking {
        CameraObjectTracking {
            hold_frames: self.hold_frames,
            last_position: self.last_position,
        }
    }
}

pub(super) fn encode(state: &LogicCameraContinuation) -> Result<Vec<u8>, String> {
    let envelope = EnvelopeV2 {
        legacy_record: codec().serialize(state)?,
        #[cfg(feature = "game_client")]
        filter: Some(codec().serialize(&state.owned_extras.filter)?),
        #[cfg(not(feature = "game_client"))]
        filter: None,
        pending_focus_previous_target: state.owned_extras.pending_focus_previous_target,
        motion: state.owned_extras.motion.clone(),
        camera: state
            .owned_extras
            .camera
            .clone()
            .ok_or("cannot encode V2 camera continuation without its owner state")?,
        target: state
            .owned_extras
            .target
            .ok_or("cannot encode V2 camera continuation without its target")?,
        path_arrived: state.owned_extras.path_arrived,
        object_tracking: state.owned_extras.object_tracking.as_ref().map(|tracking| {
            TrackingV2::capture(
                tracking,
                state
                    .owned_extras
                    .rotation_timing
                    .expect("captured rotation owner"),
            )
        }),
    };
    let v5 = EnvelopeV5 {
        base: EnvelopeV4 {
            base: EnvelopeV3 {
                base: envelope,
                follow_snap_immediate: state.owned_extras.follow_snap_immediate,
            },
            constraints: state.owned_extras.constraints,
        },
        camera_settings: state
            .owned_extras
            .camera_settings
            .ok_or("camera settings capture missing")?,
    };
    let v6 = EnvelopeV6 {
        base: v5,
        clip_policy: state
            .owned_extras
            .clip_policy
            .ok_or("camera clip capture missing")?,
        shake_policy: state
            .owned_extras
            .shake_policy
            .ok_or("camera shake capture missing")?,
    };
    let payload = codec().serialize(&EnvelopeV7 {
        base: v6,
        rotation_timing: state
            .owned_extras
            .rotation_timing
            .ok_or("scalar rotation time capture missing")?,
    })?;
    let mut bytes = Vec::with_capacity(payload.len() + 8);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&payload);
    if bytes.len() > MAX_BYTES {
        return Err("camera continuation exceeds 4 MiB".into());
    }
    Ok(bytes)
}

fn decode_filter(
    state: &mut LogicCameraContinuation,
    filter: Option<Vec<u8>>,
) -> Result<(), String> {
    #[cfg(feature = "game_client")]
    {
        state.owned_extras.filter = match filter {
            Some(bytes) => codec().deserialize(&bytes)?,
            None => Default::default(),
        };
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = state;
        if filter.is_some() {
            return Err("saved camera filter requires the GameClient feature".into());
        }
    }
    Ok(())
}

pub(super) fn decode(bytes: &[u8]) -> Result<LogicCameraContinuation, String> {
    if bytes.first() != Some(&0xff) {
        return codec().deserialize(bytes);
    }
    if bytes.len() < 8 || &bytes[..4] != MAGIC {
        return Err("invalid logic camera owner envelope".into());
    }
    match u32::from_le_bytes(bytes[4..8].try_into().unwrap()) {
        1 => {
            let envelope: EnvelopeV1 = codec().deserialize(&bytes[8..])?;
            let mut state: LogicCameraContinuation =
                codec().deserialize(&envelope.legacy_record)?;
            state.owned_extras.pending_focus_previous_target =
                envelope.pending_focus_previous_target;
            decode_filter(&mut state, envelope.filter)?;
            Ok(state)
        }
        2 | 3 | 4 | 5 | 6 | VERSION => {
            let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
            let (v6, rotation_timing) = if version == VERSION {
                let current: EnvelopeV7 = codec().deserialize(&bytes[8..])?;
                (Some(current.base), Some(current.rotation_timing))
            } else {
                (None, None)
            };
            let (current, clip_policy, shake_policy) = if version >= 6 {
                let current: EnvelopeV6 = match v6 {
                    Some(current) => current,
                    None => codec().deserialize(&bytes[8..])?,
                };
                (
                    Some(current.base),
                    Some(current.clip_policy),
                    Some(current.shake_policy.validate()?),
                )
            } else {
                (None, None, None)
            };
            let (envelope, snap, constraints, settings) = match version {
                2 => (
                    codec().deserialize::<EnvelopeV2>(&bytes[8..])?,
                    false,
                    Default::default(),
                    None,
                ),
                3 => {
                    let old: EnvelopeV3 = codec().deserialize(&bytes[8..])?;
                    (
                        old.base,
                        old.follow_snap_immediate,
                        Default::default(),
                        None,
                    )
                }
                4 => {
                    let current: EnvelopeV4 = codec().deserialize(&bytes[8..])?;
                    (
                        current.base.base,
                        current.base.follow_snap_immediate,
                        current.constraints,
                        None,
                    )
                }
                _ => {
                    let current: EnvelopeV5 = match current {
                        Some(current) => current,
                        None => codec().deserialize(&bytes[8..])?,
                    };
                    (
                        current.base.base.base,
                        current.base.base.follow_snap_immediate,
                        current.base.constraints,
                        Some(current.camera_settings),
                    )
                }
            };
            let mut state: LogicCameraContinuation =
                codec().deserialize(&envelope.legacy_record)?;
            state.owned_extras.pending_focus_previous_target =
                envelope.pending_focus_previous_target;
            state.owned_extras.motion = envelope.motion;
            state.owned_extras.camera = Some(envelope.camera);
            state.owned_extras.target = Some(envelope.target);
            state.owned_extras.path_arrived = envelope.path_arrived;
            let tracking_timing = envelope
                .object_tracking
                .as_ref()
                .map(|tracking| tracking.timing(state.owned_extras.camera.as_ref().unwrap()))
                .transpose()?;
            if let Some(canonical) = rotation_timing {
                canonical.validate()?;
                let camera = state.owned_extras.camera.as_ref().unwrap();
                camera.validate_rotation_observation(canonical)?;
                if tracking_timing.is_some_and(|observed| {
                    observed.turn_frames != canonical.turn_frames
                        || observed.duration_ms() != canonical.duration_ms()
                        || observed.time != canonical.time
                }) {
                    return Err("tracking observation disagrees with canonical timing".into());
                }
            }
            state.owned_extras.object_tracking =
                envelope.object_tracking.map(TrackingV2::into_runtime);
            // Old tracking's explicit integer duration was its authority. An
            // inconsistent old camera float observation is migrated to it.
            state.owned_extras.rotation_timing = rotation_timing.or(tracking_timing);
            // Old Rust eagerly applied snap, so no pending operation survives V2.
            state.owned_extras.follow_snap_immediate = snap;
            state.owned_extras.constraints = constraints;
            state.owned_extras.camera_settings = settings;
            state.owned_extras.clip_policy = clip_policy;
            state.owned_extras.shake_policy = shake_policy;
            decode_filter(&mut state, envelope.filter)?;
            Ok(state)
        }
        _ => Err("unknown logic camera owner envelope version".into()),
    }
}

#[cfg(test)]
#[path = "owned_envelope_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "follow_snap_v3_tests.rs"]
mod follow_snap_v3_tests;

#[cfg(test)]
#[path = "constraint_v4_tests.rs"]
mod constraint_v4_tests;

#[cfg(test)]
#[path = "clip_v6_tests.rs"]
mod clip_v6_tests;

#[cfg(test)]
#[path = "scalar_time_v7_tests.rs"]
mod scalar_time_v7_tests;
