//! Persist leftover Object / Weapon / TurretAI / Stealth / OpenContain / AIUpdate clocks.
//!
//! C++ `Object::xfer` writes vision/shroud ranges, DISABLED_HELD,
//! `m_singleUseCommandUsed`, `m_indicatorColor`, `m_weaponBonusCondition`,
//! `m_smcUntil`, `m_isReceivingDifficultyBonus`, v9 `m_safeOcclusionFrame`,
//! and `m_healthBoxOffset`. `Weapon::xfer` writes `m_whenPreAttackFinished`,
//! `m_maxShotCount`, `m_scatterTargetsUnused`, and `m_lastFireFrame`.
//! `TurretAI::xfer` v2 writes angle/pitch/target/hold/enabled/state.
//! `StealthUpdate::xfer` v2 writes `m_framesGranted`. `OpenContain::xfer` v2
//! writes `m_whichExitPath` and `m_passengerAllowedToFire`. `AIUpdateInterface::xfer` writes
//! `m_isRecruitable`, `m_nextEnemyScanTime`, `m_ignoreCollisionsUntil`,
//! `m_ignoreObstacleID`, `m_finalPosition`/`m_doFinalPosition`,
//! `m_canPathThroughUnits`, and `m_lastCommandSource`. Leftover xfer
//! already matches those tables. Live stores the same residual on host
//! `Object` / `GameLogic` scan maps but ObjectSnapshot never wrote
//! cheer/`SPECIAL_CHEERING`, mid-clip scatter unused, HORDE/ENTHUSIASTIC/
//! SUBLIMINAL bits, `safe_occlusion_frame`, `health_box_offset`,
//! `last_fire_frame`, per-object `is_recruitable`, guard/hunt scan clocks,
//! `ignore_collisions_until_frame`, `do_final_position`/`final_position`,
//! `ignored_obstacle_id`, `can_path_through_units`, or `last_command_source`
//! — load snapped the Angry Mob bar to the nexus, recloaked a frame early,
//! unlocked script-locked recruits, re-scanned every guard/hunt unit,
//! re-blocked mid-bump movers, left mid-settle units short of the leftover
//! cell, collided enter/dozer units with the building they should ignore,
//! wedged mid-exit units in the factory hull, and re-armed CommandButtonHunt
//! after a player/script cancel.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! in a versioned payload. World version 39 requires the permission records.
//! Restore writes clocks/flags only; it never re-runs create/apply.

use crate::game_logic::HUNT_CMD_FROM_AI;
use crate::game_logic::object::TurretSubState;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const OXOB_MAGIC: &[u8; 4] = b"OXOB";
const OXOB_VERSION: u32 = 24;

mod legacy_versions;
use legacy_versions::*;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayload {
    objects: Vec<ObjectXferPersist>,
    script_status: Vec<ObjectScriptStatusPersist>,
    structures: Vec<StructureTopplePersist>,
    damage_info: Vec<ObjectDamageInfoPersist>,
    transport: Vec<TransportMovementPersist>,
    body_history: Vec<body_damage_history_persist::BodyDamageHistoryPersist>,
    open_contain_rallies: Vec<OpenContainRallyPersist>,
    passenger_fire_permissions: Vec<(u32, bool)>,
    weapon_activity: Vec<(u32, bool, bool)>,
    target_positions: Vec<(u32, bool, [f32; 3])>,
    body_fx_throttles: Vec<(u32, u32, Option<crate::game_logic::combat::DamageType>)>,
    /// Owners whose ActiveBody last-damage record has been cleared for retaliation.
    cleared_last_attackers: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectXferPersistPayloadV24 {
    prior: ObjectXferPersistPayload,
    morale: Vec<(u32, bool, bool)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenContainRallyPersist {
    object_id: u32,
    point: [f32; 3],
}

#[path = "body_damage_history_persist.rs"]
mod body_damage_history_persist;

#[path = "transport_movement_persist.rs"]
mod transport_movement_persist;
use transport_movement_persist::TransportMovementPersist;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectDamageInfoPersist {
    object_id: u32,
    last_damage_type: Option<crate::game_logic::combat::DamageType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StructureTopplePersist {
    object_id: u32,
    matrix: [f32; 16],
    runtime: crate::game_logic::host_structure_topple::HostStructureToppleData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectScriptStatusPersist {
    object_id: u32,
    script_targetable: bool,
    masked: bool,
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
    // Retired absolute deadline slot. STLG owns the StealthUpdate countdown.
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
    safe_occlusion_frame: u32,
    health_box_offset: [f32; 3],
    last_fire_frame: u32,
    is_recruitable: bool,
    guard_next_enemy_scan: Option<u32>,
    hunt_next_enemy_scan: Option<u32>,
    ignore_collisions_until_frame: u32,
    do_final_position: bool,
    final_position: [f32; 3],
    ignored_obstacle_id: Option<u32>,
    can_path_through_units: bool,
    last_command_source: u32,
    /// C++ `m_upgradedLocomotors`.
    locomotor_upgrade: bool,
    /// C++ `m_curLocomotorSet` token (`SET_NORMAL`, `SET_TAXIING`, ...).
    cur_locomotor_set: Option<String>,
    /// False on upgrades from versions that did not store Locomotor::xfer scalars.
    has_locomotor_motion: bool,
    /// Main's effective braking in host sec²; preserve explicit saved values.
    braking: f32,
    /// C++ `Locomotor::m_maxLift`.
    max_lift: f32,
    /// C++ `Locomotor::m_brakingFactor`.
    braking_factor: f32,
    /// False on saves that did not store close-enough distance or preferred height.
    has_locomotor_arrival: bool,
    /// C++ `Locomotor::m_closeEnoughDist` (None = leave the create-time value).
    close_enough_dist: Option<f32>,
    /// C++ close-enough-is-3D flag.
    close_enough_dist_3d: bool,
    /// C++ `Locomotor::m_donutTimer`.
    donut_timer: u32,
    /// C++ `Locomotor::m_preferredHeight`.
    preferred_height: f32,
    /// C++ `Locomotor::m_preferredHeightDamping`.
    preferred_height_damping: f32,
    /// False on saves that did not store `m_flags`, maintain position, or wander offset.
    has_locomotor_pose: bool,
    /// C++ `Locomotor::m_flags` (`LocoFlag` bit order).
    loco_flags: u32,
    /// C++ `Locomotor::m_maintainPos` (None = invalid / unset).
    maintain_pos: Option<[f32; 3]>,
    /// C++ `Locomotor::m_angleOffset`.
    wander_angle_offset: f32,
    /// C++ `Locomotor::m_offsetIncrement`.
    wander_offset_increment: f32,
    /// False on saves that did not store the active locomotor template name.
    has_locomotor_template: bool,
    /// C++ `Locomotor::getTemplateName`, reloaded by `LocomotorSet::xfer`.
    cur_locomotor_name: Option<String>,
    /// C++ `LocomotorSet::m_validLocomotorSurfaces`.
    valid_locomotor_surfaces: u32,
    /// C++ `LocomotorSet::m_downhillOnly`.
    downhill_only: bool,
    /// C++ doQuickExit temporary-state frame. Absent before OXOB version 11.
    #[serde(default)]
    quick_exit_until: Option<u32>,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let prior = capture(game_logic);
    if prior.objects.is_empty() {
        return;
    }
    let morale = game_logic.host_objects().registered()
        .map(|(id, object)| (id.0, object.weapon_bonus_nationalism, object.weapon_bonus_fanaticism))
        .collect();
    let payload = ObjectXferPersistPayloadV24 { prior, morale };
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(OXOB_MAGIC);
    append_u32(bytes, OXOB_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

fn decode_current_payload(version: u32, bytes: &[u8]) -> SaveLoadResult<ObjectXferPersistPayload> {
    if version == 18 {
        bincode_legacy::deserialize::<ObjectXferPersistPayloadV18>(bytes)
            .map(Into::into)
            .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))
    } else if version == 19 {
        bincode_legacy::deserialize::<ObjectXferPersistPayloadV19>(bytes)
            .map(Into::into)
            .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))
    } else if version == 20 {
        bincode_legacy::deserialize::<ObjectXferPersistPayloadV20>(bytes)
            .map(Into::into)
            .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))
    } else if version == 21 {
        bincode_legacy::deserialize::<ObjectXferPersistPayloadV21>(bytes)
            .map(Into::into)
            .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))
    } else if version == 22 {
        bincode_legacy::deserialize::<ObjectXferPersistPayloadV22>(bytes)
            .map(Into::into)
            .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))
    } else if version == 23 {
        bincode_legacy::deserialize(bytes)
            .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))
    } else if version == OXOB_VERSION {
        bincode_legacy::deserialize::<ObjectXferPersistPayloadV24>(bytes)
            .map(|payload| payload.prior)
            .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))
    } else {
        Err(SaveLoadError::Corrupted(format!(
            "unsupported OXOB projection version {version}"
        )))
    }
}

