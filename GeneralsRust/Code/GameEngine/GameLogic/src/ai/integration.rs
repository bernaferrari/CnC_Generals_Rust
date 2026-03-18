//! AI Integration Module
//!
//! This module provides the integration layer between the enhanced AI systems
//! and the existing GameLogic framework, ensuring seamless interoperation
//! between modern Rust AI components and legacy systems.
//!
//! Author: Created by Claude for AI system integration

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant};

use crate::common::types::{Coord3D, Real};
use crate::common::KindOf;
use crate::common::ObjectID;
use crate::helpers::TheGameLogic;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{player_list, GameDifficulty, Player};
use crate::system::game_logic::get_game_logic;
use crate::terrain::get_terrain_logic;
use crate::world::World;

use super::ai_player::AIPlayer;
use super::ai_update::AiPlayerTrait;
use super::groups::{AiUnitGroup, FormationType, GroupBehavior, UnitRole};
use super::native::WaypointGraph;
use super::pathfind_complete::{PathRequest, PathResult};
use super::skirmish_player::AISkirmishPlayer;
use super::states::{AIStateMachine, AIStateType};
use super::{AiError, THE_AI};

fn state_from_id(state_id: u32) -> Option<AIStateType> {
    let match_id = |state: AIStateType| (state_id == state as u32).then_some(state);

    match_id(AIStateType::Idle)
        .or_else(|| match_id(AIStateType::MoveTo))
        .or_else(|| match_id(AIStateType::FollowWaypointPathAsTeam))
        .or_else(|| match_id(AIStateType::FollowWaypointPathAsIndividuals))
        .or_else(|| match_id(AIStateType::FollowWaypointPathAsTeamExact))
        .or_else(|| match_id(AIStateType::FollowWaypointPathAsIndividualsExact))
        .or_else(|| match_id(AIStateType::FollowPath))
        .or_else(|| match_id(AIStateType::FollowExitProductionPath))
        .or_else(|| match_id(AIStateType::Wait))
        .or_else(|| match_id(AIStateType::AttackPosition))
        .or_else(|| match_id(AIStateType::AttackObject))
        .or_else(|| match_id(AIStateType::ForceAttackObject))
        .or_else(|| match_id(AIStateType::AttackAndFollowObject))
        .or_else(|| match_id(AIStateType::Dead))
        .or_else(|| match_id(AIStateType::Dock))
        .or_else(|| match_id(AIStateType::Enter))
        .or_else(|| match_id(AIStateType::Guard))
        .or_else(|| match_id(AIStateType::Hunt))
        .or_else(|| match_id(AIStateType::Wander))
        .or_else(|| match_id(AIStateType::Panic))
        .or_else(|| match_id(AIStateType::AttackSquad))
        .or_else(|| match_id(AIStateType::GuardTunnelNetwork))
        .or_else(|| match_id(AIStateType::GetRepaired))
        .or_else(|| match_id(AIStateType::MoveOutOfTheWay))
        .or_else(|| match_id(AIStateType::MoveAndTighten))
        .or_else(|| match_id(AIStateType::MoveAndEvacuate))
        .or_else(|| match_id(AIStateType::MoveAndEvacuateAndExit))
        .or_else(|| match_id(AIStateType::MoveAndDelete))
        .or_else(|| match_id(AIStateType::AttackArea))
        .or_else(|| match_id(AIStateType::HackInternet))
        .or_else(|| match_id(AIStateType::AttackMoveTo))
        .or_else(|| match_id(AIStateType::AttackFollowWaypointPathAsIndividuals))
        .or_else(|| match_id(AIStateType::AttackFollowWaypointPathAsTeam))
        .or_else(|| match_id(AIStateType::FaceObject))
        .or_else(|| match_id(AIStateType::FacePosition))
        .or_else(|| match_id(AIStateType::RappelInto))
        .or_else(|| match_id(AIStateType::CombatDrop))
        .or_else(|| match_id(AIStateType::Exit))
        .or_else(|| match_id(AIStateType::PickUpCrate))
        .or_else(|| match_id(AIStateType::MoveAwayFromRepulsors))
        .or_else(|| match_id(AIStateType::WanderInPlace))
        .or_else(|| match_id(AIStateType::Busy))
        .or_else(|| match_id(AIStateType::ExitInstantly))
        .or_else(|| match_id(AIStateType::GuardRetaliate))
}

