//! Persist live team common-attack targets and AI waypoint / order state.
//!
//! C++ `Team::xfer` (`Team.cpp:2666`) writes `m_commonAttackTarget`.
//! The receiving TeamFactory chunk owns that target per Team instance.
//! C++ `AIUpdateInterface::xfer`
//! (`AIUpdate.cpp:4995-5105`) writes waypoint IDs, the waypoint queue,
//! `m_path`, `m_requestedDestination`, `m_currentVictimID`, path flags,
//! `m_lastCommandSource`, `m_ignoreObstacleID`, `m_finalPosition` /
//! `m_doFinalPosition`, and `m_canPathThroughUnits`.
//! Live `ObjectSnapshot` clones `movement` (path + index + target) but
//! those AI-order residuals never left the live object.
//! On complete world restore those object records remain authoritative for
//! Movement; the TMAI suffix applies only the AI-order fields not represented
//! there. The standalone tail API keeps its historical path-overlay behavior.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No world snapshot version bump.

use crate::game_logic::pathfinding::PendingHostPath;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use glam::Vec3;
use serde::{Deserialize, Serialize};

const TMAI_MAGIC: &[u8; 4] = b"TMAI";
/// The only TMAI layout read or written; older Rust-only layouts are not C++
/// save data and are rejected. queue_for_path_frames is the actor's absolute
/// C++ retry frame.
const TMAI_VERSION: u32 = 19;

