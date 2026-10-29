//! Two original disabled-mask bits absent from the positional host status.
use super::ObjectSnapshot;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
const TAG: &[u8; 4] = b"ODIS";
const VERSION: u32 = 2;
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Record {
    object_id: u32,
    awestruck: bool,
    brainwashed: bool,
    awestruck_until: u32,
    brainwashed_until: u32,
    script_status_disabled: Option<bool>,
}
#[derive(Deserialize)]
struct RecordV1 {
    object_id: u32,
    awestruck: bool,
    brainwashed: bool,
    awestruck_until: u32,
    brainwashed_until: u32,
}
pub(super) struct Decoded<'a> {
    pub records: Option<Vec<Record>>,
    pub legacy_tail: Cow<'a, [u8]>,
}
pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    let mut records: Vec<_> = world
        .host_objects()
        .registered()
        .map(|(id, object)| Record {
            object_id: id.0,
            awestruck: object.status.disabled_awestruck,
            brainwashed: object.status.disabled_brainwashed,
            awestruck_until: object.status.disabled_awestruck_until_frame,
            brainwashed_until: object.status.disabled_brainwashed_until_frame,
            script_status_disabled: Some(object.status.script_status_disabled),
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
/// BDKD and CPRJ have already been removed. Never scan arbitrary payload bytes.
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
    if version != 1 && version != VERSION {
        return Err(corrupted("unknown version"));
    }
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated records"))?;
    let payload_length = payload.len();
    let (records, consumed): (Vec<Record>, usize) = if version == 1 {
        let (old, consumed): (Vec<RecordV1>, usize) =
            bincode::serde::decode_from_slice(payload, bincode::config::legacy())
                .map_err(corrupted)?;
        (
            old.into_iter()
                .map(|entry| Record {
                    object_id: entry.object_id,
                    awestruck: entry.awestruck,
                    brainwashed: entry.brainwashed,
                    awestruck_until: entry.awestruck_until,
                    brainwashed_until: entry.brainwashed_until,
                    script_status_disabled: None,
                })
                .collect(),
            consumed,
        )
    } else {
        bincode::serde::decode_from_slice(payload, bincode::config::legacy()).map_err(corrupted)?
    };
    if version == VERSION
        && records
            .iter()
            .any(|record: &Record| record.script_status_disabled.is_none())
    {
        return Err(corrupted("missing script status"));
    }
    if consumed != payload_length || suffix[end..].starts_with(TAG) {
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
) -> SaveLoadResult<()> {
    let Some(records) = records else {
        return Ok(());
    };
    if records.len() != objects.len() {
        return Err(corrupted("incomplete owner roster"));
    }
    let mut seen = HashSet::new();
    for record in records {
        let id = ObjectId(record.object_id);
        if !objects.contains_key(&id) || !seen.insert(id) {
            return Err(corrupted("unknown or duplicate owner"));
        }
    }
    Ok(())
}
/// Reconstruct the saved script bit before each Object is admitted, so its
/// restore callbacks cannot observe the old combined-mask approximation.
pub(super) fn script_status_flags(records: Option<&[Record]>) -> HashMap<ObjectId, bool> {
    records
        .into_iter()
        .flatten()
        .filter_map(|record| {
            record
                .script_status_disabled
                .map(|flag| (ObjectId(record.object_id), flag))
        })
        .collect()
}
pub(super) fn apply(records: Option<&[Record]>, world: &mut GameLogic) {
    // Pre-native saves explicitly restore both unsupported bits clear.
    for object in world.objects.registered_values_mut() {
        object.status.disabled_awestruck = false;
        object.status.disabled_brainwashed = false;
        object.status.disabled_awestruck_until_frame = 0;
        object.status.disabled_brainwashed_until_frame = 0;
        // Old saves conflated both bits, so preserve their historical meaning.
        object.status.script_status_disabled = object.status.disabled_script_disabled;
    }
    for record in records.into_iter().flatten() {
        if let Some(object) = world.objects.get_mut(&ObjectId(record.object_id)) {
            object.status.disabled_awestruck = record.awestruck;
            object.status.disabled_brainwashed = record.brainwashed;
            object.status.disabled_awestruck_until_frame = record.awestruck_until;
            object.status.disabled_brainwashed_until_frame = record.brainwashed_until;
            if let Some(script_status_disabled) = record.script_status_disabled {
                object.status.script_status_disabled = script_status_disabled;
            }
        }
    }
}
fn corrupted(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("original disabled continuation: {error}"))
}
#[cfg(test)]
#[path = "original_disabled_persist_tests.rs"]
mod tests;
