//! Victory Conditions System
//!
//! This module manages victory and defeat conditions for scenarios and campaigns.

use super::{ConditionRegistry, ScriptCondition, ScriptContext, ScriptValue};
use crate::common::KindOf;
use crate::object::registry::OBJECT_REGISTRY;
use crate::{GameLogicError, GameLogicResult};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// Victory condition types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VictoryConditionType {
    /// Destroy all enemy units and buildings
    TotalDestruction,
    /// Destroy specific enemy structures
    DestroyStructures(Vec<String>),
    /// Capture specific locations
    CaptureLocations(Vec<String>),
    /// Hold positions for a certain time
    HoldPositions {
        locations: Vec<String>,
        duration: f32,
    },
    /// Gather specific resources
    GatherResources { resource_type: String, amount: i32 },
    /// Rescue or escort units
    RescueUnits(Vec<u32>),
    /// Survive for a certain time
    Survive(f32),
    /// Reach a location with specific units
    ReachLocation { location: String, units: Vec<u32> },
    /// Build specific structures
    BuildStructures(Vec<String>),
    /// Research specific technologies
    ResearchTechnologies(Vec<String>),
    /// Kill specific enemy units/heroes
    KillTargets(Vec<u32>),
    /// Custom condition using script
    Custom {
        script_condition: String,
        parameters: HashMap<String, ScriptValue>,
    },
}

/// Victory condition definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VictoryCondition {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Detailed description
    pub description: String,
    /// Condition type
    pub condition_type: VictoryConditionType,
    /// Players this applies to
    pub players: Vec<u32>,
    /// Whether this is required for victory (vs optional objective)
    pub required: bool,
    /// Whether this condition has been met
    pub completed: bool,
    /// Whether this condition has failed (cannot be completed)
    pub failed: bool,
    /// Progress towards completion (0.0 to 1.0)
    pub progress: f32,
    /// Whether condition is currently active
    pub active: bool,
    /// Prerequisites (other conditions that must be met first)
    pub prerequisites: Vec<String>,
    /// Time limit for this condition (optional)
    pub time_limit: Option<f32>,
    /// Failure conditions (if any of these are met, this condition fails)
    pub failure_conditions: Vec<String>,
    /// Rewards for completing this condition
    pub rewards: Vec<VictoryReward>,
}

/// Victory rewards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VictoryReward {
    /// Resources
    Resources { resource_type: String, amount: i32 },
    /// Units
    Units {
        unit_type: String,
        count: i32,
        location: Option<[f32; 3]>,
    },
    /// Technology/upgrades
    Technology(String),
    /// Special powers
    SpecialPower(String),
    /// Score bonus
    ScoreBonus(i32),
}

/// Victory condition state
#[derive(Debug, Clone)]
pub enum VictoryConditionState {
    /// Condition is active and being checked
    Active,
    /// Condition has been completed successfully
    Completed,
    /// Condition has failed
    Failed,
    /// Condition is inactive (prerequisites not met)
    Inactive,
    /// Condition is paused
    Paused,
}

/// Victory manager
pub struct VictoryManager {
    /// All defined victory conditions
    conditions: Arc<RwLock<HashMap<String, VictoryCondition>>>,
    /// Condition evaluation registry
    condition_registry: Arc<RwLock<ConditionRegistry>>,
    /// Victory state per player
    player_victory_states: Arc<RwLock<HashMap<u32, PlayerVictoryState>>>,
    /// Global game end state
    game_ended: Arc<RwLock<bool>>,
    /// Victory/defeat callbacks
    callbacks: Arc<RwLock<Vec<Box<dyn VictoryCallback>>>>,
}

/// Player victory state
#[derive(Debug, Clone)]
pub struct PlayerVictoryState {
    /// Player ID
    pub player_id: u32,
    /// Whether player has won
    pub victorious: bool,
    /// Whether player has been defeated
    pub defeated: bool,
    /// Completed objectives
    pub completed_objectives: HashSet<String>,
    /// Failed objectives
    pub failed_objectives: HashSet<String>,
    /// Current score
    pub score: i32,
}