fn default_fast_as_possible_order() -> f32 {
    999_999.0
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TeamCommonAttackPersist {
    team: String,
    target_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PendingPathPersist {
    start: [f32; 3],
    destination: [f32; 3],
    waypoints: Vec<[f32; 3]>,
    aircraft: bool,
    surfaces: u32,
    is_crusher: bool,
    ignore_obstacle: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectAiOrderPersist {
    object_id: u32,
    path: Vec<[f32; 3]>,
    current_path_index: u32,
    target_position: Option<[f32; 3]>,
    requested_destination: Option<[f32; 3]>,
    requested_victim_id: u32,
    waiting_for_path: bool,
    is_exact_path: bool,
    is_attack_path: bool,
    is_approach_path: bool,
    is_safe_path: bool,
    pending_waypoint_labels: Vec<String>,
    completed_waypoint_labels: Vec<String>,
    queue_for_path_frames: u32,
    group_speed_factor: f32,
    pending_path: Option<PendingPathPersist>,
    /// C++ `AIUpdateInterface::m_attitude` (Sleep=-2 … Aggressive=2).
    #[serde(default)]
    ai_attitude: i8,
    /// C++ `AIUpdateInterface::m_doFinalPosition`.
    #[serde(default)]
    do_final_position: bool,
    /// C++ `AIUpdateInterface::m_finalPosition` (host Y-up).
    #[serde(default)]
    final_position: [f32; 3],
    /// C++ `AIUpdateInterface::m_ignoreObstacleID` (0 = none).
    #[serde(default)]
    ignored_obstacle_id: u32,
    /// C++ `AIUpdateInterface::m_pathExtraDistance`, xfer'd right after the obstacle.
    #[serde(default)]
    path_extra_distance: f32,
    /// C++ `AIUpdateInterface::m_canPathThroughUnits`.
    #[serde(default)]
    can_path_through_units: bool,
    /// C++ `AIUpdateInterface::m_lastCommandSource` (CommandSourceType ordinal).
    #[serde(default = "default_last_command_source")]
    last_command_source: u32,
    /// Move kept while the unit cannot path yet. Not a locomotor goal.
    #[serde(default)]
    pending_move: Option<[f32; 3]>,
    /// C++ `AIUpdateInterface::m_repulsor2` (0 = none).
    #[serde(default)]
    safe_path_repulsor2: u32,
    /// C++ `AIUpdateInterface::m_moveOutOfWay1` (0 = none).
    #[serde(default)]
    move_away_from: u32,
    /// C++ `m_locomotorGoalType` / `m_locomotorGoalData` angle.
    #[serde(default)]
    locomotor_goal_type: u8,
    #[serde(default)]
    locomotor_goal_angle: f32,
    /// C++ `AIUpdateInterface::m_nextMoodCheckTime`.
    #[serde(default)]
    next_mood_check_time: u32,
    /// C++ `AIUpdateInterface::m_crateCreated` (0 = none).
    #[serde(default)]
    crate_created: u32,
    /// C++ `AIUpdate::m_desiredSpeed`. `999999` is as fast as possible.
    #[serde(default = "default_fast_as_possible_order")]
    desired_speed: f32,
    /// C++ `AIUpdateInterface::m_moveOutOfWay2` (0 = none).
    #[serde(default)]
    move_away_from_2: u32,
    /// Frames left in AI_MOVE_OUT_OF_THE_WAY. 0 means the ids are idle.
    #[serde(default)]
    move_away_frames: u32,
}

// Version 11 appends canonical movement runtime without changing v10 orders.
// AIInternalMoveToState::xfer preserves goal, prior path goal and timestamp;
// deriving the prior goal from a target's current pose changes repath timing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayloadV11 {
    ai: AiTeamPersistPayload,
    movement_runtime: Vec<MovementRuntimePersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WaypointIdentityPersist {
    receiver: u32,
    pending: Option<u32>,
    completed: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayloadV12 {
    previous: AiTeamPersistPayloadV11,
    waypoints: Vec<WaypointIdentityPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PendingFollowPathAppends {
    receiver: u32,
    goals: Vec<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayloadV13 {
    previous: AiTeamPersistPayloadV12,
    pending_appends: Vec<PendingFollowPathAppends>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayloadV14 {
    previous: AiTeamPersistPayloadV13,
    pending_order: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayloadV16 {
    previous: AiTeamPersistPayloadV14,
    final_goals: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PathGoalCellPersist {
    receiver: u32,
    cell: (i32, i32),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayloadV17 {
    previous: AiTeamPersistPayloadV16,
    goal_cells: Vec<PathGoalCellPersist>,
    pending_quick_goals: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PathGoalLayerPersist {
    receiver: u32,
    layer: u8,
    on_ground: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayloadV18 {
    previous: AiTeamPersistPayloadV17,
    goal_layers: Vec<PathGoalLayerPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayloadV19 {
    previous: AiTeamPersistPayloadV18,
    position_cells: Vec<PathGoalCellPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MovementRuntimePersist {
    receiver: u32,
    goal_object: Option<u32>,
    path_goal_position: Option<[f32; 3]>,
    path_timestamp: u32,
    // Guard/panic can override the host's projected template width.
    wander_width_override: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayload {
    team_targets: Vec<TeamCommonAttackPersist>,
    orders: Vec<ObjectAiOrderPersist>,
}

/// TMAI version 3: pending move, no `path_extra_distance`.
/// TMAI version 4: path extra distance, no repulsor.
/// TMAI version 5: repulsor, no move-away unit.
/// TMAI version 6: move-away unit, no locomotor goal.
/// TMAI version 7: locomotor goal, no mood-check time.
/// TMAI version 8: mood-check time, no crate id.
/// TMAI version 9: crate id, no desired speed.
fn default_last_command_source() -> u32 {
    crate::game_logic::HUNT_CMD_FROM_AI
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let ai = capture(game_logic);
    let mut movement_runtime: Vec<_> = game_logic
        .objects
        .registered()
        .map(|(id, object)| MovementRuntimePersist {
            receiver: id.0,
            goal_object: object.movement_goal_object().map(|goal| goal.0),
            path_goal_position: object.path_goal_position.as_ref().map(vec3_to_arr),
            path_timestamp: object.path_timestamp,
            wander_width_override: (object.panic_runtime.is_some()
                || object.unit_ai_runtime.guard_phase().is_some())
            .then_some(object.wander_width_factor),
        })
        .collect();
    movement_runtime.sort_by_key(|row| row.receiver);
    let payload = AiTeamPersistPayloadV11 {
        ai,
        movement_runtime,
    };
    if payload.ai.team_targets.is_empty()
        && payload.ai.orders.is_empty()
        && payload.movement_runtime.is_empty()
    {
        return;
    }
    let waypoints = game_logic
        .objects
        .registered()
        .filter_map(|(id, object)| {
            (object.pending_waypoint_id.is_some() || object.completed_waypoint_id.is_some())
                .then_some(WaypointIdentityPersist {
                    receiver: id.0,
                    pending: object.pending_waypoint_id,
                    completed: object.completed_waypoint_id,
                })
        })
        .collect();
    let payload = AiTeamPersistPayloadV12 {
        previous: payload,
        waypoints,
    };
    let pending = game_logic.snapshot_pending_host_paths();
    // The order payload contains only requests with a saved actor order. A
    // stale ID for an already retired object cannot be reconstructed.
    let admitted: std::collections::HashSet<_> = payload
        .previous
        .ai
        .orders
        .iter()
        .filter(|order| order.pending_path.is_some())
        .map(|order| order.object_id)
        .collect();
    let pending_order = pending
        .iter()
        .filter(|request| admitted.contains(&request.unit_id.0))
        .map(|request| request.unit_id.0)
        .collect();
    let pending_appends = pending
        .into_iter()
        .filter(|request| admitted.contains(&request.unit_id.0))
        .filter(|request| !request.follow_path_appends.is_empty())
        .map(|request| PendingFollowPathAppends {
            receiver: request.unit_id.0,
            goals: request
                .follow_path_appends
                .iter()
                .map(vec3_to_arr)
                .collect(),
        })
        .collect();
    let payload = AiTeamPersistPayloadV13 {
        previous: payload,
        pending_appends,
    };
    let payload = AiTeamPersistPayloadV14 {
        previous: payload,
        pending_order,
    };
    let mut final_goals: Vec<u32> = game_logic
        .host_objects()
        .registered()
        .filter_map(|(id, object)| object.is_final_goal.then_some(id.0))
        .collect();
    final_goals.sort_unstable();
    let payload = AiTeamPersistPayloadV16 {
        previous: payload,
        final_goals,
    };
    let mut goal_cells: Vec<_> = game_logic
        .host_objects()
        .registered()
        .filter_map(|(id, object)| {
            (object.pathfind_goal_cell != (-1, -1)).then_some(PathGoalCellPersist {
                receiver: id.0,
                cell: object.pathfind_goal_cell,
            })
        })
        .collect();
    goal_cells.sort_unstable_by_key(|row| row.receiver);
    let mut pending_quick_goals: Vec<_> = game_logic
        .host_objects()
        .registered()
        .filter_map(|(id, object)| object.pending_quick_path_goal.then_some(id.0))
        .collect();
    pending_quick_goals.sort_unstable();
    let payload = AiTeamPersistPayloadV17 {
        previous: payload,
        goal_cells,
        pending_quick_goals,
    };
    let mut goal_layers: Vec<_> = game_logic
        .host_objects()
        .registered()
        .filter_map(|(id, object)| {
            (object.pathfind_goal_layer != 1).then_some(PathGoalLayerPersist {
                receiver: id.0,
                layer: object.pathfind_goal_layer,
                on_ground: object.pathfind_goal_on_ground,
            })
        })
        .collect();
    goal_layers.sort_unstable_by_key(|row| row.receiver);
    let payload = AiTeamPersistPayloadV18 {
        previous: payload,
        goal_layers,
    };
    let mut position_cells: Vec<_> = game_logic
        .host_objects()
        .registered()
        .filter_map(|(id, object)| {
            (object.pathfind_cur_cell != (-1, -1)).then_some(PathGoalCellPersist {
                receiver: id.0,
                cell: object.pathfind_cur_cell,
            })
        })
        .collect();
    position_cells.sort_unstable_by_key(|row| row.receiver);
    let payload = AiTeamPersistPayloadV19 {
        previous: payload,
        position_cells,
    };
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(TMAI_MAGIC);
    append_u32(bytes, TMAI_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    apply_from_lifecycle_tail_with_object_records(bytes, game_logic, None)
}

/// Apply the TMAI residual after a complete world object snapshot has restored.
/// The object records own the canonical Movement values, including an empty
/// path or absent target; TMAI still supplies the AI-only order residuals.
/// `None` preserves the historical standalone lifecycle-tail API semantics.
pub(super) fn apply_from_lifecycle_tail_with_object_records(
    bytes: &[u8],
    game_logic: &mut GameLogic,
    object_records: Option<&std::collections::HashSet<ObjectId>>,
) -> SaveLoadResult<()> {
    let receivers = game_logic.objects.registered_keys().copied().collect();
    let terrain_owner = game_logic.world_services.terrain();
    let prepared = {
        let terrain = terrain_owner
            .read()
            .map_err(|_| SaveLoadError::Corrupted("TMAI receiving Terrain unavailable".into()))?;
        prepare_from_lifecycle_tail(bytes, &receivers, &terrain)?
    };
    apply_prepared(prepared, game_logic, object_records);
    Ok(())
}

/// Checked owned continuation; no actor or Terrain loan survives decoding.
pub(super) struct PreparedAiTeam {
    payload: AiTeamPersistPayload,
    movement_runtime: Vec<MovementRuntimePersist>,
    waypoint_identities: Vec<WaypointIdentityPersist>,
    pending_appends: Vec<PendingFollowPathAppends>,
    pending_order: Option<Vec<u32>>,
    final_goals: Option<Vec<u32>>,
    goal_cells: Option<Vec<PathGoalCellPersist>>,
    pending_quick_goals: Option<Vec<u32>>,
    goal_layers: Option<Vec<PathGoalLayerPersist>>,
    position_cells: Option<Vec<PathGoalCellPersist>>,
}
pub(super) fn prepare_from_lifecycle_tail(
    bytes: &[u8],
    receivers: &std::collections::HashSet<ObjectId>,
    terrain: &gamelogic::terrain::TerrainLogic,
) -> SaveLoadResult<Option<PreparedAiTeam>> {
    let Some(suffix) = framing::find(bytes)? else {
        return Ok(None);
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != TMAI_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown TMAI suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "TMAI payload truncated".to_string(),
        ));
    }
    let encoded = &rest[..payload_len];
    let mut movement_runtime = Vec::new();
    let mut waypoint_identities = Vec::new();
    let mut pending_appends = Vec::new();
    let mut pending_order = None;
    let mut final_goals = None;
    let mut goal_cells = None;
    let mut pending_quick_goals = None;
    let mut goal_layers = None;
    let mut position_cells = None;
    let v19: AiTeamPersistPayloadV19 = bincode_legacy::deserialize(encoded)
        .map_err(|err| SaveLoadError::Corrupted(format!("TMAI v19 decode: {err}")))?;
    position_cells = Some(v19.position_cells);
    let v18 = v19.previous;
    goal_layers = Some(v18.goal_layers);
    let v17 = v18.previous;
    goal_cells = Some(v17.goal_cells);
    pending_quick_goals = Some(v17.pending_quick_goals);
    let v16 = v17.previous;
    final_goals = Some(v16.final_goals);
    let v14 = v16.previous;
    pending_order = Some(v14.pending_order);
    let v13 = v14.previous;
    pending_appends = v13.pending_appends;
    let current = v13.previous;
    // The waypoint row resolves exactly one canonical saved order.
    // Otherwise find(first) could validate slots that apply_payload's
    // later duplicate would overwrite after preflight succeeds.
    let mut order_receivers = std::collections::HashSet::new();
    for order in &current.previous.ai.orders {
        if order.object_id == 0
            || !receivers.contains(&ObjectId(order.object_id))
            || !order_receivers.insert(order.object_id)
        {
            return Err(SaveLoadError::Corrupted(
                "invalid TMAI v12 order receiver/duplicate".into(),
            ));
        }
    }
    let mut seen = std::collections::HashSet::new();
    for row in &current.waypoints {
        let order = current
            .previous
            .ai
            .orders
            .iter()
            .find(|order| order.object_id == row.receiver);
        if row.receiver == 0
            || !seen.insert(row.receiver)
            || row.pending.is_none() && row.completed.is_none()
            || order.is_none()
            || !receivers.contains(&ObjectId(row.receiver))
            || row.pending.is_some()
                && order.is_none_or(|order| order.pending_waypoint_labels.len() != 3)
            || row.completed.is_some()
                && order.is_none_or(|order| order.completed_waypoint_labels.len() != 3)
        {
            return Err(SaveLoadError::Corrupted(
                "invalid TMAI waypoint identity/slots".into(),
            ));
        }
        let order = order.expect("validated waypoint order");
        for (id, labels) in [
            (row.pending, &order.pending_waypoint_labels),
            (row.completed, &order.completed_waypoint_labels),
        ] {
            if let Some(id) = id {
                let waypoint = terrain.get_waypoint_by_id(id).ok_or_else(|| {
                    SaveLoadError::Corrupted(
                        "TMAI waypoint absent from receiving Terrain".into(),
                    )
                })?;
                let actual = [
                    waypoint.get_path_label1().to_string(),
                    waypoint.get_path_label2().to_string(),
                    waypoint.get_path_label3().to_string(),
                ];
                if labels.as_slice() != actual.as_slice() {
                    return Err(SaveLoadError::Corrupted(
                        "TMAI waypoint slots differ from receiving Terrain".into(),
                    ));
                }
            }
        }
    }
    waypoint_identities = current.waypoints;
    let envelope = current.previous;
    let mut seen = std::collections::HashSet::new();
    for row in &envelope.movement_runtime {
        if row
            .wander_width_override
            .is_some_and(|width| !width.is_finite())
            || row.receiver == 0
            || row.goal_object == Some(0)
            || !receivers.contains(&ObjectId(row.receiver))
            || !seen.insert(row.receiver)
            || row
                .path_goal_position
                .is_some_and(|point| !arr_to_vec3(point).is_finite())
        {
            return Err(SaveLoadError::Corrupted(
                "invalid TMAI movement runtime receiver/identity/position/width".into(),
            ));
        }
    }
    // Missing target is an ordinary checked lookup failure, not corrupt.
    movement_runtime = envelope.movement_runtime;
    let payload = envelope.ai;
    if !payload.team_targets.is_empty() {
        return Err(SaveLoadError::Corrupted(
            "Legacy name-keyed Team targets cannot identify a Team instance".into(),
        ));
    }
    let mut seen_appends = std::collections::HashSet::new();
    for row in &pending_appends {
        let order = payload
            .orders
            .iter()
            .find(|order| order.object_id == row.receiver);
        if row.receiver == 0
            || !receivers.contains(&ObjectId(row.receiver))
            || !seen_appends.insert(row.receiver)
            || row.goals.is_empty()
            || row.goals.len() > 65536
            || row.goals.iter().any(|goal| !arr_to_vec3(*goal).is_finite())
            || order.is_none_or(|order| order.pending_path.is_none())
        {
            return Err(SaveLoadError::Corrupted(
                "invalid pending FollowPath continuation".into(),
            ));
        }
    }
    if let Some(order) = &pending_order {
        let expected: std::collections::HashSet<_> = payload
            .orders
            .iter()
            .filter(|row| row.pending_path.is_some())
            .map(|row| row.object_id)
            .collect();
        let actual: std::collections::HashSet<_> = order.iter().copied().collect();
        if order.len() > crate::game_logic::pathfinding::MAX_PENDING_PATH_REQUESTS
            || actual.len() != order.len()
            || actual != expected
        {
            return Err(SaveLoadError::Corrupted(
                "invalid pending path queue order".into(),
            ));
        }
    }
    if let Some(final_goals) = &final_goals {
        let orders: std::collections::HashSet<_> =
            payload.orders.iter().map(|row| row.object_id).collect();
        let actual: std::collections::HashSet<_> = final_goals.iter().copied().collect();
        if actual.len() != final_goals.len() || !actual.is_subset(&orders) {
            return Err(SaveLoadError::Corrupted(
                "invalid TMAI final-goal receivers".into(),
            ));
        }
    }
    if let Some(rows) = &goal_cells {
        let mut seen = std::collections::HashSet::new();
        if rows.iter().any(|row| {
            row.receiver == 0
                || !receivers.contains(&ObjectId(row.receiver))
                || !seen.insert(row.receiver)
                || row.cell == (-1, -1)
        }) {
            return Err(SaveLoadError::Corrupted(
                "invalid TMAI path-goal receivers".into(),
            ));
        }
    }
    if let Some(ids) = &pending_quick_goals {
        let mut seen = std::collections::HashSet::new();
        if ids.iter().any(|&id| {
            id == 0
                || !receivers.contains(&ObjectId(id))
                || !seen.insert(id)
                || !payload.orders.iter().any(|order| order.object_id == id)
        }) {
            return Err(SaveLoadError::Corrupted(
                "invalid TMAI quick-path receivers".into(),
            ));
        }
    }
    if let Some(rows) = &goal_layers {
        let mut seen = std::collections::HashSet::new();
        if rows.iter().any(|row| {
            row.receiver == 0
                || !receivers.contains(&ObjectId(row.receiver))
                || !seen.insert(row.receiver)
                || !(2..=15).contains(&row.layer)
        }) {
            return Err(SaveLoadError::Corrupted(
                "invalid TMAI path-goal layers".into(),
            ));
        }
    }
    if let Some(rows) = &position_cells {
        let mut seen = std::collections::HashSet::new();
        if rows.iter().any(|row| {
            row.receiver == 0
                || !receivers.contains(&ObjectId(row.receiver))
                || !seen.insert(row.receiver)
                || row.cell == (-1, -1)
        }) {
            return Err(SaveLoadError::Corrupted(
                "invalid TMAI path-position receivers".into(),
            ));
        }
    }
    Ok(Some(PreparedAiTeam {
        payload,
        movement_runtime,
        waypoint_identities,
        pending_appends,
        pending_order,
        final_goals,
        goal_cells,
        pending_quick_goals,
        goal_layers,
        position_cells,
    }))
}
pub(super) fn apply_prepared(
    prepared: Option<PreparedAiTeam>,
    game_logic: &mut GameLogic,
    object_records: Option<&std::collections::HashSet<ObjectId>>,
) {
    let Some(PreparedAiTeam {
        payload,
        movement_runtime,
        waypoint_identities,
        pending_appends,
        pending_order,
        final_goals,
        goal_cells,
        pending_quick_goals,
        goal_layers,
        position_cells,
    }) = prepared
    else {
        return;
    };
    if let Some(rows) = position_cells {
        for object in game_logic.objects.registered_values_mut() {
            object.pathfind_cur_cell = (-1, -1);
        }
        for row in rows {
            game_logic
                .objects
                .get_mut(&ObjectId(row.receiver))
                .expect("validated position receiver")
                .pathfind_cur_cell = row.cell;
        }
    }
    // Historical labels-only capsules retain their known label observations;
    // they cannot recover an ID or an empty completion that old producers lost.
    for object in game_logic.objects.registered_values_mut() {
        object.pending_waypoint_id = None;
        object.completed_waypoint_id = None;
    }
    if let Some(rows) = goal_cells {
        for object in game_logic.objects.registered_values_mut() {
            object.pathfind_goal_cell = (-1, -1);
        }
        for row in rows {
            game_logic
                .objects
                .get_mut(&ObjectId(row.receiver))
                .expect("validated goal receiver")
                .pathfind_goal_cell = row.cell;
        }
    }
    if let Some(rows) = goal_layers {
        for object in game_logic.objects.registered_values_mut() {
            object.pathfind_goal_layer = 1;
            object.pathfind_goal_on_ground = true;
        }
        for row in rows {
            let object = game_logic
                .objects
                .get_mut(&ObjectId(row.receiver))
                .expect("validated goal-layer receiver");
            object.pathfind_goal_layer = row.layer;
            object.pathfind_goal_on_ground = row.on_ground;
        }
    }
    if let Some(ids) = pending_quick_goals {
        for object in game_logic.objects.registered_values_mut() {
            object.pending_quick_path_goal = false;
        }
        for id in ids {
            game_logic
                .objects
                .get_mut(&ObjectId(id))
                .expect("validated quick-path receiver")
                .pending_quick_path_goal = true;
        }
    }
    if let Some(final_goals) = &final_goals {
        let selected: std::collections::HashSet<_> = final_goals.iter().copied().collect();
        for order in &payload.orders {
            if let Some(object) = game_logic.objects.get_mut(&ObjectId(order.object_id)) {
                object.is_final_goal = selected.contains(&order.object_id);
            }
        }
    }
    apply_payload(
        game_logic,
        payload,
        pending_appends,
        pending_order,
        object_records,
    );
    for row in waypoint_identities {
        let object = game_logic
            .objects
            .get_mut(&ObjectId(row.receiver))
            .expect("validated waypoint receiver");
        object.pending_waypoint_id = row.pending;
        object.completed_waypoint_id = row.completed;
    }
    for object in game_logic.objects.registered_values_mut() {
        object.set_movement_goal_object(None);
    }
    for row in movement_runtime {
        let object = game_logic
            .objects
            .get_mut(&ObjectId(row.receiver))
            .expect("validated receiver");
        object.set_movement_goal_object(row.goal_object.map(ObjectId));
        object.path_goal_position = row.path_goal_position.map(arr_to_vec3);
        object.path_timestamp = row.path_timestamp;
        if let Some(width) = row.wander_width_override {
            object.wander_width_factor = width;
        }
    }
}

fn capture(game_logic: &GameLogic) -> AiTeamPersistPayload {
    let team_targets = Vec::new();

    let pending: std::collections::HashMap<u32, PendingHostPath> = game_logic
        .snapshot_pending_host_paths()
        .into_iter()
        .map(|req| (req.unit_id.0, req))
        .collect();

    let mut object_ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
    object_ids.sort();
    let mut orders = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        let pending_path = pending.get(&id.0).map(persist_pending_path);
        let remaining = remaining_path(object);
        let interesting = !remaining.is_empty()
            || object.requested_destination.is_some()
            || object.requested_victim_id.is_some()
            || object.waiting_for_path
            || object.is_final_goal
            || object.is_exact_path
            || object.is_attack_path
            || object.is_approach_path
            || object.is_safe_path
            || !object.pending_waypoint_labels.is_empty()
            || !object.completed_waypoint_labels.is_empty()
            || pending_path.is_some()
            || object.ai_attitude != 0
            || object.do_final_position
            || object.ignored_obstacle_id.is_some()
            || object.path_extra_distance != 0.0
            || object.safe_path_repulsor2.is_some()
            || object.move_away_from.is_some()
            || object.move_away_from_2.is_some()
            || object.move_away_frames > 0
            || object.locomotor_goal_type != crate::game_logic::LocoGoalType::None
            || object.locomotor_goal_angle != 0.0
            || object.next_mood_check_time != 0
            || object.crate_created.is_some()
            || (object.desired_speed - 999_999.0).abs() > 1.0
            || object.can_path_through_units
            || object.last_command_source != default_last_command_source()
            || object.pending_move.is_some();

        if !interesting {
            continue;
        }
        orders.push(ObjectAiOrderPersist {
            object_id: id.0,
            path: remaining.iter().map(vec3_to_arr).collect(),
            current_path_index: 0,
            target_position: object.movement.target_position.as_ref().map(vec3_to_arr),
            requested_destination: object.requested_destination.as_ref().map(vec3_to_arr),
            requested_victim_id: object.requested_victim_id.map(|v| v.0).unwrap_or(0),
            waiting_for_path: object.waiting_for_path,
            is_exact_path: object.is_exact_path,
            is_attack_path: object.is_attack_path,
            is_approach_path: object.is_approach_path,
            is_safe_path: object.is_safe_path,
            pending_waypoint_labels: object.pending_waypoint_labels.clone(),
            completed_waypoint_labels: object.completed_waypoint_labels.clone(),
            queue_for_path_frames: object.queue_for_path_frame,
            group_speed_factor: object.group_speed_factor,
            ai_attitude: object.ai_attitude,
            do_final_position: object.do_final_position,
            final_position: vec3_to_arr(&object.final_position),
            ignored_obstacle_id: object.ignored_obstacle_id.map(|v| v.0).unwrap_or(0),
            path_extra_distance: object.path_extra_distance,
            can_path_through_units: object.can_path_through_units,
            last_command_source: object.last_command_source,
            pending_move: object.pending_move.as_ref().map(vec3_to_arr),
            safe_path_repulsor2: object.safe_path_repulsor2.map(|v| v.0).unwrap_or(0),
            move_away_from: object.move_away_from.map(|v| v.0).unwrap_or(0),
            move_away_from_2: object.move_away_from_2.map(|v| v.0).unwrap_or(0),
            move_away_frames: object.move_away_frames,
            locomotor_goal_type: object.locomotor_goal_type as u8,
            locomotor_goal_angle: object.locomotor_goal_angle,
            next_mood_check_time: object.next_mood_check_time,
            crate_created: object.crate_created.map(|v| v.0).unwrap_or(0),
            desired_speed: object.desired_speed,
            pending_path,
        });
    }

    AiTeamPersistPayload {
        team_targets,
        orders,
    }
}
fn apply_payload(
    game_logic: &mut GameLogic,
    payload: AiTeamPersistPayload,
    pending_appends: Vec<PendingFollowPathAppends>,
    pending_order: Option<Vec<u32>>,
    object_records: Option<&std::collections::HashSet<ObjectId>>,
) {
    let frame = game_logic.get_current_frame() as u32;
    let mut pending = Vec::new();
    let mut appends: std::collections::HashMap<_, _> = pending_appends
        .into_iter()
        .map(|row| (row.receiver, row.goals))
        .collect();
    for order in payload.orders {
        let id = ObjectId(order.object_id);
        if let Some(mut req) = order
            .pending_path
            .as_ref()
            .map(|p| restore_pending_path(id, p))
        {
            if let Some(goals) = appends.remove(&id.0) {
                req.follow_path_appends = goals.into_iter().map(arr_to_vec3).collect();
            }
            pending.push(req);
        }
        let Some(object) = game_logic.host_objects_mut().get_mut(&id) else {
            continue;
        };
        let object_snapshot_owns_movement = object_records.is_some_and(|ids| ids.contains(&id));
        if !object_snapshot_owns_movement {
            let path: Vec<Vec3> = order.path.iter().copied().map(arr_to_vec3).collect();
            if !path.is_empty() {
                object.movement.path = path;
                object.movement.current_path_index = order.current_path_index as usize;
            }
            if let Some(pos) = order.target_position {
                object.movement.target_position = Some(arr_to_vec3(pos));
            }
        }
        object.requested_destination = order.requested_destination.map(arr_to_vec3);
        object.requested_victim_id =
            (order.requested_victim_id != 0).then_some(ObjectId(order.requested_victim_id));
        object.waiting_for_path = order.waiting_for_path;
        object.is_exact_path = order.is_exact_path;
        object.is_attack_path = order.is_attack_path;
        object.is_approach_path = order.is_approach_path;
        object.is_safe_path = order.is_safe_path;
        object.pending_waypoint_labels = order.pending_waypoint_labels;
        object.completed_waypoint_labels = order.completed_waypoint_labels;
        object.queue_for_path_frame = order.queue_for_path_frames;
        object.group_speed_factor = order.group_speed_factor;
        object.ai_attitude = order.ai_attitude;
        object.do_final_position = order.do_final_position;
        object.final_position = arr_to_vec3(order.final_position);
        object.ignored_obstacle_id =
            (order.ignored_obstacle_id != 0).then_some(ObjectId(order.ignored_obstacle_id));
        object.path_extra_distance = order.path_extra_distance;
        object.can_path_through_units = order.can_path_through_units;
        object.last_command_source = order.last_command_source;
        object.pending_move = order.pending_move.map(arr_to_vec3);
        object.safe_path_repulsor2 =
            (order.safe_path_repulsor2 != 0).then_some(ObjectId(order.safe_path_repulsor2));
        object.move_away_from =
            (order.move_away_from != 0).then_some(ObjectId(order.move_away_from));
        object.move_away_from_2 =
            (order.move_away_from_2 != 0).then_some(ObjectId(order.move_away_from_2));
        object.move_away_frames = order.move_away_frames;
        object.locomotor_goal_type = match order.locomotor_goal_type {
            1 => crate::game_logic::LocoGoalType::PositionOnPath,
            2 => crate::game_logic::LocoGoalType::PositionExplicit,
            3 => crate::game_logic::LocoGoalType::Angle,
            _ => crate::game_logic::LocoGoalType::None,
        };
        object.locomotor_goal_angle = order.locomotor_goal_angle;
        object.next_mood_check_time = order.next_mood_check_time;
        object.crate_created = (order.crate_created != 0).then_some(ObjectId(order.crate_created));
        object.desired_speed = order.desired_speed;
    }
    if let Some(order) = pending_order {
        let mut by_id: std::collections::HashMap<_, _> = pending
            .into_iter()
            .map(|request| (request.unit_id.0, request))
            .collect();
        pending = order
            .into_iter()
            .map(|id| by_id.remove(&id).expect("validated pending path receiver"))
            .collect();
    }
    game_logic.restore_pending_host_paths(pending);
}

fn remaining_path(object: &crate::game_logic::Object) -> Vec<Vec3> {
    let idx = object
        .movement
        .current_path_index
        .min(object.movement.path.len());
    object.movement.path[idx..].to_vec()
}

fn persist_pending_path(req: &PendingHostPath) -> PendingPathPersist {
    PendingPathPersist {
        start: vec3_to_arr(&req.start),
        destination: vec3_to_arr(&req.destination),
        waypoints: req.waypoints.iter().map(vec3_to_arr).collect(),
        aircraft: req.aircraft,
        surfaces: req.surfaces,
        is_crusher: req.is_crusher,
        ignore_obstacle: req.ignore_obstacle.map(|id| id.0).unwrap_or(0),
    }
}

fn restore_pending_path(unit_id: ObjectId, persist: &PendingPathPersist) -> PendingHostPath {
    PendingHostPath {
        unit_id,
        start: arr_to_vec3(persist.start),
        destination: arr_to_vec3(persist.destination),
        waypoints: persist.waypoints.iter().copied().map(arr_to_vec3).collect(),
        follow_path_appends: Vec::new(),
        aircraft: persist.aircraft,
        surfaces: persist.surfaces,
        is_crusher: persist.is_crusher,
        ignore_obstacle: (persist.ignore_obstacle != 0)
            .then_some(ObjectId(persist.ignore_obstacle)),
        adjust_destinations: true,
    }
}

fn vec3_to_arr(v: &Vec3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn arr_to_vec3(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}

#[path = "ai_team_framing.rs"]
pub(super) mod framing;

/// The current TMAI owner a world writes, decoded (tests corrupt it in place).
#[cfg(test)]
fn current_tmai_payload(game_logic: &GameLogic) -> AiTeamPersistPayloadV19 {
    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, game_logic);
    let mut rest = framing::find(&bytes).unwrap().unwrap();
    assert_eq!(take_u32(&mut rest).unwrap(), TMAI_VERSION);
    let length = take_u32(&mut rest).unwrap() as usize;
    bincode_legacy::deserialize(&rest[..length]).unwrap()
}

/// One framed current TMAI owner record.
#[cfg(test)]
fn encode_tmai_owner(payload: &AiTeamPersistPayloadV19) -> Vec<u8> {
    let encoded = bincode_legacy::serialize(payload).unwrap();
    let mut owner = TMAI_MAGIC.to_vec();
    append_u32(&mut owner, TMAI_VERSION);
    append_u32(&mut owner, encoded.len() as u32);
    owner.extend(encoded);
    owner
}

#[cfg(test)]
impl AiTeamPersistPayloadV19 {
    /// The orders + waypoint-identity layer inside the current envelope.
    fn orders_mut(&mut self) -> &mut AiTeamPersistPayloadV12 {
        &mut self.previous.previous.previous.previous.previous.previous
    }
}

#[cfg(test)]
fn find_tmai_suffix(bytes: &[u8]) -> Option<&[u8]> {
    framing::find(bytes).ok().flatten()
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("TMAI u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{AIState, Player, Team};
    use crate::save_load::snapshot::{ObjectTypeSnapshot, SnapshotBuilder};

    /// An AI-driven Ranger and a depot authored as INI before NewGame; a
    /// loading game admits the same content before the save is applied.
    fn ranger_logic() -> GameLogic {
        use crate::fixture_content::{admit_fixture_objects, FixtureObject};
        let mut logic = GameLogic::new();
        admit_fixture_objects(
            &mut logic,
            &[
                FixtureObject::new("Ranger").kind_of("INFANTRY").health(100.0).walker(),
                FixtureObject::new("USASupplyDepot").health(400.0),
            ],
        );
        logic.add_player(Player::new(0, Team::USA, "USA", false));
        logic
    }

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_tmai_suffix(b"no-magic-here").is_none());
    }

    #[test]
    fn snapshot_round_trips_team_common_attack_target() {
        let mut source = ranger_logic();
        let victim = source
            .create_object("USASupplyDepot", Team::China, Vec3::new(80.0, 0.0, 20.0))
            .expect("victim");
        let attacker = source
            .create_object("Ranger", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("attacker");
        if let Some(obj) = source.host_object_mut(attacker) {
            obj.team_instance_name = "USA_AttackSquad".into();
            obj.owner_player_id = Some(0);
        }
        source.bind_common_target_fixture(attacker, "USA_AttackSquad");
        source.set_fixture_common_target(attacker, victim);

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        let teams = super::super::player_team_chunks::stamp_from_live(&source)
            .expect("capture TeamFactory")
            .teams
            .expect("TeamFactory chunk");
        assert!(
            teams
                .teams
                .iter()
                .any(|team| team.team_name == "USA_AttackSquad"
                    && team.common_attack_target == Some(victim.0))
        );

        let mut restored = ranger_logic();
        let restored_victim = restored
            .create_object("USASupplyDepot", Team::China, Vec3::new(80.0, 0.0, 20.0))
            .expect("victim fixture");
        let restored_attacker = restored
            .create_object("Ranger", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("attacker fixture");
        assert_eq!((restored_victim, restored_attacker), (victim, attacker));
        restored
            .host_object_mut(restored_attacker)
            .unwrap()
            .owner_player_id = Some(0);
        restored.bind_common_target_fixture(restored_attacker, "USA_AttackSquad");
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        assert_eq!(
            restored.fixture_common_target(attacker),
            Some(victim),
            "team common-attack target must survive load"
        );
    }

    #[test]
    fn snapshot_round_trips_waypoint_queue_and_order() {
        let mut source = ranger_logic();
        let id = source
            .create_object("Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("ranger");
        let remaining = vec![
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(40.0, 0.0, 10.0),
            Vec3::new(80.0, 0.0, 10.0),
        ];
        let frame = source.frame;
        if let Some(obj) = source.host_object_mut(id) {
            obj.movement.path = vec![Vec3::new(0.0, 0.0, 0.0)]
                .into_iter()
                .chain(remaining.iter().copied())
                .collect();
            obj.movement.current_path_index = 1;
            obj.movement.target_position = Some(remaining[0]);
            obj.requested_destination = Some(*remaining.last().unwrap());
            obj.pending_waypoint_labels = vec!["HeroPath".into()];
            obj.is_exact_path = true;
            obj.is_attack_path = true;
            obj.set_ai_state(
                AIState::AttackMoving,
                frame,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            obj.set_status_moving(true);
        }

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        match snapshot.objects.get(&id).map(|o| &o.object_type) {
            Some(ObjectTypeSnapshot::Unit(unit)) => {
                assert_eq!(
                    unit.waypoints.as_slice(),
                    remaining.as_slice(),
                    "UnitSnapshot.waypoints must keep the remaining route"
                );
            }
            other => panic!("expected unit snapshot, got {other:?}"),
        }

        let mut restored = ranger_logic();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(id).expect("restored ranger");
        // The full Object movement record owns the live route, including
        // traversed nodes and cursor. The remaining-only Unit/TMAI records
        // must not replace it during a complete-world restore.
        let original = source.host_object(id).expect("source ranger");
        assert_eq!(loaded.movement.path, original.movement.path);
        assert_eq!(
            loaded.movement.current_path_index,
            original.movement.current_path_index
        );
        assert_eq!(
            loaded.movement.target_position,
            original.movement.target_position
        );
        assert_eq!(
            loaded.requested_destination,
            Some(*remaining.last().unwrap())
        );
        assert_eq!(loaded.pending_waypoint_labels, vec!["HeroPath".to_string()]);
        assert!(loaded.is_exact_path);
        assert!(loaded.is_attack_path);
        assert_eq!(loaded.ai_state(), AIState::AttackMoving);
    }

    #[test]
    fn snapshot_round_trips_deferred_pathfind_queue() {
        let mut source = ranger_logic();
        let id = source
            .create_object("Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("ranger");
        let dest = Vec3::new(90.0, 0.0, 30.0);
        let via = vec![Vec3::new(30.0, 0.0, 10.0), Vec3::new(60.0, 0.0, 20.0)];
        let frame = source.frame;
        if let Some(obj) = source.host_object_mut(id) {
            obj.waiting_for_path = true;
            obj.requested_destination = Some(dest);
            obj.set_ai_state(
                AIState::Moving,
                frame,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            obj.set_status_moving(true);
        }
        source.restore_pending_host_paths([PendingHostPath {
            unit_id: id,
            start: Vec3::ZERO,
            destination: dest,
            waypoints: via.clone(),
            aircraft: false,
            surfaces: 1,
            is_crusher: false,
            ignore_obstacle: None,
            adjust_destinations: true,
            follow_path_appends: Vec::new(),
        }]);

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        let mut restored = ranger_logic();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(id).expect("restored ranger");
        assert!(loaded.waiting_for_path);
        assert_eq!(loaded.requested_destination, Some(dest));
        let queued = restored.snapshot_pending_host_paths();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].unit_id, id);
        assert_eq!(queued[0].destination, dest);
        assert_eq!(queued[0].waypoints, via);
    }

    #[test]
    fn snapshot_round_trips_script_sleep_attitude() {
        let mut source = ranger_logic();
        let id = source
            .create_object("Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("ranger");
        if let Some(obj) = source.host_object_mut(id) {
            // C++ AttitudeType::SLEEP = -2.
            obj.ai_attitude = -2;
        }

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        let mut restored = ranger_logic();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(id).expect("restored ranger");
        assert_eq!(
            loaded.ai_attitude, -2,
            "script Sleep attitude must survive load"
        );
    }

    #[test]
    fn snapshot_round_trips_ai_update_path_residuals() {
        use crate::game_logic::HUNT_CMD_FROM_PLAYER;

        let mut source = ranger_logic();
        let depot = source
            .create_object("USASupplyDepot", Team::USA, Vec3::new(40.0, 0.0, 10.0))
            .expect("depot");
        let id = source
            .create_object("Ranger", Team::USA, Vec3::new(8.0, 0.0, 4.0))
            .expect("ranger");
        let plant = Vec3::new(10.25, 0.0, 5.5);
        if let Some(obj) = source.host_object_mut(id) {
            // Mid-settle / mid-exit / mid-enter / mid-hunt: no remaining path.
            obj.do_final_position = true;
            obj.final_position = plant;
            obj.ignored_obstacle_id = Some(depot);
            obj.path_extra_distance = 80.0;
            obj.safe_path_repulsor2 = Some(ObjectId(4));
            obj.locomotor_goal_angle = 1.25;
            obj.next_mood_check_time = 240;
            obj.crate_created = Some(ObjectId(11));
            obj.desired_speed = 5.0;
            obj.move_away_from = Some(ObjectId(7));
            obj.move_away_from_2 = Some(ObjectId(3));
            obj.move_away_frames = 40;
            obj.locomotor_goal_type = crate::game_logic::LocoGoalType::Angle;
            obj.locomotor_goal_angle = 1.25;
            obj.can_path_through_units = true;
            obj.last_command_source = HUNT_CMD_FROM_PLAYER;
            obj.pending_move = Some(Vec3::new(12.0, 0.0, 8.0));
        }

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_tmai_suffix(&snapshot.lifecycle_tail).is_some(),
            "TMAI suffix must capture leftover AIUpdate path residuals"
        );

        let mut restored = ranger_logic();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(id).expect("restored ranger");
        assert!(
            loaded.do_final_position,
            "m_doFinalPosition must survive load"
        );
        assert_eq!(loaded.final_position, plant);
        assert_eq!(loaded.ignored_obstacle_id, Some(depot));
        assert!((loaded.path_extra_distance - 80.0).abs() < 0.01);
        assert_eq!(loaded.safe_path_repulsor2, Some(ObjectId(4)));
        assert!((loaded.locomotor_goal_angle - 1.25).abs() < 0.01);
        assert_eq!(loaded.next_mood_check_time, 240);
        assert_eq!(loaded.crate_created, Some(ObjectId(11)));
        assert!((loaded.desired_speed - 5.0).abs() < 0.01);
        assert_eq!(loaded.move_away_from, Some(ObjectId(7)));
        assert_eq!(loaded.move_away_from_2, Some(ObjectId(3)));
        assert_eq!(loaded.move_away_frames, 40);
        assert_eq!(
            loaded.locomotor_goal_type,
            crate::game_logic::LocoGoalType::Angle
        );
        assert!((loaded.locomotor_goal_angle - 1.25).abs() < 0.01);
        assert!(
            loaded.can_path_through_units,
            "m_canPathThroughUnits must survive load"
        );
        assert_eq!(
            loaded.last_command_source, HUNT_CMD_FROM_PLAYER,
            "m_lastCommandSource CMD_FROM_PLAYER must survive load"
        );
        assert_eq!(loaded.pending_move, Some(Vec3::new(12.0, 0.0, 8.0)));
    }
}

#[cfg(test)]
#[path = "movement_goal_tests.rs"]
mod movement_goal_tests;

#[cfg(test)]
#[path = "waypoint_identity_wire_tests.rs"]
mod waypoint_identity_wire_tests;
