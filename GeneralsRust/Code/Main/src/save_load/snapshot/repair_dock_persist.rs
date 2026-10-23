//! RepairDockUpdate::xfer v1 cache, separate from positional host records.
//! Authored duration reconstructs immutable module identity. The cached rate
//! preserves IEEE values, including division by authored zero duration.
use super::ObjectSnapshot;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

const TAG: &[u8; 4] = b"RDCK";
const VERSION: u32 = 1;
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Record {
    object_id: u32,
    frames: f32,
    last_repair: u32,
    health_per_frame: f32,
}
pub(super) struct Decoded<'a> {
    pub records: Option<Vec<Record>>,
    pub legacy_tail: Cow<'a, [u8]>,
}

pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    let mut records: Vec<_> = world
        .host_objects()
        .registered()
        .filter_map(|(id, object)| {
            object
                .thing()
                .template
                .repair_dock_frames_for_full_heal
                .map(|frames| Record {
                    object_id: id.0,
                    frames,
                    last_repair: object.repair_dock_last_id.map_or(0, |id| id.0),
                    health_per_frame: object.repair_dock_health_per_frame,
                })
        })
        .collect();
    records.sort_by_key(|entry| entry.object_id);
    let payload = bincode_legacy::serialize(&records).map_err(corrupted)?;
    let length = u32::try_from(payload.len()).map_err(corrupted)?;
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(&payload);
    Ok(())
}

/// Called after preceding native capsules were removed, at the exact boundary.
pub(super) fn decode(bytes: &[u8]) -> SaveLoadResult<Decoded<'_>> {
    let boundary = super::combat_projectile_persist::prefix_len(bytes)?;
    let suffix = &bytes[boundary..];
    if !suffix.is_empty() && suffix.len() < TAG.len() && TAG.starts_with(suffix) {
        return Err(corrupted("truncated tag"));
    }
    if !suffix.starts_with(TAG) {
        return Ok(Decoded {
            records: None,
            legacy_tail: Cow::Borrowed(bytes),
        });
    }
    if suffix.len() < 12 {
        return Err(corrupted("truncated header"));
    }
    let version = u32::from_le_bytes(suffix[4..8].try_into().unwrap());
    let length = u32::from_le_bytes(suffix[8..12].try_into().unwrap()) as usize;
    if version != VERSION {
        return Err(corrupted("unknown version"));
    }
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated records"))?;
    let (records, consumed): (Vec<Record>, usize) =
        bincode::serde::decode_from_slice(payload, bincode::config::legacy()).map_err(corrupted)?;
    if consumed != payload.len() || suffix[end..].starts_with(TAG) {
        return Err(corrupted("trailing or duplicate records"));
    }
    let mut tail = bytes[..boundary].to_vec();
    tail.extend_from_slice(&suffix[end..]);
    Ok(Decoded {
        records: Some(records),
        legacy_tail: Cow::Owned(tail),
    })
}

pub(super) fn validate(
    records: Option<&[Record]>,
    objects: &HashMap<ObjectId, ObjectSnapshot>,
    world: &GameLogic,
) -> SaveLoadResult<()> {
    let Some(records) = records else {
        return Ok(());
    };
    let mut seen = HashSet::new();
    let mut template_frames = HashMap::new();
    for record in records {
        let id = ObjectId(record.object_id);
        let object = objects.get(&id).ok_or_else(|| corrupted("unknown owner"))?;
        if id.0 == 0 || !seen.insert(id) {
            return Err(corrupted("invalid or duplicate owner"));
        }
        if !record.frames.is_finite() {
            return Err(corrupted("nonfinite authored duration"));
        }
        if template_frames
            .insert(object.template_name.as_str(), record.frames.to_bits())
            .is_some_and(|previous| previous != record.frames.to_bits())
        {
            return Err(corrupted("inconsistent duration for one saved template"));
        }
        if let Some(template) = world.templates.get(&object.template_name) {
            match template.repair_dock_frames_for_full_heal {
                Some(frames) if frames.to_bits() != record.frames.to_bits() => {
                    return Err(corrupted("receiving duration differs from saved module"));
                }
                None if template.native_lifecycle_definitions.is_some() => {
                    return Err(corrupted("receiving object lacks saved RepairDock module"));
                }
                _ => {}
            }
        }
    }
    // An explicitly receiving native RepairDock must not lose its cache record.
    for (id, object) in objects {
        if world
            .templates
            .get(&object.template_name)
            .is_some_and(|template| template.repair_dock_frames_for_full_heal.is_some())
            && !seen.contains(id)
        {
            return Err(corrupted("missing receiving RepairDock owner"));
        }
    }
    // last_repair is a historical ID, not a live reference: the original saves
    // it even if its object has disappeared. Cached floats also remain raw.
    Ok(())
}

pub(super) fn apply(records: Option<&[Record]>, world: &mut GameLogic) {
    for object in world.objects.registered_values_mut() {
        object.repair_dock_last_id = None;
        object.repair_dock_health_per_frame = 0.0;
    }
    for record in records.into_iter().flatten() {
        if let Some(object) = world.objects.get_mut(&ObjectId(record.object_id)) {
            let template_name = object.template_name.clone();
            object.restore_repair_dock_state(
                record.frames,
                (record.last_repair != 0).then_some(ObjectId(record.last_repair)),
                record.health_per_frame,
            );
            // Saved definitions also belong to the receiving factory catalog:
            // objects created after load must use this same admitted module.
            if let Some(template) = world.templates.get_mut(&template_name) {
                template.repair_dock_frames_for_full_heal = Some(record.frames);
            }
        }
    }
}
fn corrupted(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("repair dock continuation: {error}"))
}
#[cfg(test)]
#[path = "repair_dock_persist_tests.rs"]
mod tests;
