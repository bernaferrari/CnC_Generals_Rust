//! Saved module identity is immutable reconstruction input, not another Body.
//! The frame follows CPRJ at the structural lifecycle boundary. Old positional
//! Object/Thing records and world versions remain unchanged.
use super::ObjectSnapshot;
use crate::game_logic::thing::BodyKind;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;

const TAG: &[u8; 4] = b"BDKD";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub(super) struct Record {
    object_id: u32,
    /// Explicit wire values: 0=legacy/unbound, 1=Active family, 2=Inactive.
    kind: u8,
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
            kind: match object.thing().template.body_kind {
                None => 0,
                Some(BodyKind::Active) => 1,
                Some(BodyKind::Inactive) => 2,
            },
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

/// Called after CPRJ was removed. Never search for a tag inside user strings or
/// positional payloads; only the known structural boundary can contain BDKD.
pub(super) fn decode(bytes: &[u8]) -> SaveLoadResult<Decoded<'_>> {
    let boundary = super::combat_projectile_persist::prefix_len(bytes)?;
    let suffix = &bytes[boundary..];
    if !suffix.is_empty() && suffix.len() < TAG.len() && TAG.starts_with(suffix) {
        return Err(corrupted("truncated body-kind tag"));
    }
    if !suffix.starts_with(TAG) {
        return Ok(Decoded {
            records: None,
            legacy_tail: Cow::Borrowed(bytes),
        });
    }
    if suffix.len() < 12 {
        return Err(corrupted("truncated body-kind header"));
    }
    let version = u32::from_le_bytes(suffix[4..8].try_into().unwrap());
    let length = u32::from_le_bytes(suffix[8..12].try_into().unwrap()) as usize;
    if version != VERSION {
        return Err(corrupted("unknown body-kind version"));
    }
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("body-kind length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated body-kind records"))?;
    let (records, consumed) =
        bincode::serde::decode_from_slice(payload, bincode::config::legacy()).map_err(corrupted)?;
    if consumed != payload.len() || suffix[end..].starts_with(TAG) {
        return Err(corrupted("trailing or duplicate body-kind record"));
    }
    let mut legacy = bytes[..boundary].to_vec();
    legacy.extend_from_slice(&suffix[end..]);
    Ok(Decoded {
        records: Some(records),
        legacy_tail: Cow::Owned(legacy),
    })
}

pub(super) fn validate(
    records: Option<&[Record]>,
    objects: &HashMap<ObjectId, ObjectSnapshot>,
    world: &GameLogic,
) -> SaveLoadResult<HashMap<ObjectId, BodyKind>> {
    let Some(records) = records else {
        return Ok(HashMap::new());
    };
    if records.len() != objects.len() {
        return Err(corrupted("body-kind roster does not match saved objects"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut kinds = HashMap::new();
    for record in records {
        let id = ObjectId(record.object_id);
        let object = objects
            .get(&id)
            .ok_or_else(|| corrupted("unknown body-kind owner"))?;
        if !seen.insert(id) {
            return Err(corrupted("duplicate body-kind owner"));
        }
        let kind = match record.kind {
            0 => None,
            1 => Some(BodyKind::Active),
            2 => Some(BodyKind::Inactive),
            _ => return Err(corrupted("invalid body-kind value")),
        };
        if let Some(kind) = kind {
            if world
                .templates
                .get(&object.template_name)
                .and_then(|template| template.body_kind)
                .is_some_and(|loaded| loaded != kind)
            {
                return Err(corrupted(format!(
                    "Body class differs from saved module for {}",
                    object.template_name
                )));
            }
            kinds.insert(id, kind);
        }
    }
    Ok(kinds)
}

fn corrupted(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("body-kind continuation: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_kind_frame_rejects_unknown_version_duplicate_and_truncation() {
        let source = GameLogic::new();
        let mut bytes = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
        let boundary = bytes.len();
        append(&mut bytes, &source).unwrap();
        assert_eq!(decode(&bytes).unwrap().records.unwrap().len(), 0);
        for length in boundary + 1..bytes.len() {
            assert!(decode(&bytes[..length]).is_err(), "truncation {length}");
        }
        let mut unknown = bytes.clone();
        unknown[boundary + 4..boundary + 8].copy_from_slice(&2u32.to_le_bytes());
        assert!(decode(&unknown).is_err());
        let mut duplicate = bytes.clone();
        duplicate.extend_from_slice(&bytes[boundary..]);
        assert!(decode(&duplicate).is_err());
    }

    #[test]
    fn body_kind_roster_rejects_missing_duplicate_unknown_and_invalid_owners() {
        use crate::game_logic::{Team, ThingTemplate};
        let mut world = GameLogic::new();
        world
            .templates
            .insert("BodyRoster".into(), ThingTemplate::new("BodyRoster"));
        let id = world
            .create_object("BodyRoster", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let snapshot = super::super::SnapshotBuilder::new()
            .create_world_snapshot(&world)
            .unwrap();
        let valid = [Record {
            object_id: id.0,
            kind: 0,
        }];
        assert!(
            validate(Some(&valid), &snapshot.objects, &world)
                .unwrap()
                .is_empty()
        );
        for malformed in [
            Vec::new(),
            vec![Record {
                object_id: id.0,
                kind: 3,
            }],
            vec![Record {
                object_id: id.0 + 10,
                kind: 0,
            }],
            vec![
                Record {
                    object_id: id.0,
                    kind: 0,
                },
                Record {
                    object_id: id.0,
                    kind: 0,
                },
            ],
        ] {
            assert!(validate(Some(&malformed), &snapshot.objects, &world).is_err());
        }
        // An absent frame is an explicitly supported old save, distinct from
        // an incomplete framed roster. It does not manufacture a module kind.
        assert!(
            validate(None, &snapshot.objects, &world)
                .unwrap()
                .is_empty()
        );
    }
}