pub enum IntegratedAiPlayer {
    Standard(AIPlayer),
    Skirmish(AISkirmishPlayer),
}

impl IntegratedAiPlayer {
    pub fn update(&mut self) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => player.update(),
            IntegratedAiPlayer::Skirmish(player) => {
                player.update();
                Ok(())
            }
        }
    }

    pub fn set_difficulty(&mut self, difficulty: GameDifficulty) {
        match self {
            IntegratedAiPlayer::Standard(player) => player.set_ai_difficulty(difficulty),
            IntegratedAiPlayer::Skirmish(player) => player.set_ai_difficulty(difficulty),
        }
    }

    pub fn select_skillset(&mut self, skillset: i32) {
        match self {
            IntegratedAiPlayer::Standard(player) => player.select_skillset(skillset),
            IntegratedAiPlayer::Skirmish(player) => player.select_skillset(skillset),
        }
    }

    pub fn set_team_delay_seconds(&mut self, delay_seconds: f32) {
        match self {
            IntegratedAiPlayer::Standard(player) => player.set_team_delay_seconds(delay_seconds),
            IntegratedAiPlayer::Skirmish(player) => player.set_team_delay_seconds(delay_seconds),
        }
    }

    pub fn build_base_defense(&mut self, flank: bool) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => player.build_ai_base_defense(flank),
            IntegratedAiPlayer::Skirmish(player) => player.build_base_defense(flank),
        }
    }

    pub fn build_base_defense_structure(
        &mut self,
        structure_name: &str,
        flank: bool,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_ai_base_defense_structure(structure_name, flank)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.build_base_defense_structure(structure_name, flank)
            }
        }
    }

    pub fn build_specific_building(&mut self, building_name: &str) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_specific_ai_building(building_name)
            }
            IntegratedAiPlayer::Skirmish(player) => player.build_specific_building(building_name),
        }
    }

    pub fn build_specific_ai_team(
        &mut self,
        team_name: &str,
        priority_build: bool,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_specific_ai_team(team_name, priority_build)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.build_specific_ai_team_by_name(team_name, priority_build);
                Ok(())
            }
        }
    }

    pub fn recruit_specific_ai_team(
        &mut self,
        team_name: &str,
        recruit_radius: Real,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.recruit_specific_ai_team(team_name, recruit_radius)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.recruit_specific_ai_team_by_name(team_name, recruit_radius);
                Ok(())
            }
        }
    }

    pub fn build_by_supplies(
        &mut self,
        minimum_cash: i32,
        building_name: &str,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_by_supplies(minimum_cash, building_name)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.build_by_supplies(minimum_cash, building_name)
            }
        }
    }

    pub fn build_upgrade(&mut self, upgrade_name: &str) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => player.build_upgrade(upgrade_name),
            IntegratedAiPlayer::Skirmish(player) => player.build_upgrade(upgrade_name),
        }
    }

    pub fn build_specific_building_near_location(
        &mut self,
        building_name: &str,
        location: Coord3D,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_specific_building_near_location(building_name, location)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.build_specific_building_near_location(building_name, location)
            }
        }
    }

    pub fn repair_structure(&mut self, structure_id: ObjectID) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => player.repair_structure(structure_id),
            IntegratedAiPlayer::Skirmish(player) => player.repair_structure(structure_id),
        }
    }

    pub fn on_structure_produced(
        &mut self,
        factory_id: ObjectID,
        structure_id: ObjectID,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.on_structure_produced(factory_id, structure_id)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.on_structure_produced(factory_id, structure_id)
            }
        }
    }
}

