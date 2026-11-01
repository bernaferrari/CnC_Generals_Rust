//! Object.cpp:4248 transfers the current layer separately from destination.
use super::ObjectSnapshot;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::borrow::Cow;
use std::collections::HashMap;

const TAG: &[u8; 4] = b"OLYR";
const VERSION: u32 = 1;
const GROUND: u8 = 1;
const LAST_LAYER: u8 = 15;
const ROW_SIZE: usize = 5;

pub(super) struct Record {
    id: ObjectId,
    layer: u8,
}

pub(super) struct Decoded<'a> {
    pub records: Option<Vec<Record>>,
    pub legacy_tail: Cow<'a, [u8]>,
}

pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    let mut records = Vec::new();
    for (id, object) in world.objects.registered() {
        if object.pathfind_layer > LAST_LAYER {
            return Err(corrupted("invalid current layer"));
        }
        if object.pathfind_layer != GROUND {
            records.push((*id, object.pathfind_layer));
        }
    }
    records.sort_unstable_by_key(|(id, _)| *id);
    let length = records
        .len()
        .checked_mul(ROW_SIZE)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| corrupted("too many records"))?;
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&length.to_le_bytes());
    for (id, layer) in records {
        bytes.extend_from_slice(&id.0.to_le_bytes());
        bytes.push(layer);
    }
    Ok(())
}

pub(super) fn decode(bytes: &[u8]) -> SaveLoadResult<Decoded<'_>> {
    // Earlier capsules have already been removed; never search an opaque body.
    let boundary = super::combat_projectile_persist::prefix_len(bytes)?;
    let suffix = &bytes[boundary..];
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
    if version != VERSION || length % ROW_SIZE != 0 {
        return Err(corrupted("invalid version or record length"));
    }
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated records"))?;
    if suffix[end..].starts_with(TAG) {
        return Err(corrupted("duplicate record block"));
    }
    let records = payload
        .chunks_exact(ROW_SIZE)
        .map(|row| Record {
            id: ObjectId(u32::from_le_bytes(row[..4].try_into().unwrap())),
            layer: row[4],
        })
        .collect();
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
) -> SaveLoadResult<HashMap<ObjectId, u8>> {
    let mut layers = HashMap::new();
    let Some(records) = records else {
        return Ok(layers);
    };
    for record in records {
        if record.id.0 == 0
            || !objects.contains_key(&record.id)
            || record.layer > LAST_LAYER
            || layers.insert(record.id, record.layer).is_some()
        {
            return Err(corrupted("invalid, duplicate or missing object layer"));
        }
    }
    Ok(layers)
}

fn corrupted(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("Object current layer: {error}"))
}
