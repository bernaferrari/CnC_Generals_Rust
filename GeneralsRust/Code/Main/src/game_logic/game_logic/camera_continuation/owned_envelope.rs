//! Versioned owned state around the unchanged historical positional record.
use super::*;

// A historical record starts with Option<Vec3>'s 0/1 discriminant. FF cannot
// prefix a valid old record, so malformed envelopes never fall back to legacy.
const MAGIC: &[u8; 4] = b"\xffLCF";
const VERSION: u32 = 5;

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
    object_tracking: Option<CameraObjectTracking>,
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
        object_tracking: state.owned_extras.object_tracking.clone(),
    };
    let payload = codec().serialize(&EnvelopeV5 {
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
        2 | 3 | 4 | VERSION => {
            let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
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
                    let current: EnvelopeV5 = codec().deserialize(&bytes[8..])?;
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
            state.owned_extras.object_tracking = envelope.object_tracking;
            // Old Rust eagerly applied snap, so no pending operation survives V2.
            state.owned_extras.follow_snap_immediate = snap;
            state.owned_extras.constraints = constraints;
            state.owned_extras.camera_settings = settings;
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