/// Victory callback trait
pub trait VictoryCallback: Send + Sync {
    /// Called when a player achieves victory
    fn on_victory(&self, player_id: u32, conditions: &[String]);

    /// Called when a player is defeated
    fn on_defeat(&self, player_id: u32, reason: &str);

    /// Called when an objective is completed
    fn on_objective_completed(&self, player_id: u32, objective_id: &str);

    /// Called when an objective fails
    fn on_objective_failed(&self, player_id: u32, objective_id: &str);
}

impl VictoryManager {
    /// Create a new victory manager
    pub fn new() -> Self {
        Self {
            conditions: Arc::new(RwLock::new(HashMap::new())),
            condition_registry: Arc::new(RwLock::new(ConditionRegistry::new())),
            player_victory_states: Arc::new(RwLock::new(HashMap::new())),
            game_ended: Arc::new(RwLock::new(false)),
            callbacks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Initialize player victory state
    pub async fn initialize_player(&self, player_id: u32) -> GameLogicResult<()> {
        let mut states = self.player_victory_states.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire states lock: {}", e))
        })?;

        states.insert(
            player_id,
            PlayerVictoryState {
                player_id,
                victorious: false,
                defeated: false,
                completed_objectives: HashSet::new(),
                failed_objectives: HashSet::new(),
                score: 0,
            },
        );

        Ok(())
    }

