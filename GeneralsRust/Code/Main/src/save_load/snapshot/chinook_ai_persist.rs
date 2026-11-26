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
//! CHNK v3 stores flight continuation. Carried boxes are in the actor inventory.

use super::lifecycle_records::LifecycleRecords;
use generals_sim::chinook::ChinookAi;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::SaveLoadResult;
use serde::{Deserialize, Serialize};

const CHNK_MAGIC: &[u8; 4] = b"CHNK";
const CHNK_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ChinookAiPersistPayload {
    objects: Vec<ObjectChinookAiPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectChinookAiPersist {
    object_id: u32,
    combat: bool,
    ai: ChinookAi,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(CHNK_MAGIC);
    append_u32(bytes, CHNK_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    let Some(record) = records.get(CHNK_MAGIC) else {
        return Ok(());
    };
    let payload: ChinookAiPersistPayload = record.decode(CHNK_VERSION)?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ChinookAiPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
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
    let frame = game_logic.get_current_frame() as u32;
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        if object.chinook_ai.is_none() {
            if entry.combat {
                object.install_combat_chinook_transport(frame);
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

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use generals_sim::chinook::{
        ChinookAiState, ChinookFlightStatus, RappelJob,
    };
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_chinook_combat_drop() {
        let mut source = GameLogic::new();
        let mut combat = ThingTemplate::new("AirF_AmericaVehicleChinook");
        crate::game_logic::host_combat_chinook::author_combat_chinook_contain(&mut combat);
        source
            .templates
            .insert("AirF_AmericaVehicleChinook".to_string(), combat);
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let bird = source
            .create_object(
                "AirF_AmericaVehicleChinook",
                Team::USA,
                Vec3::new(40.0, 80.0, 24.0),
            )
            .expect("chinook");
        {
            let frame = source.get_current_frame() as u32;
            let obj = source.host_object_mut(bird).expect("chinook");
            obj.install_combat_chinook_transport(frame);
            let ai = obj.chinook_ai.as_mut().expect("ai");
            ai.flight_status = ChinookFlightStatus::DoingCombatDrop;
            ai.state = ChinookAiState::DoCombatDrop;
            ai.original_pos = [40.0, 80.0, 24.0];
            ai.combat_drop_next_release_frame = 90;
            ai.combat_drop_releases = 2;
            ai.combat_drop_target = Some(55);
            ai.pending_evac_dest = Some([10.0, 0.0, 12.0]);
            ai.airfield_id = Some(8);
            ai.rappel_into_jobs = vec![RappelJob {
                rappeller: 101,
                building: Some(55),
                dest_y: 12.0,
            }];
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(CHNK_MAGIC)
                .is_some(),
            "CHNK suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.add_player(Player::new(0, Team::USA, "USA", true));
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
        assert_eq!(ai.flight_status, ChinookFlightStatus::DoingCombatDrop);
        assert_eq!(ai.state, ChinookAiState::DoCombatDrop);
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
        assert!(
            LifecycleRecords::parse(&[])
                .unwrap()
                .get(CHNK_MAGIC)
                .is_none()
        );
        apply(
            &LifecycleRecords::parse(&[]).unwrap(),
            &mut GameLogic::new(),
        )
        .expect("apply");
    }
}

#[cfg(test)]
mod derived_ai_idle_native_tests {
    use super::*;
    #[test]
    fn native_v2_round_trips_pending_ai_idle_and_rejects_v1_without_mutation() {
        use crate::game_logic::{Player, Team, ThingTemplate};
        let mut source = GameLogic::new();
        source.add_player(Player::new(1, Team::USA, "Owner", true));
        source
            .templates
            .insert("IdleNative".into(), ThingTemplate::new("IdleNative"));
        let id = source
            .create_object_for_player("IdleNative", 1, glam::Vec3::ZERO)
            .expect("object");
        let object = source.host_object_mut(id).unwrap();
        object.install_chinook_transport();
        object.chinook_ai.as_mut().unwrap().pending_ai_idle = true;
        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        let mut restored = GameLogic::new();
        restored.add_player(Player::new(1, Team::USA, "Owner", true));
        restored
            .templates
            .insert("IdleNative".into(), ThingTemplate::new("IdleNative"));
        let restored_id = restored
            .create_object_for_player("IdleNative", 1, glam::Vec3::ZERO)
            .expect("object");
        assert_eq!(id, restored_id);
        restored.host_object_mut(id).unwrap().chinook_ai = None;
        apply(&LifecycleRecords::parse(&bytes).unwrap(), &mut restored).unwrap();
        assert!(
            restored
                .host_object(id)
                .unwrap()
                .chinook_ai
                .as_ref()
                .unwrap()
                .pending_ai_idle
        );
        let prior = restored
            .host_object(id)
            .unwrap()
            .chinook_ai
            .as_ref()
            .unwrap()
            .pending_ai_idle;
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(
            LifecycleRecords::parse(&bytes)
                .and_then(|records| apply(&records, &mut restored))
                .is_err()
        );
        assert_eq!(
            restored
                .host_object(id)
                .unwrap()
                .chinook_ai
                .as_ref()
                .unwrap()
                .pending_ai_idle,
            prior
        );
    }
}
