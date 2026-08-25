//! Persist live team common-attack targets and AI waypoint / order state.
//!
//! C++ `Team::xfer` (`Team.cpp:2666`) writes `m_commonAttackTarget`. Live
//! host stores that residual in `GameLogic.team_common_attack_targets`
//! (`world_tick/mood.rs`). C++ `AIUpdateInterface::xfer`
//! (`AIUpdate.cpp:4995-5105`) writes waypoint IDs, the waypoint queue,
//! `m_path`, `m_requestedDestination`, `m_currentVictimID`, path flags,
//! `m_lastCommandSource`, `m_ignoreObstacleID`, `m_finalPosition` /
//! `m_doFinalPosition`, and `m_canPathThroughUnits`.
//! Live `ObjectSnapshot` clones `movement` (path + index + target) but
//! those AI-order residuals never left the live object.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No world snapshot version bump.

use crate::game_logic::pathfinding::PendingHostPath;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use glam::Vec3;
use serde::{Deserialize, Serialize};

const TMAI_MAGIC: &[u8; 4] = b"TMAI";
const TMAI_VERSION: u32 = 2;

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
    /// C++ `AIUpdateInterface::m_canPathThroughUnits`.
    #[serde(default)]
    can_path_through_units: bool,
    /// C++ `AIUpdateInterface::m_lastCommandSource` (CommandSourceType ordinal).
    #[serde(default = "default_last_command_source")]
    last_command_source: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectAiOrderPersistV1 {
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
    #[serde(default)]
    ai_attitude: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayload {
    team_targets: Vec<TeamCommonAttackPersist>,
    orders: Vec<ObjectAiOrderPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AiTeamPersistPayloadV1 {
    team_targets: Vec<TeamCommonAttackPersist>,
    orders: Vec<ObjectAiOrderPersistV1>,
}

fn default_last_command_source() -> u32 {
    crate::game_logic::HUNT_CMD_FROM_AI
}

impl From<ObjectAiOrderPersistV1> for ObjectAiOrderPersist {
    fn from(v1: ObjectAiOrderPersistV1) -> Self {
        Self {
            object_id: v1.object_id,
            path: v1.path,
            current_path_index: v1.current_path_index,
            target_position: v1.target_position,
            requested_destination: v1.requested_destination,
            requested_victim_id: v1.requested_victim_id,
            waiting_for_path: v1.waiting_for_path,
            is_exact_path: v1.is_exact_path,
            is_attack_path: v1.is_attack_path,
            is_approach_path: v1.is_approach_path,
            is_safe_path: v1.is_safe_path,
            pending_waypoint_labels: v1.pending_waypoint_labels,
            completed_waypoint_labels: v1.completed_waypoint_labels,
            queue_for_path_frames: v1.queue_for_path_frames,
            group_speed_factor: v1.group_speed_factor,
            pending_path: v1.pending_path,
            ai_attitude: v1.ai_attitude,
            do_final_position: false,
            final_position: [0.0; 3],
            ignored_obstacle_id: 0,
            can_path_through_units: false,
            last_command_source: default_last_command_source(),
        }
    }
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.team_targets.is_empty() && payload.orders.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(TMAI_MAGIC);
    append_u32(bytes, TMAI_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_tmai_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != 1 && version != TMAI_VERSION {
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
    let payload = if version == 1 {
        let old: AiTeamPersistPayloadV1 = bincode::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("TMAI payload decode: {err}")))?;
        AiTeamPersistPayload {
            team_targets: old.team_targets,
            orders: old
                .orders
                .into_iter()
                .map(ObjectAiOrderPersist::from)
                .collect(),
        }
    } else {
        bincode::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("TMAI payload decode: {err}")))?
    };
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> AiTeamPersistPayload {
    let mut team_targets: Vec<TeamCommonAttackPersist> = game_logic
        .team_common_attack_targets
        .iter()
        .map(|(team, id)| TeamCommonAttackPersist {
            team: team.clone(),
            target_id: id.0,
        })
        .collect();
    team_targets.sort_by(|a, b| a.team.cmp(&b.team));

    let pending: std::collections::HashMap<u32, PendingHostPath> = game_logic
        .snapshot_pending_host_paths()
        .into_iter()
        .map(|req| (req.unit_id.0, req))
        .collect();

    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
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
            || object.can_path_through_units
            || object.last_command_source != default_last_command_source();

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
            queue_for_path_frames: object.queue_for_path_frames,
            group_speed_factor: object.group_speed_factor,
            ai_attitude: object.ai_attitude,
            do_final_position: object.do_final_position,
            final_position: vec3_to_arr(&object.final_position),
            ignored_obstacle_id: object.ignored_obstacle_id.map(|v| v.0).unwrap_or(0),
            can_path_through_units: object.can_path_through_units,
            last_command_source: object.last_command_source,
            pending_path,
        });
    }

    AiTeamPersistPayload {
        team_targets,
        orders,
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: AiTeamPersistPayload) {
    game_logic.team_common_attack_targets.clear();
    for entry in payload.team_targets {
        if entry.team.is_empty() || entry.target_id == 0 {
            continue;
        }
        game_logic
            .team_common_attack_targets
            .insert(entry.team, ObjectId(entry.target_id));
    }

    let mut pending = Vec::new();
    for order in payload.orders {
        let id = ObjectId(order.object_id);
        if let Some(req) = order
            .pending_path
            .as_ref()
            .map(|p| restore_pending_path(id, p))
        {
            pending.push(req);
        }
        let Some(object) = game_logic.host_object_mut(id) else {
            continue;
        };
        let path: Vec<Vec3> = order.path.iter().copied().map(arr_to_vec3).collect();
        if !path.is_empty() {
            object.movement.path = path;
            object.movement.current_path_index = order.current_path_index as usize;
        }
        if let Some(pos) = order.target_position {
            object.movement.target_position = Some(arr_to_vec3(pos));
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
        object.queue_for_path_frames = order.queue_for_path_frames;
        object.group_speed_factor = order.group_speed_factor;
        object.ai_attitude = order.ai_attitude;
        object.do_final_position = order.do_final_position;
        object.final_position = arr_to_vec3(order.final_position);
        object.ignored_obstacle_id =
            (order.ignored_obstacle_id != 0).then_some(ObjectId(order.ignored_obstacle_id));
        object.can_path_through_units = order.can_path_through_units;
        object.last_command_source = order.last_command_source;
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
        aircraft: persist.aircraft,
        surfaces: persist.surfaces,
        is_crusher: persist.is_crusher,
        ignore_obstacle: (persist.ignore_obstacle != 0)
            .then_some(ObjectId(persist.ignore_obstacle)),
    }
}

fn vec3_to_arr(v: &Vec3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn arr_to_vec3(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}

fn find_tmai_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == TMAI_MAGIC)
        .map(|idx| &bytes[idx + 4..])
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
    use crate::game_logic::{AIState, Player, Team, ThingTemplate};
    use crate::save_load::snapshot::{ObjectTypeSnapshot, SnapshotBuilder};

    fn ranger_logic() -> GameLogic {
        let mut logic = GameLogic::new();
        let mut ranger = ThingTemplate::new("Ranger");
        ranger.set_health(100.0);
        logic.templates.insert("Ranger".into(), ranger);
        let mut depot = ThingTemplate::new("USASupplyDepot");
        depot.set_health(400.0);
        logic.templates.insert("USASupplyDepot".into(), depot);
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
        source
            .team_common_attack_targets
            .insert("USA_AttackSquad".into(), victim);

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_tmai_suffix(&snapshot.lifecycle_tail).is_some(),
            "TMAI suffix must be appended when a team has a common target"
        );

        let mut restored = ranger_logic();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        assert_eq!(
            restored.team_common_attack_targets.get("USA_AttackSquad"),
            Some(&victim),
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
            obj.set_ai_state(AIState::AttackMoving);
            obj.set_status_moving(true);
        }

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        match snapshot.objects.get(&id).map(|o| &o.object_type) {
            Some(ObjectTypeSnapshot::Unit(unit)) => {
                assert_eq!(
                    unit.waypoints, remaining,
                    "UnitSnapshot.waypoints must keep the remaining route"
                );
            }
            other => panic!("expected unit snapshot, got {other:?}"),
        }

        let mut restored = ranger_logic();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(id).expect("restored ranger");
        assert_eq!(loaded.movement.path, remaining);
        assert_eq!(loaded.movement.current_path_index, 0);
        assert_eq!(
            loaded.requested_destination,
            Some(*remaining.last().unwrap())
        );
        assert_eq!(loaded.pending_waypoint_labels, vec!["HeroPath".to_string()]);
        assert!(loaded.is_exact_path);
        assert!(loaded.is_attack_path);
        assert_eq!(loaded.ai_state, AIState::AttackMoving);
    }

    #[test]
    fn snapshot_round_trips_deferred_pathfind_queue() {
        let mut source = ranger_logic();
        let id = source
            .create_object("Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("ranger");
        let dest = Vec3::new(90.0, 0.0, 30.0);
        let via = vec![Vec3::new(30.0, 0.0, 10.0), Vec3::new(60.0, 0.0, 20.0)];
        if let Some(obj) = source.host_object_mut(id) {
            obj.waiting_for_path = true;
            obj.requested_destination = Some(dest);
            obj.set_ai_state(AIState::Moving);
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
        }]);

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        let mut restored = ranger_logic();
        restored.templates = source.templates.clone();
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
        restored.templates = source.templates.clone();
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
            obj.can_path_through_units = true;
            obj.last_command_source = HUNT_CMD_FROM_PLAYER;
        }

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_tmai_suffix(&snapshot.lifecycle_tail).is_some(),
            "TMAI suffix must capture leftover AIUpdate path residuals"
        );

        let mut restored = ranger_logic();
        restored.templates = source.templates.clone();
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
        assert!(
            loaded.can_path_through_units,
            "m_canPathThroughUnits must survive load"
        );
        assert_eq!(
            loaded.last_command_source, HUNT_CMD_FROM_PLAYER,
            "m_lastCommandSource CMD_FROM_PLAYER must survive load"
        );
    }

    #[test]
    fn v1_suffix_defaults_new_ai_update_residuals() {
        let mut logic = ranger_logic();
        let id = logic
            .create_object("Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("ranger");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.do_final_position = true;
            obj.final_position = Vec3::new(1.0, 2.0, 3.0);
            obj.ignored_obstacle_id = Some(ObjectId(99));
            obj.can_path_through_units = true;
            obj.last_command_source = crate::game_logic::HUNT_CMD_FROM_PLAYER;
        }

        let v1 = AiTeamPersistPayloadV1 {
            team_targets: Vec::new(),
            orders: vec![ObjectAiOrderPersistV1 {
                object_id: id.0,
                ai_attitude: 0,
                ..Default::default()
            }],
        };
        let encoded = bincode::serialize(&v1).expect("v1 encode");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(TMAI_MAGIC);
        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);

        apply_from_lifecycle_tail(&bytes, &mut logic).expect("apply v1");
        let loaded = logic.host_object(id).expect("ranger");
        assert!(
            !loaded.do_final_position,
            "v1 saves default m_doFinalPosition false"
        );
        assert_eq!(loaded.final_position, Vec3::ZERO);
        assert_eq!(loaded.ignored_obstacle_id, None);
        assert!(!loaded.can_path_through_units);
        assert_eq!(
            loaded.last_command_source,
            crate::game_logic::HUNT_CMD_FROM_AI
        );
    }
}
