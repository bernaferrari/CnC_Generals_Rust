//! Persist live TurretAI aim / hold / idle-scan residual.
//!
//! C++ `TurretAI::xfer` v2 (`TurretAI.cpp:319-353`) writes the turret state
//! machine, `m_angle`, `m_pitch`, `m_enableSweepUntil`, `m_target`,
//! `m_continuousFireExpirationFrame`, sweep/didFire/enabled flags, and
//! `m_sleepUntil`. Leftover `turret.rs` Snapshotable xfer already matches
//! that table. Live stores the same residual on host `Object`
//! (`turret_angle_deg` / `turret_pitch_deg` / `turret_substate` /
//! `turret_target_id` / hold / idle-scan). Object snapshots never wrote
//! those fields — a mid-aim save snapped the barrel back to the template
//! natural angle and dropped the hold/target.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes clocks/flags only; it never re-runs aim or notifyFired.
//! Sweep extras live in the per-tick attack cache (3-frame window) and are
//! rebuilt on the next fire.

use crate::game_logic::object::TurretSubState;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const TRAI_MAGIC: &[u8; 4] = b"TRAI";
const TRAI_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TurretAimPersistPayload {
    objects: Vec<TurretAimPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TurretAimPersist {
    object_id: u32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(TRAI_MAGIC);
    append_u32(bytes, TRAI_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    reset_turret_aim(game_logic);
    let Some(suffix) = find_trai_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != TRAI_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown TRAI suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "TRAI payload truncated".to_string(),
        ));
    }
    let payload: TurretAimPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("TRAI payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> TurretAimPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if !object_has_turret_residual(object) {
            continue;
        }
        objects.push(TurretAimPersist {
            object_id: id.0,
            turret_angle_deg: object.turret_angle_deg,
            turret_pitch_deg: object.turret_pitch_deg,
            turret_idle_scan_next_frame: object.turret_idle_scan_next_frame,
            turret_idle_scanning: object.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: object.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: object.turret_idle_scan_index,
            turret_holding: object.turret_holding,
            turret_hold_until_frame: object.turret_hold_until_frame,
            turret_idle_recentering: object.turret_idle_recentering,
            turret_mood_target: object.turret_mood_target,
            turret_target_id: object.turret_target_id.map(|id| id.0),
            turret_force_attacking: object.turret_force_attacking,
            turret_enabled: object.turret_enabled,
            turret_substate: object.turret_substate.ordinal(),
            turret_rotating: object.turret_rotating,
        });
    }
    TurretAimPersistPayload { objects }
}

fn object_has_turret_residual(object: &crate::game_logic::Object) -> bool {
    object.turret_target_id.is_some()
        || object.turret_substate != TurretSubState::Idle
        || object.turret_rotating
        || object.turret_holding
        || object.turret_idle_scanning
        || object.turret_mood_target
        || object.turret_force_attacking
        || object.turret_idle_recentering
        || object.turret_hold_until_frame != 0
        || object.turret_idle_scan_next_frame != 0
        || (object.turret_angle_deg - object.turret_natural_angle_deg).abs() > 1e-3
        || (object.turret_pitch_deg - object.turret_natural_pitch_deg).abs() > 1e-3
}

fn reset_turret_aim(game_logic: &mut GameLogic) {
    let ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    for id in ids {
        let Some(object) = game_logic.host_object_mut(id) else {
            continue;
        };
        // Keep constructed natural angle/pitch/enabled so old saves without a
        // TRAI suffix do not snap every turret to 0°.
        object.turret_idle_scan_next_frame = 0;
        object.turret_idle_scanning = false;
        object.turret_idle_scan_desired_angle_deg = 0.0;
        object.turret_idle_scan_index = 0;
        object.turret_holding = false;
        object.turret_hold_until_frame = 0;
        object.turret_idle_recentering = false;
        object.turret_mood_target = false;
        object.turret_target_id = None;
        object.turret_force_attacking = false;
        object.turret_substate = TurretSubState::Idle;
        object.turret_rotating = false;
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: TurretAimPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.turret_angle_deg = entry.turret_angle_deg;
        object.turret_pitch_deg = entry.turret_pitch_deg;
        object.turret_idle_scan_next_frame = entry.turret_idle_scan_next_frame;
        object.turret_idle_scanning = entry.turret_idle_scanning;
        object.turret_idle_scan_desired_angle_deg = entry.turret_idle_scan_desired_angle_deg;
        object.turret_idle_scan_index = entry.turret_idle_scan_index;
        object.turret_holding = entry.turret_holding;
        object.turret_hold_until_frame = entry.turret_hold_until_frame;
        object.turret_idle_recentering = entry.turret_idle_recentering;
        object.turret_mood_target = entry.turret_mood_target;
        object.turret_target_id = entry.turret_target_id.map(ObjectId);
        object.turret_force_attacking = entry.turret_force_attacking;
        object.turret_enabled = entry.turret_enabled;
        object.turret_substate = TurretSubState::from_ordinal(entry.turret_substate);
        object.turret_rotating = entry.turret_rotating;
    }
}

fn find_trai_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == TRAI_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("TRAI u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::object::TurretSubState;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_turret_aim() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaTankCrusader".to_string(),
            ThingTemplate::new("AmericaTankCrusader"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let id = source
            .create_object("AmericaTankCrusader", Team::USA, Vec3::new(12.0, 0.0, 8.0))
            .expect("tank");
        {
            let object = source.host_object_mut(id).expect("tank obj");
            object.turret_enabled = true;
            object.turret_angle_deg = 45.0;
            object.turret_pitch_deg = 8.0;
            object.turret_substate = TurretSubState::Aim;
            object.turret_target_id = Some(ObjectId(9));
            object.turret_holding = true;
            object.turret_hold_until_frame = 240;
            object.turret_rotating = true;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_trai_suffix(&snapshot.lifecycle_tail).is_some(),
            "TRAI suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(id).expect("restored tank");
        assert!((loaded.turret_angle_deg - 45.0).abs() < 1e-4);
        assert!((loaded.turret_pitch_deg - 8.0).abs() < 1e-4);
        assert_eq!(loaded.turret_substate, TurretSubState::Aim);
        assert_eq!(loaded.turret_target_id, Some(ObjectId(9)));
        assert!(loaded.turret_holding);
        assert_eq!(loaded.turret_hold_until_frame, 240);
        assert!(loaded.turret_rotating);
        assert!(loaded.turret_enabled);
    }

    #[test]
    fn absent_suffix_clears_stale_turret_aim() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaTankCrusader".to_string(),
            ThingTemplate::new("AmericaTankCrusader"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let id = logic
            .create_object("AmericaTankCrusader", Team::USA, Vec3::ZERO)
            .expect("tank");
        {
            let object = logic.host_object_mut(id).expect("tank");
            object.turret_substate = TurretSubState::Aim;
            object.turret_target_id = Some(ObjectId(9));
            object.turret_holding = true;
            object.turret_hold_until_frame = 99;
        }
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let object = logic.host_object(id).expect("tank");
        assert_eq!(object.turret_substate, TurretSubState::Idle);
        assert!(object.turret_target_id.is_none());
        assert!(!object.turret_holding);
        assert_eq!(object.turret_hold_until_frame, 0);
    }
}
