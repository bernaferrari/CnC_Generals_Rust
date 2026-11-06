//! Persist Object CAN_STEALTH and StealthUpdate grant state.
//!
//! C++ Object::xfer saves the status bit independently of STEALTHED and
//! StealthUpdate::xfer saves m_enabled and, in version 2, m_framesGranted.
//! The scheduler's awake state is also retained. Restore after construction
//! and upgrades, without replaying grants.
//! Pulse rate is invariant at the native constructor's 0.2 in host-created state.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const STLG_MAGIC: &[u8; 4] = b"STLG";
const STLG_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StealthGrantPersistPayload {
    objects: Vec<StealthGrantPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StealthGrantPersist {
    object_id: u32,
    can_stealth: bool,
    enabled: bool,
    awake: bool,
    frames_granted: u32,
    pulse_phase: f32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(STLG_MAGIC);
    append_u32(bytes, STLG_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    reset_stealth_grant(game_logic);
    let Some(suffix) = find_stlg_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != STLG_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown STLG suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "STLG payload truncated".to_string(),
        ));
    }
    let payload: StealthGrantPersistPayload = bincode_legacy::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("STLG payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> StealthGrantPersistPayload {
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
        // False values must override constructor and upgrade side effects.
        objects.push(StealthGrantPersist {
            object_id: id.0,
            can_stealth: object.innate_stealth,
            enabled: object.stealth_update_enabled,
            awake: object.stealth_update_awake,
            frames_granted: object.stealth_frames_granted,
            pulse_phase: object.camo_opacity_pulse_phase,
        });
    }
    StealthGrantPersistPayload { objects }
}

fn reset_stealth_grant(game_logic: &mut GameLogic) {
    let ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
    for id in ids {
        if let Some(object) = game_logic.host_objects_mut().get_mut(&id) {
            object.stealth_frames_granted = 0;
        }
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: StealthGrantPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.innate_stealth = entry.can_stealth;
        object.stealth_update_enabled = entry.enabled;
        object.stealth_update_awake = entry.awake;
        object.stealth_frames_granted = entry.frames_granted;
        object.camo_opacity_pulse_phase = entry.pulse_phase;
    }
}

fn find_stlg_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == STLG_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("STLG u32 truncated".to_string()));
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
    fn snapshot_round_trips_temporary_stealth_grant() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "GLAInfantryWorker".to_string(),
            ThingTemplate::new("GLAInfantryWorker"),
        );
        source.add_player(Player::new(0, Team::GLA, "GLA", true));
        let id = source
            .create_object("GLAInfantryWorker", Team::GLA, Vec3::new(6.0, 0.0, 4.0))
            .expect("worker");
        {
            let object = source.host_object_mut(id).expect("worker obj");
            object.stealth_frames_granted = 600;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_stlg_suffix(&snapshot.lifecycle_tail).is_some(),
            "STLG suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.add_player(Player::new(0, Team::GLA, "GLA", true));
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(id).expect("restored worker");
        assert_eq!(loaded.stealth_frames_granted, 600);
    }

    #[test]
    fn snapshot_preserves_capability_independently_of_visible_stealth_and_template() {
        use crate::game_logic::object::StealthUpdateData;

        let mut source = GameLogic::new();
        let mut innate = ThingTemplate::new("Innate");
        innate.stealth_update = Some(StealthUpdateData {
            innate_stealth: true,
            ..Default::default()
        });
        source.templates.insert("Innate".into(), innate);
        let mut special = ThingTemplate::new("Granted");
        special.stealth_update = Some(StealthUpdateData {
            innate_stealth: false,
            granted_by_special_power: true,
            ..Default::default()
        });
        source.templates.insert("Granted".into(), special);
        source.add_player(Player::new(0, Team::GLA, "GLA", true));
        let revoked = source.create_object("Innate", Team::GLA, Vec3::ZERO).unwrap();
        let granted = source.create_object("Granted", Team::GLA, Vec3::ZERO).unwrap();
        let sleeping = source.create_object("Granted", Team::GLA, Vec3::ZERO).unwrap();
        source.host_object_mut(revoked).unwrap().revoke_grant_stealth();
        let object = source.host_object_mut(granted).unwrap();
        object.apply_grant_stealth(0);
        object.set_status_stealthed(false);
        object.stealth_frames_granted = 2;
        object.camo_opacity_pulse_phase = 1.25;

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).unwrap();
        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        restored.add_player(Player::new(0, Team::GLA, "GLA", true));
        builder.restore_from_snapshot(&snapshot, &mut restored).unwrap();

        let object = restored.host_object(revoked).unwrap();
        assert!(!object.innate_stealth, "revocation overrides innate constructor");
        assert!(!object.stealth_update_enabled);
        let object = restored.host_object(granted).unwrap();
        assert!(object.innate_stealth, "grant survives while revealed");
        assert!(object.stealth_update_enabled);
        assert!(object.stealth_update_awake);
        assert!(!object.status.stealthed);
        assert_eq!(object.stealth_frames_granted, 2);
        assert_eq!(object.camo_opacity_pulse_phase, 1.25);
        let object = restored.host_object(sleeping).unwrap();
        assert!(object.stealth_update_enabled);
        assert!(!object.stealth_update_awake);
        assert!(!object.innate_stealth);

        restored.frame += 1;
        restored.update_stealth_and_detection();
        let object = restored.host_object(granted).unwrap();
        assert_eq!(object.stealth_frames_granted, 1);
        assert!(object.innate_stealth);
        assert!((object.camo_opacity_pulse_phase - 1.45).abs() < 1e-5);
        restored.frame += 1;
        restored.update_stealth_and_detection();
        let object = restored.host_object(granted).unwrap();
        assert_eq!(object.stealth_frames_granted, 0);
        assert!(!object.innate_stealth);
    }

    #[test]
    fn absent_suffix_clears_stale_stealth_grant() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "GLAInfantryWorker".to_string(),
            ThingTemplate::new("GLAInfantryWorker"),
        );
        logic.add_player(Player::new(0, Team::GLA, "GLA", true));
        let id = logic
            .create_object("GLAInfantryWorker", Team::GLA, Vec3::ZERO)
            .expect("worker");
        {
            let object = logic.host_object_mut(id).expect("worker");
            object.stealth_frames_granted = 600;
        }
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let object = logic.host_object(id).expect("worker");
        assert_eq!(object.stealth_frames_granted, 0);
    }
}
