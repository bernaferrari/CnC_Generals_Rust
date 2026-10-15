//! GameLogic.cpp:4699-4786 transfers Objects in global-list order. Loading
//! recreates them in that order; registerObject prepends each new Object.
//! Keep this order independent of the ID lookup table and native map encoding.
use super::ObjectSnapshot;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

const TAG: &[u8; 4] = b"OLST";
const VERSION: u32 = 1;

pub(super) struct Decoded<'a> {
    pub saved_order: Option<Vec<ObjectId>>,
    pub legacy_tail: Cow<'a, [u8]>,
}

pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    let ids: Vec<_> = world.objects.ordered_ids().to_vec();
    let count = u32::try_from(ids.len()).map_err(corrupted)?;
    let length = count
        .checked_mul(4)
        .ok_or_else(|| corrupted("list too large"))?;
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&length.to_le_bytes());
    for id in ids {
        bytes.extend_from_slice(&id.0.to_le_bytes());
    }
    Ok(())
}

/// Preceding native capsules have been removed. Never search inside an opaque
/// module payload for a tag: its bytes may legitimately contain these letters.
pub(super) fn decode(bytes: &[u8]) -> SaveLoadResult<Decoded<'_>> {
    let boundary = super::combat_projectile_persist::prefix_len(bytes)?;
    let suffix = &bytes[boundary..];
    if !suffix.is_empty() && suffix.len() < TAG.len() && TAG.starts_with(suffix) {
        return Err(corrupted("truncated tag"));
    }
    if !suffix.starts_with(TAG) {
        return Ok(Decoded {
            saved_order: None,
            legacy_tail: Cow::Borrowed(bytes),
        });
    }
    if suffix.len() < 12 {
        return Err(corrupted("truncated header"));
    }
    let version = u32::from_le_bytes(suffix[4..8].try_into().unwrap());
    let length = u32::from_le_bytes(suffix[8..12].try_into().unwrap()) as usize;
    if version != VERSION || length % 4 != 0 {
        return Err(corrupted("invalid version or list length"));
    }
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated list"))?;
    if suffix[end..].starts_with(TAG) {
        return Err(corrupted("duplicate list"));
    }
    let saved_order = payload
        .chunks_exact(4)
        .map(|word| ObjectId(u32::from_le_bytes(word.try_into().unwrap())))
        .collect();
    let mut tail = bytes[..boundary].to_vec();
    tail.extend_from_slice(&suffix[end..]);
    Ok(Decoded {
        saved_order: Some(saved_order),
        legacy_tail: Cow::Owned(tail),
    })
}

pub(super) fn validate(
    order: Option<&[ObjectId]>,
    objects: &HashMap<ObjectId, ObjectSnapshot>,
) -> SaveLoadResult<()> {
    let Some(order) = order else {
        // Historical native saves did not record list order. Their former
        // sorted-ID restoration remains the explicitly limited fallback.
        return Ok(());
    };
    if order.len() != objects.len() {
        return Err(corrupted("list does not cover all object records"));
    }
    let mut seen = HashSet::with_capacity(order.len());
    for id in order {
        if id.0 == 0 || !objects.contains_key(id) || !seen.insert(*id) {
            return Err(corrupted("invalid, absent or duplicate object"));
        }
    }
    Ok(())
}

fn corrupted(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("Object list order: {error}"))
}

#[cfg(test)]
#[path = "object_order_persist_tests.rs"]
mod tests;
