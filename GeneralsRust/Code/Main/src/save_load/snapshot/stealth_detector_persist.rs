//! Persist live `StealthDetectorUpdate::m_enabled` and its wake clock.
//!
//! C++ `StealthDetectorUpdate::xfer` writes version + UpdateModule base +
//! `m_enabled`. The Strategy Center detector is armed by SearchAndDestroy
//! (`setSDEnabled(true)` + DetectionRange 500 when the plan goes ACTIVE).
//! Leftover `stealth_detector_update.rs` Snapshotable::xfer already writes
//! the update-module base + `enabled`.
//!
//! Live stores that as `Object.is_detector` / `detection_range` /
//! `detection_rate_frames` / `next_detection_scan_frame`. Those fields are
//! set only on the Unpacking→Active edge; `battle_plan_persist` restores an
//! already-Active registry so the edge never re-fires and detection dies
//! until the player re-picks the plan. Append a tagged suffix after the
//! historical v9 contain/producer payload so older decoders ignore the extra
//! bytes. No WorldSnapshot version bump. Restore writes enabled + range/rate
//! + the scan clock only; it never re-runs `apply_battle_plan_set_battle_plan`.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const SDEN_MAGIC: &[u8; 4] = b"SDEN";
const SDEN_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StealthDetectorPersistPayload {
    objects: Vec<StealthDetectorPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StealthDetectorPersist {
    object_id: u32,
    enabled: bool,
    detection_range: f32,
    detection_rate_frames: u32,
    next_detection_scan_frame: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(SDEN_MAGIC);
    append_u32(bytes, SDEN_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_sden_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != SDEN_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown SDEN suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "SDEN payload truncated".to_string(),
        ));
    }
    let payload: StealthDetectorPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("SDEN payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> StealthDetectorPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if !object.is_detector
            && object.detection_rate_frames == 0
            && object.next_detection_scan_frame == 0
        {
            continue;
        }
        objects.push(StealthDetectorPersist {
            object_id: id.0,
            enabled: object.is_detector,
            detection_range: object.detection_range,
            detection_rate_frames: object.detection_rate_frames,
            next_detection_scan_frame: object.next_detection_scan_frame,
        });
    }
    StealthDetectorPersistPayload { objects }
}

fn apply_payload(game_logic: &mut GameLogic, payload: StealthDetectorPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.set_detector_state(
            entry.enabled,
            entry.detection_range,
            entry.detection_rate_frames,
        );
        object.next_detection_scan_frame = entry.next_detection_scan_frame;
    }
}

fn find_sden_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == SDEN_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("SDEN u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_strategy_center::{
        STRATEGY_CENTER_STEALTH_DETECTION_RANGE, STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES,
    };
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_strategy_center_detector() {
        let mut source = GameLogic::new();
        let mut center = ThingTemplate::new("AmericaStrategyCenter");
        center.add_kind_of(KindOf::Structure);
        source
            .templates
            .insert("AmericaStrategyCenter".to_string(), center);
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let id = source
            .create_object(
                "AmericaStrategyCenter",
                Team::USA,
                Vec3::new(12.0, 0.0, 8.0),
            )
            .expect("strategy center");
        {
            let object = source.host_object_mut(id).expect("center obj");
            object.set_detector_state(
                true,
                STRATEGY_CENTER_STEALTH_DETECTION_RANGE,
                STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES,
            );
            object.next_detection_scan_frame = 42;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_sden_suffix(&snapshot.lifecycle_tail).is_some(),
            "SDEN suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(id).expect("restored center");
        assert!(
            loaded.is_detector,
            "S&D detector must stay armed after load"
        );
        assert!((loaded.detection_range - STRATEGY_CENTER_STEALTH_DETECTION_RANGE).abs() < 1e-4);
        assert_eq!(
            loaded.detection_rate_frames,
            STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES
        );
        assert_eq!(loaded.next_detection_scan_frame, 42);
    }

    #[test]
    fn absent_suffix_does_not_invent_detector() {
        let mut logic = GameLogic::new();
        let mut center = ThingTemplate::new("AmericaStrategyCenter");
        center.add_kind_of(KindOf::Structure);
        logic
            .templates
            .insert("AmericaStrategyCenter".to_string(), center);
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let id = logic
            .create_object("AmericaStrategyCenter", Team::USA, Vec3::ZERO)
            .expect("center");
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let object = logic.host_object(id).expect("center");
        assert!(!object.is_detector);
        assert_eq!(object.detection_rate_frames, 0);
        assert_eq!(object.next_detection_scan_frame, 0);
    }
}
