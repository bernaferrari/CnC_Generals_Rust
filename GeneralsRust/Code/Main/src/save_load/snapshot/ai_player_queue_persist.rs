//! Persist leftover AIPlayer team queues and build timers.
//!
//! C++ `AIPlayer::xfer` (`AI/AIPlayer.cpp:3278-3446`) writes TeamBuildQueue +
//! TeamReadyQueue `TeamInQueue` snapshots (work orders numRequired/numCompleted),
//! readyToBuild flags, teamTimer/structureTimer/buildDelay/teamDelay, warehouse
//! ID, repair-dozer, and structuresToRepair. Leftover `team_in_queue.rs` /
//! `work_order.rs` already match that table. Live `restore_players_from_save`
//! zeroes those queues and timers — a mid-wave skirmish load discarded
//! partially built teams and reset cadence.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes queues/clocks only; it never re-runs selectTeamToBuild.

use crate::ai::{AITeamQueue, AIWorkOrder};
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const AITQ_MAGIC: &[u8; 4] = b"AITQ";
const AITQ_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AIPlayerQueuePersistPayload {
    players: Vec<AIPlayerQueuePersist>,
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

pub fn apply_from_lifecycle_tail(
    bytes: &[u8],
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    game_logic.clear_ai_player_queue_persist();
    let Some(suffix) = find_aitq_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != AITQ_VERSION {
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
    let payload: AIPlayerQueuePersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("AITQ payload decode: {err}")))?;
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
        logic.templates.insert(
            "ChinaDozer".into(),
            ThingTemplate::new("ChinaDozer"),
        );
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
            last_update_time: 40.0,
            current_warehouse_id: Some(warehouse.0),
            repair_dozer: Some(dozer.0),
            repair_dozer_origin: [1.0, 0.0, 2.0],
            structures_to_repair: vec![factory.0],
            dozer_queued_for_repair: true,
            dozer_is_repairing: true,
            last_bridge_repair_time: 22.0,
            skillset_selector: 2,
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
        assert!((row.next_team_time - 11.0).abs() < 1e-4);
        assert!((row.team_seconds - 10.0).abs() < 1e-4);
        assert_eq!(row.repair_dozer, Some(dozer.0));
        assert_eq!(row.current_warehouse_id, Some(warehouse.0));
        assert_eq!(row.structures_to_repair, vec![factory.0]);
        assert!(row.dozer_queued_for_repair);
        assert!(row.dozer_is_repairing);
        assert!((row.last_bridge_repair_time - 22.0).abs() < 1e-4);
        assert_eq!(row.skillset_selector, 2);
    }
}
