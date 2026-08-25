//! Persist leftover AIPlayer team queues, build timers, and BuildList remaining rebuilds.
//!
//! C++ `AIPlayer::xfer` (`AI/AIPlayer.cpp:3278-3446`) writes TeamBuildQueue +
//! TeamReadyQueue `TeamInQueue` snapshots (work orders numRequired/numCompleted),
//! readyToBuild flags, teamTimer/structureTimer/buildDelay/teamDelay, warehouse
//! ID, repair-dozer, and structuresToRepair. Leftover `team_in_queue.rs` /
//! `work_order.rs` already match that table. C++ / leftover `Player::xfer` also
//! writes each `BuildListInfo.num_rebuilds` remaining count,
//! `BuildListInfo.object_timestamp` rebuild-delay clock, `m_objectID`,
//! `m_underConstruction`, and `m_priorityBuild`. Live spends remaining rebuilds
//! as `AIBuildingInfo.rebuild_count`, the clock as `destroyed_at_time`, and the
//! pad binding as `object_id` / `is_built` / `is_priority`.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes queues/clocks, remaining rebuild counts, rebuild-delay
//! timestamps, and pad object bindings; it never re-runs selectTeamToBuild or
//! rebuilds the INI layout.

use crate::ai::{AITeamQueue, AIWorkOrder};
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const AITQ_MAGIC: &[u8; 4] = b"AITQ";
const AITQ_VERSION: u32 = 5;
const AITQ_VERSION_V4: u32 = 4;
const AITQ_VERSION_V3: u32 = 3;
const AITQ_VERSION_V2: u32 = 2;
const AITQ_VERSION_V1: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AIPlayerQueuePersistPayload {
    players: Vec<AIPlayerQueuePersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AIPlayerQueuePersistPayloadV1 {
    players: Vec<AIPlayerQueuePersistV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AIPlayerQueuePersistPayloadV2 {
    players: Vec<AIPlayerQueuePersistV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AIPlayerQueuePersistPayloadV3 {
    players: Vec<AIPlayerQueuePersistV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AIPlayerQueuePersistPayloadV4 {
    players: Vec<AIPlayerQueuePersistV4>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIPlayerQueuePersist {
    pub player_id: u32,
    pub team_queue: Vec<AITeamQueuePersist>,
    pub team_ready_queue: Vec<AITeamQueuePersist>,
    pub next_building_time: f32,
    pub next_team_queue_time: f32,
    pub next_team_time: f32,
    pub team_seconds: f32,
    pub last_update_time: f32,
    pub current_warehouse_id: Option<u32>,
    pub repair_dozer: Option<u32>,
    pub repair_dozer_origin: [f32; 3],
    pub structures_to_repair: Vec<u32>,
    pub dozer_queued_for_repair: bool,
    pub dozer_is_repairing: bool,
    pub last_bridge_repair_time: f32,
    pub skillset_selector: i32,
    pub cur_front_base_defense: i32,
    pub cur_flank_base_defense: i32,
    pub cur_front_left_defense_angle: f32,
    pub cur_front_right_defense_angle: f32,
    pub cur_left_flank_left_defense_angle: f32,
    pub cur_left_flank_right_defense_angle: f32,
    pub cur_right_flank_left_defense_angle: f32,
    pub cur_right_flank_right_defense_angle: f32,
    /// Live `AIBuildingInfo.rebuild_count` per pad, leftover remaining spend.
    pub building_rebuild_counts: Vec<u32>,
    /// Live `AIBuildingInfo.destroyed_at_time` per pad, leftover object_timestamp.
    pub building_destroyed_at_times: Vec<Option<f32>>,
    /// Live `AIBuildingInfo.object_id` per pad, leftover `BuildListInfo.object_id`.
    pub building_object_ids: Vec<Option<u32>>,
    /// Live `AIBuildingInfo.is_built` per pad, leftover `!under_construction` when bound.
    pub building_is_built: Vec<bool>,
    /// Live `AIBuildingInfo.is_priority` per pad, leftover `priority_build`.
    pub building_is_priority: Vec<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AIPlayerQueuePersistV1 {
    player_id: u32,
    team_queue: Vec<AITeamQueuePersist>,
    team_ready_queue: Vec<AITeamQueuePersist>,
    next_building_time: f32,
    next_team_queue_time: f32,
    next_team_time: f32,
    team_seconds: f32,
    last_update_time: f32,
    current_warehouse_id: Option<u32>,
    repair_dozer: Option<u32>,
    repair_dozer_origin: [f32; 3],
    structures_to_repair: Vec<u32>,
    dozer_queued_for_repair: bool,
    dozer_is_repairing: bool,
    last_bridge_repair_time: f32,
    skillset_selector: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AIPlayerQueuePersistV2 {
    player_id: u32,
    team_queue: Vec<AITeamQueuePersist>,
    team_ready_queue: Vec<AITeamQueuePersist>,
    next_building_time: f32,
    next_team_queue_time: f32,
    next_team_time: f32,
    team_seconds: f32,
    last_update_time: f32,
    current_warehouse_id: Option<u32>,
    repair_dozer: Option<u32>,
    repair_dozer_origin: [f32; 3],
    structures_to_repair: Vec<u32>,
    dozer_queued_for_repair: bool,
    dozer_is_repairing: bool,
    last_bridge_repair_time: f32,
    skillset_selector: i32,
    cur_front_base_defense: i32,
    cur_flank_base_defense: i32,
    cur_front_left_defense_angle: f32,
    cur_front_right_defense_angle: f32,
    cur_left_flank_left_defense_angle: f32,
    cur_left_flank_right_defense_angle: f32,
    cur_right_flank_left_defense_angle: f32,
    cur_right_flank_right_defense_angle: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AIPlayerQueuePersistV3 {
    player_id: u32,
    team_queue: Vec<AITeamQueuePersist>,
    team_ready_queue: Vec<AITeamQueuePersist>,
    next_building_time: f32,
    next_team_queue_time: f32,
    next_team_time: f32,
    team_seconds: f32,
    last_update_time: f32,
    current_warehouse_id: Option<u32>,
    repair_dozer: Option<u32>,
    repair_dozer_origin: [f32; 3],
    structures_to_repair: Vec<u32>,
    dozer_queued_for_repair: bool,
    dozer_is_repairing: bool,
    last_bridge_repair_time: f32,
    skillset_selector: i32,
    cur_front_base_defense: i32,
    cur_flank_base_defense: i32,
    cur_front_left_defense_angle: f32,
    cur_front_right_defense_angle: f32,
    cur_left_flank_left_defense_angle: f32,
    cur_left_flank_right_defense_angle: f32,
    cur_right_flank_left_defense_angle: f32,
    cur_right_flank_right_defense_angle: f32,
    building_rebuild_counts: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AIPlayerQueuePersistV4 {
    player_id: u32,
    team_queue: Vec<AITeamQueuePersist>,
    team_ready_queue: Vec<AITeamQueuePersist>,
    next_building_time: f32,
    next_team_queue_time: f32,
    next_team_time: f32,
    team_seconds: f32,
    last_update_time: f32,
    current_warehouse_id: Option<u32>,
    repair_dozer: Option<u32>,
    repair_dozer_origin: [f32; 3],
    structures_to_repair: Vec<u32>,
    dozer_queued_for_repair: bool,
    dozer_is_repairing: bool,
    last_bridge_repair_time: f32,
    skillset_selector: i32,
    cur_front_base_defense: i32,
    cur_flank_base_defense: i32,
    cur_front_left_defense_angle: f32,
    cur_front_right_defense_angle: f32,
    cur_left_flank_left_defense_angle: f32,
    cur_left_flank_right_defense_angle: f32,
    cur_right_flank_left_defense_angle: f32,
    cur_right_flank_right_defense_angle: f32,
    building_rebuild_counts: Vec<u32>,
    building_destroyed_at_times: Vec<Option<f32>>,
}

impl From<AIPlayerQueuePersistV1> for AIPlayerQueuePersist {
    fn from(old: AIPlayerQueuePersistV1) -> Self {
        Self {
            player_id: old.player_id,
            team_queue: old.team_queue,
            team_ready_queue: old.team_ready_queue,
            next_building_time: old.next_building_time,
            next_team_queue_time: old.next_team_queue_time,
            next_team_time: old.next_team_time,
            team_seconds: old.team_seconds,
            last_update_time: old.last_update_time,
            current_warehouse_id: old.current_warehouse_id,
            repair_dozer: old.repair_dozer,
            repair_dozer_origin: old.repair_dozer_origin,
            structures_to_repair: old.structures_to_repair,
            dozer_queued_for_repair: old.dozer_queued_for_repair,
            dozer_is_repairing: old.dozer_is_repairing,
            last_bridge_repair_time: old.last_bridge_repair_time,
            skillset_selector: old.skillset_selector,
            cur_front_base_defense: 0,
            cur_flank_base_defense: 0,
            cur_front_left_defense_angle: 0.0,
            cur_front_right_defense_angle: 0.0,
            cur_left_flank_left_defense_angle: 0.0,
            cur_left_flank_right_defense_angle: 0.0,
            cur_right_flank_left_defense_angle: 0.0,
            cur_right_flank_right_defense_angle: 0.0,
            building_rebuild_counts: Vec::new(),
            building_destroyed_at_times: Vec::new(),
            building_object_ids: Vec::new(),
            building_is_built: Vec::new(),
            building_is_priority: Vec::new(),
        }
    }
}

impl From<AIPlayerQueuePersistV2> for AIPlayerQueuePersist {
    fn from(old: AIPlayerQueuePersistV2) -> Self {
        Self {
            player_id: old.player_id,
            team_queue: old.team_queue,
            team_ready_queue: old.team_ready_queue,
            next_building_time: old.next_building_time,
            next_team_queue_time: old.next_team_queue_time,
            next_team_time: old.next_team_time,
            team_seconds: old.team_seconds,
            last_update_time: old.last_update_time,
            current_warehouse_id: old.current_warehouse_id,
            repair_dozer: old.repair_dozer,
            repair_dozer_origin: old.repair_dozer_origin,
            structures_to_repair: old.structures_to_repair,
            dozer_queued_for_repair: old.dozer_queued_for_repair,
            dozer_is_repairing: old.dozer_is_repairing,
            last_bridge_repair_time: old.last_bridge_repair_time,
            skillset_selector: old.skillset_selector,
            cur_front_base_defense: old.cur_front_base_defense,
            cur_flank_base_defense: old.cur_flank_base_defense,
            cur_front_left_defense_angle: old.cur_front_left_defense_angle,
            cur_front_right_defense_angle: old.cur_front_right_defense_angle,
            cur_left_flank_left_defense_angle: old.cur_left_flank_left_defense_angle,
            cur_left_flank_right_defense_angle: old.cur_left_flank_right_defense_angle,
            cur_right_flank_left_defense_angle: old.cur_right_flank_left_defense_angle,
            cur_right_flank_right_defense_angle: old.cur_right_flank_right_defense_angle,
            building_rebuild_counts: Vec::new(),
            building_destroyed_at_times: Vec::new(),
            building_object_ids: Vec::new(),
            building_is_built: Vec::new(),
            building_is_priority: Vec::new(),
        }
    }
}

impl From<AIPlayerQueuePersistV3> for AIPlayerQueuePersist {
    fn from(old: AIPlayerQueuePersistV3) -> Self {
        Self {
            player_id: old.player_id,
            team_queue: old.team_queue,
            team_ready_queue: old.team_ready_queue,
            next_building_time: old.next_building_time,
            next_team_queue_time: old.next_team_queue_time,
            next_team_time: old.next_team_time,
            team_seconds: old.team_seconds,
            last_update_time: old.last_update_time,
            current_warehouse_id: old.current_warehouse_id,
            repair_dozer: old.repair_dozer,
            repair_dozer_origin: old.repair_dozer_origin,
            structures_to_repair: old.structures_to_repair,
            dozer_queued_for_repair: old.dozer_queued_for_repair,
            dozer_is_repairing: old.dozer_is_repairing,
            last_bridge_repair_time: old.last_bridge_repair_time,
            skillset_selector: old.skillset_selector,
            cur_front_base_defense: old.cur_front_base_defense,
            cur_flank_base_defense: old.cur_flank_base_defense,
            cur_front_left_defense_angle: old.cur_front_left_defense_angle,
            cur_front_right_defense_angle: old.cur_front_right_defense_angle,
            cur_left_flank_left_defense_angle: old.cur_left_flank_left_defense_angle,
            cur_left_flank_right_defense_angle: old.cur_left_flank_right_defense_angle,
            cur_right_flank_left_defense_angle: old.cur_right_flank_left_defense_angle,
            cur_right_flank_right_defense_angle: old.cur_right_flank_right_defense_angle,
            building_rebuild_counts: old.building_rebuild_counts,
            building_destroyed_at_times: Vec::new(),
            building_object_ids: Vec::new(),
            building_is_built: Vec::new(),
            building_is_priority: Vec::new(),
        }
    }
}

impl From<AIPlayerQueuePersistV4> for AIPlayerQueuePersist {
    fn from(old: AIPlayerQueuePersistV4) -> Self {
        Self {
            player_id: old.player_id,
            team_queue: old.team_queue,
            team_ready_queue: old.team_ready_queue,
            next_building_time: old.next_building_time,
            next_team_queue_time: old.next_team_queue_time,
            next_team_time: old.next_team_time,
            team_seconds: old.team_seconds,
            last_update_time: old.last_update_time,
            current_warehouse_id: old.current_warehouse_id,
            repair_dozer: old.repair_dozer,
            repair_dozer_origin: old.repair_dozer_origin,
            structures_to_repair: old.structures_to_repair,
            dozer_queued_for_repair: old.dozer_queued_for_repair,
            dozer_is_repairing: old.dozer_is_repairing,
            last_bridge_repair_time: old.last_bridge_repair_time,
            skillset_selector: old.skillset_selector,
            cur_front_base_defense: old.cur_front_base_defense,
            cur_flank_base_defense: old.cur_flank_base_defense,
            cur_front_left_defense_angle: old.cur_front_left_defense_angle,
            cur_front_right_defense_angle: old.cur_front_right_defense_angle,
            cur_left_flank_left_defense_angle: old.cur_left_flank_left_defense_angle,
            cur_left_flank_right_defense_angle: old.cur_left_flank_right_defense_angle,
            cur_right_flank_left_defense_angle: old.cur_right_flank_left_defense_angle,
            cur_right_flank_right_defense_angle: old.cur_right_flank_right_defense_angle,
            building_rebuild_counts: old.building_rebuild_counts,
            building_destroyed_at_times: old.building_destroyed_at_times,
            building_object_ids: Vec::new(),
            building_is_built: Vec::new(),
            building_is_priority: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AITeamQueuePersist {
    pub name: String,
    pub team_id: Option<u32>,
    pub work_orders: Vec<AIWorkOrderPersist>,
    pub priority_build: bool,
    pub frame_started: u32,
    pub completed: bool,
    pub execute_actions: bool,
    pub sent_to_start_location: bool,
    pub activated: bool,
    pub reinforcement: bool,
    pub reinforcement_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIWorkOrderPersist {
    pub template_name: String,
    pub factory_id: Option<u32>,
    pub queued_count: u32,
    pub num_completed: u32,
    pub num_required: u32,
    pub is_required: bool,
    pub priority: u32,
    pub observed_unit_ids: Vec<u32>,
    pub is_resource_gatherer: bool,
    pub supply_center_id: Option<u32>,
}

impl AITeamQueuePersist {
    pub fn from_live(team: &AITeamQueue) -> Self {
        Self {
            name: team.name.clone(),
            team_id: team.team_id,
            work_orders: team
                .work_orders
                .iter()
                .map(AIWorkOrderPersist::from_live)
                .collect(),
            priority_build: team.priority_build,
            frame_started: team.frame_started,
            completed: team.completed,
            execute_actions: team.execute_actions,
            sent_to_start_location: team.sent_to_start_location,
            activated: team.activated,
            reinforcement: team.reinforcement,
            reinforcement_id: team.reinforcement_id.map(|id| id.0),
        }
    }

    pub fn into_live(self) -> AITeamQueue {
        AITeamQueue {
            name: self.name,
            team_id: self.team_id,
            work_orders: self
                .work_orders
                .into_iter()
                .map(AIWorkOrderPersist::into_live)
                .collect(),
            priority_build: self.priority_build,
            frame_started: self.frame_started,
            completed: self.completed,
            execute_actions: self.execute_actions,
            sent_to_start_location: self.sent_to_start_location,
            activated: self.activated,
            reinforcement: self.reinforcement,
            reinforcement_id: self.reinforcement_id.map(ObjectId),
        }
    }
}

impl AIWorkOrderPersist {
    pub fn from_live(order: &AIWorkOrder) -> Self {
        Self {
            template_name: order.template_name.clone(),
            factory_id: order.factory_id.map(|id| id.0),
            queued_count: order.queued_count,
            num_completed: order.num_completed,
            num_required: order.num_required,
            is_required: order.is_required,
            priority: order.priority,
            observed_unit_ids: order.observed_unit_ids.iter().map(|id| id.0).collect(),
            is_resource_gatherer: order.is_resource_gatherer,
            supply_center_id: order.supply_center_id.map(|id| id.0),
        }
    }

    pub fn into_live(self) -> AIWorkOrder {
        AIWorkOrder {
            template_name: self.template_name,
            factory_id: self.factory_id.map(ObjectId),
            queued_count: self.queued_count,
            num_completed: self.num_completed,
            num_required: self.num_required,
            is_required: self.is_required,
            priority: self.priority,
            observed_unit_ids: self.observed_unit_ids.into_iter().map(ObjectId).collect(),
            is_resource_gatherer: self.is_resource_gatherer,
            supply_center_id: self.supply_center_id.map(ObjectId),
        }
    }
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.players.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(AITQ_MAGIC);
    append_u32(bytes, AITQ_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    game_logic.clear_ai_player_queue_persist();
    let Some(suffix) = find_aitq_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != AITQ_VERSION
        && version != AITQ_VERSION_V4
        && version != AITQ_VERSION_V3
        && version != AITQ_VERSION_V2
        && version != AITQ_VERSION_V1
    {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown AITQ suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "AITQ payload truncated".to_string(),
        ));
    }
    let encoded = &rest[..payload_len];
    let payload = if version == AITQ_VERSION_V1 {
        let old: AIPlayerQueuePersistPayloadV1 = bincode::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("AITQ v1 payload decode: {err}")))?;
        AIPlayerQueuePersistPayload {
            players: old
                .players
                .into_iter()
                .map(AIPlayerQueuePersist::from)
                .collect(),
        }
    } else if version == AITQ_VERSION_V2 {
        let old: AIPlayerQueuePersistPayloadV2 = bincode::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("AITQ v2 payload decode: {err}")))?;
        AIPlayerQueuePersistPayload {
            players: old
                .players
                .into_iter()
                .map(AIPlayerQueuePersist::from)
                .collect(),
        }
    } else if version == AITQ_VERSION_V3 {
        let old: AIPlayerQueuePersistPayloadV3 = bincode::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("AITQ v3 payload decode: {err}")))?;
        AIPlayerQueuePersistPayload {
            players: old
                .players
                .into_iter()
                .map(AIPlayerQueuePersist::from)
                .collect(),
        }
    } else if version == AITQ_VERSION_V4 {
        let old: AIPlayerQueuePersistPayloadV4 = bincode::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("AITQ v4 payload decode: {err}")))?;
        AIPlayerQueuePersistPayload {
            players: old
                .players
                .into_iter()
                .map(AIPlayerQueuePersist::from)
                .collect(),
        }
    } else {
        bincode::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("AITQ payload decode: {err}")))?
    };
    game_logic.apply_ai_player_queue_persist(payload.players);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> AIPlayerQueuePersistPayload {
    AIPlayerQueuePersistPayload {
        players: game_logic.capture_ai_player_queue_persist(),
    }
}

fn find_aitq_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == AITQ_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("AITQ u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AIDifficulty;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use crate::save_load::snapshot::SnapshotBuilder;
    use glam::Vec3;

    fn china_ai_logic() -> GameLogic {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "ChinaTankBattleMaster".into(),
            ThingTemplate::new("ChinaTankBattleMaster"),
        );
        logic
            .templates
            .insert("ChinaDozer".into(), ThingTemplate::new("ChinaDozer"));
        logic.templates.insert(
            "ChinaSupplyWarehouse".into(),
            ThingTemplate::new("ChinaSupplyWarehouse"),
        );
        logic.add_player(Player::new(1, Team::China, "China AI", false));
        logic.add_ai_opponent(1, Team::China, AIDifficulty::Hard);
        logic
    }

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_aitq_suffix(b"no-magic-here").is_none());
    }

    #[test]
    fn snapshot_round_trips_team_build_queue_and_timers() {
        let mut source = china_ai_logic();
        let factory = source
            .create_object(
                "ChinaTankBattleMaster",
                Team::China,
                Vec3::new(40.0, 0.0, 10.0),
            )
            .expect("factory");
        let dozer = source
            .create_object("ChinaDozer", Team::China, Vec3::new(8.0, 0.0, 8.0))
            .expect("dozer");
        let warehouse = source
            .create_object(
                "ChinaSupplyWarehouse",
                Team::China,
                Vec3::new(80.0, 0.0, 20.0),
            )
            .expect("warehouse");
        source.apply_ai_player_queue_persist(vec![AIPlayerQueuePersist {
            player_id: 1,
            team_queue: vec![AITeamQueuePersist {
                name: "China_TankSquad".into(),
                team_id: Some(7),
                work_orders: vec![AIWorkOrderPersist {
                    template_name: "ChinaTankBattleMaster".into(),
                    factory_id: Some(factory.0),
                    queued_count: 1,
                    num_completed: 1,
                    num_required: 3,
                    is_required: true,
                    priority: 50,
                    observed_unit_ids: Vec::new(),
                    is_resource_gatherer: false,
                    supply_center_id: None,
                }],
                priority_build: true,
                frame_started: 120,
                completed: false,
                execute_actions: true,
                sent_to_start_location: true,
                activated: false,
                reinforcement: false,
                reinforcement_id: None,
            }],
            team_ready_queue: vec![AITeamQueuePersist {
                name: "China_ReadyDozer".into(),
                team_id: Some(8),
                work_orders: vec![AIWorkOrderPersist {
                    template_name: "ChinaDozer".into(),
                    factory_id: None,
                    queued_count: 0,
                    num_completed: 1,
                    num_required: 1,
                    is_required: true,
                    priority: 10,
                    observed_unit_ids: Vec::new(),
                    is_resource_gatherer: false,
                    supply_center_id: None,
                }],
                priority_build: false,
                frame_started: 30,
                completed: true,
                execute_actions: false,
                sent_to_start_location: true,
                activated: true,
                reinforcement: false,
                reinforcement_id: None,
            }],
            next_building_time: 8.5,
            next_team_queue_time: 3.0,
            next_team_time: 11.0,
            team_seconds: 10.0,
            cur_front_base_defense: 3,
            cur_flank_base_defense: 2,
            cur_front_left_defense_angle: 0.5,
            cur_front_right_defense_angle: -0.25,
            cur_left_flank_left_defense_angle: 1.1,
            cur_left_flank_right_defense_angle: -1.2,
            cur_right_flank_left_defense_angle: 0.75,
            cur_right_flank_right_defense_angle: -0.8,
            last_update_time: 40.0,
            current_warehouse_id: Some(warehouse.0),
            repair_dozer: Some(dozer.0),
            repair_dozer_origin: [1.0, 0.0, 2.0],
            structures_to_repair: vec![factory.0],
            dozer_queued_for_repair: true,
            dozer_is_repairing: true,
            last_bridge_repair_time: 22.0,
            skillset_selector: 2,
            building_rebuild_counts: vec![2, 1, 0],
            building_destroyed_at_times: vec![Some(12.5), None, Some(3.0)],
            building_object_ids: vec![Some(factory.0), None, Some(dozer.0)],
            building_is_built: vec![true, false, false],
            building_is_priority: vec![false, true, false],
        }]);

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_aitq_suffix(&snapshot.lifecycle_tail).is_some(),
            "AITQ suffix must be appended when AI queues are live"
        );

        let mut restored = china_ai_logic();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let persisted = restored.capture_ai_player_queue_persist();
        let row = persisted
            .iter()
            .find(|row| row.player_id == 1)
            .expect("persisted row");
        assert_eq!(row.team_queue.len(), 1);
        assert_eq!(row.team_queue[0].name, "China_TankSquad");
        assert_eq!(row.team_queue[0].team_id, Some(7));
        assert!(row.team_queue[0].priority_build);
        assert_eq!(row.team_queue[0].work_orders.len(), 1);
        assert_eq!(row.team_queue[0].work_orders[0].num_completed, 1);
        assert_eq!(row.team_queue[0].work_orders[0].num_required, 3);
        assert_eq!(row.team_queue[0].work_orders[0].factory_id, Some(factory.0));
        assert_eq!(row.team_ready_queue.len(), 1);
        assert_eq!(row.team_ready_queue[0].name, "China_ReadyDozer");
        assert!(row.team_ready_queue[0].completed);
        assert!((row.next_building_time - 8.5).abs() < 1e-4);
        assert!((row.next_team_queue_time - 3.0).abs() < 1e-4);
        assert_eq!(row.cur_front_base_defense, 3);
        assert_eq!(row.cur_flank_base_defense, 2);
        assert!((row.cur_front_left_defense_angle - 0.5).abs() < 1e-4);
        assert!((row.cur_front_right_defense_angle + 0.25).abs() < 1e-4);
        assert!((row.cur_left_flank_left_defense_angle - 1.1).abs() < 1e-4);
        assert!((row.cur_left_flank_right_defense_angle + 1.2).abs() < 1e-4);
        assert!((row.cur_right_flank_left_defense_angle - 0.75).abs() < 1e-4);
        assert!((row.cur_right_flank_right_defense_angle + 0.8).abs() < 1e-4);
        assert!((row.next_team_time - 11.0).abs() < 1e-4);
        assert!((row.team_seconds - 10.0).abs() < 1e-4);
        assert_eq!(row.repair_dozer, Some(dozer.0));
        assert_eq!(row.current_warehouse_id, Some(warehouse.0));
        assert_eq!(row.structures_to_repair, vec![factory.0]);
        assert!(row.dozer_queued_for_repair);
        assert!(row.dozer_is_repairing);
        assert!((row.last_bridge_repair_time - 22.0).abs() < 1e-4);
        assert_eq!(row.skillset_selector, 2);
        assert!(row.building_rebuild_counts.len() >= 3);
        assert_eq!(&row.building_rebuild_counts[..3], &[2, 1, 0]);
        assert!(row.building_destroyed_at_times.len() >= 3);
        assert_eq!(
            &row.building_destroyed_at_times[..3],
            &[Some(12.5), None, Some(3.0)]
        );
        assert!(row.building_object_ids.len() >= 3);
        assert_eq!(
            &row.building_object_ids[..3],
            &[Some(factory.0), None, Some(dozer.0)]
        );
        assert!(row.building_is_built.len() >= 3);
        assert_eq!(&row.building_is_built[..3], &[true, false, false]);
        assert!(row.building_is_priority.len() >= 3);
        assert_eq!(&row.building_is_priority[..3], &[false, true, false]);
    }

    #[test]
    fn v1_suffix_loads_queues_and_leaves_defense_fan_at_zero() {
        let mut logic = china_ai_logic();
        let v1 = AIPlayerQueuePersistPayloadV1 {
            players: vec![AIPlayerQueuePersistV1 {
                player_id: 1,
                team_queue: vec![AITeamQueuePersist {
                    name: "LegacyTeam".into(),
                    team_id: Some(4),
                    work_orders: Vec::new(),
                    priority_build: false,
                    frame_started: 0,
                    completed: false,
                    execute_actions: false,
                    sent_to_start_location: false,
                    activated: false,
                    reinforcement: false,
                    reinforcement_id: None,
                }],
                team_ready_queue: Vec::new(),
                next_building_time: 4.0,
                next_team_queue_time: 1.0,
                next_team_time: 2.0,
                team_seconds: 10.0,
                last_update_time: 0.0,
                current_warehouse_id: None,
                repair_dozer: None,
                repair_dozer_origin: [0.0, 0.0, 0.0],
                structures_to_repair: Vec::new(),
                dozer_queued_for_repair: false,
                dozer_is_repairing: false,
                last_bridge_repair_time: -1.0,
                skillset_selector: 0,
            }],
        };
        let encoded = bincode::serialize(&v1).expect("encode v1");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(AITQ_MAGIC);
        append_u32(&mut bytes, AITQ_VERSION_V1);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("apply v1");
        let row = logic
            .capture_ai_player_queue_persist()
            .into_iter()
            .find(|row| row.player_id == 1)
            .expect("row");
        assert_eq!(row.team_queue[0].name, "LegacyTeam");
        assert!((row.next_building_time - 4.0).abs() < 1e-4);
        assert_eq!(row.cur_front_base_defense, 0);
        assert_eq!(row.cur_flank_base_defense, 0);
    }

    #[test]
    fn snapshot_round_trips_spent_rebuild_counts() {
        let mut source = china_ai_logic();
        let mut rows = source.capture_ai_player_queue_persist();
        {
            let row = rows
                .iter_mut()
                .find(|row| row.player_id == 1)
                .expect("china persist row");
            assert!(
                row.building_rebuild_counts.len() >= 2,
                "layout pads exist to spend rebuilds"
            );
            row.building_rebuild_counts[0] = 3;
            row.building_rebuild_counts[1] = 1;
        }
        source.apply_ai_player_queue_persist(rows);

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        let mut restored = china_ai_logic();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let row = restored
            .capture_ai_player_queue_persist()
            .into_iter()
            .find(|row| row.player_id == 1)
            .expect("restored row");
        assert_eq!(row.building_rebuild_counts[0], 3);
        assert_eq!(row.building_rebuild_counts[1], 1);
        assert!(
            row.building_rebuild_counts
                .iter()
                .skip(2)
                .all(|count| *count == 0),
            "unspent pads stay at INI remaining"
        );
    }

    #[test]
    fn snapshot_round_trips_rebuild_delay_timestamps() {
        let mut source = china_ai_logic();
        let mut rows = source.capture_ai_player_queue_persist();
        {
            let row = rows
                .iter_mut()
                .find(|row| row.player_id == 1)
                .expect("china persist row");
            assert!(
                row.building_destroyed_at_times.len() >= 2,
                "layout pads exist to stamp rebuild delay"
            );
            row.building_destroyed_at_times[0] = Some(18.0);
            row.building_destroyed_at_times[1] = Some(4.5);
        }
        source.apply_ai_player_queue_persist(rows);

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        let mut restored = china_ai_logic();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let row = restored
            .capture_ai_player_queue_persist()
            .into_iter()
            .find(|row| row.player_id == 1)
            .expect("restored row");
        assert_eq!(row.building_destroyed_at_times[0], Some(18.0));
        assert_eq!(row.building_destroyed_at_times[1], Some(4.5));
        assert!(
            row.building_destroyed_at_times
                .iter()
                .skip(2)
                .all(|stamp| stamp.is_none()),
            "unstamped pads stay without a rebuild-delay clock"
        );
    }

    #[test]
    fn v2_suffix_loads_queues_and_leaves_rebuild_counts_empty() {
        let mut logic = china_ai_logic();
        let v2 = AIPlayerQueuePersistPayloadV2 {
            players: vec![AIPlayerQueuePersistV2 {
                player_id: 1,
                team_queue: vec![AITeamQueuePersist {
                    name: "LegacyV2".into(),
                    team_id: Some(5),
                    work_orders: Vec::new(),
                    priority_build: false,
                    frame_started: 0,
                    completed: false,
                    execute_actions: false,
                    sent_to_start_location: false,
                    activated: false,
                    reinforcement: false,
                    reinforcement_id: None,
                }],
                team_ready_queue: Vec::new(),
                next_building_time: 4.0,
                next_team_queue_time: 1.0,
                next_team_time: 2.0,
                team_seconds: 10.0,
                last_update_time: 0.0,
                current_warehouse_id: None,
                repair_dozer: None,
                repair_dozer_origin: [0.0, 0.0, 0.0],
                structures_to_repair: Vec::new(),
                dozer_queued_for_repair: false,
                dozer_is_repairing: false,
                last_bridge_repair_time: -1.0,
                skillset_selector: 0,
                cur_front_base_defense: 1,
                cur_flank_base_defense: 0,
                cur_front_left_defense_angle: 0.0,
                cur_front_right_defense_angle: 0.0,
                cur_left_flank_left_defense_angle: 0.0,
                cur_left_flank_right_defense_angle: 0.0,
                cur_right_flank_left_defense_angle: 0.0,
                cur_right_flank_right_defense_angle: 0.0,
            }],
        };
        let encoded = bincode::serialize(&v2).expect("encode v2");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(AITQ_MAGIC);
        append_u32(&mut bytes, AITQ_VERSION_V2);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("apply v2");
        let row = logic
            .capture_ai_player_queue_persist()
            .into_iter()
            .find(|row| row.player_id == 1)
            .expect("row");
        assert_eq!(row.team_queue[0].name, "LegacyV2");
        assert_eq!(row.cur_front_base_defense, 1);
        assert!(
            row.building_rebuild_counts.iter().all(|count| *count == 0),
            "v2 saves have no remaining-rebuild table"
        );
        assert!(
            row.building_destroyed_at_times
                .iter()
                .all(|stamp| stamp.is_none()),
            "v2 saves have no rebuild-delay clock table"
        );
        assert!(
            row.building_object_ids.iter().all(|id| id.is_none()),
            "v2 saves have no pad object-id table"
        );
    }

    #[test]
    fn v3_suffix_loads_rebuild_counts_and_leaves_timestamps_empty() {
        let mut logic = china_ai_logic();
        let v3 = AIPlayerQueuePersistPayloadV3 {
            players: vec![AIPlayerQueuePersistV3 {
                player_id: 1,
                team_queue: vec![AITeamQueuePersist {
                    name: "LegacyV3".into(),
                    team_id: Some(6),
                    work_orders: Vec::new(),
                    priority_build: false,
                    frame_started: 0,
                    completed: false,
                    execute_actions: false,
                    sent_to_start_location: false,
                    activated: false,
                    reinforcement: false,
                    reinforcement_id: None,
                }],
                team_ready_queue: Vec::new(),
                next_building_time: 4.0,
                next_team_queue_time: 1.0,
                next_team_time: 2.0,
                team_seconds: 10.0,
                last_update_time: 0.0,
                current_warehouse_id: None,
                repair_dozer: None,
                repair_dozer_origin: [0.0, 0.0, 0.0],
                structures_to_repair: Vec::new(),
                dozer_queued_for_repair: false,
                dozer_is_repairing: false,
                last_bridge_repair_time: -1.0,
                skillset_selector: 0,
                cur_front_base_defense: 1,
                cur_flank_base_defense: 0,
                cur_front_left_defense_angle: 0.0,
                cur_front_right_defense_angle: 0.0,
                cur_left_flank_left_defense_angle: 0.0,
                cur_left_flank_right_defense_angle: 0.0,
                cur_right_flank_left_defense_angle: 0.0,
                cur_right_flank_right_defense_angle: 0.0,
                building_rebuild_counts: vec![2, 1],
            }],
        };
        let encoded = bincode::serialize(&v3).expect("encode v3");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(AITQ_MAGIC);
        append_u32(&mut bytes, AITQ_VERSION_V3);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("apply v3");
        let row = logic
            .capture_ai_player_queue_persist()
            .into_iter()
            .find(|row| row.player_id == 1)
            .expect("row");
        assert_eq!(row.team_queue[0].name, "LegacyV3");
        assert_eq!(&row.building_rebuild_counts[..2], &[2, 1]);
        assert!(
            row.building_destroyed_at_times
                .iter()
                .all(|stamp| stamp.is_none()),
            "v3 saves have no rebuild-delay clock table"
        );
        assert!(
            row.building_object_ids.iter().all(|id| id.is_none()),
            "v3 saves have no pad object-id table"
        );
    }

    #[test]
    fn snapshot_round_trips_build_list_object_binding() {
        let mut source = china_ai_logic();
        source.templates.insert(
            "ChinaPowerPlant".into(),
            ThingTemplate::new("ChinaPowerPlant"),
        );
        let plant = source
            .create_object("ChinaPowerPlant", Team::China, Vec3::new(-50.0, 0.0, 0.0))
            .expect("power plant");
        if let Some(object) = source.host_object_mut(plant) {
            object.status.under_construction = true;
        }
        let mut rows = source.capture_ai_player_queue_persist();
        {
            let row = rows
                .iter_mut()
                .find(|row| row.player_id == 1)
                .expect("china persist row");
            assert!(
                row.building_object_ids.len() >= 3,
                "layout pads exist to bind"
            );
            // China layout: [CC, Supply, Power, Barracks, Factory, Propaganda, Airfield]
            row.building_object_ids[2] = Some(plant.0);
            row.building_is_built[2] = false;
            row.building_is_priority[1] = true;
            row.building_is_built[0] = true;
        }
        source.apply_ai_player_queue_persist(rows);

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        let mut restored = china_ai_logic();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let row = restored
            .capture_ai_player_queue_persist()
            .into_iter()
            .find(|row| row.player_id == 1)
            .expect("restored row");
        assert_eq!(row.building_object_ids[2], Some(plant.0));
        assert!(!row.building_is_built[2]);
        assert!(row.building_is_priority[1]);
        assert!(row.building_is_built[0]);
        assert!(
            restored
                .host_object(plant)
                .is_some_and(|object| object.status.under_construction),
            "scaffold status must survive so resume_interrupted_construction can find it"
        );
    }

    #[test]
    fn v4_suffix_loads_timestamps_and_leaves_bindings_empty() {
        let mut logic = china_ai_logic();
        let v4 = AIPlayerQueuePersistPayloadV4 {
            players: vec![AIPlayerQueuePersistV4 {
                player_id: 1,
                team_queue: vec![AITeamQueuePersist {
                    name: "LegacyV4".into(),
                    team_id: Some(7),
                    work_orders: Vec::new(),
                    priority_build: false,
                    frame_started: 0,
                    completed: false,
                    execute_actions: false,
                    sent_to_start_location: false,
                    activated: false,
                    reinforcement: false,
                    reinforcement_id: None,
                }],
                team_ready_queue: Vec::new(),
                next_building_time: 4.0,
                next_team_queue_time: 1.0,
                next_team_time: 2.0,
                team_seconds: 10.0,
                last_update_time: 0.0,
                current_warehouse_id: None,
                repair_dozer: None,
                repair_dozer_origin: [0.0, 0.0, 0.0],
                structures_to_repair: Vec::new(),
                dozer_queued_for_repair: false,
                dozer_is_repairing: false,
                last_bridge_repair_time: -1.0,
                skillset_selector: 0,
                cur_front_base_defense: 1,
                cur_flank_base_defense: 0,
                cur_front_left_defense_angle: 0.0,
                cur_front_right_defense_angle: 0.0,
                cur_left_flank_left_defense_angle: 0.0,
                cur_left_flank_right_defense_angle: 0.0,
                cur_right_flank_left_defense_angle: 0.0,
                cur_right_flank_right_defense_angle: 0.0,
                building_rebuild_counts: vec![2, 1],
                building_destroyed_at_times: vec![Some(9.0), None],
            }],
        };
        let encoded = bincode::serialize(&v4).expect("encode v4");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(AITQ_MAGIC);
        append_u32(&mut bytes, AITQ_VERSION_V4);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("apply v4");
        let row = logic
            .capture_ai_player_queue_persist()
            .into_iter()
            .find(|row| row.player_id == 1)
            .expect("row");
        assert_eq!(row.team_queue[0].name, "LegacyV4");
        assert_eq!(&row.building_rebuild_counts[..2], &[2, 1]);
        assert_eq!(row.building_destroyed_at_times[0], Some(9.0));
        assert!(
            row.building_object_ids.iter().all(|id| id.is_none()),
            "v4 saves have no pad object-id table"
        );
        assert!(
            row.building_is_built.iter().all(|built| !*built),
            "v4 saves have no is_built table"
        );
        assert!(
            row.building_is_priority.iter().all(|priority| !*priority),
            "v4 saves have no priority stamp table"
        );
    }

    #[test]
    fn retain_drops_stale_pad_object_ids() {
        let mut logic = china_ai_logic();
        let mut rows = logic.capture_ai_player_queue_persist();
        {
            let row = rows.iter_mut().find(|row| row.player_id == 1).expect("row");
            row.building_object_ids[0] = Some(9999);
            row.building_is_built[0] = true;
        }
        logic.apply_ai_player_queue_persist(rows);
        let row = logic
            .capture_ai_player_queue_persist()
            .into_iter()
            .find(|row| row.player_id == 1)
            .expect("row");
        assert!(
            row.building_object_ids[0].is_none(),
            "missing world objects must not stay bound after retain"
        );
        assert!(
            !row.building_is_built[0],
            "stale binding must not keep is_built"
        );
    }
}
