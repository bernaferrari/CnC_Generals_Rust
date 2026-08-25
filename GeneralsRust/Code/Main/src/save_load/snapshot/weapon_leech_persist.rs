//! Persist leftover `Weapon::xfer` leech-range latch onto the live host.
//!
//! C++ `Weapon::xfer` v3 writes `m_leechWeaponRangeActive` after pitch-limited.
//! Leftover `xfer_weapon_crc_like_cpp` already matches that slot. Live stores
//! the latch as `Object.leech_range_active_primary` /
//! `leech_range_active_secondary`. Restore constructs via `new_with_logic_frame`,
//! so both flags stayed false and a mid-chase Terrorist / Burton knife dropped
//! out of `is_within_attack_range`.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes the latch only; it never re-runs pre-fire / fire.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const WLCH_MAGIC: &[u8; 4] = b"WLCH";
const WLCH_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WeaponLeechPersistPayload {
    objects: Vec<WeaponLeechPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WeaponLeechPersist {
    object_id: u32,
    leech_range_active_primary: bool,
    leech_range_active_secondary: bool,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(WLCH_MAGIC);
    append_u32(bytes, WLCH_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    reset_weapon_leech(game_logic);
    let Some(suffix) = find_wlch_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != WLCH_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown WLCH suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "WLCH payload truncated".to_string(),
        ));
    }
    let payload: WeaponLeechPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("WLCH payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> WeaponLeechPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if !object.leech_range_active_primary && !object.leech_range_active_secondary {
            continue;
        }
        objects.push(WeaponLeechPersist {
            object_id: id.0,
            leech_range_active_primary: object.leech_range_active_primary,
            leech_range_active_secondary: object.leech_range_active_secondary,
        });
    }
    WeaponLeechPersistPayload { objects }
}

fn reset_weapon_leech(game_logic: &mut GameLogic) {
    let ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    for id in ids {
        if let Some(object) = game_logic.host_object_mut(id) {
            object.leech_range_active_primary = false;
            object.leech_range_active_secondary = false;
        }
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: WeaponLeechPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.leech_range_active_primary = entry.leech_range_active_primary;
        object.leech_range_active_secondary = entry.leech_range_active_secondary;
    }
}

fn find_wlch_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == WLCH_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("WLCH u32 truncated".to_string()));
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
    fn snapshot_round_trips_weapon_leech_latch() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "GLAInfantryTerrorist".to_string(),
            ThingTemplate::new("GLAInfantryTerrorist"),
        );
        source.add_player(Player::new(0, Team::GLA, "GLA", true));
        let id = source
            .create_object("GLAInfantryTerrorist", Team::GLA, Vec3::new(8.0, 0.0, 6.0))
            .expect("terrorist");
        {
            let object = source.host_object_mut(id).expect("terrorist obj");
            object.leech_range_active_primary = true;
            object.leech_range_active_secondary = true;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_wlch_suffix(&snapshot.lifecycle_tail).is_some(),
            "WLCH suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(id).expect("restored terrorist");
        assert!(
            loaded.leech_range_active_primary,
            "primary leech latch must survive load"
        );
        assert!(
            loaded.leech_range_active_secondary,
            "secondary leech latch must survive load"
        );
    }

    #[test]
    fn absent_suffix_clears_stale_weapon_leech() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "GLAInfantryTerrorist".to_string(),
            ThingTemplate::new("GLAInfantryTerrorist"),
        );
        logic.add_player(Player::new(0, Team::GLA, "GLA", true));
        let id = logic
            .create_object("GLAInfantryTerrorist", Team::GLA, Vec3::ZERO)
            .expect("terrorist");
        {
            let object = logic.host_object_mut(id).expect("terrorist");
            object.leech_range_active_primary = true;
            object.leech_range_active_secondary = true;
        }
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let object = logic.host_object(id).expect("terrorist");
        assert!(!object.leech_range_active_primary);
        assert!(!object.leech_range_active_secondary);
    }
}
