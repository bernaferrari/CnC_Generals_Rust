//! Persist live RadarScan / SpySatellite / SpyDrone DynamicShroud curves.
//!
//! C++ `DynamicShroudClearingRangeUpdate::xfer`
//! (`DynamicShroudClearingRangeUpdate.cpp:328-361`) writes v1 UpdateModule
//! base plus grow/shrink countdown fields. Leftover DSCR xfer already
//! matches that table. Live host does not attach leftover DSCR — RadarScan /
//! SpySatellite / SpyDrone FOW lives on `GameLogic.radar_scans` /
//! `spy_satellites` / `spy_drones` (`activate_frame` / `expires_frame` /
//! `last_applied_radius` / grow_index). Those registries were live-only —
//! a mid-scan save restored ping objects (if any) but empty registries, so
//! shrink/undo never ran and a 150wu radar disk or 300wu spy-sat disk
//! stayed permanently revealed (or the remaining grow pulse never continued).
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.

use crate::game_logic::GameLogic;
use crate::game_logic::host_radar_scan::HostRadarScanRegistry;
use crate::game_logic::host_spy_drone::HostSpyDroneRegistry;
use crate::game_logic::host_spy_satellite::HostSpySatelliteRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const DSCR_MAGIC: &[u8; 4] = b"DSCR";
const DSCR_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DynamicShroudPersistPayload {
    radar: HostRadarScanRegistry,
    satellites: HostSpySatelliteRegistry,
    drones: HostSpyDroneRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.radar.active_count() == 0
        && payload.satellites.active_count() == 0
        && payload.drones.last().is_none()
        && payload.radar.activations() == 0
        && payload.satellites.activations() == 0
        && payload.drones.activations() == 0
    {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(DSCR_MAGIC);
    append_u32(bytes, DSCR_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load FOW pulses.
    game_logic.radar_scans.clear();
    game_logic.spy_satellites.clear();
    game_logic.spy_drones.clear();
    let Some(suffix) = find_dscr_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != DSCR_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown DSCR suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "DSCR payload truncated".to_string(),
        ));
    }
    let payload: DynamicShroudPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("DSCR payload decode: {err}")))?;
    game_logic.radar_scans = payload.radar;
    game_logic.spy_satellites = payload.satellites;
    game_logic.spy_drones = payload.drones;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> DynamicShroudPersistPayload {
    DynamicShroudPersistPayload {
        radar: game_logic.radar_scans.clone(),
        satellites: game_logic.spy_satellites.clone(),
        drones: game_logic.spy_drones.clone(),
    }
}

fn find_dscr_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == DSCR_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("DSCR u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;
    use crate::game_logic::host_radar_scan::{HostRadarScan, RADAR_SCAN_RADIUS};
    use crate::game_logic::host_spy_drone::HostSpyDrone;
    use crate::game_logic::host_spy_satellite::{HostSpySatellite, SPY_SATELLITE_RADIUS};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_dynamic_shroud_registries() {
        let mut source = GameLogic::new();
        let radar_id = source.radar_scans.alloc_id();
        source.radar_scans.record_activation(HostRadarScan {
            id: radar_id,
            player_id: 1,
            player_mask: 1,
            location: Vec3::new(10.0, 0.0, 20.0),
            radius: RADAR_SCAN_RADIUS,
            activate_frame: 50,
            expires_frame: 350,
            caster_id: Some(ObjectId(4)),
            fow_reveal_ok: true,
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
            last_applied_radius: 150.0,
        });
        let sat_id = source.spy_satellites.alloc_id();
        source.spy_satellites.record_activation(HostSpySatellite {
            id: sat_id,
            player_id: 1,
            player_mask: 1,
            location: Vec3::new(200.0, 0.0, 80.0),
            radius: SPY_SATELLITE_RADIUS,
            activate_frame: 40,
            expires_frame: 430,
            caster_id: Some(ObjectId(8)),
            fow_reveal_ok: true,
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
            last_applied_radius: 180.0,
        });
        let drone_id = source.spy_drones.alloc_id();
        source.spy_drones.record_activation(HostSpyDrone {
            id: drone_id,
            player_id: 1,
            player_mask: 1,
            location: Vec3::new(5.0, 0.0, 5.0),
            radius: 80.0,
            activate_frame: 10,
            expires_frame: 130,
            caster_id: Some(ObjectId(2)),
            spawned_id: Some(ObjectId(11)),
            fow_reveal_ok: true,
            spawn_ok: true,
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
            grow_index: 12,
            growing: true,
        });
        source.spy_drones.record_grow_pulse();

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_dscr_suffix(&snapshot.lifecycle_tail).is_some(),
            "DSCR suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.radar_scans.record_activation(HostRadarScan {
            id: 99,
            player_id: 0,
            player_mask: 1,
            location: Vec3::ZERO,
            radius: 1.0,
            activate_frame: 0,
            expires_frame: 1,
            caster_id: None,
            fow_reveal_ok: false,
            dynamic_shroud_applied: false,
            stealth_detector_applied: false,
            last_applied_radius: 0.0,
        });
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        assert_eq!(restored.radar_scans.active_count(), 1);
        let radar = &restored.radar_scans.active_scans()[0];
        assert_eq!(radar.activate_frame, 50);
        assert_eq!(radar.expires_frame, 350);
        assert!((radar.last_applied_radius - 150.0).abs() < 0.01);
        assert!((radar.radius - RADAR_SCAN_RADIUS).abs() < 0.01);

        assert_eq!(restored.spy_satellites.active_count(), 1);
        let sat = &restored.spy_satellites.active_scans()[0];
        assert_eq!(sat.activate_frame, 40);
        assert_eq!(sat.expires_frame, 430);
        assert!((sat.last_applied_radius - 180.0).abs() < 0.01);
        assert!((sat.radius - SPY_SATELLITE_RADIUS).abs() < 0.01);

        let drone = restored.spy_drones.last().expect("drone");
        assert_eq!(drone.grow_index, 12);
        assert!(drone.growing);
        assert_eq!(drone.spawned_id, Some(ObjectId(11)));
        assert_eq!(drone.expires_frame, 130);
        assert_eq!(restored.spy_drones.grow_pulses, 1);
    }

    #[test]
    fn absent_suffix_clears_stale_dynamic_shroud() {
        let mut logic = GameLogic::new();
        logic.radar_scans.record_activation(HostRadarScan {
            id: 1,
            player_id: 0,
            player_mask: 1,
            location: Vec3::ZERO,
            radius: RADAR_SCAN_RADIUS,
            activate_frame: 0,
            expires_frame: 300,
            caster_id: None,
            fow_reveal_ok: true,
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
            last_applied_radius: 150.0,
        });
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert_eq!(logic.radar_scans.active_count(), 0);
        assert_eq!(logic.spy_satellites.active_count(), 0);
        assert!(logic.spy_drones.last().is_none());
    }
}
