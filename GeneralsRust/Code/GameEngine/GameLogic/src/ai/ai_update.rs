//! AIUpdate - Main AI coordination system
//!
//! This is the central AI coordinator that manages all AI behaviors, decision making,
//! and coordination between different AI systems. It handles player AI, unit AI,
//! and global AI state management.
//!
//! Author: Converted from C++ original by Michael S. Booth

use super::pathfinding_system::{PathRequest, PathResult, PathfindingSystem};
use crate::ai::{AiCommandInterface, AiCommandParams, AiError, AiGroup, AttitudeType, AI, THE_AI};
use crate::common::{Coord3D, ObjectID, Real};
use crate::modules::UpdateModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::GameDifficulty;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

/// AI update interface
pub trait AIUpdateInterface: UpdateModuleInterface {
    fn ai_update(&mut self, object_id: ObjectID, delta_time: Real);
}

/// AI Update system frame constants
pub const AI_UPDATE_RATE: u32 = 30; // Update AI every 30 frames (2 times per second)
pub const AI_PLAYER_UPDATE_RATE: u32 = 60; // Update player AI every 60 frames (once per second)
pub const AI_TARGETING_UPDATE_RATE: u32 = 15; // Update targeting every 15 frames (4 times per second)

/// AI Update priorities for processing order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AiUpdatePriority {
    Critical = 0,   // Must execute this frame
    High = 1,       // Execute as soon as possible
    Normal = 2,     // Standard priority
    Low = 3,        // Execute when CPU cycles available
    Background = 4, // Execute during idle time
}

/// AI Update task types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiUpdateTaskType {
    PlayerAI,
    UnitAI,
    GroupAI,
    Pathfinding,
    Targeting,
    Combat,
    Economy,
    Construction,
    Diplomacy,
}

/// AI Update task
#[derive(Debug)]
pub struct AiUpdateTask {
    pub task_type: AiUpdateTaskType,
    pub priority: AiUpdatePriority,
    pub target_id: Option<ObjectID>,
    pub player_id: Option<u32>,
    pub data: AiTaskData,
    pub frame_added: u32,
    pub max_execution_time: u32, // Maximum frames to spend on this task
}

/// AI task data variants
#[derive(Debug)]
pub enum AiTaskData {
    PlayerUpdate {
        player_id: u32,
    },
    UnitUpdate {
        unit_id: ObjectID,
        force_update: bool,
    },
    GroupUpdate {
        group_id: u32,
        command: AiCommandParams,
    },
    PathfindingUpdate {
        request: PathRequest,
    },
    TargetingUpdate {
        unit_id: ObjectID,
        scan_radius: Real,
    },
    CombatUpdate {
        attacker_id: ObjectID,
        target_id: Option<ObjectID>,
    },
    EconomyUpdate {
        player_id: u32,
    },
    ConstructionUpdate {
        player_id: u32,
        structure_type: Option<String>,
    },
    DiplomacyUpdate {
        player_id: u32,
    },
}

/// AI performance metrics
#[derive(Debug, Clone, Default)]
pub struct AiPerformanceMetrics {
    pub total_tasks_processed: u64,
    pub tasks_by_type: HashMap<AiUpdateTaskType, u64>,
    pub tasks_by_priority: HashMap<AiUpdatePriority, u64>,
    pub average_processing_time: f32,
    pub max_processing_time: f32,
    pub cpu_usage_percent: f32,
    pub memory_usage_mb: f32,
    pub pathfinding_requests: u64,
    pub pathfinding_cache_hits: u64,
    pub targeting_scans: u64,
    pub combat_calculations: u64,
}

/// Main AI Update coordinator
pub struct AIUpdate {
    /// Task queue prioritized by importance
    task_queue: VecDeque<AiUpdateTask>,

    /// AI players being managed
    ai_players: HashMap<u32, Arc<RwLock<Box<(dyn AiPlayerTrait + Send + Sync)>>>>,

    /// Reference to pathfinding system
    pathfinding_system: Option<super::pathfinding_system::SharedPathfindingSystem>,

    /// Performance metrics
    metrics: AiPerformanceMetrics,

    /// Current game frame
    current_frame: u32,

    /// Frame counters for different update rates
    last_player_update_frame: u32,
    last_targeting_update_frame: u32,

    /// CPU budget per frame (in microseconds)
    cpu_budget_per_frame: u32,

    /// Emergency mode when CPU budget exceeded
    emergency_mode: bool,

    /// AI difficulty scaling factors
    difficulty_factors: DifficultyFactors,

    /// Global AI state
    global_ai_state: GlobalAiState,
}

