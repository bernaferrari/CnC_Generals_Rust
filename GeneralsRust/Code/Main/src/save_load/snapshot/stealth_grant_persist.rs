//! Persist Object CAN_STEALTH and StealthUpdate grant state.
//!
//! C++ Object::xfer saves the status bit independently of STEALTHED and
//! StealthUpdate::xfer saves m_enabled and, in version 2, m_framesGranted.
//! The scheduler's awake state is also retained. Restore after construction
//! and upgrades, without replaying grants.
//! Pulse phase and rate are restored together, as in StealthUpdate::xfer.

use super::ObjectSnapshot;
use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const STLG_MAGIC: &[u8; 4] = b"STLG";
const STLG_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(super) struct StealthGrantPersistPayload {
    objects: Vec<StealthGrantPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StealthGrantPersist {
    object_id: u32,
    can_stealth: bool,
    enabled: bool,
    awake: bool,
    frames_granted: u32,
    pulse_phase_rate: f32,
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

pub(super) fn prepare(
    records: &LifecycleRecords<'_>,
    objects: &HashMap<ObjectId, ObjectSnapshot>,
) -> SaveLoadResult<Option<StealthGrantPersistPayload>> {
    let Some(record) = records.get(STLG_MAGIC) else {
        return Ok(None);
    };
    let payload: StealthGrantPersistPayload = record.decode(STLG_VERSION)?;
    let mut seen = HashSet::with_capacity(objects.len());
    if payload.objects.len() != objects.len()
        || payload.objects.iter().any(|entry| {
            let id = ObjectId(entry.object_id);
            !objects.contains_key(&id) || !seen.insert(id)
        })
    {
        return Err(SaveLoadError::Corrupted(
            "STLG roster does not match world objects".into(),
        ));
    }
    Ok(Some(payload))
}

fn capture(game_logic: &GameLogic) -> StealthGrantPersistPayload {
    let mut objects = Vec::with_capacity(game_logic.host_objects().registered_len());
    for (id, object) in game_logic.host_objects().registered() {
        // False values must override constructor and upgrade side effects.
        objects.push(StealthGrantPersist {
            object_id: id.0,
            can_stealth: object.innate_stealth,
            enabled: object.stealth_update_enabled,
            awake: object.stealth_update_awake,
            frames_granted: object.stealth_frames_granted,
            pulse_phase_rate: object.camo_opacity_pulse_phase_rate,
            pulse_phase: object.camo_opacity_pulse_phase,
        });
    }
    objects.sort_unstable_by_key(|entry| entry.object_id);
    StealthGrantPersistPayload { objects }
}

pub(super) fn apply(payload: Option<StealthGrantPersistPayload>, game_logic: &mut GameLogic) {
    let Some(payload) = payload else {
        for object in game_logic.host_objects_mut().registered_values_mut() {
            object.stealth_frames_granted = 0;
        }
        return;
    };
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.innate_stealth = entry.can_stealth;
        object.stealth_update_enabled = entry.enabled;
        object.stealth_update_awake = entry.awake;
        object.stealth_frames_granted = entry.frames_granted;
        object.camo_opacity_pulse_phase_rate = entry.pulse_phase_rate;
        object.camo_opacity_pulse_phase = entry.pulse_phase;
    }
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
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
            prepare(
                &LifecycleRecords::parse(&snapshot.lifecycle_tail).unwrap(),
                &snapshot.objects
            )
            .unwrap()
            .is_some(),
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
        let revoked = source
            .create_object("Innate", Team::GLA, Vec3::ZERO)
            .unwrap();
        let granted = source
            .create_object("Granted", Team::GLA, Vec3::ZERO)
            .unwrap();
        let sleeping = source
            .create_object("Granted", Team::GLA, Vec3::ZERO)
            .unwrap();
        source
            .host_object_mut(revoked)
            .unwrap()
            .revoke_grant_stealth();
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
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .unwrap();

        let object = restored.host_object(revoked).unwrap();
        assert!(
            !object.innate_stealth,
            "revocation overrides innate constructor"
        );
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
        restored.update_stealth_and_detection().unwrap();
        let object = restored.host_object(granted).unwrap();
        assert_eq!(object.stealth_frames_granted, 1);
        assert!(object.innate_stealth);
        assert!((object.camo_opacity_pulse_phase - 1.45).abs() < 1e-5);
        restored.frame += 1;
        restored.update_stealth_and_detection().unwrap();
        let object = restored.host_object(granted).unwrap();
        assert_eq!(object.stealth_frames_granted, 0);
        assert!(!object.innate_stealth);
    }

    #[test]
    fn duplicate_grant_owner_rejects_before_restore_changes_live_objects() {
        let mut world = GameLogic::new();
        world
            .templates
            .insert("Worker".into(), ThingTemplate::new("Worker"));
        world.add_player(Player::new(0, Team::GLA, "GLA", true));
        let first = world
            .create_object("Worker", Team::GLA, Vec3::ZERO)
            .unwrap();
        let second = world.create_object("Worker", Team::GLA, Vec3::X).unwrap();
        let builder = super::super::SnapshotBuilder::new();
        let mut snapshot = builder.create_world_snapshot(&world).unwrap();
        let records = LifecycleRecords::parse(&snapshot.lifecycle_tail).unwrap();
        let range = records.get(STLG_MAGIC).unwrap().range();
        let mut payload = prepare(&records, &snapshot.objects).unwrap().unwrap();
        payload.objects[1].object_id = payload.objects[0].object_id;
        let bytes = bincode_legacy::serialize(&payload).unwrap();
        snapshot.lifecycle_tail[range.start + 12..range.end].copy_from_slice(&bytes);

        world.host_object_mut(first).unwrap().stealth_frames_granted = 900;
        world
            .host_object_mut(second)
            .unwrap()
            .stealth_frames_granted = 600;
        let error = builder
            .restore_from_snapshot(&snapshot, &mut world)
            .unwrap_err();
        assert!(
            matches!(error, SaveLoadError::Corrupted(message) if message.contains("STLG roster"))
        );
        assert_eq!(
            world.host_object(first).unwrap().stealth_frames_granted,
            900
        );
        assert_eq!(
            world.host_object(second).unwrap().stealth_frames_granted,
            600
        );
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
        let payload = prepare(&LifecycleRecords::default(), &HashMap::new()).unwrap();
        apply(payload, &mut logic);
        let object = logic.host_object(id).expect("worker");
        assert_eq!(object.stealth_frames_granted, 0);
    }
}
