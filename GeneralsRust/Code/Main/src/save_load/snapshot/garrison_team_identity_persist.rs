//! Exact original Garrison Team identity in the existing lifecycle byte tail.
//!
//! WorldSnapshot's positional bincode schema is unchanged; the GOTS suffix is
//! bounded by the existing APRO/CPAD footer chain and restored before it is
//! attached to any BuildingData.

use super::ObjectGarrisonOriginalTeamSnapshot;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::collections::HashSet;

const MAGIC: &[u8; 4] = b"GOTS";
const VERSION: u32 = 1;

pub(super) fn append_to_lifecycle_tail(
    bytes: &mut Vec<u8>,
    world: &GameLogic,
) -> SaveLoadResult<()> {
    let mut rows = Vec::new();
    for object in world.host_objects().registered_values() {
        let Some(building) = object.building_data.as_ref() else {
            continue;
        };
        match (
            building.original_team_instance_id,
            building.original_team_instance_name.as_deref(),
        ) {
            (None, None) => {}
            (Some(team_instance_id), Some(team_instance_name))
                if team_instance_id != 0
                    && object.id != ObjectId(0)
                    && !team_instance_name.is_empty()
                    && !team_instance_name.contains('\0') =>
            {
                let receiving_name = world
                    .team_factory
                    .with_read_without_flushing(|factory| {
                        factory.find_team_by_id(team_instance_id).and_then(|team| {
                            team.read().ok().map(|team| team.get_name().to_owned())
                        })
                    })
                    .map_err(|error| {
                        SaveLoadError::Serialization(format!(
                            "Cannot read receiving TeamFactory while saving Object {}: {error}",
                            object.id.0
                        ))
                    })?;
                if receiving_name.as_deref() != Some(team_instance_name) {
                    return Err(SaveLoadError::Serialization(format!(
                        "Object {} has a missing or inconsistent original Garrison Team identity",
                        object.id.0
                    )));
                }
                rows.push(ObjectGarrisonOriginalTeamSnapshot {
                    object_id: object.id,
                    team_instance_id,
                    team_instance_name: team_instance_name.to_owned(),
                });
            }
            _ => {
                return Err(SaveLoadError::Serialization(format!(
                    "Object {} has an incomplete or invalid original Garrison Team identity",
                    object.id.0
                )));
            }
        }
    }
    rows.sort_unstable_by_key(|row| row.object_id);
    bytes.extend_from_slice(&encode_capsule(&rows)?);
    Ok(())
}

/// Ensure a current-format snapshot carries the empty capsule when there are
/// no original Garrison Team rows. The capsule belongs immediately before the
/// APRO/CPAT/CPAD footer chain, after all lifecycle prefix records.
pub(super) fn ensure_empty_capsule(bytes: &mut Vec<u8>) -> SaveLoadResult<()> {
    if frame(bytes)?.is_some() {
        return Ok(());
    }
    let boundary = super::player_academy_persist::suffix_start(bytes)?;
    let capsule = encode_capsule(&[])?;
    bytes.splice(boundary..boundary, capsule);
    Ok(())
}

fn encode_capsule(rows: &[ObjectGarrisonOriginalTeamSnapshot]) -> SaveLoadResult<Vec<u8>> {
    let payload = bincode_legacy::serialize(rows)
        .map_err(|error| SaveLoadError::Serialization(error.to_string()))?;
    let length = u32::try_from(payload.len())
        .map_err(|_| SaveLoadError::Serialization("GOTS payload exceeds u32 framing".into()))?;
    let mut capsule = payload;
    capsule.extend_from_slice(&VERSION.to_le_bytes());
    capsule.extend_from_slice(&length.to_le_bytes());
    capsule.extend_from_slice(MAGIC);
    Ok(capsule)
}

/// Decode the GOTS capsule while validating its footer against the known
/// completion/particle/APRO suffix chain. A raw magic match inside a name or
/// serialized payload cannot select an arbitrary interior record.
pub(super) fn rows_from_lifecycle_tail(
    bytes: &[u8],
) -> SaveLoadResult<Vec<ObjectGarrisonOriginalTeamSnapshot>> {
    let Some((start, footer)) = frame(bytes)? else {
        return Ok(Vec::new());
    };
    let rows: Vec<ObjectGarrisonOriginalTeamSnapshot> =
        bincode_legacy::deserialize(&bytes[start..footer]).map_err(|_| invalid())?;
    let mut objects = HashSet::new();
    for row in &rows {
        if row.object_id == ObjectId(0)
            || row.team_instance_name.is_empty()
            || row.team_instance_name.contains('\0')
            || !objects.insert(row.object_id)
        {
            return Err(invalid());
        }
    }
    if rows
        .windows(2)
        .any(|pair| pair[0].object_id >= pair[1].object_id)
    {
        return Err(invalid());
    }
    Ok(rows)
}