/// Validate owned object continuations before replacing the receiving session.
pub(super) fn validate_owned_object_continuations_before_restore(
    bytes: &[u8],
    snapshot: &super::WorldSnapshot,
) -> SaveLoadResult<()> {
    if snapshot.objects.is_empty() {
        return Ok(());
    }
    let Some(mut rest) = find_oxob_suffix(bytes) else {
        return if snapshot.version < 39 {
            Ok(())
        } else {
            Err(SaveLoadError::Corrupted(
                "missing passenger-fire continuation".into(),
            ))
        };
    };
    let version = take_u32(&mut rest)?;
    if version < 19 && snapshot.version < 39 {
        return Ok(());
    }
    if version != 19 && version != 20 && version != 21 && version != 22 && version != 23 && version != OXOB_VERSION {
        return Err(SaveLoadError::Corrupted(
            "missing current passenger-fire continuation".into(),
        ));
    }
    let length = take_u32(&mut rest)? as usize;
    let encoded = rest
        .get(..length)
        .ok_or_else(|| SaveLoadError::Corrupted("truncated passenger-fire continuation".into()))?;
    if version == OXOB_VERSION {
        let saved: ObjectXferPersistPayloadV24 = bincode_legacy::deserialize(encoded)
            .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))?;
        let mut ids = std::collections::HashSet::new();
        for (id, _, _) in &saved.morale {
            if !ids.insert(*id) || !snapshot.objects.contains_key(&ObjectId(*id)) {
                return Err(SaveLoadError::Corrupted("invalid morale continuation owner".into()));
            }
        }
        if ids.len() != snapshot.objects.len() {
            return Err(SaveLoadError::Corrupted("incomplete morale continuation owners".into()));
        }
    }
    let payload = decode_current_payload(version, encoded)?;
    let mut ids = std::collections::HashSet::new();
    for (id, _) in payload.passenger_fire_permissions {
        if !ids.insert(id) || !snapshot.objects.contains_key(&ObjectId(id)) {
            return Err(SaveLoadError::Corrupted(
                "invalid passenger-fire continuation owner".into(),
            ));
        }
    }
    if ids.len() != snapshot.objects.len() {
        return Err(SaveLoadError::Corrupted(
            "incomplete passenger-fire continuation".into(),
        ));
    }
    if version >= 20 {
        let mut activity_ids = std::collections::HashSet::new();
        for (id, _, _) in &payload.weapon_activity {
            if !activity_ids.insert(*id) || !snapshot.objects.contains_key(&ObjectId(*id)) {
                return Err(SaveLoadError::Corrupted(
                    "invalid weapon-activity continuation owner".into(),
                ));
            }
        }
        if activity_ids.len() != snapshot.objects.len() {
            return Err(SaveLoadError::Corrupted(
                "incomplete weapon-activity continuation".into(),
            ));
        }
    }
    if version >= 21 {
        let mut target_ids = std::collections::HashSet::new();
        for (id, _, position) in &payload.target_positions {
            if !target_ids.insert(*id)
                || !snapshot.objects.contains_key(&ObjectId(*id))
                || position.iter().any(|value| !value.is_finite())
            {
                return Err(SaveLoadError::Corrupted(
                    "invalid target-position continuation".into(),
                ));
            }
        }
        if target_ids.len() != snapshot.objects.len() {
            return Err(SaveLoadError::Corrupted(
                "incomplete target-position continuation".into(),
            ));
        }
    }
    if version >= 22 {
        let mut owners = std::collections::HashSet::new();
        for (id, _, _) in &payload.body_fx_throttles {
            if !owners.insert(*id) || !snapshot.objects.contains_key(&ObjectId(*id)) {
                return Err(SaveLoadError::Corrupted(
                    "invalid body FX throttle owner".into(),
                ));
            }
        }
        if owners.len() != snapshot.objects.len() {
            return Err(SaveLoadError::Corrupted(
                "incomplete body FX throttle owners".into(),
            ));
        }
    }
    let mut cleared = std::collections::HashSet::new();
    for id in &payload.cleared_last_attackers {
        if !cleared.insert(*id) || !snapshot.objects.contains_key(&ObjectId(*id)) {
            return Err(SaveLoadError::Corrupted(
                "invalid cleared last-attacker owner".into(),
            ));
        }
    }
    Ok(())
}

