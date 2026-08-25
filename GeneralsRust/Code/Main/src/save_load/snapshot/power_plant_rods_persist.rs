//! Persist live PowerPlant rod extend state.
//!
//! C++ `PowerPlantUpdate::xfer` v1 (`PowerPlantUpdate.cpp:117-131`) writes
//! UpdateModule base plus `m_extended`. Leftover `PowerPlantUpdate::xfer`
//! writes the same `extended` bool. Live stores
//! `Object.power_plant_rods_extended` + `power_plant_rods_done_frame`.
//! Overcharge persist restores only `overcharge_enabled` and must not
//! re-fire the enable path (power_provided already includes the bonus).
//! Rods were never snapshotted — a mid-overcharge save dropped
//! POWER_PLANT_UPGRADED / in-flight extend.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes the two fields only; it never calls
//! `begin_power_plant_rods_extend` or `enable_overcharge`.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const PPRD_MAGIC: &[u8; 4] = b"PPRD";
const PPRD_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PowerPlantRodsPersistPayload {
    plants: Vec<PowerPlantRodsPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PowerPlantRodsPersist {
    object_id: u32,
    rods_extended: bool,
    done_frame: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.plants.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(PPRD_MAGIC);
    append_u32(bytes, PPRD_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: reconstructed objects start idle; a reused GameLogic
    // must not keep the previous session's rod clocks.
    reset_rods(game_logic);
    let Some(suffix) = find_pprd_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != PPRD_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown PPRD suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "PPRD payload truncated".to_string(),
        ));
    }
    let payload: PowerPlantRodsPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("PPRD payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> PowerPlantRodsPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut plants = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if !object.power_plant_rods_extended && object.power_plant_rods_done_frame == 0 {
            continue;
        }
        plants.push(PowerPlantRodsPersist {
            object_id: id.0,
            rods_extended: object.power_plant_rods_extended,
            done_frame: object.power_plant_rods_done_frame,
        });
    }
    PowerPlantRodsPersistPayload { plants }
}

fn reset_rods(game_logic: &mut GameLogic) {
    let ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    for id in ids {
        if let Some(object) = game_logic.host_object_mut(id) {
            object.power_plant_rods_extended = false;
            object.power_plant_rods_done_frame = 0;
        }
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: PowerPlantRodsPersistPayload) {
    for entry in payload.plants {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        // C++ loadPostProcess does not re-run extendRods. Live overcharge
        // persist already stamps the module flag without re-firing enable.
        object.power_plant_rods_extended = entry.rods_extended;
        object.power_plant_rods_done_frame = entry.done_frame;
    }
}

fn find_pprd_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == PPRD_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("PPRD u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_power_plant_rods() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaPowerPlant".to_string(),
            ThingTemplate::new("AmericaPowerPlant"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let plant = source
            .create_object("AmericaPowerPlant", Team::USA, Vec3::new(20.0, 0.0, 16.0))
            .expect("plant");
        {
            let object = source.host_object_mut(plant).expect("plant obj");
            object.power_plant_rods_extended = true;
            object.power_plant_rods_done_frame = 120;
            object.set_overcharge_enabled(true);
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_pprd_suffix(&snapshot.lifecycle_tail).is_some(),
            "PPRD suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(plant).expect("restored plant");
        assert!(loaded.power_plant_rods_extended);
        assert_eq!(loaded.power_plant_rods_done_frame, 120);
        assert!(
            loaded.overcharge_enabled,
            "overcharge flag stays; enable path is not re-fired"
        );
    }

    #[test]
    fn absent_suffix_clears_stale_rods() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaPowerPlant".to_string(),
            ThingTemplate::new("AmericaPowerPlant"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let plant = logic
            .create_object("AmericaPowerPlant", Team::USA, Vec3::ZERO)
            .expect("plant");
        {
            let object = logic.host_object_mut(plant).expect("plant obj");
            object.power_plant_rods_extended = true;
            object.power_plant_rods_done_frame = 44;
        }
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let object = logic.host_object(plant).expect("plant");
        assert!(!object.power_plant_rods_extended);
        assert_eq!(object.power_plant_rods_done_frame, 0);
    }
}
