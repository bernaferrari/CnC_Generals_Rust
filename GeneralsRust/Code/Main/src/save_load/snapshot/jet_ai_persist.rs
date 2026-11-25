//! Persist JetAI mid-flight RTB / takeoff / lock-on (C++ JetAIUpdate).
//!
//! C++ `JetAIUpdate::xfer` v2 (`JetAIUpdate.cpp:2528-2585`) writes producer
//! location, most-recent command, attack/sneaky expire frames, RTB frame,
//! targeted-by, untargetable expire, lock-on drawable, flags (takeoff /
//! landing / air-loco / interrupt-for-reload), and engines-on. A Raptor
//! mid-RTB or Stealth Fighter mid-lock-on resumes after load. Live
//! `Object.jet_ai` was constructed as `HostJetAi::default()` on restore —
//! mid-takeoff jets snapped, RTB was forgotten, stealth lock-on vanished.
//!
//! JTAI v4 carries the engine ambient latch. World snapshot version 52
//! requires this layout for serialized JetAI state.

use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::object::HostJetAi;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::SaveLoadResult;
use serde::{Deserialize, Serialize};

const JTAI_MAGIC: &[u8; 4] = b"JTAI";
const JTAI_VERSION: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct JetAiPersistPayload {
    objects: Vec<ObjectJetAiPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectJetAiPersist {
    object_id: u32,
    jet_ai: HostJetAi,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(JTAI_MAGIC);
    append_u32(bytes, JTAI_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    let Some(record) = records.get(JTAI_MAGIC) else {
        return Ok(());
    };
    let payload: JetAiPersistPayload = record.decode(JTAI_VERSION)?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> JetAiPersistPayload {
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
        if object.jet_ai == HostJetAi::default() {
            continue;
        }
        objects.push(ObjectJetAiPersist {
            object_id: id.0,
            jet_ai: object.jet_ai.clone(),
        });
    }
    JetAiPersistPayload { objects }
}

fn apply_payload(game_logic: &mut GameLogic, payload: JetAiPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.jet_ai = entry.jet_ai;
    }
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::object::HostJetPendingResume;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_jet_rtb_takeoff_lockon_and_engine_ambient() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaJetRaptor".to_string(),
            ThingTemplate::new("AmericaJetRaptor"),
        );
        source.templates.insert(
            "AmericaJetStealthFighter".to_string(),
            ThingTemplate::new("AmericaJetStealthFighter"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let raptor = source
            .create_object("AmericaJetRaptor", Team::USA, Vec3::new(40.0, 80.0, 24.0))
            .expect("raptor");
        let parked = source
            .create_object("AmericaJetRaptor", Team::USA, Vec3::new(60.0, 0.0, 24.0))
            .expect("parked raptor");
        source.host_object_mut(parked).unwrap().jet_ai.engines_stopped = true;
        let stealth = source
            .create_object(
                "AmericaJetStealthFighter",
                Team::USA,
                Vec3::new(80.0, 80.0, 24.0),
            )
            .expect("stealth");
        {
            let jet = source.host_object_mut(raptor).expect("raptor");
            jet.jet_ai.return_to_base_frame = 240;
            jet.jet_ai.takeoff_in_progress = true;
            jet.jet_ai.allow_air_loco = true;
            jet.jet_ai.allow_interrupt_for_reload = true;
            jet.jet_ai.has_pending_command = true;
            jet.jet_ai.pending_resume = HostJetPendingResume::GuardArea;
            jet.jet_ai.rtb_landing_phase = 1;
            jet.jet_ai.afterburners_on = true;
        }
        {
            let jet = source.host_object_mut(stealth).expect("stealth");
            jet.jet_ai.lockon_pos = Some([12.0, 4.0, 8.0]);
            jet.jet_ai.lockon_drawable_template = Some("LockonCursor_JTAI".into());
            jet.jet_ai.lockon_tick_pending = true;
            jet.jet_ai.untargetable_expire_frame = 310;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(JTAI_MAGIC)
                .is_some(),
            "JTAI suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.add_player(Player::new(0, Team::USA, "USA", true));
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(raptor).expect("restored raptor");
        assert_eq!(loaded.jet_ai.return_to_base_frame, 240);
        assert!(loaded.jet_ai.takeoff_in_progress);
        assert!(loaded.jet_ai.allow_air_loco);
        assert!(loaded.jet_ai.allow_interrupt_for_reload);
        assert!(loaded.jet_ai.has_pending_command);
        assert_eq!(
            loaded.jet_ai.pending_resume,
            HostJetPendingResume::GuardArea
        );
        assert_eq!(loaded.jet_ai.rtb_landing_phase, 1);
        assert!(loaded.jet_ai.afterburners_on);
        assert!(!loaded.jet_ai.engines_stopped, "a new JetAI starts with engines on");
        assert!(restored.host_object(parked).unwrap().jet_ai.engines_stopped);

        let loaded_sf = restored.host_object(stealth).expect("restored stealth");
        assert_eq!(loaded_sf.jet_ai.lockon_pos, Some([12.0, 4.0, 8.0]));
        assert_eq!(
            loaded_sf.jet_ai.lockon_drawable_template.as_deref(),
            Some("LockonCursor_JTAI")
        );
        assert!(loaded_sf.jet_ai.lockon_runtime.is_none());
        assert!(loaded_sf.jet_ai.lockon_tick_pending);
        assert_eq!(loaded_sf.jet_ai.untargetable_expire_frame, 310);
    }

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(
            LifecycleRecords::parse(&[])
                .unwrap()
                .get(JTAI_MAGIC)
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
    fn native_v2_round_trips_ai_idle_and_rejects_v1_without_mutation() {
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
        object.jet_ai.has_pending_command = true;
        object.jet_ai.pending_resume = crate::game_logic::object::HostJetPendingResume::AiIdle;
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
        restored.host_object_mut(id).unwrap().jet_ai = HostJetAi::default();
        apply(&LifecycleRecords::parse(&bytes).unwrap(), &mut restored).unwrap();
        assert_eq!(
            restored.host_object(id).unwrap().jet_ai.pending_resume,
            crate::game_logic::object::HostJetPendingResume::AiIdle
        );
        let prior = restored.host_object(id).unwrap().jet_ai.clone();
        bytes[4..8].copy_from_slice(&(JTAI_VERSION - 1).to_le_bytes());
        assert!(
            LifecycleRecords::parse(&bytes)
                .and_then(|records| apply(&records, &mut restored))
                .is_err()
        );
        assert_eq!(restored.host_object(id).unwrap().jet_ai, prior);
    }
}
