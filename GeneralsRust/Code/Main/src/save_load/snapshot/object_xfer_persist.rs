//! Persist leftover Object / Weapon / TurretAI / Stealth / OpenContain clocks.
//!
//! C++ `Object::xfer` writes vision/shroud ranges, DISABLED_HELD,
//! `m_singleUseCommandUsed`, `m_indicatorColor`, `m_weaponBonusCondition`,
//! and `m_isReceivingDifficultyBonus`. `Weapon::xfer` writes
//! `m_whenPreAttackFinished` + `m_maxShotCount`. `TurretAI::xfer` v2 writes
//! angle/pitch/target/hold/enabled/state. `StealthUpdate::xfer` v2 writes
//! `m_framesGranted`. `OpenContain::xfer` v2 writes `m_whichExitPath`.
//! Leftover xfer already matches those tables. Live stores the same residual
//! on host `Object` but ObjectSnapshot never wrote it — load dropped
//! script-held units, spent SingleUse buttons, painted radar colors, hijack
//! sight, Burton knife wind-up, turret aim, supply-dock stealth, and exit
//! door cycling.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes clocks/flags only; it never re-runs create/apply.

use crate::game_logic::object::TurretSubState;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const OXOB_MAGIC: &[u8; 4] = b"OXOB";
const OXOB_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayload {
    objects: Vec<ObjectXferPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectXferPersist {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
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
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(OXOB_MAGIC);
    append_u32(bytes, OXOB_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(
    bytes: &[u8],
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    reset_object_xfer(game_logic);
    let Some(suffix) = find_oxob_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != OXOB_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown OXOB suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "OXOB payload truncated".to_string(),
        ));
    }
    let payload: ObjectXferPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ObjectXferPersistPayload {
    // C++ Object::xfer always writes vision/shroud. Restore constructs via
    // new_with_logic_frame, which zeros those ranges, so every host object
    // must be captured — residual-only would drop hijack/S&D/shroud overrides.
    let mut objects = Vec::new();
    for (id, object) in game_logic.host_objects() {
        objects.push(ObjectXferPersist {
            object_id: id.0,
            disabled_held: object.status.disabled_held,
            single_use_command_used: object.single_use_command_used,
            ai_attitude: object.ai_attitude,
            custom_indicator_color: object.custom_indicator_color,
            vision_range: object.vision_range,
            shroud_clearing_range: object.shroud_clearing_range,
            shroud_range: object.shroud_range,
            pre_attack_ready_at: object.pre_attack_ready_at,
            pre_attack_target: object.pre_attack_target.map(|id| id.0),
            consecutive_shot_target: object.consecutive_shot_target.map(|id| id.0),
            max_shots_to_fire: object.max_shots_to_fire,
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
            temporary_stealth_expires_frame: object.temporary_stealth_expires_frame,
            weapon_bonus_solo: object.weapon_bonus_solo,
            which_exit_path: object.which_exit_path,
        });
    }
    ObjectXferPersistPayload { objects }
}

fn reset_object_xfer(game_logic: &mut GameLogic) {
    let ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    for id in ids {
        let Some(object) = game_logic.host_object_mut(id) else {
            continue;
        };
        object.status.disabled_held = false;
        object.single_use_command_used = false;
        object.ai_attitude = 0;
        object.custom_indicator_color = None;
        object.pre_attack_ready_at = 0.0;
        object.pre_attack_target = None;
        object.consecutive_shot_target = None;
        object.max_shots_to_fire = -1;
        object.turret_idle_scan_next_frame = 0;
        object.turret_idle_scanning = false;
        object.turret_holding = false;
        object.turret_hold_until_frame = 0;
        object.turret_idle_recentering = false;
        object.turret_mood_target = false;
        object.turret_target_id = None;
        object.turret_force_attacking = false;
        object.turret_substate = TurretSubState::Idle;
        object.turret_rotating = false;
        object.temporary_stealth_expires_frame = 0;
        object.weapon_bonus_solo = 0;
        object.is_receiving_difficulty_bonus = false;
        object.which_exit_path = 0;
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: ObjectXferPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.status.disabled_held = entry.disabled_held;
        object.single_use_command_used = entry.single_use_command_used;
        object.ai_attitude = entry.ai_attitude;
        object.custom_indicator_color = entry.custom_indicator_color;
        object.vision_range = entry.vision_range;
        object.shroud_clearing_range = entry.shroud_clearing_range;
        object.shroud_range = entry.shroud_range;
        object.pre_attack_ready_at = entry.pre_attack_ready_at;
        object.pre_attack_target = entry.pre_attack_target.map(ObjectId);
        object.consecutive_shot_target = entry.consecutive_shot_target.map(ObjectId);
        object.max_shots_to_fire = entry.max_shots_to_fire;
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
        object.temporary_stealth_expires_frame = entry.temporary_stealth_expires_frame;
        object.weapon_bonus_solo = entry.weapon_bonus_solo;
        object.is_receiving_difficulty_bonus = entry.weapon_bonus_solo != 0;
        object.which_exit_path = entry.which_exit_path;
    }
}

fn find_oxob_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == OXOB_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("OXOB u32 truncated".to_string()));
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
    fn snapshot_round_trips_object_xfer_clocks() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let id = source
            .create_object(
                "AmericaInfantryRanger",
                Team::USA,
                Vec3::new(10.0, 0.0, 8.0),
            )
            .expect("unit");
        {
            let object = source.host_object_mut(id).expect("unit obj");
            object.status.disabled_held = true;
            object.single_use_command_used = true;
            object.ai_attitude = -1;
            object.custom_indicator_color = Some(0xFFFF_0000);
            object.vision_range = 220.0;
            object.shroud_clearing_range = 240.0;
            object.shroud_range = 80.0;
            object.pre_attack_ready_at = 12.5;
            object.pre_attack_target = Some(ObjectId(9));
            object.consecutive_shot_target = Some(ObjectId(9));
            object.max_shots_to_fire = 3;
            object.turret_angle_deg = 45.0;
            object.turret_substate = TurretSubState::Aim;
            object.turret_target_id = Some(ObjectId(9));
            object.temporary_stealth_expires_frame = 600;
            object.weapon_bonus_solo = 16;
            object.which_exit_path = 2;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_oxob_suffix(&snapshot.lifecycle_tail).is_some(),
            "OXOB suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(id).expect("restored unit");
        assert!(loaded.status.disabled_held);
        assert!(loaded.single_use_command_used);
        assert_eq!(loaded.ai_attitude, -1);
        assert_eq!(loaded.custom_indicator_color, Some(0xFFFF_0000));
        assert_eq!(loaded.vision_range, 220.0);
        assert_eq!(loaded.shroud_clearing_range, 240.0);
        assert_eq!(loaded.shroud_range, 80.0);
        assert!((loaded.pre_attack_ready_at - 12.5).abs() < 1e-4);
        assert_eq!(loaded.pre_attack_target, Some(ObjectId(9)));
        assert_eq!(loaded.consecutive_shot_target, Some(ObjectId(9)));
        assert_eq!(loaded.max_shots_to_fire, 3);
        assert!((loaded.turret_angle_deg - 45.0).abs() < 1e-4);
        assert_eq!(loaded.turret_substate, TurretSubState::Aim);
        assert_eq!(loaded.turret_target_id, Some(ObjectId(9)));
        assert_eq!(loaded.temporary_stealth_expires_frame, 600);
        assert_eq!(loaded.weapon_bonus_solo, 16);
        assert!(
            loaded.is_receiving_difficulty_bonus,
            "m_isReceivingDifficultyBonus latch follows restored SOLO bits"
        );
        assert_eq!(loaded.which_exit_path, 2);
    }

    #[test]
    fn absent_suffix_clears_stale_object_xfer() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let id = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = logic.host_object_mut(id).expect("unit");
            object.status.disabled_held = true;
            object.single_use_command_used = true;
            object.custom_indicator_color = Some(0xFF00_FF00);
            object.pre_attack_ready_at = 4.0;
            object.pre_attack_target = Some(ObjectId(2));
            object.consecutive_shot_target = Some(ObjectId(2));
            object.max_shots_to_fire = 1;
            object.weapon_bonus_solo = 18;
            object.which_exit_path = 3;
        }
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let object = logic.host_object(id).expect("unit");
        assert!(!object.status.disabled_held);
        assert!(!object.single_use_command_used);
        assert!(!object.is_receiving_difficulty_bonus);
        assert_eq!(object.custom_indicator_color, None);
        assert_eq!(object.pre_attack_ready_at, 0.0);
        assert_eq!(object.pre_attack_target, None);
        assert_eq!(object.consecutive_shot_target, None);
        assert_eq!(object.max_shots_to_fire, -1);
        assert_eq!(object.weapon_bonus_solo, 0);
        assert_eq!(object.which_exit_path, 0);
    }

    #[test]
    fn oxob_round_trips_indicator_vision_single_use_preattack() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let id = source
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = source.host_object_mut(id).expect("unit");
            object.single_use_command_used = true;
            object.custom_indicator_color = Some(0xFFFF_0000);
            object.vision_range = 220.0;
            object.shroud_clearing_range = 240.0;
            object.shroud_range = 80.0;
            object.pre_attack_ready_at = 12.5;
            object.pre_attack_target = Some(ObjectId(9));
            object.consecutive_shot_target = Some(ObjectId(9));
            object.max_shots_to_fire = 3;
        }

        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        assert!(
            find_oxob_suffix(&bytes).is_some(),
            "OXOB suffix must encode indicator/vision/single-use/pre-attack"
        );

        let mut dest = GameLogic::new();
        dest.templates = source.templates.clone();
        dest.add_player(Player::new(0, Team::USA, "USA", true));
        let dest_id = dest
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("dest unit");
        assert_eq!(dest_id, id);
        apply_from_lifecycle_tail(&bytes, &mut dest).expect("apply");

        let loaded = dest.host_object(dest_id).expect("loaded");
        assert!(loaded.single_use_command_used);
        assert_eq!(loaded.custom_indicator_color, Some(0xFFFF_0000));
        assert_eq!(loaded.vision_range, 220.0);
        assert_eq!(loaded.shroud_clearing_range, 240.0);
        assert_eq!(loaded.shroud_range, 80.0);
        assert!((loaded.pre_attack_ready_at - 12.5).abs() < 1e-4);
        assert_eq!(loaded.pre_attack_target, Some(ObjectId(9)));
        assert_eq!(loaded.consecutive_shot_target, Some(ObjectId(9)));
        assert_eq!(loaded.max_shots_to_fire, 3);
    }
}
