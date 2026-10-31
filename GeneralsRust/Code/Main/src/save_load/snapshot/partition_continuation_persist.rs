//! Exact native partition history. PCOI is a native extension, not original wire parity.
use super::ObjectSnapshot;
use crate::game_logic::partition_manager::PartitionContinuation;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
const TAG: &[u8; 4] = b"PCOI";
const VERSION: u32 = 3;
// Match the save container's maximum uncompressed size; populated scalar
// grids can legitimately exceed the smaller limits used for metadata chunks.
const MAX_PAYLOAD_BYTES: usize = crate::save_load::compression::MAX_UNCOMPRESSED_SIZE;
#[derive(Deserialize)]
struct NativeV1 {
    modules: Vec<(u32, Vec<(i32, i32)>)>,
    cells: Vec<((i32, i32), Vec<u32>)>,
    dirty: Vec<(u32, bool, bool)>,
}

#[derive(Deserialize)]
struct NativeV2 {
    modules: Vec<(u32, Vec<(i32, i32)>)>,
    cells: Vec<((i32, i32), Vec<u32>)>,
    dirty: Vec<(u32, bool, bool)>,
    observations: Vec<(u32, Option<(i32, i32)>)>,
    scalar_stamps: Vec<(
        u32,
        crate::game_logic::partition_manager::HostPartitionAffectStamp,
        bool,
    )>,
}
pub(super) struct Decoded<'a> {
    pub state: Option<PartitionContinuation>,
    pub legacy_tail: Cow<'a, [u8]>,
}
pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    let state = world.capture_partition_continuation().map_err(corrupted)?;
    let payload = bincode_legacy::serialize(&state).map_err(corrupted)?;
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(corrupted("payload exceeds save size limit"));
    }
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
            state: None,
            legacy_tail: Cow::Borrowed(bytes),
        });
    }
    if suffix.len() < 12 {
        return Err(corrupted("truncated header"));
    }
    let version = u32::from_le_bytes(suffix[4..8].try_into().unwrap());
    if version != 1 && version != 2 && version != VERSION {
        return Err(corrupted("unknown version"));
    }
    let length = u32::from_le_bytes(suffix[8..12].try_into().unwrap()) as usize;
    if length > MAX_PAYLOAD_BYTES {
        return Err(corrupted("payload exceeds save size limit"));
    }
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated payload"))?;
    let (state, used): (PartitionContinuation, usize) = if version == 1 {
        let (legacy, used): (NativeV1, usize) = bincode::serde::decode_from_slice(
            payload,
            bincode::config::legacy().with_limit::<MAX_PAYLOAD_BYTES>(),
        )
        .map_err(corrupted)?;
        (
            PartitionContinuation::from_native_v1(legacy.modules, legacy.cells, legacy.dirty),
            used,
        )
    } else if version == 2 {
        let (legacy, used): (NativeV2, usize) = bincode::serde::decode_from_slice(
            payload,
            bincode::config::legacy().with_limit::<MAX_PAYLOAD_BYTES>(),
        )
        .map_err(corrupted)?;
        (
            PartitionContinuation::from_native_v2(
                legacy.modules,
                legacy.cells,
                legacy.dirty,
                legacy.observations,
                legacy.scalar_stamps,
            ),
            used,
        )
    } else {
        bincode::serde::decode_from_slice(
            payload,
            bincode::config::legacy().with_limit::<MAX_PAYLOAD_BYTES>(),
        )
        .map_err(corrupted)?
    };
    if used != payload.len() || suffix[end..].starts_with(TAG) {
        return Err(corrupted("trailing payload or duplicate frame"));
    }
    let mut legacy = bytes[..boundary].to_vec();
    legacy.extend_from_slice(&suffix[end..]);
    Ok(Decoded {
        state: Some(state),
        legacy_tail: Cow::Owned(legacy),
    })
}
pub(super) fn validate(
    state: Option<&PartitionContinuation>,
    objects: &HashMap<ObjectId, ObjectSnapshot>,
    shroud: &gamelogic::system::shroud_manager::ShroudSnapshot,
) -> SaveLoadResult<()> {
    if let Some(state) = state {
        state
            .validate_cell_bounds(shroud.grid.as_ref().map(|grid| [grid.width, grid.height]))
            .map_err(corrupted)?;
        state
            .validate(&objects.keys().map(|id| id.0).collect())
            .map_err(corrupted)?;
    }
    Ok(())
}

