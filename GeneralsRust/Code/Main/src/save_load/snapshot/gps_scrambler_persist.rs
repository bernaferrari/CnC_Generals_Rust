//! Persist live GPS Scrambler GrantStealth grow-radius pulse.
//!
//! C++ `GrantStealthBehavior::xfer` (`GrantStealthBehavior.cpp:201-218`)
//! writes v1 UpdateModule base plus `m_radiusParticleSystemID` and
//! `m_currentScanRadius`. Leftover `GrantStealthBehavior::xfer` already
//! writes the particle-system id and `current_scan_radius`. Live GPS is
//! `GameLogic.gps_scramblers` (`HostGpsScrambler`: grow_index, growing,
//! marker_id). Those records were live-only — a mid-GPS save kept already-
//! granted stealth but dropped the remaining grow pulse so units between
//! the current radius and 100wu never received stealth.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.

use crate::game_logic::GameLogic;
use crate::game_logic::host_gps_scrambler::HostGpsScramblerRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const GPSG_MAGIC: &[u8; 4] = b"GPSG";
const GPSG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct GpsScramblerPersistPayload {
    registry: HostGpsScramblerRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.activations().is_empty() && payload.registry.activation_count() == 0 {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(GPSG_MAGIC);
    append_u32(bytes, GPSG_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load grow pulses.
    game_logic.gps_scramblers.clear();
    let Some(suffix) = find_gpsg_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != GPSG_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown GPSG suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "GPSG payload truncated".to_string(),
        ));
    }
    let payload: GpsScramblerPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("GPSG payload decode: {err}")))?;
    game_logic.gps_scramblers = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> GpsScramblerPersistPayload {
    GpsScramblerPersistPayload {
        registry: game_logic.gps_scramblers.clone(),
    }
}

fn find_gpsg_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == GPSG_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("GPSG u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;
    use crate::game_logic::host_gps_scrambler::{
        GPS_SCRAMBLER_START_RADIUS, HOST_GPS_SCRAMBLER_RADIUS, HostGpsScrambler,
    };
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_gps_grow_pulse() {
        let mut source = GameLogic::new();
        let id = source.gps_scramblers.alloc_id();
        source.gps_scramblers.record_activation(HostGpsScrambler {
            id,
            player_id: 1,
            location: Vec3::new(80.0, 0.0, 20.0),
            radius: GPS_SCRAMBLER_START_RADIUS + 30.0,
            activate_frame: 40,
            caster_id: Some(ObjectId(5)),
            grants: 3,
            grow_index: 3,
            growing: true,
            marker_id: Some(ObjectId(9)),
        });
        source.gps_scramblers.record_marker_spawn();
        source.gps_scramblers.record_grow_pulse();

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_gpsg_suffix(&snapshot.lifecycle_tail).is_some(),
            "GPSG suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.gps_scramblers.record_activation(HostGpsScrambler {
            id: 99,
            player_id: 0,
            location: Vec3::ZERO,
            radius: HOST_GPS_SCRAMBLER_RADIUS,
            activate_frame: 1,
            caster_id: None,
            grants: 0,
            grow_index: 8,
            growing: false,
            marker_id: None,
        });
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let reg = restored.gps_scramblers();
        assert_eq!(reg.activation_count(), 1);
        assert_eq!(reg.grant_count(), 3);
        assert_eq!(reg.activations().len(), 1);
        let pulse = &reg.activations()[0];
        assert_eq!(pulse.grow_index, 3);
        assert!(pulse.growing);
        assert_eq!(pulse.marker_id, Some(ObjectId(9)));
        assert_eq!(pulse.caster_id, Some(ObjectId(5)));
        assert!((pulse.radius - (GPS_SCRAMBLER_START_RADIUS + 30.0)).abs() < 0.01);
        assert_eq!(reg.grow_pulses, 1);
        assert_eq!(reg.markers_spawned, 1);
        assert!(reg.activations().iter().any(|pulse| pulse.growing));
    }

    #[test]
    fn absent_suffix_clears_stale_gps_pulses() {
        let mut logic = GameLogic::new();
        logic.gps_scramblers.record_activation(HostGpsScrambler {
            id: 1,
            player_id: 0,
            location: Vec3::ZERO,
            radius: HOST_GPS_SCRAMBLER_RADIUS,
            activate_frame: 0,
            caster_id: None,
            grants: 1,
            grow_index: 2,
            growing: true,
            marker_id: Some(ObjectId(3)),
        });
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(logic.gps_scramblers.activations().is_empty());
        assert_eq!(logic.gps_scramblers.activation_count(), 0);
    }
}
