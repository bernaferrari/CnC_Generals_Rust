//! Two original disabled-mask bits absent from the positional host status.
use super::ObjectSnapshot;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
const TAG: &[u8; 4] = b"ODIS";
const VERSION: u32 = 1;
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Record {
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
        .iter()
        .map(|(id, object)| Record {
            object_id: id.0,
            awestruck: object.status.disabled_awestruck,
            brainwashed: object.status.disabled_brainwashed,
            awestruck_until: object.status.disabled_awestruck_until_frame,
            brainwashed_until: object.status.disabled_brainwashed_until_frame,
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
    if version != VERSION {
        return Err(corrupted("unknown version"));
    }
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated records"))?;
    let payload_length = payload.len();
    let (records, consumed): (Vec<Record>, usize) =
        bincode::serde::decode_from_slice(payload, bincode::config::legacy()).map_err(corrupted)?;
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
pub(super) fn apply(records: Option<&[Record]>, world: &mut GameLogic) {
    // Pre-native saves explicitly restore both unsupported bits clear.
    for object in world.objects.values_mut() {
        object.status.disabled_awestruck = false;
        object.status.disabled_brainwashed = false;
        object.status.disabled_awestruck_until_frame = 0;
        object.status.disabled_brainwashed_until_frame = 0;
    }
    for record in records.into_iter().flatten() {
        if let Some(object) = world.objects.get_mut(&ObjectId(record.object_id)) {
            object.status.disabled_awestruck = record.awestruck;
            object.status.disabled_brainwashed = record.brainwashed;
            object.status.disabled_awestruck_until_frame = record.awestruck_until;
            object.status.disabled_brainwashed_until_frame = record.brainwashed_until;
        }
    }
}
fn corrupted(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("original disabled continuation: {error}"))
}
#[cfg(test)]
#[path = "original_disabled_persist_tests.rs"]
mod tests;
