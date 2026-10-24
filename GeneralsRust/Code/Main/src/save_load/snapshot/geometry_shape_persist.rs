//! Actual mutable Thing geometry, separate from legacy positional bounds caches.
//! GMSH is admitted only at the known CPRJ structural boundary.
use super::ObjectSnapshot;
use crate::game_logic::{GameLogic, HostGeometryInfo, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
const TAG: &[u8; 4] = b"GMSH";
const VERSION: u32 = 1;
#[derive(Serialize, Deserialize)]
pub(super) struct Record {
    object_id: u32,
    shape: HostGeometryInfo,
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
            shape: *object.geometry_info(),
        })
        .collect();
    records.sort_by_key(|record| record.object_id);
    let payload = bincode_legacy::serialize(&records).map_err(corrupted)?;
    let length = u32::try_from(payload.len()).map_err(corrupted)?;
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(&payload);
    Ok(())
}
pub(super) fn decode(bytes: &[u8]) -> SaveLoadResult<Decoded<'_>> {
    let boundary = super::combat_projectile_persist::prefix_len(bytes)?;
    let suffix = &bytes[boundary..];
    if !suffix.is_empty() && suffix.len() < 4 && TAG.starts_with(suffix) {
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
    if version != VERSION {
        return Err(corrupted("unknown version"));
    }
    let length = u32::from_le_bytes(suffix[8..12].try_into().unwrap()) as usize;
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated payload"))?;
    let (records, used): (Vec<Record>, usize) =
        bincode::serde::decode_from_slice(payload, bincode::config::legacy()).map_err(corrupted)?;
    if used != payload.len() || suffix[end..].starts_with(TAG) {
        return Err(corrupted("trailing payload or duplicate frame"));
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
) -> SaveLoadResult<()> {
    let Some(records) = records else {
        return Ok(());
    };
    let mut ids = HashSet::new();
    for record in records {
        if !objects.contains_key(&ObjectId(record.object_id)) || !ids.insert(record.object_id) {
            return Err(corrupted("unknown or duplicate Object"));
        }
        let shape = &record.shape;
        if [shape.height, shape.major_radius, shape.minor_radius]
            .into_iter()
            .any(|value| !value.is_finite() || value < 0.)
        {
            return Err(corrupted("invalid shape extent"));
        }
    }
    if ids.len() != objects.len() {
        return Err(corrupted("incomplete Object roster"));
    }
    Ok(())
}
pub(super) fn shapes(records: Option<&[Record]>) -> HashMap<ObjectId, HostGeometryInfo> {
    records
        .into_iter()
        .flatten()
        .map(|record| (ObjectId(record.object_id), record.shape))
        .collect()
}

fn corrupted(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("mutable geometry continuation: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{HostGeometryType, Team, ThingTemplate};
    use crate::save_load::SnapshotBuilder;
    #[test]
    fn actual_bincode_world_keeps_copied_mutable_shape_and_same_id_peer_reset_is_inert() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "actual_bincode_world_keeps_copied_mutable_shape_and_same_id_peer_reset_is_inert",
            || {
                let mut owner = GameLogic::new();
                let mut peer = GameLogic::new();
                for world in [&mut owner, &mut peer] {
                    world
                        .templates
                        .insert("MutableShape".into(), ThingTemplate::new("MutableShape"));
                }
                let id = owner
                    .create_object("MutableShape", Team::USA, glam::Vec3::new(10., 2., 15.))
                    .unwrap();
                let other = peer
                    .create_object("MutableShape", Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                assert_eq!(id, other);
                let shape = HostGeometryInfo {
                    geom_type: HostGeometryType::Box,
                    is_small: false,
                    height: 17.,
                    major_radius: 30.,
                    minor_radius: 9.,
                    authored: true,
                };
                owner.set_fixture_object_geometry(id, shape.to_host_geometry());
                let copied = owner.host_object(id).unwrap().thing().geometry.clone();
                // Actual rebuild-hole producer uses this exact whole-geometry copy.
                peer.set_fixture_object_geometry(other, copied);
                assert_eq!(*peer.host_object(other).unwrap().geometry_info(), shape);
                let builder = SnapshotBuilder::new();
                let snapshot = builder.create_world_snapshot(&owner).unwrap();
                let bytes = bincode_legacy::serialize(&snapshot).unwrap();
                let decoded =
                    crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
                let mut restored = GameLogic::new();
                restored
                    .templates
                    .insert("MutableShape".into(), ThingTemplate::new("MutableShape"));
                builder
                    .restore_from_snapshot(&decoded, &mut restored)
                    .unwrap();
                assert_eq!(*restored.host_object(id).unwrap().geometry_info(), shape);
                assert_eq!(
                    restored.host_object(id).unwrap().get_position(),
                    glam::Vec3::new(10., 2., 15.)
                );
                peer.reset();
                drop(peer);
                assert_eq!(*owner.host_object(id).unwrap().geometry_info(), shape);
                assert_eq!(*restored.host_object(id).unwrap().geometry_info(), shape);
            },
        );
    }
    #[test]
    fn geometry_capsule_rejects_truncation_duplicate_unknown_version_and_cross_roster() {
        let mut world = GameLogic::new();
        world.templates.insert(
            "GeometryRecord".into(),
            ThingTemplate::new("GeometryRecord"),
        );
        let id = world
            .create_object("GeometryRecord", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let snapshot = SnapshotBuilder::new()
            .create_world_snapshot(&world)
            .unwrap();
        let mut prefix = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
        let boundary = prefix.len();
        append(&mut prefix, &world).unwrap();
        for length in boundary + 1..prefix.len() {
            assert!(decode(&prefix[..length]).is_err(), "truncation {length}");
        }
        let mut wrong = prefix.clone();
        wrong[boundary + 4..boundary + 8].copy_from_slice(&2_u32.to_le_bytes());
        assert!(decode(&wrong).is_err());
        let mut duplicate = prefix.clone();
        duplicate.extend_from_slice(&prefix[boundary..]);
        assert!(decode(&duplicate).is_err());
        let mut records = decode(&prefix).unwrap().records.unwrap();
        validate(Some(&records), &snapshot.objects).unwrap();
        records[0].object_id = id.0 + 1;
        assert!(validate(Some(&records), &snapshot.objects).is_err());
        records[0].object_id = id.0;
        records[0].shape.minor_radius = f32::NAN;
        assert!(validate(Some(&records), &snapshot.objects).is_err());
    }
}
