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
//! Append a tagged suffix after the historical v9 contain/producer payload so
//! older decoders ignore the extra bytes. No WorldSnapshot version bump.

use crate::game_logic::object::HostJetAi;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const JTAI_MAGIC: &[u8; 4] = b"JTAI";
const JTAI_VERSION: u32 = 1;

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
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(JTAI_MAGIC);
    append_u32(bytes, JTAI_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_jtai_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != JTAI_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown JTAI suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "JTAI payload truncated".to_string(),
        ));
    }
    let payload: JetAiPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("JTAI payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> JetAiPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
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

fn find_jtai_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == JTAI_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("JTAI u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::object::HostJetPendingResume;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_jet_rtb_takeoff_and_lockon() {
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
            jet.jet_ai.lockon_drawable_id = Some(77);
            jet.jet_ai.lockon_tick_pending = true;
            jet.jet_ai.untargetable_expire_frame = 310;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_jtai_suffix(&snapshot.lifecycle_tail).is_some(),
            "JTAI suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
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

        let loaded_sf = restored.host_object(stealth).expect("restored stealth");
        assert_eq!(loaded_sf.jet_ai.lockon_pos, Some([12.0, 4.0, 8.0]));
        assert_eq!(loaded_sf.jet_ai.lockon_drawable_id, Some(77));
        assert!(loaded_sf.jet_ai.lockon_tick_pending);
        assert_eq!(loaded_sf.jet_ai.untargetable_expire_frame, 310);
    }

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_jtai_suffix(b"no-magic-here").is_none());
        apply_from_lifecycle_tail(b"no-magic-here", &mut GameLogic::new()).expect("apply");
    }
}
