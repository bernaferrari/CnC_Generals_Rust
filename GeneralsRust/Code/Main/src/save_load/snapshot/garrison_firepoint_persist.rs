//! Persist live garrison FIREPOINT / STATION slot assignments.
//!
//! C++ `OpenContain::xfer` writes the fire-point block (`m_firePoints`
//! matrices, `m_firePointStart` / `Next` / `Size`, `m_noFirePointsInArt`).
//! `GarrisonContain::xfer` layers `m_garrisonPointsInitialized`,
//! `m_garrisonPointData` occupant ids + `m_placeFrame` / `m_lastEffectFrame`,
//! and the exit rally. Leftover `open_contain.rs` / `garrison_contain.rs`
//! Snapshotable::xfer already match that table.
//!
//! Live window assignment is `BuildingData.garrison_point_occupant` plus the
//! cached bone sets and `garrison_points_initialized`. Object snapshots never
//! wrote those fields — restore only rebuilt `garrisoned_units` from occupants,
//! so every FIREPOINT/STATION slot loaded free and the next shot re-picked
//! closest-empty. Append a tagged suffix after the historical v9
//! contain/producer payload so older decoders ignore the extra bytes. No
//! WorldSnapshot version bump. Restore writes slots / clocks / cached bones
//! only; it never re-runs `ensure_garrison_bones` or occupant reassignment.

use crate::game_logic::buildings::{BuildingData, BuildingType};
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use glam::Vec3;
use serde::{Deserialize, Serialize};

const GFPT_MAGIC: &[u8; 4] = b"GFPT";
const GFPT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct GarrisonFirepointPersistPayload {
    objects: Vec<GarrisonFirepointPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GarrisonFirepointPersist {
    object_id: u32,
    garrison_points_initialized: bool,
    garrison_points_condition: u8,
    occupants: Vec<Option<u32>>,
    fire_points: Vec<[f32; 3]>,
    fire_points_damaged: Vec<[f32; 3]>,
    fire_points_really_damaged: Vec<[f32; 3]>,
    station_points: Vec<[f32; 3]>,
    gun_last_effect_frames: Vec<u32>,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(GFPT_MAGIC);
    append_u32(bytes, GFPT_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_gfpt_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != GFPT_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown GFPT suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "GFPT payload truncated".to_string(),
        ));
    }
    let payload: GarrisonFirepointPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("GFPT payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> GarrisonFirepointPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        let Some(bd) = object.building_data.as_ref() else {
            continue;
        };
        if !has_firepoint_state(bd) {
            continue;
        }
        objects.push(GarrisonFirepointPersist {
            object_id: id.0,
            garrison_points_initialized: bd.garrison_points_initialized,
            garrison_points_condition: bd.garrison_points_condition,
            occupants: bd
                .garrison_point_occupant
                .iter()
                .map(|slot| slot.map(|occupant| occupant.0))
                .collect(),
            fire_points: vec3s_to_arr(&bd.garrison_fire_points),
            fire_points_damaged: vec3s_to_arr(&bd.garrison_fire_points_damaged),
            fire_points_really_damaged: vec3s_to_arr(&bd.garrison_fire_points_really_damaged),
            station_points: vec3s_to_arr(&bd.garrison_station_points),
            gun_last_effect_frames: bd
                .garrison_guns
                .iter()
                .map(|gun| gun.last_effect_frame)
                .collect(),
        });
    }
    GarrisonFirepointPersistPayload { objects }
}

fn apply_payload(game_logic: &mut GameLogic, payload: GarrisonFirepointPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        if object.building_data.is_none() {
            object.building_data = Some(BuildingData::new(BuildingType::Bunker));
        }
        let Some(bd) = object.building_data.as_mut() else {
            continue;
        };
        bd.garrison_points_initialized = entry.garrison_points_initialized;
        bd.garrison_points_condition = entry.garrison_points_condition;
        bd.garrison_point_occupant = entry
            .occupants
            .into_iter()
            .map(|slot| slot.map(ObjectId))
            .collect();
        bd.garrison_fire_points = arr_to_vec3s(&entry.fire_points);
        bd.garrison_fire_points_damaged = arr_to_vec3s(&entry.fire_points_damaged);
        bd.garrison_fire_points_really_damaged = arr_to_vec3s(&entry.fire_points_really_damaged);
        bd.garrison_station_points = arr_to_vec3s(&entry.station_points);
        if bd.garrison_guns.len() < entry.gun_last_effect_frames.len() {
            bd.garrison_guns
                .resize(entry.gun_last_effect_frames.len(), Default::default());
        }
        for (gun, frame) in bd
            .garrison_guns
            .iter_mut()
            .zip(entry.gun_last_effect_frames.iter())
        {
            gun.last_effect_frame = *frame;
        }
    }
}