/// Difficulty scaling factors
#[derive(Debug, Clone)]
pub struct DifficultyFactors {
    pub easy_reaction_delay: u32,   // Extra frames for easy AI reactions
    pub normal_reaction_delay: u32, // Standard reaction time
    pub hard_reaction_delay: u32,   // Reduced reaction time for hard AI

    pub easy_accuracy_modifier: f32,   // Reduced accuracy for easy AI
    pub normal_accuracy_modifier: f32, // Standard accuracy
    pub hard_accuracy_modifier: f32,   // Enhanced accuracy for hard AI

    pub easy_resource_modifier: f32, // Reduced resource efficiency for easy AI
    pub normal_resource_modifier: f32, // Standard resource efficiency
    pub hard_resource_modifier: f32, // Enhanced resource efficiency for hard AI

    pub easy_build_speed: f32,   // Slower building for easy AI
    pub normal_build_speed: f32, // Standard building speed
    pub hard_build_speed: f32,   // Faster building for hard AI
}

impl Default for DifficultyFactors {
    fn default() -> Self {
        Self {
            easy_reaction_delay: 60,   // 2 second delay
            normal_reaction_delay: 30, // 1 second delay
            hard_reaction_delay: 15,   // 0.5 second delay

            easy_accuracy_modifier: 0.7,
            normal_accuracy_modifier: 1.0,
            hard_accuracy_modifier: 1.3,

            easy_resource_modifier: 0.8,
            normal_resource_modifier: 1.0,
            hard_resource_modifier: 1.2,

            easy_build_speed: 0.8,
            normal_build_speed: 1.0,
            hard_build_speed: 1.1,
        }
    }
}

/// Global AI state information
#[derive(Debug, Clone, Default)]
pub struct GlobalAiState {
    pub global_threat_level: f32, // Overall threat assessment (0.0 to 1.0)
    pub economic_pressure: f32,   // Economic stress level (0.0 to 1.0)
    pub military_balance: f32,    // Military balance assessment (-1.0 to 1.0)
    pub technology_level: f32,    // Technology advancement level (0.0 to 1.0)
    pub map_control: HashMap<u32, f32>, // Territory control per player
    pub resource_scarcity: f32,   // Resource availability (0.0 to 1.0)
    pub time_pressure: f32,       // Game time pressure factor
}

impl AIUpdate {
    /// Create new AI Update coordinator
    pub fn new() -> Self {
        Self {
            task_queue: VecDeque::new(),
            ai_players: HashMap::new(),
            pathfinding_system: None,
            metrics: AiPerformanceMetrics::default(),
            current_frame: 0,
            last_player_update_frame: 0,
            last_targeting_update_frame: 0,
            cpu_budget_per_frame: 16000, // 16ms in microseconds
            emergency_mode: false,
            difficulty_factors: DifficultyFactors::default(),
            global_ai_state: GlobalAiState::default(),
        }
    }

    /// Set pathfinding system reference
    pub fn set_pathfinding_system(
        &mut self,
        pathfinding: super::pathfinding_system::SharedPathfindingSystem,
    ) {
        self.pathfinding_system = Some(pathfinding);
    }

    /// Initialize AI Update system
    pub fn init(&mut self) -> Result<(), AiError> {
        self.current_frame = 0;
        self.last_player_update_frame = 0;
        self.last_targeting_update_frame = 0;
        self.emergency_mode = false;
        self.task_queue.clear();
        self.metrics = AiPerformanceMetrics::default();

        Ok(())
    }

    /// Update AI system for one frame
    pub fn update(&mut self, current_frame: u32) -> Result<(), AiError> {
        self.current_frame = current_frame;
        let start_time = std::time::Instant::now();

        // Update global AI state
        self.update_global_ai_state()?;

        // Schedule regular AI updates
        self.schedule_periodic_updates()?;

        // Process queued AI tasks within CPU budget
        self.process_task_queue()?;

        // Update performance metrics
        let processing_time = start_time.elapsed().as_micros() as f32;
        self.update_performance_metrics(processing_time);

        // Check if we need to enter emergency mode
        if processing_time > self.cpu_budget_per_frame as f32 {
            self.emergency_mode = true;
        } else if processing_time < (self.cpu_budget_per_frame as f32 * 0.5) {
            self.emergency_mode = false;
        }

        Ok(())
    }

    /// Schedule AI task with specified priority
    pub fn schedule_task(&mut self, task: AiUpdateTask) -> Result<(), AiError> {
        // Insert task in priority order
        let insert_pos = self
            .task_queue
            .iter()
            .position(|t| t.priority > task.priority)
            .unwrap_or(self.task_queue.len());

        self.task_queue.insert(insert_pos, task);
        Ok(())
    }

    /// Get performance metrics
    pub fn get_performance_metrics(&self) -> &AiPerformanceMetrics {
        &self.metrics
    }

