//! Original disabled-mask state absent from the positional host status.
use super::ObjectSnapshot;
use crate::game_logic::object::ScriptDisable;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
const TAG: &[u8; 4] = b"ODIS";
const VERSION_WITH_SCRIPT_STATUS: u32 = 2;
const VERSION_WITH_HELD_DEADLINES: u32 = 3;
const VERSION_WITH_UNDERPOWERED_DEADLINE: u32 = 4;
const VERSION: u32 = 5;
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
#[derive(Serialize, Deserialize)]
pub(super) struct HeldDeadline {
    object_id: u32,
    deadline: u32,
}

#[derive(Serialize, Deserialize)]
struct RecordPayload {
    records: Vec<Record>,
    held_deadlines: Vec<HeldDeadline>,
}

#[derive(Serialize, Deserialize)]
struct RecordPayloadWithUnderpoweredDeadline {
    disabled: RecordPayload,
    inactive_underpowered: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct ScriptDisableRecord {
    object_id: u32,
    script_underpowered: bool,
    inactive_disabled_deadline: bool,
    inactive_unpowered_deadline: bool,
}

#[derive(Serialize, Deserialize)]
struct RecordPayloadWithScriptDisables {
    records: Vec<Record>,
    held_deadlines: Vec<HeldDeadline>,
    inactive_underpowered: Vec<u32>,
    script_disables: Vec<ScriptDisableRecord>,
}

pub(super) struct Decoded<'a> {
    pub records: Option<Vec<Record>>,
    pub held_deadlines: Vec<HeldDeadline>,
    pub inactive_underpowered: Vec<u32>,
    pub script_disables: Vec<ScriptDisableRecord>,
    pub legacy_tail: Cow<'a, [u8]>,
}
pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    use game_engine::common::game_common::{FOREVER, NEVER};

    let mut records = Vec::with_capacity(world.host_objects().registered_len());
    let mut held_deadlines = Vec::new();
    let mut inactive_underpowered = Vec::new();
    let mut script_disables = Vec::new();
    for (id, object) in world.host_objects().registered() {
        records.push(Record {
            object_id: id.0,
            awestruck: object.status.disabled_awestruck,
            brainwashed: object.status.disabled_brainwashed,
            awestruck_until: object.status.disabled_awestruck_until_frame,
            brainwashed_until: object.status.disabled_brainwashed_until_frame,
            script_status_disabled: Some(object.status.script_status_disabled),
        });
        let deadline = object.held_disable_deadline();
        let implied = if object.status.disabled_held { FOREVER } else { NEVER };
        if deadline != implied {
            held_deadlines.push(HeldDeadline {
                object_id: id.0,
                deadline,
            });
        }
        if !object.status.disabled_underpowered && object.underpowered_disable_deadline() == FOREVER {
            inactive_underpowered.push(id.0);
        }
        let inactive_disabled_deadline = !object.status.disabled_script_disabled
            && object.script_disable_deadline(ScriptDisable::Disabled) == FOREVER;
        let inactive_unpowered_deadline = !object.status.disabled_script_underpowered
            && object.script_disable_deadline(ScriptDisable::Underpowered) == FOREVER;
        let script_underpowered = object.is_script_underpowered();
        if script_underpowered != object.status.disabled_script_underpowered
            || inactive_disabled_deadline
            || inactive_unpowered_deadline
        {
            script_disables.push(ScriptDisableRecord {
                object_id: id.0,
                script_underpowered,
                inactive_disabled_deadline,
                inactive_unpowered_deadline,
            });
        }
    }
    records.sort_by_key(|entry| entry.object_id);
    held_deadlines.sort_by_key(|entry| entry.object_id);
    // Most HELD deadlines follow directly from the saved mask bit.
    let (version, payload) = if !script_disables.is_empty() {
        script_disables.sort_by_key(|entry| entry.object_id);
        inactive_underpowered.sort_unstable();
        (
            VERSION,
            bincode_legacy::serialize(&RecordPayloadWithScriptDisables {
                records,
                held_deadlines,
                inactive_underpowered,
                script_disables,
            }),
        )
    } else if !inactive_underpowered.is_empty() {
        inactive_underpowered.sort_unstable();
        (
            VERSION_WITH_UNDERPOWERED_DEADLINE,
            bincode_legacy::serialize(&RecordPayloadWithUnderpoweredDeadline {
                disabled: RecordPayload { records, held_deadlines },
                inactive_underpowered,
            }),
        )
    } else if held_deadlines.is_empty() {
        (VERSION_WITH_SCRIPT_STATUS, bincode_legacy::serialize(&records))
    } else {
        (
            VERSION_WITH_HELD_DEADLINES,
            bincode_legacy::serialize(&RecordPayload {
                records,
                held_deadlines,
            }),
        )
    };
    let payload = payload.map_err(corrupted)?;
    let length = u32::try_from(payload.len()).map_err(corrupted)?;
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&version.to_le_bytes());
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
            held_deadlines: Vec::new(),
            inactive_underpowered: Vec::new(),
            script_disables: Vec::new(),
            legacy_tail: Cow::Borrowed(bytes),
        });
    }
    if suffix.len() < 12 {
        return Err(corrupted("truncated header"));
    }
    let version = u32::from_le_bytes(suffix[4..8].try_into().unwrap());
    let length = u32::from_le_bytes(suffix[8..12].try_into().unwrap()) as usize;
    if !matches!(
        version,
        1 | VERSION_WITH_SCRIPT_STATUS | VERSION_WITH_HELD_DEADLINES
            | VERSION_WITH_UNDERPOWERED_DEADLINE | VERSION
    ) {
        return Err(corrupted("unknown version"));
    }
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated records"))?;
    let payload_length = payload.len();
    let (records, held_deadlines, inactive_underpowered, script_disables, consumed) = match version {
        1 => {
            let (old, consumed): (Vec<RecordV1>, usize) =
                bincode::serde::decode_from_slice(payload, bincode::config::legacy())
                    .map_err(corrupted)?;
            let records = old
                .into_iter()
                .map(|entry| Record {
                    object_id: entry.object_id,
                    awestruck: entry.awestruck,
                    brainwashed: entry.brainwashed,
                    awestruck_until: entry.awestruck_until,
                    brainwashed_until: entry.brainwashed_until,
                    script_status_disabled: None,
                })
                .collect();
            (records, Vec::new(), Vec::new(), Vec::new(), consumed)
        }
        VERSION_WITH_SCRIPT_STATUS => {
            let (records, consumed) =
                bincode::serde::decode_from_slice(payload, bincode::config::legacy())
                    .map_err(corrupted)?;
            (records, Vec::new(), Vec::new(), Vec::new(), consumed)
        }
        VERSION_WITH_HELD_DEADLINES => {
            let (decoded, consumed): (RecordPayload, usize) =
                bincode::serde::decode_from_slice(payload, bincode::config::legacy())
                    .map_err(corrupted)?;
            (decoded.records, decoded.held_deadlines, Vec::new(), Vec::new(), consumed)
        }
        VERSION_WITH_UNDERPOWERED_DEADLINE => {
            let (decoded, consumed): (RecordPayloadWithUnderpoweredDeadline, usize) =
                bincode::serde::decode_from_slice(payload, bincode::config::legacy())
                    .map_err(corrupted)?;
            (
                decoded.disabled.records,
                decoded.disabled.held_deadlines,
                decoded.inactive_underpowered,
                Vec::new(),
                consumed,
            )
        }
        VERSION => {
            let (decoded, consumed): (RecordPayloadWithScriptDisables, usize) =
                bincode::serde::decode_from_slice(payload, bincode::config::legacy())
                    .map_err(corrupted)?;
            (
                decoded.records,
                decoded.held_deadlines,
                decoded.inactive_underpowered,
                decoded.script_disables,
                consumed,
            )
        }
        _ => unreachable!("disabled continuation version checked above"),
    };
    if version >= VERSION_WITH_SCRIPT_STATUS
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
        held_deadlines,
        inactive_underpowered,
        script_disables,
        legacy_tail: Cow::Owned(tail),
    })
}