pub(super) fn has_lifecycle_capsule(bytes: &[u8]) -> SaveLoadResult<bool> {
    Ok(frame(bytes)?.is_some())
}

pub(super) fn suffix_start(bytes: &[u8]) -> SaveLoadResult<usize> {
    let prior_end = super::player_academy_persist::suffix_start(bytes)?;
    let prefix = bytes.get(..prior_end).ok_or_else(invalid)?;
    Ok(frame(bytes)?
        .map(|(start, _)| start)
        .unwrap_or(prefix.len()))
}

fn frame(bytes: &[u8]) -> SaveLoadResult<Option<(usize, usize)>> {
    let end = super::player_academy_persist::suffix_start(bytes)?;
    let prefix = bytes.get(..end).ok_or_else(invalid)?;
    if !prefix.ends_with(MAGIC) {
        return Ok(None);
    }
    if prefix.len() < 12 {
        return Err(invalid());
    }
    let footer = prefix.len() - 12;
    let version = u32::from_le_bytes(prefix[footer..footer + 4].try_into().unwrap());
    let length = u32::from_le_bytes(prefix[footer + 4..footer + 8].try_into().unwrap()) as usize;
    let start = footer.checked_sub(length).ok_or_else(invalid)?;
    if version != VERSION || length == 0 || prefix[..start].ends_with(MAGIC) {
        return Err(invalid());
    }
    Ok(Some((start, footer)))
}

pub(super) fn validate_snapshot_rows(
    rows: &[ObjectGarrisonOriginalTeamSnapshot],
    snapshot: &super::WorldSnapshot,
    teams: &super::player_team_persist::TeamFactoryChunkPersist,
) -> SaveLoadResult<()> {
    let mut objects = HashSet::new();
    for row in rows {
        let matching_team = teams.teams.iter().any(|team| {
            team.team_id == row.team_instance_id && team.team_name == row.team_instance_name
        });
        let is_building = snapshot.objects.get(&row.object_id).is_some_and(|object| {
            matches!(object.object_type, super::ObjectTypeSnapshot::Building(_))
        });
        if !is_building || !objects.insert(row.object_id) || !matching_team {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Restore exact identities into the matching object births after the full
/// world and receiving TeamFactory have been reconstructed.
pub(super) fn apply_rows(
    rows: &[ObjectGarrisonOriginalTeamSnapshot],
    world: &mut GameLogic,
) -> SaveLoadResult<()> {
    for row in rows {
        let resolved_name = world
            .team_factory
            .with_read_without_flushing(|factory| {
                factory
                    .find_team_by_id(row.team_instance_id)
                    .and_then(|team| team.read().ok().map(|team| team.get_name().to_owned()))
            })
            .map_err(|_| invalid())?
            .ok_or_else(invalid)?;
        if resolved_name != row.team_instance_name {
            return Err(invalid());
        }
        let Some(object) = world.host_object_mut(row.object_id) else {
            return Err(invalid());
        };
        let building = object.building_data.as_mut().ok_or_else(invalid)?;
        building.original_team_instance_id = Some(row.team_instance_id);
        building.original_team_instance_name = Some(row.team_instance_name.clone());
    }
    Ok(())
}

fn invalid() -> SaveLoadError {
    SaveLoadError::Corrupted("invalid exact original Garrison Team identity capsule".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capsule(rows: &[ObjectGarrisonOriginalTeamSnapshot]) -> Vec<u8> {
        let payload = bincode_legacy::serialize(rows).unwrap();
        let mut bytes = payload.clone();
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(MAGIC);
        bytes
    }

    fn row(object_id: u32) -> ObjectGarrisonOriginalTeamSnapshot {
        ObjectGarrisonOriginalTeamSnapshot {
            object_id: ObjectId(object_id),
            team_instance_id: object_id + 10,
            team_instance_name: format!("Team{object_id}"),
        }
    }

    #[test]
    fn capsule_requires_complete_payload_and_unique_sorted_object_rows() {
        let rows = [row(1), row(2)];
        assert_eq!(rows_from_lifecycle_tail(&capsule(&rows)).unwrap(), rows);

        let mut truncated = capsule(&rows);
        truncated.drain(..1);
        assert!(rows_from_lifecycle_tail(&truncated).is_err());

        let duplicate = capsule(&[row(1), row(1)]);
        assert!(rows_from_lifecycle_tail(&duplicate).is_err());

        let unordered = capsule(&[row(2), row(1)]);
        assert!(rows_from_lifecycle_tail(&unordered).is_err());
    }
}
