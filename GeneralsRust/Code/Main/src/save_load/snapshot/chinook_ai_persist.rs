//! Persist ChinookAI flight status + combat-drop (C++ ChinookAIUpdate).
//!
//! C++ `ChinookAIUpdate::xfer` v2 (`ChinookAIUpdate.cpp:1338-1361`) writes
//! SupplyTruckAI base, pending command, `m_flightStatus`,
//! `m_airfieldForHealing`, and `m_originalPos`. A Chinook mid combat-drop,
//! landing, or evac-pending takeoff keeps that pose after load. Live
//! `Object.chinook_ai` was left `None` on restore — only Listening Outpost
//! was reinstalled — so a mid-drop Chinook lost ropes/stagger and the next
//! combat-drop started from Flying.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload so
//! older decoders ignore the extra bytes. No WorldSnapshot version bump.

use crate::game_logic::host_combat_chinook::HostChinookAI;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const CHNK_MAGIC: &[u8; 4] = b"CHNK";
const CHNK_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ChinookAiPersistPayload {
    objects: Vec<ObjectChinookAiPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectChinookAiPersist {
    object_id: u32,
    combat: bool,
    ai: HostChinookAI,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(CHNK_MAGIC);
    append_u32(bytes, CHNK_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_chnk_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != CHNK_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown CHNK suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "CHNK payload truncated".to_string(),
        ));
    }
    let payload: ChinookAiPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("CHNK payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ChinookAiPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        let Some(ai) = object.chinook_ai.clone() else {
            continue;
        };
        objects.push(ObjectChinookAiPersist {
            object_id: id.0,
            combat: object.is_combat_chinook_style_container(),
            ai,
        });
    }
    ChinookAiPersistPayload { objects }
}

fn apply_payload(game_logic: &mut GameLogic, payload: ChinookAiPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        if object.chinook_ai.is_none() {
            if entry.combat {
                object.install_combat_chinook_transport();
            } else {
                object.install_chinook_transport();
            }
        }
        object.chinook_ai = Some(entry.ai);
        if entry.combat {
            object.is_combat_chinook_transport = true;
        }
    }
}

fn find_chnk_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == CHNK_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("CHNK u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_combat_chinook::{
        HostChinookAIState, HostChinookFlightStatus, HostRappelJob,
    };
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_chinook_combat_drop() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AirF_AmericaVehicleChinook".to_string(),
            ThingTemplate::new("AirF_AmericaVehicleChinook"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let bird = source
            .create_object(
                "AirF_AmericaVehicleChinook",
                Team::USA,
                Vec3::new(40.0, 80.0, 24.0),
            )
            .expect("chinook");
        {
            let obj = source.host_object_mut(bird).expect("chinook");
            obj.install_combat_chinook_transport();
            let ai = obj.chinook_ai.as_mut().expect("ai");
            ai.flight_status = HostChinookFlightStatus::DoingCombatDrop;
            ai.state = HostChinookAIState::DoCombatDrop;
            ai.original_pos = [40.0, 80.0, 24.0];
            ai.combat_drop_next_release_frame = 90;
            ai.combat_drop_releases = 2;
            ai.combat_drop_target = Some(55);
            ai.pending_evac_dest = Some([10.0, 0.0, 12.0]);
            ai.airfield_id = Some(8);
            ai.rappel_into_jobs = vec![HostRappelJob {
                rappeller: 101,
                building: Some(55),
                dest_y: 12.0,
            }];
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_chnk_suffix(&snapshot.lifecycle_tail).is_some(),
            "CHNK suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(bird).expect("restored chinook");
        assert!(
            loaded.chinook_ai.is_some(),
            "chinook_ai must be reinstalled"
        );
        let ai = loaded.chinook_ai.as_ref().expect("ai");
        assert_eq!(ai.flight_status, HostChinookFlightStatus::DoingCombatDrop);
        assert_eq!(ai.state, HostChinookAIState::DoCombatDrop);
        assert_eq!(ai.original_pos, [40.0, 80.0, 24.0]);
        assert_eq!(ai.combat_drop_next_release_frame, 90);
        assert_eq!(ai.combat_drop_releases, 2);
        assert_eq!(ai.combat_drop_target, Some(55));
        assert_eq!(ai.pending_evac_dest, Some([10.0, 0.0, 12.0]));
        assert_eq!(ai.airfield_id, Some(8));
        assert_eq!(ai.rappel_into_jobs.len(), 1);
        assert_eq!(ai.rappel_into_jobs[0].rappeller, 101);
        assert!(loaded.is_combat_chinook_style_container());
    }

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_chnk_suffix(b"no-magic-here").is_none());
        apply_from_lifecycle_tail(b"no-magic-here", &mut GameLogic::new()).expect("apply");
    }
}