/// Inspect the historical XP-only receipt before any receiving-world mutation.
pub(super) fn saved_kill_experience_receipts(
    bytes: &[u8],
) -> SaveLoadResult<std::collections::HashSet<ObjectId>> {
    let Some(mut rest) = find_oxob_suffix(bytes) else {
        return Ok(Default::default());
    };
    let version = take_u32(&mut rest)?;
    if version < 17 {
        return Ok(Default::default());
    }
    if version > OXOB_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown OXOB suffix version {version}"
        )));
    }
    let length = take_u32(&mut rest)? as usize;
    let bytes = rest
        .get(..length)
        .ok_or_else(|| SaveLoadError::Corrupted("OXOB payload truncated".into()))?;
    let body_history = if version == 17 {
        bincode_legacy::deserialize::<ObjectXferPersistPayloadV17>(bytes)
            .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))?
            .body_history
    } else {
        decode_current_payload(version, bytes)?.body_history
    };
    Ok(body_history
        .into_iter()
        .filter(|record| record.kill_experience_awarded)
        .map(|record| ObjectId(record.object_id))
        .collect())
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_oxob_suffix(bytes) else {
        reset_object_xfer(game_logic);
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != 1
        && version != 2
        && version != 3
        && version != 4
        && version != 5
        && version != 6
        && version != 7
        && version != 8
        && version != 9
        && version != 10
        && version != 11
        && version != 12
        && version != 13
        && version != 14
        && version != 15
        && version != 16
        && version != 17
        && version != 18
        && version != 19
        && version != 20
        && version != 21
        && version != 22
        && version != 23
        && version != OXOB_VERSION
    {
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
    let encoded = &rest[..payload_len];
    let morale = if version == OXOB_VERSION {
        let saved: ObjectXferPersistPayloadV24 = bincode_legacy::deserialize(encoded)
            .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))?;
        let mut ids = std::collections::HashSet::new();
        for (id, _, _) in &saved.morale {
            if !ids.insert(*id) || game_logic.host_object(ObjectId(*id)).is_none() {
                return Err(SaveLoadError::Corrupted("invalid saved morale owner".into()));
            }
        }
        if ids.len() != game_logic.host_objects().registered_len() {
            return Err(SaveLoadError::Corrupted("incomplete saved morale owners".into()));
        }
        saved.morale
    } else {
        Vec::new()
    };
    let payload = if version == 1 {
        let old: ObjectXferPersistPayloadV1 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 2 {
        let old: ObjectXferPersistPayloadV2 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 3 {
        let old: ObjectXferPersistPayloadV3 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 4 {
        let old: ObjectXferPersistPayloadV4 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 5 {
        let old: ObjectXferPersistPayloadV5 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 6 {
        let old: ObjectXferPersistPayloadV6 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 7 {
        let old: ObjectXferPersistPayloadV7 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 8 {
        let old: ObjectXferPersistPayloadV8 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 9 {
        let old: ObjectXferPersistPayloadV9 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 10 {
        let old: ObjectXferPersistPayloadV10 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 11 {
        let old: ObjectXferPersistPayloadV11 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 12 {
        let old: ObjectXferPersistPayloadV12 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 13 {
        let old: ObjectXferPersistPayloadV13 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: old.structures,
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 14 {
        let old: ObjectXferPersistPayloadV14 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: old.structures,
            damage_info: old.damage_info,
            transport: Vec::new(),
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 15 {
        let old: ObjectXferPersistPayloadV15 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: old.structures,
            damage_info: old.damage_info,
            transport: old.transport,
            body_history: Vec::new(),
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 16 {
        let old: ObjectXferPersistPayloadV16 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: old.structures,
            damage_info: old.damage_info,
            transport: old.transport,
            body_history: body_damage_history_persist::migrate_v16(game_logic, old.body_history)?,
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else if version == 17 {
        let old: ObjectXferPersistPayloadV17 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: old.structures,
            damage_info: old.damage_info,
            transport: old.transport,
            body_history: old.body_history,
            open_contain_rallies: Vec::new(),
            passenger_fire_permissions: Vec::new(),
            weapon_activity: Vec::new(),
            target_positions: Vec::new(),
            body_fx_throttles: Vec::new(),
            cleared_last_attackers: Vec::new(),
        }
    } else {
        decode_current_payload(version, encoded)?
    };
    body_damage_history_persist::validate(game_logic, &payload.body_history)?;
    let mut cleared = std::collections::HashSet::new();
    for id in &payload.cleared_last_attackers {
        if !cleared.insert(*id) || game_logic.host_object(ObjectId(*id)).is_none() {
            return Err(SaveLoadError::Corrupted(
                "invalid saved cleared last-attacker owner".into(),
            ));
        }
    }
    validate_open_contain_rallies(game_logic, &payload.open_contain_rallies)?;
    if version >= 19 {
        let mut ids = std::collections::HashSet::new();
        for (id, _) in &payload.passenger_fire_permissions {
            if !ids.insert(*id) || game_logic.host_object(ObjectId(*id)).is_none() {
                return Err(SaveLoadError::Corrupted(
                    "invalid saved passenger-fire permission owner".into(),
                ));
            }
        }
        if ids.len() != game_logic.host_objects().registered_len() {
            return Err(SaveLoadError::Corrupted(
                "incomplete saved passenger-fire permission owners".into(),
            ));
        }
    }
    if version >= 20 {
        let mut ids = std::collections::HashSet::new();
        for (id, _, _) in &payload.weapon_activity {
            if !ids.insert(*id) || game_logic.host_object(ObjectId(*id)).is_none() {
                return Err(SaveLoadError::Corrupted(
                    "invalid saved weapon-activity owner".into(),
                ));
            }
        }
        if ids.len() != game_logic.host_objects().registered_len() {
            return Err(SaveLoadError::Corrupted(
                "incomplete saved weapon-activity owners".into(),
            ));
        }
    }
    if version >= 21 {
        let mut ids = std::collections::HashSet::new();
        for (id, _, position) in &payload.target_positions {
            if !ids.insert(*id)
                || game_logic.host_object(ObjectId(*id)).is_none()
                || position.iter().any(|value| !value.is_finite())
            {
                return Err(SaveLoadError::Corrupted(
                    "invalid saved target-position owner or coordinates".into(),
                ));
            }
        }
        if ids.len() != game_logic.host_objects().registered_len() {
            return Err(SaveLoadError::Corrupted(
                "incomplete saved target-position owners".into(),
            ));
        }
    }
    if version >= 22 {
        let mut owners = std::collections::HashSet::new();
        for (id, _, _) in &payload.body_fx_throttles {
            if !owners.insert(*id) || game_logic.host_object(ObjectId(*id)).is_none() {
                return Err(SaveLoadError::Corrupted(
                    "invalid saved body FX throttle owner".into(),
                ));
            }
        }
        if owners.len() != game_logic.host_objects().registered_len() {
            return Err(SaveLoadError::Corrupted(
                "incomplete saved body FX throttle owners".into(),
            ));
        }
    }
    if version >= 16 && payload.body_history.len() != game_logic.host_objects().registered_len() {
        return Err(SaveLoadError::Corrupted(
            "incomplete saved body history owners".into(),
        ));
    }
    transport_movement_persist::validate(game_logic, &payload.transport)?;
    if version >= 15 && payload.transport.len() != game_logic.host_objects().registered_len() {
        return Err(SaveLoadError::Corrupted(
            "incomplete saved transport movement owners".into(),
        ));
    }
    validate_structure_records(game_logic, &payload.structures)?;
    let mut damage_ids = std::collections::HashSet::new();
    for entry in &payload.damage_info {
        if !damage_ids.insert(entry.object_id)
            || game_logic.host_object(ObjectId(entry.object_id)).is_none()
        {
            return Err(SaveLoadError::Corrupted(
                "invalid saved body damage-info owner".into(),
            ));
        }
    }
    if version >= 14 && damage_ids.len() != game_logic.host_objects().registered_len() {
        return Err(SaveLoadError::Corrupted(
            "incomplete saved body damage-info owners".into(),
        ));
    }
    if game_logic.locomotor_definitions.is_some() {
        for entry in &payload.objects {
            if entry.has_locomotor_template {
                if let Some(name) = entry.cur_locomotor_name.as_deref() {
                    if game_logic.resolve_receiving_locomotor(name).is_none() {
                        return Err(SaveLoadError::Corrupted(format!(
                            "saved object {} references an unavailable receiving locomotor {name}",
                            entry.object_id
                        )));
                    }
                }
            }
        }
    }
    // Decode the complete suffix before changing the receiving objects. A bad
    // record must not reset their existing clocks or script state.
    reset_object_xfer(game_logic);
    apply_payload(game_logic, payload);
    for (id, nationalism, fanaticism) in morale {
        let object = game_logic.host_object_mut(ObjectId(id)).expect("validated morale owner");
        object.weapon_bonus_nationalism = nationalism;
        object.weapon_bonus_fanaticism = fanaticism;
    }
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ObjectXferPersistPayload {
    // C++ Object::xfer always writes vision/shroud. Restore constructs via
    // new_with_logic_frame, which zeros those ranges, so every host object
    // must be captured — residual-only would drop hijack/S&D/shroud overrides.
    let mut objects = Vec::new();
    for (id, object) in game_logic.host_objects().registered() {
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
            turret_angle_deg: object.turret_ai.turret_angle_deg,
            turret_pitch_deg: object.turret_ai.turret_pitch_deg,
            turret_idle_scan_next_frame: object.turret_ai.turret_idle_scan_next_frame,
            turret_idle_scanning: object.turret_ai.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: object.turret_ai.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: object.turret_ai.turret_idle_scan_index,
            turret_holding: object.turret_ai.turret_holding,
            turret_hold_until_frame: object.turret_ai.turret_hold_until_frame,
            turret_idle_recentering: object.turret_ai.turret_idle_recentering,
            turret_mood_target: object.turret_ai.turret_mood_target,
            turret_target_id: object.turret_ai.turret_target_id.map(|id| id.0),
            turret_force_attacking: object.turret_ai.turret_force_attacking,
            turret_enabled: object.turret_ai.turret_enabled,
            turret_substate: object.turret_ai.turret_substate.ordinal(),
            turret_rotating: object.turret_ai.turret_rotating,
            temporary_stealth_expires_frame: 0,
            weapon_bonus_solo: object.weapon_bonus_solo,
            which_exit_path: object.which_exit_path,
            cheer_timer: object.cheer_timer,
            special_cheering: object_has_special_cheering(object),
            weapon_scatter_targets_unused: object.weapon_scatter_targets_unused.clone(),
            weapon_scatter_targets_inited: object.weapon_scatter_targets_inited,
            weapon_bonus_horde: object.weapon_bonus_horde,
            weapon_bonus_enthusiastic: object.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: object.weapon_bonus_subliminal,
            safe_occlusion_frame: object.safe_occlusion_frame,
            health_box_offset: object.health_box_offset,
            last_fire_frame: object.last_fire_frame,
            is_recruitable: object.is_recruitable,
            guard_next_enemy_scan: object.unit_ai_runtime.guard_scan_deadline(),
            quick_exit_until: object.unit_ai_runtime.quick_exit_deadline(),
            hunt_next_enemy_scan: object.unit_ai_runtime.hunt_scan_deadline(),
            ignore_collisions_until_frame: object.ignore_collisions_until_frame,
            do_final_position: object.do_final_position,
            final_position: object.final_position.to_array(),
            ignored_obstacle_id: object.ignored_obstacle_id.map(|id| id.0),
            can_path_through_units: object.can_path_through_units,
            last_command_source: object.last_command_source,
            locomotor_upgrade: object.locomotor_upgrade,
            cur_locomotor_set: object.jet_ai.cur_locomotor_set.clone(),
            has_locomotor_motion: true,
            braking: object.braking,
            max_lift: object.max_lift,
            braking_factor: object.braking_factor,
            has_locomotor_arrival: true,
            close_enough_dist: object.close_enough_dist,
            close_enough_dist_3d: object.close_enough_dist_3d,
            donut_timer: object.donut_timer,
            preferred_height: object.loco_preferred_height,
            preferred_height_damping: object.loco_preferred_height_damping,
            has_locomotor_pose: true,
            loco_flags: pack_loco_flags(object),
            maintain_pos: object.maintain_pos.map(|p| [p.x, p.y, p.z]),
            wander_angle_offset: object.wander_angle_offset,
            wander_offset_increment: object.wander_offset_increment,
            has_locomotor_template: true,
            cur_locomotor_name: object.cur_locomotor_name.clone(),
            valid_locomotor_surfaces: object.locomotor_surfaces,
            downhill_only: object.downhill_only,
        });
    }
    let script_status = game_logic
        .host_objects()
        .registered()
        .map(|(id, object)| ObjectScriptStatusPersist {
            object_id: id.0,
            script_targetable: object.script_targetable,
            masked: object.status.masked,
        })
        .collect();
    let structures = game_logic
        .host_objects()
        .registered()
        .filter_map(|(id, object)| {
            object
                .structure_topple_data
                .as_ref()
                .map(|runtime| StructureTopplePersist {
                    object_id: id.0,
                    matrix: object.get_transform_matrix().to_cols_array(),
                    runtime: runtime.clone(),
                })
        })
        .collect();
    let mut damage_info: Vec<_> = game_logic
        .host_objects()
        .registered()
        .map(|(id, object)| ObjectDamageInfoPersist {
            object_id: id.0,
            last_damage_type: object.last_damage_info_type,
        })
        .collect();
    damage_info.sort_by_key(|entry| entry.object_id);
    let mut open_contain_rallies: Vec<_> = game_logic
        .host_objects()
        .registered()
        .filter_map(|(id, object)| {
            object
                .open_contain_rally_point
                .map(|point| OpenContainRallyPersist {
                    object_id: id.0,
                    point: point.to_array(),
                })
        })
        .collect();
    open_contain_rallies.sort_by_key(|entry| entry.object_id);
    let mut passenger_fire_permissions: Vec<_> = game_logic
        .host_objects()
        .registered()
        .map(|(id, object)| (id.0, object.passengers_allowed_to_fire))
        .collect();
    passenger_fire_permissions.sort_by_key(|entry| entry.0);
    let mut weapon_activity: Vec<_> = game_logic
        .host_objects()
        .registered()
        .map(|(id, object)| {
            (
                id.0,
                object.status.is_aiming_weapon,
                object.status.is_firing_weapon,
            )
        })
        .collect();
    weapon_activity.sort_by_key(|entry| entry.0);
    let mut target_positions: Vec<_> = game_logic
        .host_objects()
        .registered()
        .map(|(id, object)| {
            (
                id.0,
                object.target_location.is_some(),
                object
                    .target_location
                    .unwrap_or(glam::Vec3::ZERO)
                    .to_array(),
            )
        })
        .collect();
    target_positions.sort_by_key(|entry| entry.0);
    let mut body_fx_throttles: Vec<_> = game_logic
        .host_objects()
        .registered()
        .map(|(id, object)| (id.0, object.next_damage_fx_time, object.last_damage_fx_done))
        .collect();
    body_fx_throttles.sort_by_key(|entry| entry.0);
    let mut cleared_last_attackers: Vec<_> = game_logic
        .host_objects()
        .registered()
        .filter_map(|(id, object)| object.last_damage_cleared.then_some(id.0))
        .collect();
    cleared_last_attackers.sort_unstable();
    ObjectXferPersistPayload {
        objects,
        script_status,
        structures,
        damage_info,
        transport: transport_movement_persist::capture(game_logic),
        body_history: body_damage_history_persist::capture(game_logic),
        open_contain_rallies,
        passenger_fire_permissions,
        weapon_activity,
        target_positions,
        body_fx_throttles,
        cleared_last_attackers,
    }
}

fn validate_structure_records(
    game_logic: &GameLogic,
    records: &[StructureTopplePersist],
) -> SaveLoadResult<()> {
    let mut ids = std::collections::HashSet::new();
    for entry in records {
        let data = &entry.runtime;
        let object = game_logic
            .host_object(ObjectId(entry.object_id))
            .ok_or_else(|| {
                SaveLoadError::Corrupted("saved StructureTopple owner is absent".into())
            })?;
        let values = [
            data.dir_x,
            data.dir_y,
            data.topple_velocity,
            data.accumulated_angle,
            data.structural_integrity,
            data.structural_decay,
            data.lean_radians,
            data.last_crushed_location,
            data.building_height,
            data.facing_width,
            data.crush_damage,
            data.crush_radius,
            data.major_radius,
            data.minor_radius,
            data.orientation,
            data.building_x,
            data.building_y,
            data.building_z,
            data.delay_burst_x,
            data.delay_burst_y,
            data.delay_burst_z,
            data.last_polled_angle,
        ];
        if !ids.insert(entry.object_id)
            || !object.thing().template.authored_structure_topple
            || object
                .thing()
                .template
                .structure_topple_definition
                .as_ref()
                .is_some_and(Result::is_err)
            || !entry.matrix.iter().all(|value| value.is_finite())
            || !values.iter().all(|value| value.is_finite())
            || !data.angle_fx.iter().all(|(angle, _)| angle.is_finite())
            || !data.pending_fx.iter().all(|effect| {
                [effect.x, effect.y, effect.z]
                    .iter()
                    .all(|value| value.is_finite())
            })
        {
            return Err(SaveLoadError::Corrupted(
                "invalid saved StructureTopple state or receiving module".into(),
            ));
        }
    }
    Ok(())
}

fn validate_open_contain_rallies(
    game_logic: &GameLogic,
    records: &[OpenContainRallyPersist],
) -> SaveLoadResult<()> {
    let mut ids = std::collections::HashSet::new();
    for entry in records {
        let object = game_logic
            .host_object(ObjectId(entry.object_id))
            .ok_or_else(|| {
                SaveLoadError::Corrupted("saved OpenContain rally owner is absent".into())
            })?;
        if object.thing().template.contain_module.kind == crate::game_logic::ContainModuleKind::None
        {
            return Err(SaveLoadError::Corrupted(
                "saved OpenContain rally owner has no receiving contain module".into(),
            ));
        }
        if !ids.insert(entry.object_id) {
            return Err(SaveLoadError::Corrupted(
                "duplicate saved OpenContain rally owner".into(),
            ));
        }
    }
    Ok(())
}

fn reset_object_xfer(game_logic: &mut GameLogic) {
    transport_movement_persist::reset(game_logic);
    body_damage_history_persist::apply(game_logic, Vec::new());
    let ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
    for id in ids {
        let Some(object) = game_logic.host_objects_mut().get_mut(&id) else {
            continue;
        };
        object.script_targetable = false;
        object.status.masked = false;
        object.structure_topple_data = None;
        object.last_damage_info_type = None;
        object.last_damage_fx_done = None;
        object.next_damage_fx_time = 0;
        object.status.disabled_held = false;
        object.single_use_command_used = false;
        object.ai_attitude = 0;
        object.custom_indicator_color = None;
        object.pre_attack_ready_at = 0.0;
        object.pre_attack_target = None;
        object.consecutive_shot_target = None;
        object.max_shots_to_fire = -1;
        object.turret_ai.turret_idle_scan_next_frame = 0;
        object.turret_ai.turret_idle_scanning = false;
        object.turret_ai.turret_holding = false;
        object.turret_ai.turret_hold_until_frame = 0;
        object.turret_ai.turret_idle_recentering = false;
        object.turret_ai.turret_mood_target = false;
        object.turret_ai.turret_target_id = None;
        object.turret_ai.turret_force_attacking = false;
        object.turret_ai.turret_substate = TurretSubState::Idle;
        object.turret_ai.turret_rotating = false;
        object.weapon_bonus_solo = 0;
        object.is_receiving_difficulty_bonus = false;
        object.which_exit_path = 0;
        object.open_contain_rally_point = None;
        object.cheer_timer = 0.0;
        set_object_special_cheering(object, false);
        object.weapon_scatter_targets_unused = Default::default();
        object.weapon_scatter_targets_inited = [false; 3];
        object.weapon_bonus_horde = false;
        object.weapon_bonus_nationalism = false;
        object.weapon_bonus_fanaticism = false;
        object.weapon_bonus_enthusiastic = false;
        object.weapon_bonus_subliminal = false;
        object.health_box_offset = [0.0; 3];
        object.last_fire_frame = 0;
        object.is_recruitable = true;
        object.ignore_collisions_until_frame = 0;
        object.do_final_position = false;
        object.final_position = glam::Vec3::ZERO;
        object.ignored_obstacle_id = None;
        object.can_path_through_units = false;
        object.last_command_source = HUNT_CMD_FROM_AI;
        object.locomotor_upgrade = false;
        object.jet_ai.cur_locomotor_set = None;
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: ObjectXferPersistPayload) {
    for (id, deadline, damage_type) in payload.body_fx_throttles {
        if let Some(object) = game_logic.host_objects_mut().get_mut(&ObjectId(id)) {
            object.next_damage_fx_time = deadline;
            object.last_damage_fx_done = damage_type;
        }
    }
    for (id, present, position) in payload.target_positions {
        if let Some(object) = game_logic.host_objects_mut().get_mut(&ObjectId(id)) {
            object.target_location = present.then(|| glam::Vec3::from_array(position));
        }
    }
    for (id, aiming, firing) in payload.weapon_activity {
        if let Some(object) = game_logic.host_objects_mut().get_mut(&ObjectId(id)) {
            object.status.is_aiming_weapon = aiming;
            object.status.is_firing_weapon = firing;
        }
    }
    for (id, allowed) in payload.passenger_fire_permissions {
        if let Some(object) = game_logic.host_objects_mut().get_mut(&ObjectId(id)) {
            object.passengers_allowed_to_fire = allowed;
        }
    }
    transport_movement_persist::apply(game_logic, payload.transport);
    body_damage_history_persist::apply(game_logic, payload.body_history);
    for id in payload.cleared_last_attackers {
        game_logic
            .host_object_mut(ObjectId(id))
            .expect("validated cleared last-attacker owner")
            .last_damage_cleared = true;
    }
    let object_ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
    for id in object_ids {
        if let Some(object) = game_logic.host_objects_mut().get_mut(&id) {
            object.open_contain_rally_point = None;
        }
    }
    for entry in payload.open_contain_rallies {
        if let Some(object) = game_logic
            .host_objects_mut()
            .get_mut(&ObjectId(entry.object_id))
        {
            object.open_contain_rally_point = Some(glam::Vec3::from_array(entry.point));
        }
    }
    // Original Object::xfer restores raw status; it does not replay mask or
    // script commands. The receiving world already owns these exact objects.
    for entry in payload.damage_info {
        if let Some(object) = game_logic
            .host_objects_mut()
            .get_mut(&ObjectId(entry.object_id))
        {
            object.last_damage_info_type = entry.last_damage_type;
        }
    }
    for entry in payload.structures {
        if let Some(object) = game_logic
            .host_objects_mut()
            .get_mut(&ObjectId(entry.object_id))
        {
            object.restore_owned_structure_topple(
                entry.runtime,
                glam::Mat4::from_cols_array(&entry.matrix),
            );
        }
    }
    for entry in payload.script_status {
        if let Some(object) = game_logic
            .host_objects_mut()
            .get_mut(&ObjectId(entry.object_id))
        {
            object.script_targetable = entry.script_targetable;
            object.status.masked = entry.masked;
        }
    }
    for entry in payload.objects {
        let id = ObjectId(entry.object_id);
        let selected_locomotor = entry
            .has_locomotor_template
            .then(|| entry.cur_locomotor_name.as_deref())
            .flatten()
            .and_then(|name| game_logic.resolve_receiving_locomotor(name));
        let locomotors = game_logic.locomotor_definitions.as_ref();
        if let Some(object) = game_logic.objects.get_mut(&id) {
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
            object.turret_ai.turret_angle_deg = entry.turret_angle_deg;
            object.turret_ai.turret_pitch_deg = entry.turret_pitch_deg;
            object.turret_ai.turret_idle_scan_next_frame = entry.turret_idle_scan_next_frame;
            object.turret_ai.turret_idle_scanning = entry.turret_idle_scanning;
            object.turret_ai.turret_idle_scan_desired_angle_deg = entry.turret_idle_scan_desired_angle_deg;
            object.turret_ai.turret_idle_scan_index = entry.turret_idle_scan_index;
            object.turret_ai.turret_holding = entry.turret_holding;
            object.turret_ai.turret_hold_until_frame = entry.turret_hold_until_frame;
            object.turret_ai.turret_idle_recentering = entry.turret_idle_recentering;
            object.turret_ai.turret_mood_target = entry.turret_mood_target;
            object.turret_ai.turret_target_id = entry.turret_target_id.map(ObjectId);
            object.turret_ai.turret_force_attacking = entry.turret_force_attacking;
            object.turret_ai.turret_enabled = entry.turret_enabled;
            object.turret_ai.turret_substate = TurretSubState::from_ordinal(entry.turret_substate);
            object.turret_ai.turret_rotating = entry.turret_rotating;
            object.weapon_bonus_solo = entry.weapon_bonus_solo;
            object.is_receiving_difficulty_bonus = entry.weapon_bonus_solo != 0;
            object.which_exit_path = entry.which_exit_path;
            object.cheer_timer = entry.cheer_timer;
            set_object_special_cheering(object, entry.special_cheering);
            object.weapon_scatter_targets_unused = entry.weapon_scatter_targets_unused;
            object.weapon_scatter_targets_inited = entry.weapon_scatter_targets_inited;
            object.weapon_bonus_horde = entry.weapon_bonus_horde;
            object.weapon_bonus_enthusiastic = entry.weapon_bonus_enthusiastic;
            object.weapon_bonus_subliminal = entry.weapon_bonus_subliminal;
            object.safe_occlusion_frame = entry.safe_occlusion_frame;
            object.health_box_offset = entry.health_box_offset;
            object.last_fire_frame = entry.last_fire_frame;
            object.is_recruitable = entry.is_recruitable;
            object.ignore_collisions_until_frame = entry.ignore_collisions_until_frame;
            object.do_final_position = entry.do_final_position;
            object.final_position = glam::Vec3::from_array(entry.final_position);
            object.ignored_obstacle_id = entry.ignored_obstacle_id.map(ObjectId);
            object.can_path_through_units = entry.can_path_through_units;
            object.last_command_source = entry.last_command_source;
            if entry.has_locomotor_template {
                object.selected_locomotor_definition = None;
                object.cur_locomotor_name = entry.cur_locomotor_name;
                // Definition facts come from the receiving world; transferred
                // motion, arrival, and pose remain runtime state below.
                if let Some(binding) = selected_locomotor {
                    // Older guard/panic module capsules already restored their
                    // explicit width projection. Keep it through this definition
                    // reconstruction; v11 later restores the canonical row.
                    let saved_width = (object.panic_runtime.is_some()
                        || object.unit_ai_runtime.guard_phase().is_some())
                    .then_some(object.wander_width_factor);
                    crate::game_logic::locomotor_bootstrap::apply_host_locomotor_template_facts(
                        object, &binding,
                    );
                    if let Some(width) = saved_width {
                        object.wander_width_factor = width;
                    }
                }
            }
            object.restore_saved_locomotor(entry.locomotor_upgrade, entry.cur_locomotor_set);
            if entry.has_locomotor_motion {
                object.braking = entry.braking;
                object.max_lift = entry.max_lift;
                object.braking_factor = entry.braking_factor;
            }
            if entry.has_locomotor_arrival {
                object.close_enough_dist = entry.close_enough_dist;
                object.close_enough_dist_3d = entry.close_enough_dist_3d;
                object.donut_timer = entry.donut_timer;
                object.loco_preferred_height = entry.preferred_height;
                object.loco_preferred_height_damping = entry.preferred_height_damping;
            }
            if entry.has_locomotor_pose {
                apply_loco_flags(object, entry.loco_flags);
                object.maintain_pos = entry.maintain_pos.map(glam::Vec3::from_array);
                object.wander_angle_offset = entry.wander_angle_offset;
                object.wander_offset_increment = entry.wander_offset_increment;
            }
            if entry.has_locomotor_template {
                object.locomotor_surfaces = entry.valid_locomotor_surfaces;
                object.downhill_only = entry.downhill_only;
            }
            let locomotor_body_saved = entry.has_locomotor_template
                || entry.has_locomotor_motion
                || entry.has_locomotor_pose
                || entry.has_locomotor_arrival;
            if !locomotor_body_saved {
                if let Some(token) = object.jet_ai.cur_locomotor_set.clone() {
                    if let Some(kind) = crate::game_logic::host_upgrade_module_residuals::locomotor_set_kind_from_token(&token)
                    {
                        object.jet_ai.cur_locomotor_set = None;
                        crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_kind_for_surfaces_in(
                            object,
                            kind,
                            0,
                            locomotors,
                        );
                        object.jet_ai.cur_locomotor_set = Some(token);
                    }
                }
            }
        }
    }
}

fn pack_loco_flags(object: &crate::game_logic::Object) -> u32 {
    let mut flags = 0u32;
    let mut set = |bit: u32, on: bool| {
        if on {
            flags |= 1 << bit;
        }
    };
    // C++ LocoFlag order (Locomotor.h:397-408).
    set(0, object.is_braking);
    set(1, object.allow_invalid_position);
    set(2, object.maintain_pos_valid);
    set(3, object.precise_z_pos);
    set(4, object.no_slow_down_as_approaching_dest);
    set(5, object.over_water);
    set(6, object.ultra_accurate);
    set(7, object.moving_backwards);
    set(8, object.doing_three_point_turn);
    set(9, object.is_climbing);
    set(10, object.close_enough_dist_3d);
    set(11, object.wander_offset_increasing);
    flags
}

fn apply_loco_flags(object: &mut crate::game_logic::Object, flags: u32) {
    let on = |bit: u32| flags & (1 << bit) != 0;
    object.is_braking = on(0);
    object.allow_invalid_position = on(1);
    object.maintain_pos_valid = on(2);
    object.precise_z_pos = on(3);
    object.no_slow_down_as_approaching_dest = on(4);
    object.over_water = on(5);
    object.ultra_accurate = on(6);
    object.moving_backwards = on(7);
    object.doing_three_point_turn = on(8);
    object.is_climbing = on(9);
    object.close_enough_dist_3d = on(10);
    object.wander_offset_increasing = on(11);
}

fn special_cheering_mask() -> u128 {
    1u128 << crate::game_logic::host_enum_table_residual::special_cheering_model_bit()
}

fn object_has_special_cheering(object: &crate::game_logic::Object) -> bool {
    object.model_condition_bits & special_cheering_mask() != 0
}

fn set_object_special_cheering(object: &mut crate::game_logic::Object, on: bool) {
    let mask = special_cheering_mask();
    if on {
        object.model_condition_bits |= mask;
    } else {
        object.model_condition_bits &= !mask;
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
#[path = "locomotor_restore_tests.rs"]
mod locomotor_restore_tests;

/// Current OO deadline fields are immutable projections. Validate only after
/// native14 has been installed and all physical/definition tails have finished.
pub(super) fn validate_base_ai_projection_after_restore(
    bytes: &[u8],
    game_logic: &GameLogic,
) -> SaveLoadResult<()> {
    let Some(mut rest) = find_oxob_suffix(bytes) else {
        return Ok(());
    };
    let version = take_u32(&mut rest)?;
    if version != 18
        && version != 19
        && version != 20
        && version != 21
        && version != 22
        && version != OXOB_VERSION
    {
        return Err(SaveLoadError::Corrupted(
            "native14 requires current OO projection layout".into(),
        ));
    }
    let length = take_u32(&mut rest)? as usize;
    let encoded = rest
        .get(..length)
        .ok_or_else(|| SaveLoadError::Corrupted("truncated OO projection".into()))?;
    let payload = decode_current_payload(version, encoded)?;
    for entry in &payload.objects {
        let runtime = game_logic
            .unit_ai_runtime(ObjectId(entry.object_id))
            .ok_or_else(|| SaveLoadError::Corrupted("missing OO projection owner".into()))?;
        if runtime.guard_scan_deadline() != entry.guard_next_enemy_scan
            || runtime.hunt_scan_deadline() != entry.hunt_next_enemy_scan
            || runtime.quick_exit_deadline() != entry.quick_exit_until
        {
            return Err(SaveLoadError::Corrupted(
                "OO deadline projection disagrees with native14 owner".into(),
            ));
        }
    }
    transport_movement_persist::validate_base_ai_projection_after_restore(
        game_logic,
        &payload.transport,
    )
}

pub(super) fn validate_base_ai_projection_before_restore(
    bytes: &[u8],
    snapshot: &super::WorldSnapshot,
) -> SaveLoadResult<()> {
    let Some(mut rest) = find_oxob_suffix(bytes) else {
        return Ok(());
    };
    let version = take_u32(&mut rest)?;
    if version != 18
        && version != 19
        && version != 20
        && version != 21
        && version != 22
        && version != OXOB_VERSION
    {
        return Err(SaveLoadError::Corrupted(
            "native14 requires current OO projection layout".into(),
        ));
    }
    let length = take_u32(&mut rest)? as usize;
    let encoded = rest
        .get(..length)
        .ok_or_else(|| SaveLoadError::Corrupted("truncated OO projection".into()))?;
    let payload = decode_current_payload(version, encoded)?;
    for entry in &payload.objects {
        let saved = snapshot
            .objects
            .get(&ObjectId(entry.object_id))
            .ok_or_else(|| SaveLoadError::Corrupted("missing detached OO owner".into()))?;
        let control = match saved.modules.get("ReceivingBaseAI") {
            Some(super::ModuleSnapshot::BaseAi(owner)) => owner.continuation(),
            _ => {
                return Err(SaveLoadError::Corrupted(
                    "missing detached BaseAI capsule".into(),
                ));
            }
        };
        let expected = control.map_or((None, None, None), |control| control.deadline_projection());
        if expected
            != (
                entry.guard_next_enemy_scan,
                entry.hunt_next_enemy_scan,
                entry.quick_exit_until,
            )
        {
            return Err(SaveLoadError::Corrupted(
                "detached OO deadline projection disagrees with native14 owner".into(),
            ));
        }
    }
    transport_movement_persist::validate_base_ai_projection_before_restore(
        &payload.transport,
        snapshot,
    )
}