pub(super) fn validate(
    records: Option<&[Record]>,
    held_deadlines: &[HeldDeadline],
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
    seen.clear();
    for held in held_deadlines {
        let id = ObjectId(held.object_id);
        if held.deadline == game_engine::common::game_common::NEVER
            || !objects.contains_key(&id)
            || !seen.insert(id)
        {
            return Err(corrupted("invalid HELD deadline or owner"));
        }
    }
    Ok(())
}

/// Apply after OXOB has restored the active bit and its implied deadline.
pub(super) fn apply_held_deadlines(held_deadlines: &[HeldDeadline], world: &mut GameLogic) {
    for entry in held_deadlines {
        if let Some(object) = world.objects.get_mut(&ObjectId(entry.object_id)) {
            object.status.disabled_held_until_frame = entry.deadline;
        }
    }
}

pub(super) fn validate_inactive_underpowered_deadlines(
    ids: &[u32],
    objects: &HashMap<ObjectId, ObjectSnapshot>,
) -> SaveLoadResult<()> {
    let mut seen = HashSet::new();
    for &id in ids {
        if objects
            .get(&ObjectId(id))
            .is_none_or(|object| object.status.disabled_underpowered)
            || !seen.insert(id)
        {
            return Err(corrupted("invalid or duplicate inactive UNDERPOWERED deadline owner"));
        }
    }
    Ok(())
}

pub(super) fn apply_inactive_underpowered_deadlines(ids: &[u32], world: &mut GameLogic) {
    for &id in ids {
        if let Some(object) = world.objects.get_mut(&ObjectId(id)) {
            object.status.disabled_underpowered_until_frame =
                game_engine::common::game_common::FOREVER;
        }
    }
}

pub(super) fn validate_script_disables(
    records: &[ScriptDisableRecord],
    objects: &HashMap<ObjectId, ObjectSnapshot>,
) -> SaveLoadResult<()> {
    let mut seen = HashSet::new();
    for record in records {
        let Some(object) = objects.get(&ObjectId(record.object_id)) else {
            return Err(corrupted("unknown script disable owner"));
        };
        if !seen.insert(record.object_id)
            || (record.inactive_disabled_deadline && object.status.disabled_script_disabled)
            || (record.inactive_unpowered_deadline && object.status.disabled_script_underpowered)
        {
            return Err(corrupted("invalid or duplicate script disable continuation"));
        }
    }
    Ok(())
}

pub(super) fn apply_script_disables(records: &[ScriptDisableRecord], world: &mut GameLogic) {
    use game_engine::common::game_common::FOREVER;

    for record in records {
        if let Some(object) = world.objects.get_mut(&ObjectId(record.object_id)) {
            object.status.script_status_underpowered = record.script_underpowered;
            if record.inactive_disabled_deadline {
                object.status.disabled_script_disabled_until_frame = FOREVER;
            }
            if record.inactive_unpowered_deadline {
                object.status.disabled_script_underpowered_until_frame = FOREVER;
            }
        }
    }
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