/// AI Integration Manager - Coordinates all AI subsystems
pub struct AiIntegrationManager {
    /// AI players by player ID (C++-faithful player controllers)
    ai_players: HashMap<u32, IntegratedAiPlayer>,
    /// AI unit groups by group ID
    unit_groups: HashMap<u32, Arc<RwLock<AiUnitGroup>>>,
    /// Object state machines by object ID
    object_state_machines: HashMap<ObjectID, Arc<RwLock<AIStateMachine>>>,
    /// Shared waypoint graph for native pathing
    waypoint_graph: Option<Arc<RwLock<WaypointGraph>>>,
    /// Completed pathfinding results keyed by requester object ID
    path_results: HashMap<ObjectID, PathResult>,
    /// Cached player handles for AI integration
    player_handles: HashMap<u32, Weak<RwLock<Player>>>,
    /// Next available group ID
    next_group_id: u32,
    /// Performance metrics
    performance_stats: AiPerformanceStats,
    /// Last update time
    last_update: Instant,
    /// Frame start timestamp used for diagnostics
    frame_start: Option<Instant>,
}

/// Performance statistics for AI integration
#[derive(Debug, Default)]
pub struct AiPerformanceStats {
    pathfinding_requests_per_frame: u32,
    state_machine_updates_per_frame: u32,
    group_updates_per_frame: u32,
    player_updates_per_frame: u32,
    average_frame_time: Duration,
    total_ai_objects: u32,
}

impl AiIntegrationManager {
    /// Create new AI integration manager
    pub fn new() -> Self {
        Self {
            ai_players: HashMap::new(),
            unit_groups: HashMap::new(),
            object_state_machines: HashMap::new(),
            waypoint_graph: None,
            path_results: HashMap::new(),
            player_handles: HashMap::new(),
            next_group_id: 1,
            performance_stats: AiPerformanceStats::default(),
            last_update: Instant::now(),
            frame_start: None,
        }
    }

    /// Initialize AI integration systems
    pub fn initialize(&mut self) -> Result<(), AiError> {
        // Reset all systems
        self.ai_players.clear();
        self.unit_groups.clear();
        self.object_state_machines.clear();
        self.waypoint_graph = None;
        self.path_results.clear();
        self.player_handles.clear();
        self.next_group_id = 1;
        self.frame_start = None;

        log::info!("AI Integration Manager initialized");
        Ok(())
    }

    /// Run AI sensing phase for the current frame.
    pub fn sense(&mut self, _world: &World, _delta: Duration) -> Result<(), AiError> {
        self.frame_start = Some(Instant::now());
        self.performance_stats = AiPerformanceStats::default();
        Ok(())
    }

    /// Run AI decision making for the current frame.
    pub fn decide(&mut self, frame_time: Instant) -> Result<(), AiError> {
        self.update_ai_players()?;
        let _ = frame_time;
        Ok(())
    }

    /// Execute AI actions for the current frame.
    pub fn execute(&mut self, frame_time: Instant) -> Result<(), AiError> {
        if let Some(start) = self.frame_start.take() {
            self.performance_stats.average_frame_time = start.elapsed();
        }
        self.performance_stats.total_ai_objects = self.object_state_machines.len() as u32;
        self.last_update = frame_time;
        Ok(())
    }

    /// Update all AI systems for one frame (legacy helper).
    pub fn update(
        &mut self,
        world: &World,
        delta: Duration,
        frame_time: Instant,
    ) -> Result<(), AiError> {
        self.sense(world, delta)?;
        self.decide(frame_time)?;
        self.execute(frame_time)
    }

    fn attach_waypoint_graph(&self, _state_machine: &Arc<RwLock<AIStateMachine>>) {}