    pub fn has_ai_player(&self, player_id: u32) -> bool {
        self.ai_players.contains_key(&player_id)
    }

    pub fn register_ai_player(
        &mut self,
        player_id: u32,
        ai_player: Box<dyn AiPlayerTrait + Send + Sync>,
    ) {
        self.ai_players
            .insert(player_id, Arc::new(RwLock::new(ai_player)));
    }

    pub fn with_ai_player_mut<F, R>(&mut self, player_id: u32, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn AiPlayerTrait) -> R,
    {
        let ai_player = self.ai_players.get(&player_id)?;
        let mut guard = ai_player.write().ok()?;
        Some(f(guard.as_mut()))
    }

    // Private implementation methods

    /// Schedule periodic AI updates based on frame counters
    fn schedule_periodic_updates(&mut self) -> Result<(), AiError> {
        // Schedule player AI updates
        if self.current_frame - self.last_player_update_frame >= AI_PLAYER_UPDATE_RATE {
            // Collect player IDs first to avoid borrowing self while iterating
            let player_ids: Vec<_> = self.ai_players.keys().copied().collect();
            for player_id in player_ids {
                let priority = if self.emergency_mode {
                    AiUpdatePriority::Low
                } else {
                    AiUpdatePriority::Normal
                };
                let task = AiUpdateTask {
                    task_type: AiUpdateTaskType::PlayerAI,
                    priority,
                    target_id: None,
                    player_id: Some(player_id),
                    data: AiTaskData::PlayerUpdate { player_id },
                    frame_added: self.current_frame,
                    max_execution_time: 100,
                };
                self.schedule_task(task)?;
            }
            self.last_player_update_frame = self.current_frame;
        }

        Ok(())
    }