fn has_firepoint_state(bd: &BuildingData) -> bool {
    bd.garrison_points_initialized
        || bd.garrison_point_occupant.iter().any(Option::is_some)
        || !bd.garrison_fire_points.is_empty()
        || !bd.garrison_fire_points_damaged.is_empty()
        || !bd.garrison_fire_points_really_damaged.is_empty()
        || !bd.garrison_station_points.is_empty()
        || bd
            .garrison_guns
            .iter()
            .any(|gun| gun.last_effect_frame != 0)
}

fn vec3s_to_arr(points: &[Vec3]) -> Vec<[f32; 3]> {
    points.iter().map(|p| [p.x, p.y, p.z]).collect()
}

fn arr_to_vec3s(points: &[[f32; 3]]) -> Vec<Vec3> {
    points.iter().map(|p| Vec3::new(p[0], p[1], p[2])).collect()
}

fn find_gfpt_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == GFPT_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("GFPT u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::buildings::GarrisonGunEffect;
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_garrison_firepoint_slots() {
        let mut source = GameLogic::new();
        let mut bunker = ThingTemplate::new("TestBunker");
        bunker.add_kind_of(KindOf::Structure);
        source.templates.insert("TestBunker".to_string(), bunker);
        let mut ranger = ThingTemplate::new("TestRanger");
        ranger.add_kind_of(KindOf::Infantry);
        source.templates.insert("TestRanger".to_string(), ranger);
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let bunker_id = source
            .create_object("TestBunker", Team::USA, Vec3::ZERO)
            .expect("bunker");
        let ranger_id = source
            .create_object("TestRanger", Team::USA, Vec3::new(5.0, 0.0, 0.0))
            .expect("ranger");
        {
            let bunker = source.host_object_mut(bunker_id).expect("bunker obj");
            bunker.occupants.push(ranger_id);
            if bunker.building_data.is_none() {
                bunker.building_data = Some(BuildingData::new(BuildingType::Bunker));
            }
            let bd = bunker.building_data.as_mut().expect("building data");
            bd.garrisoned_units = vec![ranger_id];
            bd.garrison_points_initialized = true;
            bd.garrison_points_condition = 0;
            bd.garrison_fire_points = vec![Vec3::new(-20.0, 0.0, 4.0), Vec3::new(20.0, 0.0, 4.0)];
            bd.garrison_point_occupant = vec![None, Some(ranger_id)];
            bd.garrison_guns = vec![
                GarrisonGunEffect {
                    last_effect_frame: 0,
                    ..Default::default()
                },
                GarrisonGunEffect {
                    last_effect_frame: 77,
                    ..Default::default()
                },
            ];
        }
        if let Some(ranger) = source.host_object_mut(ranger_id) {
            ranger.contained_by = Some(bunker_id);
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_gfpt_suffix(&snapshot.lifecycle_tail).is_some(),
            "GFPT suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(bunker_id).expect("restored bunker");
        let bd = loaded
            .building_data
            .as_ref()
            .expect("restored building data");
        assert!(bd.garrison_points_initialized);
        assert_eq!(bd.garrison_points_condition, 0);
        assert_eq!(bd.garrison_point_occupant, vec![None, Some(ranger_id)]);
        assert_eq!(
            bd.garrison_fire_points,
            vec![Vec3::new(-20.0, 0.0, 4.0), Vec3::new(20.0, 0.0, 4.0)]
        );
        assert_eq!(
            bd.garrison_guns.get(1).map(|gun| gun.last_effect_frame),
            Some(77)
        );
    }

    #[test]
    fn absent_suffix_leaves_empty_slots() {
        let mut logic = GameLogic::new();
        let mut bunker = ThingTemplate::new("TestBunker");
        bunker.add_kind_of(KindOf::Structure);
        logic.templates.insert("TestBunker".to_string(), bunker);
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let bunker_id = logic
            .create_object("TestBunker", Team::USA, Vec3::ZERO)
            .expect("bunker");
        {
            let bunker = logic.host_object_mut(bunker_id).expect("bunker");
            if let Some(bd) = bunker.building_data.as_mut() {
                bd.garrison_point_occupant = vec![Some(ObjectId(99))];
            }
        }
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let bunker = logic.host_object(bunker_id).expect("bunker");
        let slots = bunker
            .building_data
            .as_ref()
            .map(|bd| bd.garrison_point_occupant.clone())
            .unwrap_or_default();
        assert_eq!(slots, vec![Some(ObjectId(99))]);
    }
}