    fn create_classic_state_machine(
        &self,
        object_id: ObjectID,
        name: &str,
    ) -> Result<Arc<RwLock<AIStateMachine>>, AiError> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Err(AiError::InvalidObject);
        };
        let state_machine = Arc::new(RwLock::new(AIStateMachine::new(
            Arc::downgrade(&obj_arc),
            name,
        )));
        self.attach_waypoint_graph(&state_machine);
        Ok(state_machine)
    }

    /// Create enhanced AI player for given player
    pub fn create_ai_player(&mut self, player_id: u32) -> Result<(), AiError> {
        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| {
                list.get_player(player_id as crate::player::PlayerIndex)
                    .cloned()
            })
            .ok_or(AiError::InvalidObject)?;
        let player_weak = Arc::downgrade(&player_arc);
        self.player_handles.insert(player_id, player_weak.clone());

        let is_skirmish = get_game_logic()
            .lock()
            .map(|logic| logic.is_in_skirmish_game())
            .unwrap_or(false);

        let ai_player = if is_skirmish {
            IntegratedAiPlayer::Skirmish(AISkirmishPlayer::new(player_id))
        } else {
            IntegratedAiPlayer::Standard(AIPlayer::new(player_id))
        };
        self.ai_players.insert(player_id, ai_player);

        log::info!("Created AI player for player {}", player_id);
        Ok(())
    }

    /// Remove AI player
    pub fn remove_ai_player(&mut self, player_id: u32) -> Result<(), AiError> {
        if self.ai_players.remove(&player_id).is_some() {
            log::info!("Removed AI player {}", player_id);
            Ok(())
        } else {
            Err(AiError::InvalidObject)
        }
    }

    /// Set AI player difficulty
    pub fn set_ai_player_difficulty(
        &mut self,
        player_id: u32,
        difficulty: GameDifficulty,
    ) -> Result<(), AiError> {
        if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
            ai_player.set_difficulty(difficulty);
            Ok(())
        } else {
            Err(AiError::InvalidObject)
        }
    }

    pub fn with_ai_player_mut<F, R>(&mut self, player_id: u32, f: F) -> Option<R>
    where
        F: FnOnce(&mut IntegratedAiPlayer) -> R,
    {
        let ai_player = self.ai_players.get_mut(&player_id)?;
        Some(f(ai_player))
    }

    /// Notify AI players about a new map load.
    pub fn new_map(&mut self) -> Result<(), AiError> {
        for ai_player in self.ai_players.values_mut() {
            match ai_player {
                IntegratedAiPlayer::Standard(player) => player.new_map(),
                IntegratedAiPlayer::Skirmish(player) => player.new_map(),
            }
        }

        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    if let Ok(terrain) = get_terrain_logic().read() {
                        pf.rebuild_from_terrain(&terrain);
                    }

                    for obj_arc in OBJECT_REGISTRY.get_all_objects() {
                        let Ok(obj_guard) = obj_arc.read() else {
                            continue;
                        };
                        if obj_guard.is_kind_of(KindOf::Structure)
                            || obj_guard.is_kind_of(KindOf::Building)
                            || obj_guard.is_kind_of(KindOf::Bridge)
                            || obj_guard.is_kind_of(KindOf::Barrier)
                        {
                            pf.create_wall_from_object(&obj_guard);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Create AI unit group
    pub fn create_unit_group(&mut self, name: String, player_id: u32) -> Result<u32, AiError> {
        let group_id = self.next_group_id;
        self.next_group_id += 1;

        let group = Arc::new(RwLock::new(AiUnitGroup::new(group_id, name, None)));

        self.unit_groups.insert(group_id, group);

        log::info!(
            "Created AI unit group {} for player {}",
            group_id,
            player_id
        );
        Ok(group_id)
    }

    /// Add unit to AI group
    pub fn add_unit_to_group(
        &mut self,
        group_id: u32,
        object_id: ObjectID,
        role: UnitRole,
    ) -> Result<(), AiError> {
        if let Some(group) = self.unit_groups.get(&group_id) {
            if let Ok(mut g) = group.write() {
                g.add_unit(object_id, role)?;

                // Create state machine for this object if it doesn't exist
                if !self.object_state_machines.contains_key(&object_id) {
                    let state_machine = self
                        .create_classic_state_machine(object_id, &format!("Unit_{}", object_id))?;
                    self.object_state_machines.insert(object_id, state_machine);
                }

                Ok(())
            } else {
                Err(AiError::InvalidObject)
            }
        } else {
            Err(AiError::InvalidObject)
        }
    }

    /// Remove unit from AI group
    pub fn remove_unit_from_group(
        &mut self,
        group_id: u32,
        object_id: ObjectID,
    ) -> Result<(), AiError> {
        let should_destroy_group = {
            if let Some(group) = self.unit_groups.get(&group_id) {
                if let Ok(mut g) = group.write() {
                    g.remove_unit(object_id)?
                } else {
                    return Err(AiError::InvalidObject);
                }
            } else {
                return Err(AiError::InvalidObject);
            }
        };

        // Remove object's state machine
        self.object_state_machines.remove(&object_id);

        // Destroy group if empty (separate borrow scope)
        if should_destroy_group {
            self.unit_groups.remove(&group_id);
            log::info!("Destroyed empty AI unit group {}", group_id);
        }

        Ok(())
    }

    /// Set group formation
    pub fn set_group_formation(
        &mut self,
        group_id: u32,
        formation: FormationType,
        spacing: Real,
    ) -> Result<(), AiError> {
        if let Some(group) = self.unit_groups.get(&group_id) {
            if let Ok(mut g) = group.write() {
                g.set_formation(formation, spacing)?;
                Ok(())
            } else {
                Err(AiError::InvalidObject)
            }
        } else {
            Err(AiError::InvalidObject)
        }
    }

    /// Command group to move to position
    pub fn command_group_move(&mut self, group_id: u32, position: Coord3D) -> Result<(), AiError> {
        if let Some(group) = self.unit_groups.get(&group_id) {
            if let Ok(mut g) = group.write() {
                g.move_to_position(position)?;
                Ok(())
            } else {
                Err(AiError::InvalidObject)
            }
        } else {
            Err(AiError::InvalidObject)
        }
    }

    /// Command group to attack target
    pub fn command_group_attack(
        &mut self,
        group_id: u32,
        target_id: ObjectID,
    ) -> Result<(), AiError> {
        if let Some(group) = self.unit_groups.get(&group_id) {
            if let Ok(mut g) = group.write() {
                g.attack_target(target_id)?;
                Ok(())
            } else {
                Err(AiError::InvalidObject)
            }
        } else {
            Err(AiError::InvalidObject)
        }
    }

    /// Command group to guard position
    pub fn command_group_guard(
        &mut self,
        group_id: u32,
        position: Coord3D,
        radius: Real,
    ) -> Result<(), AiError> {
        if let Some(group) = self.unit_groups.get(&group_id) {
            if let Ok(mut g) = group.write() {
                g.guard_position(position, radius)?;
                Ok(())
            } else {
                Err(AiError::InvalidObject)
            }
        } else {
            Err(AiError::InvalidObject)
        }
    }

    /// Set object AI state directly
    pub fn set_object_state(
        &mut self,
        object_id: ObjectID,
        state: AIStateType,
    ) -> Result<(), AiError> {
        if !self.object_state_machines.contains_key(&object_id) {
            // Create state machine if it doesn't exist
            let state_machine =
                self.create_classic_state_machine(object_id, &format!("Object_{}", object_id))?;
            self.object_state_machines.insert(object_id, state_machine);
        }

        if let Some(state_machine) = self.object_state_machines.get(&object_id) {
            if let Ok(mut sm) = state_machine.write() {
                let _ = sm.set_state(state as u32);
                match sm.get_current_state_id().and_then(state_from_id) {
                    Some(current) if current == state => Ok(()),
                    _ => Err(AiError::InvalidCommand),
                }
            } else {
                Err(AiError::InvalidObject)
            }
        } else {
            Err(AiError::InvalidObject)
        }
    }

    /// Get object AI state
    pub fn get_object_state(&self, object_id: ObjectID) -> Option<AIStateType> {
        self.object_state_machines.get(&object_id).and_then(|sm| {
            sm.read()
                .ok()
                .and_then(|s| s.get_current_state_id())
                .and_then(state_from_id)
        })
    }

    /// Add object to pathfinding map as obstacle
    pub fn add_pathfinding_obstacle(
        &self,
        object_id: ObjectID,
        positions: &[Coord3D],
        is_fence: bool,
    ) -> Result<(), AiError> {
        let Some(ai) = THE_AI.read().ok() else {
            return Err(AiError::NoPathfinder);
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return Err(AiError::NoPathfinder);
        };
        let pathfinder_lock = pathfinder.write();
        if let Ok(mut pf) = pathfinder_lock {
            pf.add_object_to_map(object_id, positions, is_fence);
            Ok(())
        } else {
            Err(AiError::NoPathfinder)
        }
    }

    /// Remove object from pathfinding map
    pub fn remove_pathfinding_obstacle(
        &self,
        object_id: ObjectID,
        positions: &[Coord3D],
    ) -> Result<(), AiError> {
        let Some(ai) = THE_AI.read().ok() else {
            return Err(AiError::NoPathfinder);
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return Err(AiError::NoPathfinder);
        };
        let pathfinder_lock = pathfinder.write();
        if let Ok(mut pf) = pathfinder_lock {
            pf.remove_object_from_map(object_id, positions);
            Ok(())
        } else {
            Err(AiError::NoPathfinder)
        }
    }

    /// Request pathfinding between two points
    pub fn request_pathfinding(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        acceptable_surfaces: u32,
        is_crusher: bool,
    ) -> Result<Option<Vec<Coord3D>>, AiError> {
        let Some(ai) = THE_AI.read().ok() else {
            return Err(AiError::NoPathfinder);
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return Err(AiError::NoPathfinder);
        };
        let pathfinder_lock = pathfinder.read();
        if let Ok(pf) = pathfinder_lock {
            Ok(pf.find_path(from, to, acceptable_surfaces, is_crusher))
        } else {
            Err(AiError::NoPathfinder)
        }
    }

    /// Integration with legacy AI system - handles object updates
    pub fn notify_object_created(
        &mut self,
        object_id: ObjectID,
        position: Coord3D,
        is_ai_controlled: bool,
        is_obstacle: bool,
    ) -> Result<(), AiError> {
        if is_ai_controlled {
            let state_machine =
                self.create_classic_state_machine(object_id, &format!("Object_{}", object_id))?;
            self.object_state_machines.insert(object_id, state_machine);
        }

        if is_obstacle {
            if let Ok(ai_guard) = THE_AI.read() {
                if let Some(pathfinder) = ai_guard.pathfinder() {
                    if let Ok(mut pf) = pathfinder.write() {
                        if let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) {
                            if let Ok(obj_guard) = obj_arc.read() {
                                pf.create_wall_from_object(&obj_guard);
                                return Ok(());
                            }
                        }
                    }
                }
            }
            let positions = vec![position];
            self.add_pathfinding_obstacle(object_id, &positions, false)?;
        }

        Ok(())
    }

    /// Handle object destruction
    pub fn notify_object_destroyed(
        &mut self,
        object_id: ObjectID,
        positions: &[Coord3D],
    ) -> Result<(), AiError> {
        // Remove state machine
        self.object_state_machines.remove(&object_id);

        // Remove from pathfinding map
        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    if let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) {
                        if let Ok(obj_guard) = obj_arc.read() {
                            pf.remove_wall_from_object(&obj_guard);
                            return Ok(());
                        }
                    }
                }
            }
        }
        self.remove_pathfinding_obstacle(object_id, positions)?;

        // Remove from any groups
        let groups_to_check: Vec<u32> = self.unit_groups.keys().cloned().collect();
        for group_id in groups_to_check {
            let should_remove = {
                if let Some(group) = self.unit_groups.get(&group_id) {
                    if let Ok(group_read) = group.read() {
                        group_read.contains_unit(object_id)
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            if should_remove {
                self.remove_unit_from_group(group_id, object_id)?;
            }
        }

        Ok(())
    }

    /// Integration with team system - create group from team
    pub fn create_group_from_team(
        &mut self,
        team_name: String,
        team_members: Vec<ObjectID>,
        player_id: u32,
    ) -> Result<u32, AiError> {
        let group_id = self.create_unit_group(team_name, player_id)?;

        // Add all team members to the group
        for member_id in team_members {
            // Determine unit role based on object type
            let role = self.determine_unit_role(member_id);
            self.add_unit_to_group(group_id, member_id, role)?;
        }

        Ok(group_id)
    }

    // Internal helper methods

    /// Update pathfinding system
    fn update_pathfinding_system(&mut self) -> Result<(), AiError> {
        self.performance_stats.pathfinding_requests_per_frame = 0;
        Ok(())
    }

    /// Update all AI players
    fn update_ai_players(&mut self) -> Result<(), AiError> {
        for (player_id, ai_player) in &mut self.ai_players {
            if let Err(e) = ai_player.update() {
                log::warn!("Failed to update AI player {}: {:?}", player_id, e);
            }
        }
        self.performance_stats.player_updates_per_frame = self.ai_players.len() as u32;
        Ok(())
    }

    /// Update AI players without running sensing/decision pipelines.
    pub fn update_ai_players_only(&mut self) -> Result<(), AiError> {
        self.update_ai_players()
    }

    /// Update all unit groups
    fn update_unit_groups(&mut self, frame_time: Instant) -> Result<(), AiError> {
        let _ = frame_time;
        self.performance_stats.group_updates_per_frame = 0;
        Ok(())
    }

    /// Update all object state machines
    fn update_object_state_machines(&mut self) -> Result<(), AiError> {
        self.performance_stats.state_machine_updates_per_frame = 0;
        Ok(())
    }

    // Helper methods removed - functionality moved to public interface

    /// Determine unit role based on object ID/type
    fn determine_unit_role(&self, object_id: ObjectID) -> UnitRole {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return UnitRole::DamageDealer;
        };
        let Ok(obj) = obj_arc.read() else {
            return UnitRole::DamageDealer;
        };

        if obj.is_kind_of(KindOf::Hero) {
            return UnitRole::Leader;
        }
        if obj.is_kind_of(KindOf::Dozer)
            || obj.is_kind_of(KindOf::Hacker)
            || obj.is_kind_of(KindOf::Saboteur)
            || obj.is_kind_of(KindOf::Salvager)
            || obj.is_kind_of(KindOf::WeaponSalvager)
            || obj.is_kind_of(KindOf::ArmorSalvager)
        {
            return UnitRole::Support;
        }
        if obj.is_kind_of(KindOf::Drone) {
            return UnitRole::Scout;
        }
        if obj.is_kind_of(KindOf::Aircraft) {
            return UnitRole::Light;
        }
        if obj.is_kind_of(KindOf::Infantry) {
            return UnitRole::Light;
        }
        if obj.is_kind_of(KindOf::Vehicle) {
            return UnitRole::Tank;
        }
        if obj.is_kind_of(KindOf::Building) || obj.is_kind_of(KindOf::Structure) {
            return UnitRole::Support;
        }

        UnitRole::DamageDealer
    }

    /// Get performance statistics
    pub fn get_performance_stats(&self) -> &AiPerformanceStats {
        &self.performance_stats
    }

    /// Get number of active AI players
    pub fn get_ai_player_count(&self) -> usize {
        self.ai_players.len()
    }

    /// Get number of active unit groups
    pub fn get_unit_group_count(&self) -> usize {
        self.unit_groups.len()
    }

    /// Get number of objects with AI state machines
    pub fn get_ai_object_count(&self) -> usize {
        self.object_state_machines.len()
    }

    /// Take the latest pathfinding result for a requester, if any.
    pub fn take_path_result(&mut self, requester_id: ObjectID) -> Option<PathResult> {
        self.path_results.remove(&requester_id)
    }

    /// Submit a pathfinding request using the classic pathfinder.
    pub fn request_path(&mut self, request: PathRequest) -> Result<(), AiError> {
        let Some(ai) = THE_AI.read().ok() else {
            return Err(AiError::NoPathfinder);
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return Err(AiError::NoPathfinder);
        };
        let pathfinder_lock = pathfinder.read();
        if let Ok(pf) = pathfinder_lock {
            let requester_id = request.object_id;
            let result = pf.find_path_result(request);
            self.path_results.insert(requester_id, result);
            Ok(())
        } else {
            Err(AiError::NoPathfinder)
        }
    }

    /// Force pathfinder reset (for map changes)
    pub fn reset_pathfinder(&mut self) -> Result<(), AiError> {
        let Some(ai) = THE_AI.read().ok() else {
            return Err(AiError::NoPathfinder);
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return Err(AiError::NoPathfinder);
        };
        let pathfinder_lock = pathfinder.write();
        if let Ok(mut pf) = pathfinder_lock {
            pf.reset();
            log::info!("Pathfinder reset");
            Ok(())
        } else {
            Err(AiError::NoPathfinder)
        }
    }
}