fn corrupted(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("native partition continuation: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Team, ThingTemplate};
    use crate::save_load::SnapshotBuilder;
    #[test]
    fn malformed_native_partition_frame_fails_before_receiving_roster_retirement() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "malformed_native_partition_frame_fails_before_receiving_roster_retirement",
            || {
                let mut owner = GameLogic::new();
                owner.templates.insert(
                    "PartitionPreflight".into(),
                    ThingTemplate::new("PartitionPreflight"),
                );
                let id = owner
                    .create_object("PartitionPreflight", Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                let token = owner.objects[&id].visual_draw_identity_token.clone();
                let builder = SnapshotBuilder::new();
                let mut snapshot = builder.create_world_snapshot(&owner).unwrap();
                let projectile_boundary =
                    super::super::combat_projectile_persist::prefix_len(&snapshot.lifecycle_tail)
                        .unwrap();
                let length = u32::from_le_bytes(
                    snapshot.lifecycle_tail[projectile_boundary + 8..projectile_boundary + 12]
                        .try_into()
                        .unwrap(),
                ) as usize;
                let boundary = projectile_boundary + 12 + length;
                assert_eq!(&snapshot.lifecycle_tail[boundary..boundary + 4], TAG);
                snapshot.lifecycle_tail[boundary + 4..boundary + 8]
                    .copy_from_slice(&u32::MAX.to_le_bytes());
                assert!(
                    builder
                        .restore_from_snapshot(&snapshot, &mut owner)
                        .is_err()
                );
                assert!(std::sync::Arc::ptr_eq(
                    &token,
                    &owner.objects[&id].visual_draw_identity_token
                ));
                assert_eq!(
                    owner.objects[&id].health.current,
                    owner.objects[&id].health.maximum
                );
            },
        );
    }
    #[test]
    fn malformed_native_scalar_cells_and_roster_fail_before_live_replacement() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "malformed_native_scalar_cells_and_roster_fail_before_live_replacement",
            || {
                let mut owner = GameLogic::new();
                owner.initialize_receiving_partition_grid().unwrap();
                owner.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
                "scalar-preflight.ini".into(),
                "Object ScalarPreflight\nKindOf = VEHICLE\nVisionRange = 80\nShroudClearingRange = 80\nThreatValue = 17\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nEnd\n".into(),
            )]).unwrap()).unwrap();
                owner.add_player(crate::game_logic::Player::new(
                    0,
                    Team::USA,
                    "Scalar preflight",
                    true,
                ));
                let id = owner
                    .create_object("ScalarPreflight", Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                owner.update_owned_partition_with_effects(&mut Default::default()).unwrap();
                let token = owner
                    .host_object(id)
                    .unwrap()
                    .visual_draw_identity_token
                    .clone();
                let builder = SnapshotBuilder::new();
                for kind in 0..4 {
                    let mut snapshot = builder.create_world_snapshot(&owner).unwrap();
                    let prefix = super::super::combat_projectile_persist::prefix_len(
                        &snapshot.lifecycle_tail,
                    )
                    .unwrap();
                    assert_eq!(&snapshot.lifecycle_tail[prefix..prefix + 4], b"CPRJ");
                    let projectile_len = u32::from_le_bytes(
                        snapshot.lifecycle_tail[prefix + 8..prefix + 12]
                            .try_into()
                            .unwrap(),
                    ) as usize;
                    let start = prefix + 12 + projectile_len;
                    assert_eq!(&snapshot.lifecycle_tail[start..start + 4], TAG);
                    let length = u32::from_le_bytes(
                        snapshot.lifecycle_tail[start + 8..start + 12]
                            .try_into()
                            .unwrap(),
                    ) as usize;
                    let state: PartitionContinuation = bincode_legacy::deserialize(
                        &snapshot.lifecycle_tail[start + 12..start + 12 + length],
                    )
                    .unwrap();
                    let mut json = serde_json::to_value(state).unwrap();
                    match kind {
                        0 => json["scalar_cells"]["width"] = serde_json::json!(1),
                        1 => json["scalar_cells"]["cells"][0][0] = serde_json::json!(u32::MAX),
                        2 => {
                            let first = json["scalar_cells"]["cells"][0].clone();
                            json["scalar_cells"]["cells"]
                                .as_array_mut()
                                .unwrap()
                                .push(first);
                        }
                        _ => json["scalar_stamps"][0][0] = serde_json::json!(u32::MAX),
                    }
                    let state: PartitionContinuation = serde_json::from_value(json).unwrap();
                    let payload = bincode_legacy::serialize(&state).unwrap();
                    let mut frame = TAG.to_vec();
                    frame.extend_from_slice(&VERSION.to_le_bytes());
                    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                    frame.extend_from_slice(&payload);
                    snapshot
                        .lifecycle_tail
                        .splice(start..start + 12 + length, frame);
                    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
                    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
                    assert!(builder.restore_from_snapshot(&decoded, &mut owner).is_err());
                    assert!(std::sync::Arc::ptr_eq(
                        &token,
                        &owner.host_object(id).unwrap().visual_draw_identity_token
                    ));
                    assert_eq!(owner.host_object(id).unwrap().health.current, 40.);
                    assert_eq!(owner.world_services.shroud().lock().unwrap().host_scalar_value_at(
                    &gamelogic::common::Coord3D::new(0.,0.,0.),0,
                    gamelogic::object::collide::partition_manager::ValueOrThreat::ThreatValue),Some(17));
                }
            },
        );
    }
}