    /// Add a victory condition
    pub async fn add_victory_condition(&self, condition: VictoryCondition) -> GameLogicResult<()> {
        let mut conditions = self.conditions.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire conditions lock: {}", e))
        })?;

        log::info!(
            "Adding victory condition: {} - {}",
            condition.id,
            condition.name
        );
        conditions.insert(condition.id.clone(), condition);

        Ok(())
    }

    /// Remove a victory condition
    pub async fn remove_victory_condition(&self, condition_id: &str) -> GameLogicResult<()> {
        let mut conditions = self.conditions.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire conditions lock: {}", e))
        })?;

        if conditions.remove(condition_id).is_some() {
            log::info!("Removed victory condition: {}", condition_id);
        }

        Ok(())
    }

    /// Update all victory conditions
    pub async fn update(&self, context: &ScriptContext) -> GameLogicResult<()> {
        // Check if game has already ended
        {
            let game_ended = self.game_ended.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read game_ended: {}", e))
            })?;
            if *game_ended {
                return Ok(());
            }
        }

        // Get all active conditions
        let active_conditions = {
            let conditions = self.conditions.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read conditions: {}", e))
            })?;
            conditions.values().cloned().collect::<Vec<_>>()
        };

        // Evaluate each condition
        for mut condition in active_conditions {
            if !condition.active || condition.completed || condition.failed {
                continue;
            }

            // Check prerequisites
            if !self.check_prerequisites(&condition).await? {
                continue;
            }

            // Check time limit
            if let Some(time_limit) = condition.time_limit {
                if context.game_time.as_secs_f32() > time_limit {
                    self.fail_condition(&mut condition, "Time limit exceeded")
                        .await?;
                    self.store_condition(&condition)?;
                    continue;
                }
            }

            // Check failure conditions
            if self.check_failure_conditions(&condition, context).await? {
                self.fail_condition(&mut condition, "Failure condition met")
                    .await?;
                self.store_condition(&condition)?;
                continue;
            }

            // Evaluate main condition
            let (completed, progress) = self.evaluate_condition(&condition, context).await?;
            condition.progress = progress;

            if completed {
                self.complete_condition(&mut condition).await?;
            }

            // Update condition in storage
            self.store_condition(&condition)?;
        }

        // Check for overall victory/defeat
        self.check_game_end_conditions().await?;

        Ok(())
    }

    fn store_condition(&self, condition: &VictoryCondition) -> GameLogicResult<()> {
        let mut conditions = self
            .conditions
            .write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to write conditions: {}", e)))?;
        conditions.insert(condition.id.clone(), condition.clone());
        Ok(())
    }

    /// Evaluate a specific condition
    async fn evaluate_condition(
        &self,
        condition: &VictoryCondition,
        context: &ScriptContext,
    ) -> GameLogicResult<(bool, f32)> {
        match &condition.condition_type {
            VictoryConditionType::TotalDestruction => {
                // Check if all enemy units/buildings are destroyed
                let progress = self.calculate_destruction_progress(context).await?;
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::DestroyStructures(structures) => {
                let progress = self
                    .calculate_structure_destruction_progress(structures, context)
                    .await?;
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::CaptureLocations(locations) => {
                let progress = self.calculate_capture_progress(locations, context).await?;
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::HoldPositions {
                locations,
                duration,
            } => {
                let progress = self
                    .calculate_hold_progress(locations, *duration, context)
                    .await?;
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::GatherResources {
                resource_type,
                amount,
            } => {
                let progress = self
                    .calculate_resource_progress(resource_type, *amount, context)
                    .await?;
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::RescueUnits(units) => {
                let progress = self.calculate_rescue_progress(units, context).await?;
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::Survive(duration) => {
                let progress = (context.game_time.as_secs_f32() / duration).min(1.0);
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::ReachLocation { location, units } => {
                let progress = self
                    .calculate_reach_progress(location, units, context)
                    .await?;
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::BuildStructures(structures) => {
                let progress = self.calculate_build_progress(structures, context).await?;
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::ResearchTechnologies(technologies) => {
                let progress = self
                    .calculate_research_progress(technologies, context)
                    .await?;
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::KillTargets(targets) => {
                let progress = self.calculate_kill_progress(targets, context).await?;
                Ok((progress >= 1.0, progress))
            }
            VictoryConditionType::Custom {
                script_condition,
                parameters,
            } => {
                let completed = self
                    .evaluate_custom_condition(script_condition, parameters, context)
                    .await?;
                Ok((completed, if completed { 1.0 } else { 0.0 }))
            }
        }
    }

    /// Check prerequisites for a condition
    async fn check_prerequisites(&self, condition: &VictoryCondition) -> GameLogicResult<bool> {
        if condition.prerequisites.is_empty() {
            return Ok(true);
        }

        let conditions = self
            .conditions
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read conditions: {}", e)))?;

        for prereq_id in &condition.prerequisites {
            if let Some(prereq) = conditions.get(prereq_id) {
                if !prereq.completed {
                    return Ok(false);
                }
            } else {
                log::warn!("Prerequisite condition '{}' not found", prereq_id);
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check failure conditions
    async fn check_failure_conditions(
        &self,
        condition: &VictoryCondition,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        for failure_condition_id in &condition.failure_conditions {
            if self.is_objective_condition_completed(failure_condition_id)? {
                return Ok(true);
            }

            let parameters = HashMap::new();
            if self
                .evaluate_custom_condition(failure_condition_id, &parameters, context)
                .await?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn is_objective_condition_completed(&self, condition_id: &str) -> GameLogicResult<bool> {
        let conditions = self
            .conditions
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read conditions: {}", e)))?;

        Ok(conditions
            .get(condition_id)
            .map(|condition| condition.completed)
            .unwrap_or(false))
    }

    /// Complete a victory condition
    async fn complete_condition(&self, condition: &mut VictoryCondition) -> GameLogicResult<()> {
        condition.completed = true;
        condition.progress = 1.0;

        log::info!(
            "Victory condition completed: {} - {}",
            condition.id,
            condition.name
        );

        // Award rewards
        for reward in &condition.rewards {
            self.award_reward(reward, &condition.players).await?;
        }

        // Notify callbacks
        {
            let callbacks = self.callbacks.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read callbacks: {}", e))
            })?;

            for player_id in &condition.players {
                for callback in callbacks.iter() {
                    callback.on_objective_completed(*player_id, &condition.id);
                }
            }
        }

        // Update player states
        {
            let mut states = self
                .player_victory_states
                .write()
                .map_err(|e| GameLogicError::Threading(format!("Failed to write states: {}", e)))?;

            for player_id in &condition.players {
                if let Some(state) = states.get_mut(player_id) {
                    state.completed_objectives.insert(condition.id.clone());
                }
            }
        }

        Ok(())
    }

    /// Fail a victory condition
    async fn fail_condition(
        &self,
        condition: &mut VictoryCondition,
        reason: &str,
    ) -> GameLogicResult<()> {
        condition.failed = true;

        log::info!(
            "Victory condition failed: {} - {} ({})",
            condition.id,
            condition.name,
            reason
        );

        // Notify callbacks
        {
            let callbacks = self.callbacks.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read callbacks: {}", e))
            })?;

            for player_id in &condition.players {
                for callback in callbacks.iter() {
                    callback.on_objective_failed(*player_id, &condition.id);
                }
            }
        }

        // Update player states
        {
            let mut states = self
                .player_victory_states
                .write()
                .map_err(|e| GameLogicError::Threading(format!("Failed to write states: {}", e)))?;

            for player_id in &condition.players {
                if let Some(state) = states.get_mut(player_id) {
                    state.failed_objectives.insert(condition.id.clone());
                }
            }
        }

        Ok(())
    }

    /// Award reward to players
    async fn award_reward(&self, reward: &VictoryReward, players: &[u32]) -> GameLogicResult<()> {
        for &player_id in players {
            match reward {
                VictoryReward::Resources {
                    resource_type,
                    amount,
                } => {
                    log::info!(
                        "Awarding {} {} to player {}",
                        amount,
                        resource_type,
                        player_id
                    );
                    // In a real implementation, this would add resources to the player
                }
                VictoryReward::Units {
                    unit_type,
                    count,
                    location: _,
                } => {
                    log::info!(
                        "Awarding {} {} units to player {}",
                        count,
                        unit_type,
                        player_id
                    );
                    // In a real implementation, this would spawn units for the player
                }
                VictoryReward::Technology(tech) => {
                    log::info!("Awarding technology '{}' to player {}", tech, player_id);
                    // In a real implementation, this would unlock technology for the player
                }
                VictoryReward::SpecialPower(power) => {
                    log::info!("Awarding special power '{}' to player {}", power, player_id);
                    // In a real implementation, this would enable the special power
                }
                VictoryReward::ScoreBonus(points) => {
                    log::info!("Awarding {} score points to player {}", points, player_id);
                    let mut states = self.player_victory_states.write().map_err(|e| {
                        GameLogicError::Threading(format!("Failed to write states: {}", e))
                    })?;

                    if let Some(state) = states.get_mut(&player_id) {
                        state.score += points;
                    }
                }
            }
        }

        Ok(())
    }

    /// Check for overall game end conditions
    async fn check_game_end_conditions(&self) -> GameLogicResult<()> {
        enum EndDecision {
            Victory(u32),
            Defeat(u32),
        }

        let decisions = {
            let conditions = self.conditions.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read conditions: {}", e))
            })?;

            let states = self
                .player_victory_states
                .read()
                .map_err(|e| GameLogicError::Threading(format!("Failed to read states: {}", e)))?;

            let mut decisions = Vec::new();
            for (player_id, state) in states.iter() {
                if state.victorious || state.defeated {
                    continue;
                }

                let player_conditions: Vec<_> = conditions
                    .values()
                    .filter(|c| c.players.contains(player_id) && c.required)
                    .collect();

                let all_completed =
                    !player_conditions.is_empty() && player_conditions.iter().all(|c| c.completed);
                let any_critical_failed = player_conditions.iter().any(|c| c.failed);

                if all_completed {
                    decisions.push(EndDecision::Victory(*player_id));
                } else if any_critical_failed {
                    decisions.push(EndDecision::Defeat(*player_id));
                }
            }
            decisions
        };

        for decision in decisions {
            match decision {
                EndDecision::Victory(player_id) => self.set_player_victorious(player_id).await?,
                EndDecision::Defeat(player_id) => {
                    self.set_player_defeated(player_id, "Critical objective failed")
                        .await?
                }
            }
        }

        Ok(())
    }

    /// Set player as victorious
    async fn set_player_victorious(&self, player_id: u32) -> GameLogicResult<()> {
        {
            let mut states = self
                .player_victory_states
                .write()
                .map_err(|e| GameLogicError::Threading(format!("Failed to write states: {}", e)))?;

            if let Some(state) = states.get_mut(&player_id) {
                state.victorious = true;
                log::info!("Player {} achieved victory!", player_id);
            }
        }

        // Check if we should end the game
        let should_end = {
            let states = self
                .player_victory_states
                .read()
                .map_err(|e| GameLogicError::Threading(format!("Failed to read states: {}", e)))?;

            states.values().any(|s| s.victorious)
        };

        if should_end {
            let mut game_ended = self.game_ended.write().map_err(|e| {
                GameLogicError::Threading(format!("Failed to write game_ended: {}", e))
            })?;
            *game_ended = true;
        }

        // Notify callbacks
        {
            let callbacks = self.callbacks.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read callbacks: {}", e))
            })?;

            let completed_conditions: Vec<String> = {
                let states = self.player_victory_states.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to read states: {}", e))
                })?;

                if let Some(state) = states.get(&player_id) {
                    state.completed_objectives.iter().cloned().collect()
                } else {
                    Vec::new()
                }
            };

            for callback in callbacks.iter() {
                callback.on_victory(player_id, &completed_conditions);
            }
        }

        Ok(())
    }

    /// Set player as defeated
    async fn set_player_defeated(&self, player_id: u32, reason: &str) -> GameLogicResult<()> {
        {
            let mut states = self
                .player_victory_states
                .write()
                .map_err(|e| GameLogicError::Threading(format!("Failed to write states: {}", e)))?;

            if let Some(state) = states.get_mut(&player_id) {
                state.defeated = true;
                log::info!("Player {} was defeated: {}", player_id, reason);
            }
        }

        // Notify callbacks
        {
            let callbacks = self.callbacks.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read callbacks: {}", e))
            })?;

            for callback in callbacks.iter() {
                callback.on_defeat(player_id, reason);
            }
        }

        Ok(())
    }

    /// Add victory callback
    pub async fn add_callback(&self, callback: Box<dyn VictoryCallback>) -> GameLogicResult<()> {
        let mut callbacks = self
            .callbacks
            .write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to write callbacks: {}", e)))?;
        callbacks.push(callback);
        Ok(())
    }

    /// Get player victory state
    pub async fn get_player_state(
        &self,
        player_id: u32,
    ) -> GameLogicResult<Option<PlayerVictoryState>> {
        let states = self
            .player_victory_states
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read states: {}", e)))?;
        Ok(states.get(&player_id).cloned())
    }

    /// Get all victory conditions
    pub async fn get_all_conditions(&self) -> GameLogicResult<Vec<VictoryCondition>> {
        let conditions = self
            .conditions
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read conditions: {}", e)))?;
        Ok(conditions.values().cloned().collect())
    }

    /// Check if game has ended
    pub async fn is_game_ended(&self) -> GameLogicResult<bool> {
        let game_ended = self
            .game_ended
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read game_ended: {}", e)))?;
        Ok(*game_ended)
    }

    async fn calculate_destruction_progress(
        &self,
        context: &ScriptContext,
    ) -> GameLogicResult<f32> {
        let active_player = context.active_player;
        if !context.game_state.players.is_empty() {
            let enemy_players: Vec<&super::PlayerInfo> = context
                .game_state
                .players
                .iter()
                .filter(|p| Some(p.id) != active_player)
                .collect();
            if enemy_players.is_empty() {
                return Ok(1.0);
            }
            let defeated = enemy_players.iter().filter(|p| !p.is_alive).count();
            return Ok(defeated as f32 / enemy_players.len() as f32);
        }

        let enemy_ids: HashSet<u32> = active_player.into_iter().collect();
        // Host path: dual-world factory empty — do not treat as "all enemies dead".
        if OBJECT_REGISTRY.is_empty() {
            return Ok(0.0);
        }
        let mut enemy_alive = 0usize;
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if obj.is_destroyed() {
                continue;
            }
            let Some(player_id) = obj.get_controlling_player_id().map(|id| id as u32) else {
                continue;
            };
            if enemy_ids.contains(&player_id) {
                continue;
            }
            if obj.is_kind_of(KindOf::Unit) || obj.is_kind_of(KindOf::Structure) {
                enemy_alive += 1;
            }
        }

        Ok(if enemy_alive == 0 { 1.0 } else { 0.0 })
    }

    async fn calculate_structure_destruction_progress(
        &self,
        structures: &[String],
        context: &ScriptContext,
    ) -> GameLogicResult<f32> {
        if structures.is_empty() {
            return Ok(1.0);
        }

        let active_player = context.active_player;
        let enemy_ids: HashSet<u32> = context
            .game_state
            .players
            .iter()
            .filter(|p| Some(p.id) != active_player)
            .map(|p| p.id)
            .collect();

        // Host path: empty dual-world registry — no structure residual to score.
        if OBJECT_REGISTRY.is_empty() {
            return Ok(0.0);
        }
        let mut remaining = 0usize;
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if obj.is_destroyed() {
                continue;
            }
            let Some(player_id) = obj.get_controlling_player_id().map(|id| id as u32) else {
                continue;
            };
            if !enemy_ids.is_empty() && !enemy_ids.contains(&player_id) {
                continue;
            }
            if !structures
                .iter()
                .any(|name| name.eq_ignore_ascii_case(obj.get_template_name()))
            {
                continue;
            }
            remaining += 1;
        }

        let total = structures.len().max(1);
        let destroyed = total.saturating_sub(remaining);
        Ok(destroyed as f32 / total as f32)
    }

    async fn calculate_capture_progress(
        &self,
        locations: &[String],
        context: &ScriptContext,
    ) -> GameLogicResult<f32> {
        if locations.is_empty() {
            return Ok(1.0);
        }

        let completed = context
            .game_state
            .objectives
            .iter()
            .filter(|objective| {
                locations.iter().any(|loc| {
                    loc.eq_ignore_ascii_case(&objective.id)
                        || loc.eq_ignore_ascii_case(&objective.name)
                }) && objective.completed
            })
            .count();

        Ok(completed as f32 / locations.len() as f32)
    }

    async fn calculate_hold_progress(
        &self,
        _locations: &[String],
        _duration: f32,
        context: &ScriptContext,
    ) -> GameLogicResult<f32> {
        // Simple time-based progress
        Ok((context.game_time.as_secs_f32() / _duration).min(1.0))
    }

    async fn calculate_resource_progress(
        &self,
        resource_type: &str,
        amount: i32,
        context: &ScriptContext,
    ) -> GameLogicResult<f32> {
        if amount <= 0 {
            return Ok(1.0);
        }

        let mut value = 0.0_f32;
        if let Some(val) = context.variables.get(resource_type) {
            value = script_value_to_f32(val);
        } else if let Some(val) = context.variables.get(&format!("resource:{resource_type}")) {
            value = script_value_to_f32(val);
        }

        Ok((value / amount as f32).clamp(0.0, 1.0))
    }

    async fn calculate_rescue_progress(
        &self,
        units: &[u32],
        context: &ScriptContext,
    ) -> GameLogicResult<f32> {
        if units.is_empty() {
            return Ok(1.0);
        }

        let active_player = context.active_player;
        let mut rescued = 0usize;
        for unit_id in units {
            if OBJECT_REGISTRY
                .with_object(*unit_id, |obj| {
                    if obj.is_destroyed() {
                        return false;
                    }
                    obj.get_controlling_player_id().map(|id| id as u32) == active_player
                })
                .unwrap_or(false)
            {
                rescued += 1;
            }
        }

        Ok(rescued as f32 / units.len() as f32)
    }

    async fn calculate_reach_progress(
        &self,
        location: &str,
        _units: &[u32],
        context: &ScriptContext,
    ) -> GameLogicResult<f32> {
        if location.is_empty() {
            return Ok(1.0);
        }

        let reached = context.game_state.objectives.iter().any(|objective| {
            (objective.id.eq_ignore_ascii_case(location)
                || objective.name.eq_ignore_ascii_case(location))
                && objective.completed
        });
        Ok(if reached { 1.0 } else { 0.0 })
    }

    async fn calculate_build_progress(
        &self,
        structures: &[String],
        context: &ScriptContext,
    ) -> GameLogicResult<f32> {
        if structures.is_empty() {
            return Ok(1.0);
        }

        let active_player = context.active_player;
        // Host path: empty dual-world registry — nothing built residual.
        if OBJECT_REGISTRY.is_empty() {
            return Ok(0.0);
        }
        let mut built = 0usize;
        for name in structures {
            let mut found = false;
            for obj_arc in OBJECT_REGISTRY.get_all_objects() {
                let Ok(obj) = obj_arc.read() else {
                    continue;
                };
                if obj.is_destroyed() {
                    continue;
                }
                if Some(
                    obj.get_controlling_player_id()
                        .map(|id| id as u32)
                        .unwrap_or(0),
                ) != active_player
                {
                    continue;
                }
                if name.eq_ignore_ascii_case(obj.get_template_name()) {
                    found = true;
                    break;
                }
            }
            if found {
                built += 1;
            }
        }

        Ok(built as f32 / structures.len() as f32)
    }

    async fn calculate_research_progress(
        &self,
        technologies: &[String],
        context: &ScriptContext,
    ) -> GameLogicResult<f32> {
        if technologies.is_empty() {
            return Ok(1.0);
        }

        let mut completed = 0usize;
        for tech in technologies {
            let mut value = 0.0_f32;
            if let Some(val) = context.variables.get(tech) {
                value = script_value_to_f32(val);
            } else if let Some(val) = context.variables.get(&format!("tech:{tech}")) {
                value = script_value_to_f32(val);
            }
            if value >= 1.0 {
                completed += 1;
            }
        }

        Ok(completed as f32 / technologies.len() as f32)
    }

    async fn calculate_kill_progress(
        &self,
        targets: &[u32],
        _context: &ScriptContext,
    ) -> GameLogicResult<f32> {
        if targets.is_empty() {
            return Ok(1.0);
        }

        let mut killed = 0usize;
        for target_id in targets {
            if OBJECT_REGISTRY
                .with_object(*target_id, |obj| obj.is_destroyed())
                .unwrap_or(true)
            {
                killed += 1;
            }
        }

        Ok(killed as f32 / targets.len() as f32)
    }

    async fn evaluate_custom_condition(
        &self,
        condition_name: &str,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let registry = self
            .condition_registry
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read registry: {}", e)))?;
        let Some(handler) = registry.get_condition(condition_name) else {
            return Ok(false);
        };
        handler.evaluate(parameters, context).await
    }
}

