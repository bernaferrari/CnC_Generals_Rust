//! PointDefenseLaserUpdate continuation: one state per module on each Object.
//! C++ PointDefenseLaserUpdate.cpp:363-386 writes target, range and both timers.

use crate::game_logic::{GameLogic, ObjectId};
use crate::game_logic::host_point_defense::PointDefenseState;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const PDLS_MAGIC: &[u8; 4] = b"PDLS";
const PDLS_VERSION: u32 = 3;

#[derive(Serialize, Deserialize)]
struct Record {
    object_id: u32,
    states: Vec<PointDefenseState>,
}
#[derive(Deserialize)]
struct LegacyEntry { object_id: u32, next_ready_frame: u32 }
#[derive(Deserialize)]
struct LegacyV1 { ready: Vec<LegacyEntry> }
#[derive(Deserialize)]
struct LegacyV2 { ready: Vec<LegacyEntry>, ready_1: Vec<LegacyEntry> }

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, world: &GameLogic) {
    let mut rows: Vec<_> = world.objects.registered().filter_map(|(&id, object)| {
        (!object.point_defense_states.is_empty()).then(|| Record {
            object_id: id.0, states: object.point_defense_states.clone(),
        })
    }).collect();
    rows.sort_by_key(|row| row.object_id);
    if rows.is_empty() { return; }
    let Ok(payload) = bincode_legacy::serialize(&rows) else { return };
    bytes.extend_from_slice(PDLS_MAGIC);
    bytes.extend_from_slice(&PDLS_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], world: &mut GameLogic) -> SaveLoadResult<()> {
    for object in world.objects.registered_values_mut() { object.point_defense_states.clear(); }
    let Some(index) = bytes.windows(4).rposition(|part| part == PDLS_MAGIC) else { return Ok(()) };
    let mut rest = &bytes[index + 4..];
    let version = take_u32(&mut rest)?;
    let length = take_u32(&mut rest)? as usize;
    if rest.len() < length { return Err(SaveLoadError::Corrupted("PDLS payload truncated".into())); }
    let encoded = &rest[..length];
    let decode_error = |err| SaveLoadError::Corrupted(format!("PDLS payload decode: {err}"));
    match version {
        1 | 2 => {
            let (first, second) = if version == 1 {
                let old: LegacyV1 = bincode_legacy::deserialize(encoded).map_err(decode_error)?;
                (old.ready, Vec::new())
            } else {
                let old: LegacyV2 = bincode_legacy::deserialize(encoded).map_err(decode_error)?;
                (old.ready, old.ready_1)
            };
            for (index, entries) in [(0, first), (1, second)] {
                for entry in entries {
                    if let Some(object) = world.objects.get_mut(&ObjectId(entry.object_id)) {
                        object.point_defense_states.resize_with(index + 1, PointDefenseState::default);
                        object.point_defense_states[index].next_shot = entry.next_ready_frame.saturating_sub(world.frame);
                    }
                }
            }
        }
        PDLS_VERSION => {
            let rows: Vec<Record> = bincode_legacy::deserialize(encoded).map_err(decode_error)?;
            for row in rows {
                if let Some(object) = world.objects.get_mut(&ObjectId(row.object_id)) {
                    object.point_defense_states = row.states;
                }
            }
        }
        _ => return Err(SaveLoadError::Corrupted(format!("unknown PDLS suffix version {version}"))),
    }
    Ok(())
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 { return Err(SaveLoadError::Corrupted("PDLS u32 truncated".into())); }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Team, ThingTemplate};
    use glam::Vec3;

    fn world() -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.templates.insert("DefenseSnapshot".into(), ThingTemplate::new("DefenseSnapshot"));
        let id = world.create_object("DefenseSnapshot", Team::USA, Vec3::ZERO).unwrap();
        (world, id)
    }

    #[test]
    fn snapshot_restores_each_module_and_clears_absent_state() {
        let (mut source, id) = world();
        source.host_object_mut(id).unwrap().point_defense_states = vec![
            PointDefenseState { next_shot: 17, next_scan: 4, target: Some(ObjectId(99)), in_range: true },
            PointDefenseState { next_shot: 31, ..Default::default() },
        ];
        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        let (mut restored, other_id) = world();
        assert_eq!(id, other_id);
        apply_from_lifecycle_tail(&bytes, &mut restored).unwrap();
        let states = &restored.host_object(id).unwrap().point_defense_states;
        assert_eq!((states[0].next_shot, states[0].next_scan, states[0].target, states[0].in_range), (17, 4, Some(ObjectId(99)), true));
        assert_eq!(states[1].next_shot, 31);
        apply_from_lifecycle_tail(b"no-magic-here", &mut restored).unwrap();
        assert!(restored.host_object(id).unwrap().point_defense_states.is_empty());
        assert_eq!(source.host_object(id).unwrap().point_defense_states[1].next_shot, 31);
    }
}