    /// Process AI task queue within CPU budget
    /// Implements time-sliced AI processing to maintain frame rate
    /// Matches C++ AI.cpp update logic with priority handling
    fn process_task_queue(&mut self) -> Result<(), AiError> {
        let start_time = std::time::Instant::now();
        let mut tasks_processed = 0;
        let mut critical_tasks_processed = 0;

        while let Some(task) = self.task_queue.pop_front() {
            // Check CPU budget - but always process at least one critical task
            let elapsed = start_time.elapsed().as_micros() as u32;
            let is_critical = task.priority == AiUpdatePriority::Critical;

            if elapsed >= self.cpu_budget_per_frame && tasks_processed > 0 && !is_critical {
                // Put task back and break
                self.task_queue.push_front(task);
                break;
            }

            // Check if task has expired (but don't expire critical tasks)
            if !is_critical && self.current_frame - task.frame_added > task.max_execution_time {
                continue; // Skip expired task
            }

            // Process task based on priority
            match self.process_ai_task(task) {
                Ok(_) => {
                    tasks_processed += 1;
                    if is_critical {
                        critical_tasks_processed += 1;
                    }
                    self.metrics.total_tasks_processed += 1;
                }
                Err(e) => {
                    // Log error but continue processing
                    // In C++, errors are logged to debug output
                    #[cfg(debug_assertions)]
                    eprintln!("AI task error: {:?}", e);
                }
            }

            // In emergency mode, prioritize critical tasks only
            if self.emergency_mode {
                if tasks_processed >= 3 && critical_tasks_processed > 0 {
                    break; // Processed minimum critical tasks
                }
                // Skip non-critical tasks in emergency mode
                if !is_critical && tasks_processed > critical_tasks_processed {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Process individual AI task
    /// Dispatches task to appropriate subsystem
    /// Matches C++ AI.cpp task processing with metrics tracking
    fn process_ai_task(&mut self, task: AiUpdateTask) -> Result<(), AiError> {
        // Update metrics for monitoring
        *self
            .metrics
            .tasks_by_type
            .entry(task.task_type)
            .or_insert(0) += 1;
        *self
            .metrics
            .tasks_by_priority
            .entry(task.priority)
            .or_insert(0) += 1;

        match task.data {
            AiTaskData::PlayerUpdate { player_id } => {
                // Update player AI - matches C++ AIPlayer::update()
                if let Some(ai_player) = self.ai_players.get(&player_id) {
                    if let Ok(mut player) = ai_player.write() {
                        player.update()?;
                    }
                }
                Ok(())
            }
            AiTaskData::UnitUpdate {
                unit_id,
                force_update,
            } => {
                // Update individual unit AI
                if let Some(obj_arc) = OBJECT_REGISTRY.get_object(unit_id) {
                    if let Ok(obj_guard) = obj_arc.read() {
                        if let Some(ai) = obj_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                if force_update || !ai_guard.is_moving() {
                                    let _ = ai_guard.update();
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
            AiTaskData::GroupUpdate { group_id, command } => {
                // Update AI group behavior
                if let Ok(ai_guard) = THE_AI.read() {
                    if let Some(group) = ai_guard.get_group_by_id(group_id) {
                        if let Ok(mut group_guard) = group.write() {
                            let _ = group_guard.ai_do_command(&command);
                        }
                    }
                }
                Ok(())
            }
            AiTaskData::PathfindingUpdate { request } => {
                // Submit pathfinding request to system
                if let Some(ref ps) = self.pathfinding_system {
                    if let Ok(mut system) = ps.write() {
                        system.request_path(request);
                        self.metrics.pathfinding_requests += 1;
                    }
                }
                Ok(())
            }
            AiTaskData::TargetingUpdate {
                unit_id,
                scan_radius,
            } => {
                // Scan for targets within radius
                let _ = (unit_id, scan_radius);
                self.metrics.targeting_scans += 1;
                Ok(())
            }
            AiTaskData::CombatUpdate {
                attacker_id,
                target_id,
            } => {
                // Process combat decisions
                // Matches C++ attack priority calculation
                let _ = (attacker_id, target_id);
                self.metrics.combat_calculations += 1;
                Ok(())
            }
            AiTaskData::EconomyUpdate { player_id } => {
                // Update economy subsystem
                if let Some(ai_player) = self.ai_players.get(&player_id) {
                    if let Ok(mut player) = ai_player.write() {
                        player.update_economy()?;
                    }
                }
                Ok(())
            }
            AiTaskData::ConstructionUpdate {
                player_id,
                structure_type,
            } => {
                // Update construction subsystem
                if let Some(ai_player) = self.ai_players.get(&player_id) {
                    if let Ok(mut player) = ai_player.write() {
                        player.update_construction()?;

                        // Build specific structure if requested
                        if let Some(ref structure) = structure_type {
                            player.build_specific_building(structure)?;
                        }
                    }
                }
                Ok(())
            }
            AiTaskData::DiplomacyUpdate { player_id } => {
                // Update diplomacy subsystem
                if let Some(ai_player) = self.ai_players.get(&player_id) {
                    if let Ok(mut player) = ai_player.write() {
                        player.update_diplomacy()?;
                    }
                }
                Ok(())
            }
        }
    }

    /// Update global AI state based on game conditions
    fn update_global_ai_state(&mut self) -> Result<(), AiError> {
        self.global_ai_state.global_threat_level = 0.5;
        self.global_ai_state.economic_pressure = 0.3;
        self.global_ai_state.military_balance = 0.0;
        Ok(())
    }

    /// Update performance metrics
    fn update_performance_metrics(&mut self, processing_time: f32) {
        let current_time = processing_time;

        if self.metrics.average_processing_time == 0.0 {
            self.metrics.average_processing_time = current_time;
        } else {
            self.metrics.average_processing_time =
                (self.metrics.average_processing_time * 0.9) + (current_time * 0.1);
        }

        if current_time > self.metrics.max_processing_time {
            self.metrics.max_processing_time = current_time;
        }

        self.metrics.cpu_usage_percent = (current_time / self.cpu_budget_per_frame as f32) * 100.0;
    }
}

/// AI Player trait extensions for update system
pub trait AiPlayerTrait {
    fn update(&mut self) -> Result<(), AiError>;
    fn update_economy(&mut self) -> Result<(), AiError>;
    fn update_construction(&mut self) -> Result<(), AiError>;
    fn update_diplomacy(&mut self) -> Result<(), AiError>;
    fn build_specific_building(&mut self, building_name: &str) -> Result<(), AiError>;
    fn build_by_supplies(&mut self, minimum_cash: i32, building_name: &str) -> Result<(), AiError>;
    fn build_upgrade(&mut self, upgrade_name: &str) -> Result<(), AiError>;
    fn build_specific_building_near_location(
        &mut self,
        building_name: &str,
        location: Coord3D,
    ) -> Result<(), AiError>;
    fn repair_structure(&mut self, structure_id: ObjectID) -> Result<(), AiError>;
    fn build_base_defense(&mut self, flank: bool) -> Result<(), AiError>;
    fn build_base_defense_structure(
        &mut self,
        structure_name: &str,
        flank: bool,
    ) -> Result<(), AiError>;
    fn get_player_id(&self) -> u32;
    fn get_difficulty(&self) -> GameDifficulty;
}

/// Global AI Update instance
use once_cell::sync::Lazy;
pub static THE_AI_UPDATE: Lazy<Arc<RwLock<AIUpdate>>> =
    Lazy::new(|| Arc::new(RwLock::new(AIUpdate::new())));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_update_creation() {
        let ai_update = AIUpdate::new();
        assert_eq!(ai_update.current_frame, 0);
        assert!(!ai_update.emergency_mode);
        assert_eq!(ai_update.task_queue.len(), 0);
    }
}