fn script_value_to_f32(value: &ScriptValue) -> f32 {
    match value {
        ScriptValue::Int(v) => *v as f32,
        ScriptValue::Float(v) => *v as f32,
        ScriptValue::Bool(v) => {
            if *v {
                1.0
            } else {
                0.0
            }
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_victory_manager_creation() {
        let manager = VictoryManager::new();

        manager.initialize_player(1).await.unwrap();
        let state = manager.get_player_state(1).await.unwrap();

        assert!(state.is_some());
        let state = state.unwrap();
        assert_eq!(state.player_id, 1);
        assert!(!state.victorious);
        assert!(!state.defeated);
    }

    #[tokio::test]
    async fn test_victory_condition_creation() {
        let manager = VictoryManager::new();

        let condition = VictoryCondition {
            id: "test_victory".to_string(),
            name: "Test Victory".to_string(),
            description: "Test victory condition".to_string(),
            condition_type: VictoryConditionType::Survive(60.0),
            players: vec![1],
            required: true,
            completed: false,
            failed: false,
            progress: 0.0,
            active: true,
            prerequisites: Vec::new(),
            time_limit: None,
            failure_conditions: Vec::new(),
            rewards: Vec::new(),
        };

        manager.add_victory_condition(condition).await.unwrap();

        let conditions = manager.get_all_conditions().await.unwrap();
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].id, "test_victory");
    }

    #[tokio::test]
    async fn test_victory_condition_evaluation() {
        let manager = VictoryManager::new();
        manager.initialize_player(1).await.unwrap();

        let condition = VictoryCondition {
            id: "survive_test".to_string(),
            name: "Survive Test".to_string(),
            description: "Survive for 30 seconds".to_string(),
            condition_type: VictoryConditionType::Survive(30.0),
            players: vec![1],
            required: true,
            completed: false,
            failed: false,
            progress: 0.0,
            active: true,
            prerequisites: Vec::new(),
            time_limit: None,
            failure_conditions: Vec::new(),
            rewards: Vec::new(),
        };

        manager.add_victory_condition(condition).await.unwrap();

        // Test with 15 seconds elapsed (50% progress)
        let context = ScriptContext {
            game_time: Duration::from_secs(15),
            active_player: Some(1),
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        manager.update(&context).await.unwrap();

        let conditions = manager.get_all_conditions().await.unwrap();
        assert_eq!(conditions[0].progress, 0.5);
        assert!(!conditions[0].completed);
    }

    #[tokio::test]
    async fn test_failure_conditions_evaluate_registered_script_conditions() {
        let manager = VictoryManager::new();
        manager.initialize_player(1).await.unwrap();

        let condition = VictoryCondition {
            id: "survive_with_failure".to_string(),
            name: "Survive With Failure".to_string(),
            description: "Should fail when its failure condition is true".to_string(),
            condition_type: VictoryConditionType::Survive(30.0),
            players: vec![1],
            required: true,
            completed: false,
            failed: false,
            progress: 0.0,
            active: true,
            prerequisites: Vec::new(),
            time_limit: None,
            failure_conditions: vec!["condition_true".to_string()],
            rewards: Vec::new(),
        };

        manager.add_victory_condition(condition).await.unwrap();

        let context = ScriptContext {
            game_time: Duration::from_secs(1),
            active_player: Some(1),
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        manager.update(&context).await.unwrap();

        let conditions = manager.get_all_conditions().await.unwrap();
        let condition = conditions
            .iter()
            .find(|condition| condition.id == "survive_with_failure")
            .expect("survive_with_failure condition");
        assert!(condition.failed);

        let state = manager.get_player_state(1).await.unwrap().unwrap();
        assert!(state.defeated);
        assert!(state.failed_objectives.contains("survive_with_failure"));
    }

    #[tokio::test]
    async fn test_failure_conditions_can_reference_completed_objectives() {
        let manager = VictoryManager::new();
        manager.initialize_player(1).await.unwrap();

        manager
            .add_victory_condition(VictoryCondition {
                id: "tripwire".to_string(),
                name: "Tripwire".to_string(),
                description: "Already completed failure tripwire".to_string(),
                condition_type: VictoryConditionType::Survive(0.0),
                players: vec![1],
                required: false,
                completed: true,
                failed: false,
                progress: 1.0,
                active: true,
                prerequisites: Vec::new(),
                time_limit: None,
                failure_conditions: Vec::new(),
                rewards: Vec::new(),
            })
            .await
            .unwrap();

        manager
            .add_victory_condition(VictoryCondition {
                id: "main".to_string(),
                name: "Main".to_string(),
                description: "Fails because referenced objective is complete".to_string(),
                condition_type: VictoryConditionType::Survive(30.0),
                players: vec![1],
                required: true,
                completed: false,
                failed: false,
                progress: 0.0,
                active: true,
                prerequisites: Vec::new(),
                time_limit: None,
                failure_conditions: vec!["tripwire".to_string()],
                rewards: Vec::new(),
            })
            .await
            .unwrap();

        let context = ScriptContext {
            game_time: Duration::from_secs(1),
            active_player: Some(1),
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        manager.update(&context).await.unwrap();

        let conditions = manager.get_all_conditions().await.unwrap();
        let main = conditions
            .iter()
            .find(|condition| condition.id == "main")
            .expect("main condition");
        assert!(main.failed);
    }
}
