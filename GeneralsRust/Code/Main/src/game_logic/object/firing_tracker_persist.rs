//! One canonical tracker: CPP FiringTracker::xfer340–360 saves consecutive
//! shots, victim and coast. Mean/Fast are Object WeaponBonus conditions,
//! represented by `continuous_fire_level`. Reload/audio timers are not saved.
use super::super::Object;
use super::{decode_payload, encode_payload};
use gamelogic::world::entities::EntityLifecycleCodecError;
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 4] = b"FTRK";
const VERSION: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FiringTrackerPersist {
    consecutive_shots: u32,
    consecutive_target: Option<crate::game_logic::ObjectId>,
    continuous_fire_level: u8,
    coast_until_frame: u32,
}

/// Exact prior Rust payload layout; its extra idle-reload timer is decoded
/// for compatibility, but omitted from the original-compatible new stream.
#[derive(Serialize, Deserialize)]
struct FiringTrackerV1 {
    consecutive_shots: u32,
    consecutive_target: Option<crate::game_logic::ObjectId>,
    continuous_fire_level: u8,
    coast_until_frame: u32,
    force_reload_frame: u32,
}

impl FiringTrackerPersist {
    pub(crate) fn present(object: &Object) -> bool {
        object.consecutive_shots_at_target != 0
            || object.consecutive_shot_target.is_some()
            || object.continuous_fire_level != 0
            || object.continuous_fire_coast_until_frame != 0
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.consecutive_shots_at_target = self.consecutive_shots;
        object.consecutive_shot_target = self.consecutive_target;
        object.continuous_fire_level = self.continuous_fire_level;
        object.continuous_fire_coast_until_frame = self.coast_until_frame;
        // CPP constructor clears this non-Xfer timer before load.
        object.frame_to_force_reload = 0;
    }
}

pub(crate) fn encode(object: &Object) -> Result<Vec<u8>, EntityLifecycleCodecError> {
    let state = FiringTrackerPersist {
        consecutive_shots: object.consecutive_shots_at_target,
        consecutive_target: object.consecutive_shot_target,
        continuous_fire_level: object.continuous_fire_level,
        coast_until_frame: object.continuous_fire_coast_until_frame,
    };
    validate(&state)?;
    let mut bytes = MAGIC.to_vec();
    bytes.push(VERSION);
    bytes.extend_from_slice(&encode_payload(&state)?);
    Ok(bytes)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<FiringTrackerPersist, EntityLifecycleCodecError> {
    // The old Option discriminant at byte4 is only0/1, so versions>1 after
    // this prefix cannot collide with a valid positional v1 payload.
    let state = if bytes.starts_with(MAGIC) && bytes.get(4).is_some_and(|version| *version > 1) {
        if bytes[4] != VERSION {
            return Err(EntityLifecycleCodecError::InvalidOwnerComponent {
                component: "firing tracker version",
            });
        }
        decode_payload::<FiringTrackerPersist>(&bytes[5..])?
    } else {
        let old = decode_payload::<FiringTrackerV1>(bytes)?;
        let _ = old.force_reload_frame;
        FiringTrackerPersist {
            consecutive_shots: old.consecutive_shots,
            consecutive_target: old.consecutive_target,
            continuous_fire_level: old.continuous_fire_level,
            coast_until_frame: old.coast_until_frame,
        }
    };
    validate(&state)?;
    Ok(state)
}

fn validate(state: &FiringTrackerPersist) -> Result<(), EntityLifecycleCodecError> {
    if state.consecutive_shots > i32::MAX as u32
        || state.continuous_fire_level > 2
        || state.consecutive_target.is_some_and(|id| id.0 == 0)
    {
        return Err(EntityLifecycleCodecError::InvalidOwnerComponent {
            component: "firing tracker",
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "firing_tracker_persist/tests.rs"]
mod tests;