// Global AI integration manager instance
lazy_static::lazy_static! {
    static ref AI_INTEGRATION_MANAGER: Arc<RwLock<Option<AiIntegrationManager>>> =
        Arc::new(RwLock::new(None));
}

/// Initialize global AI integration manager
pub fn initialize_ai_integration() -> Result<(), AiError> {
    let mut manager_guard = AI_INTEGRATION_MANAGER.write().unwrap();
    let mut manager = AiIntegrationManager::new();
    manager.initialize()?;
    *manager_guard = Some(manager);

    log::info!("AI Integration Manager initialized globally");
    Ok(())
}

/// Get reference to global AI integration manager
pub fn with_ai_integration<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&AiIntegrationManager) -> R,
{
    AI_INTEGRATION_MANAGER.read().unwrap().as_ref().map(f)
}

/// Get mutable reference to global AI integration manager
pub fn with_ai_integration_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AiIntegrationManager) -> R,
{
    AI_INTEGRATION_MANAGER.write().unwrap().as_mut().map(f)
}

/// Update global AI integration manager
pub fn ai_sense(world: &World, delta: Duration) -> Result<(), AiError> {
    with_ai_integration_mut(|manager| manager.sense(world, delta))
        .unwrap_or(Err(AiError::NotInitialized))
}

