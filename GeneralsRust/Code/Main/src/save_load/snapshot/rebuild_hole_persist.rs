//! Persist C++ `RebuildHoleBehavior::xfer` onto the live host.
//!
//! C++ (`RebuildHoleBehavior.cpp:382-456`) writes worker id, reconstructing
//! id, spawner id, worker-wait counter, and the rebuild template name. After
//! load the 20s worker clock and scaffold resume. Live host stores the same
//! values as `Object.is_rebuild_hole` / `rebuild_template_name` /
//! `rebuild_ready_frame` / worker / reconstructing / spawner ids, but
//! `save_load/` had zero coverage — restore rebuilt a fresh Object from the
//! hole template with constructor defaults, so the crater sat forever.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No world snapshot version bump.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const RHBH_MAGIC: &[u8; 4] = b"RHBH";
const RHBH_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RebuildHolePersistPayload {
    objects: Vec<ObjectRebuildHolePersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectRebuildHolePersist {
    object_id: u32,
    is_rebuild_hole: bool,
    rebuild_template_name: String,
    rebuild_ready_frame: u32,
    rebuild_spawner_id: u32,
    rebuild_worker_id: u32,
    rebuild_reconstructing_id: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(RHBH_MAGIC);
    append_u32(bytes, RHBH_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_rhbh_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != RHBH_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown RHBH suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "RHBH payload truncated".to_string(),
        ));
    }
    let payload: RebuildHolePersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("RHBH payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> RebuildHolePersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if !object.is_rebuild_hole
            && object.rebuild_template_name.is_none()
            && object.rebuild_ready_frame == 0
            && object.rebuild_spawner_id.is_none()
            && object.rebuild_worker_id.is_none()
            && object.rebuild_reconstructing_id.is_none()
        {
            continue;
        }
        objects.push(ObjectRebuildHolePersist {
            object_id: id.0,
            is_rebuild_hole: object.is_rebuild_hole,
            rebuild_template_name: object.rebuild_template_name.clone().unwrap_or_default(),
            rebuild_ready_frame: object.rebuild_ready_frame,
            rebuild_spawner_id: object.rebuild_spawner_id.map(|id| id.0).unwrap_or(0),
            rebuild_worker_id: object.rebuild_worker_id.map(|id| id.0).unwrap_or(0),
            rebuild_reconstructing_id: object.rebuild_reconstructing_id.map(|id| id.0).unwrap_or(0),
        });
    }
    RebuildHolePersistPayload { objects }
}

fn apply_payload(game_logic: &mut GameLogic, payload: RebuildHolePersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.is_rebuild_hole = entry.is_rebuild_hole;
        object.rebuild_template_name = if entry.rebuild_template_name.is_empty() {
            None
        } else {
            Some(entry.rebuild_template_name)
        };
        object.rebuild_ready_frame = entry.rebuild_ready_frame;
        object.rebuild_spawner_id =
            (entry.rebuild_spawner_id != 0).then_some(ObjectId(entry.rebuild_spawner_id));
        object.rebuild_worker_id =
            (entry.rebuild_worker_id != 0).then_some(ObjectId(entry.rebuild_worker_id));
        object.rebuild_reconstructing_id = (entry.rebuild_reconstructing_id != 0)
            .then_some(ObjectId(entry.rebuild_reconstructing_id));
    }
}

fn find_rhbh_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == RHBH_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("RHBH u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_rhbh_suffix(b"no-magic-here").is_none());
        apply_from_lifecycle_tail(b"no-magic-here", &mut GameLogic::new()).expect("apply");
    }

    #[test]
    fn snapshot_round_trips_rebuild_hole_reconstruction() {
        let mut source = GameLogic::new();
        source
            .templates
            .insert("GLAHole".to_string(), ThingTemplate::new("GLAHole"));
        source.add_player(Player::new(0, Team::GLA, "GLA", true));
        let hole_id = source
            .create_object("GLAHole", Team::GLA, Vec3::new(40.0, 0.0, 24.0))
            .expect("hole");
        {
            let hole = source.host_object_mut(hole_id).expect("hole");
            hole.is_rebuild_hole = true;
            hole.rebuild_template_name = Some("GLABarracks".to_string());
            hole.rebuild_ready_frame = 780;
            hole.rebuild_spawner_id = Some(ObjectId(11));
            hole.rebuild_worker_id = Some(ObjectId(22));
            hole.rebuild_reconstructing_id = Some(ObjectId(33));
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_rhbh_suffix(&snapshot.lifecycle_tail).is_some(),
            "RHBH suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(hole_id).expect("restored hole");
        assert!(loaded.is_rebuild_hole, "hole must stay a rebuild crater");
        assert_eq!(
            loaded.rebuild_template_name.as_deref(),
            Some("GLABarracks"),
            "m_rebuildTemplate must survive load"
        );
        assert_eq!(
            loaded.rebuild_ready_frame, 780,
            "worker-wait / ready frame must survive load"
        );
        assert_eq!(loaded.rebuild_spawner_id, Some(ObjectId(11)));
        assert_eq!(loaded.rebuild_worker_id, Some(ObjectId(22)));
        assert_eq!(loaded.rebuild_reconstructing_id, Some(ObjectId(33)));
    }
}