pub fn ai_decide(frame_time: Instant) -> Result<(), AiError> {
    with_ai_integration_mut(|manager| manager.decide(frame_time))
        .unwrap_or(Err(AiError::NotInitialized))
}

pub fn ai_execute(frame_time: Instant) -> Result<(), AiError> {
    with_ai_integration_mut(|manager| manager.execute(frame_time))
        .unwrap_or(Err(AiError::NotInitialized))?;

    // Update the legacy AI singleton after modern systems have produced orders.
    let mut ai = THE_AI.write().map_err(|_| AiError::InvalidObject)?;
    ai.update(TheGameLogic::get_frame())?;
    Ok(())
}

pub fn update_ai_integration(
    world: &World,
    delta: Duration,
    frame_time: Instant,
) -> Result<(), AiError> {
    ai_sense(world, delta)?;
    ai_decide(frame_time)?;
    ai_execute(frame_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use std::sync::{Arc, RwLock};

    #[test]
    fn test_integration_manager_creation() {
        let manager = AiIntegrationManager::new();

        assert_eq!(manager.get_ai_player_count(), 0);
        assert_eq!(manager.get_unit_group_count(), 0);
        assert_eq!(manager.get_ai_object_count(), 0);
    }

    #[test]
    fn test_unit_group_management() {
        let mut manager = AiIntegrationManager::new();
        manager.initialize().unwrap();

        let object_a = Arc::new(RwLock::new(Object::new_test(100, 100.0)));
        let object_b = Arc::new(RwLock::new(Object::new_test(101, 100.0)));
        OBJECT_REGISTRY.register_object(100, &object_a);
        OBJECT_REGISTRY.register_object(101, &object_b);

        // Create group
        let group_id = manager
            .create_unit_group("TestGroup".to_string(), 1)
            .unwrap();
        assert_eq!(manager.get_unit_group_count(), 1);

        // Add units to group
        assert!(manager
            .add_unit_to_group(group_id, 100, UnitRole::Leader)
            .is_ok());
        assert!(manager
            .add_unit_to_group(group_id, 101, UnitRole::Tank)
            .is_ok());

        // Remove unit from group
        assert!(manager.remove_unit_from_group(group_id, 100).is_ok());

        // Group should still exist with remaining unit
        assert_eq!(manager.get_unit_group_count(), 1);

        OBJECT_REGISTRY.unregister_object(100);
        OBJECT_REGISTRY.unregister_object(101);
    }

    #[test]
    fn test_object_state_management() {
        let mut manager = AiIntegrationManager::new();
        manager.initialize().unwrap();

        let object_id = 42;
        let object = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object);

        // Set object state
        assert_eq!(
            manager.set_object_state(object_id, AIStateType::MoveTo),
            Err(AiError::InvalidCommand)
        );
        assert_eq!(manager.get_ai_object_count(), 1);

        // Non-AI test objects cannot enter MoveTo and stay in Idle.
        assert_eq!(manager.get_object_state(object_id), Some(AIStateType::Idle));

        // Notify object destroyed
        assert!(manager.notify_object_destroyed(object_id, &[]).is_ok());
        assert_eq!(manager.get_ai_object_count(), 0);

        OBJECT_REGISTRY.unregister_object(object_id);
    }
}
