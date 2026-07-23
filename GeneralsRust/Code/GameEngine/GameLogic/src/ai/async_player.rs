//! Modern Async AI Player System
//! 
//! This module provides an advanced AI player implementation using async/await patterns,
//! parallel processing, and sophisticated decision-making algorithms.

use crate::{GameLogicResult, GameLogicError};
use super::{ObjectId, PlayerId, Coord3D, Real};
use crate::common::{KindOf, LOGICFRAMES_PER_SECOND};
use crate::helpers::TheGameLogic;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::player_list;

use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::task::{JoinHandle, JoinSet};
use async_trait::async_trait;

/// AI difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiDifficulty {
    Easy,
    Normal,
    Hard,
    Brutal,
}

/// AI personality types affecting behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiPersonality {
    /// Aggressive, attacks frequently
    Aggressive,
    /// Defensive, focuses on base building
    Defensive,
    /// Balanced approach
    Balanced,
    /// Economic focus
    Economic,
    /// Rush tactics
    Rusher,
    /// Turtle strategy
    Turtle,
}

/// Strategic goals for AI planning
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrategyGoal {
    /// Build up economy
    EconomicExpansion,
    /// Expand territory
    TerritorialExpansion,
    /// Military buildup
    MilitaryBuildup,
    /// Attack enemy
    OffensiveAction,
    /// Defend base
    DefensiveAction,
    /// Research upgrades
    TechnologicalAdvancement,
    /// Support allies
    AllySupport,
    /// Harass enemy
    Harassment,
}

/// AI decision context
#[derive(Debug, Clone)]
pub struct DecisionContext {
    /// Current game state snapshot
    pub game_state: GameStateSnapshot,
    /// Available resources
    pub resources: ResourceState,
    /// Current threats
    pub threat_assessment: ThreatAssessment,
    /// Strategic objectives
    pub objectives: Vec<StrategyGoal>,
    /// Time pressure factor
    pub urgency: f32,
}

/// Game state snapshot for AI decision making
#[derive(Debug, Clone)]
pub struct GameStateSnapshot {
    /// Current game time
    pub game_time: Duration,
    /// Owned units and buildings
    pub owned_objects: HashSet<ObjectId>,
    /// Enemy units in sight
    pub known_enemies: HashMap<ObjectId, EnemyInfo>,
    /// Resource locations
    pub resource_sites: Vec<ResourceSite>,
    /// Strategic points
    pub strategic_points: Vec<StrategyPoint>,
    /// Current map control percentage
    pub map_control: f32,
}

/// Resource state information
#[derive(Debug, Clone)]
pub struct ResourceState {
    /// Current money/credits
    pub money: i32,
    /// Money income rate
    pub income_rate: f32,
    /// Energy/power level
    pub power: i32,
    /// Supply count used/max
    pub supply_used: i32,
    pub supply_max: i32,
}

/// Threat assessment data
#[derive(Debug, Clone)]
pub struct ThreatAssessment {
    /// Overall threat level (0.0 to 1.0)
    pub overall_threat: f32,
    /// Immediate threats requiring attention
    pub immediate_threats: Vec<ThreatInfo>,
    /// Enemy force strength estimate
    pub enemy_strength: f32,
    /// Own force strength
    pub own_strength: f32,
    /// Threat trend (increasing/decreasing)
    pub threat_trend: f32,
}

/// Information about a specific threat
#[derive(Debug, Clone)]
pub struct ThreatInfo {
    /// Source of the threat
    pub threat_object: ObjectId,
    /// Threat position
    pub position: Coord3D,
    /// Threat level (0.0 to 1.0)
    pub threat_level: f32,
    /// Time when threat was detected
    pub detection_time: Instant,
    /// Recommended response
    pub recommended_response: ThreatResponse,
}

/// Recommended response to a threat
#[derive(Debug, Clone)]
pub enum ThreatResponse {
    /// Evacuate units from area
    Evacuate,
    /// Send counter-attack force
    CounterAttack,
    /// Reinforce defenses
    Reinforce,
    /// Use special ability
    SpecialAbility(String),
    /// Ignore (low priority)
    Ignore,
}

/// Enemy unit information
#[derive(Debug, Clone)]
pub struct EnemyInfo {
    /// Object ID
    pub object_id: ObjectId,
    /// Last known position
    pub position: Coord3D,
    /// Unit type/class
    pub unit_type: String,
    /// Estimated health
    pub health_estimate: f32,
    /// Last seen time
    pub last_seen: Instant,
    /// Movement pattern
    pub movement_pattern: MovementPattern,
}

/// Movement pattern analysis
#[derive(Debug, Clone)]
pub enum MovementPattern {
    /// Stationary
    Static,
    /// Moving in straight line
    Linear(Coord3D), // Direction vector
    /// Patrolling
    Patrol(Vec<Coord3D>),
    /// Circling
    Circular { center: Coord3D, radius: f32 },
    /// Erratic movement
    Erratic,
}

/// Resource site information
#[derive(Debug, Clone)]
pub struct ResourceSite {
    /// Position of resource
    pub position: Coord3D,
    /// Type of resource
    pub resource_type: String,
    /// Estimated remaining amount
    pub amount_remaining: i32,
    /// Whether site is contested
    pub contested: bool,
    /// Safety level for harvesting
    pub safety_level: f32,
}

/// Strategic point on map
#[derive(Debug, Clone)]
pub struct StrategyPoint {
    /// Position of point
    pub position: Coord3D,
    /// Type of strategic value
    pub point_type: StrategyPointType,
    /// Control status
    pub control_status: ControlStatus,
    /// Strategic value (0.0 to 1.0)
    pub strategic_value: f32,
}

/// Types of strategic points
#[derive(Debug, Clone)]
pub enum StrategyPointType {
    /// Chokepoint for movement
    Chokepoint,
    /// High ground advantage
    HighGround,
    /// Resource cluster
    ResourceCluster,
    /// Forward base location
    ForwardBase,
    /// Defensive position
    DefensivePosition,
}

/// Control status of strategic points
#[derive(Debug, Clone, Copy)]
pub enum ControlStatus {
    /// Under our control
    Friendly,
    /// Under enemy control
    Enemy,
    /// Neutral/uncontrolled
    Neutral,
    /// Contested area
    Contested,
}

/// AI task types
#[derive(Debug, Clone)]
pub enum AiTask {
    /// Build a specific structure
    BuildStructure {
        structure_type: String,
        location: Option<Coord3D>,
        priority: f32,
    },
    /// Train/produce units
    ProduceUnits {
        unit_type: String,
        quantity: u32,
        priority: f32,
    },
    /// Attack a target
    AttackTarget {
        target: ObjectId,
        force_size: u32,
        priority: f32,
    },
    /// Defend a location
    DefendLocation {
        location: Coord3D,
        force_size: u32,
        priority: f32,
    },
    /// Explore/scout area
    Scout {
        area: Coord3D,
        unit_count: u32,
        priority: f32,
    },
    /// Gather resources
    GatherResources {
        resource_site: Coord3D,
        harvester_count: u32,
        priority: f32,
    },
    /// Research upgrade
    Research {
        research_type: String,
        priority: f32,
    },
    /// Use special power
    UseSpecialPower {
        power_type: String,
        target: Option<ObjectId>,
        location: Option<Coord3D>,
        priority: f32,
    },
}

/// Task execution result
#[derive(Debug, Clone)]
pub enum TaskResult {
    /// Task completed successfully
    Success,
    /// Task failed
    Failed(String),
    /// Task still in progress
    InProgress,
    /// Task was cancelled
    Cancelled,
}

/// Modern async AI player
pub struct AsyncAiPlayer {
    /// Player ID
    player_id: PlayerId,
    /// AI difficulty setting
    difficulty: AiDifficulty,
    /// AI personality
    personality: AiPersonality,
    /// Current strategic goals
    strategy_goals: Arc<RwLock<Vec<StrategyGoal>>>,
    /// Active tasks
    active_tasks: Arc<RwLock<HashMap<u64, AiTask>>>,
    /// Task execution handles
    task_handles: Arc<Mutex<JoinSet<TaskExecutionResult>>>,
    /// Decision making context
    context: Arc<RwLock<DecisionContext>>,
    /// Performance metrics
    metrics: Arc<RwLock<AiMetrics>>,
    /// Communication channels
    task_sender: mpsc::UnboundedSender<AiTaskMessage>,
    task_receiver: Arc<Mutex<mpsc::UnboundedReceiver<AiTaskMessage>>>,
    /// Concurrency limiter
    task_semaphore: Arc<Semaphore>,
    /// Next task ID
    next_task_id: Arc<Mutex<u64>>,
}

/// Task execution result with metadata
#[derive(Debug)]
struct TaskExecutionResult {
    task_id: u64,
    result: TaskResult,
    execution_time: Duration,
}

/// AI performance metrics
#[derive(Debug, Clone)]
pub struct AiMetrics {
    /// Decisions per second
    pub decisions_per_second: f32,
    /// Average decision time
    pub avg_decision_time: Duration,
    /// Task success rate
    pub task_success_rate: f32,
    /// Resource efficiency
    pub resource_efficiency: f32,
    /// Combat effectiveness
    pub combat_effectiveness: f32,
    /// Strategic score
    pub strategic_score: f32,
}

/// Internal AI task message
#[derive(Debug)]
enum AiTaskMessage {
    /// Execute a new task
    ExecuteTask {
        task_id: u64,
        task: AiTask,
        response: oneshot::Sender<TaskResult>,
    },
    /// Cancel a task
    CancelTask {
        task_id: u64,
    },
    /// Update task priority
    UpdatePriority {
        task_id: u64,
        new_priority: f32,
    },
}

impl AsyncAiPlayer {
    /// Create a new async AI player
    pub fn new(
        player_id: PlayerId,
        difficulty: AiDifficulty,
        personality: AiPersonality,
    ) -> Self {
        let (task_sender, task_receiver) = mpsc::unbounded_channel();
        let max_concurrent_tasks = match difficulty {
            AiDifficulty::Easy => 5,
            AiDifficulty::Normal => 10,
            AiDifficulty::Hard => 20,
            AiDifficulty::Brutal => 50,
        };

        Self {
            player_id,
            difficulty,
            personality,
            strategy_goals: Arc::new(RwLock::new(Vec::new())),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            task_handles: Arc::new(Mutex::new(JoinSet::new())),
            context: Arc::new(RwLock::new(Self::create_initial_context())),
            metrics: Arc::new(RwLock::new(AiMetrics::default())),
            task_sender,
            task_receiver: Arc::new(Mutex::new(task_receiver)),
            task_semaphore: Arc::new(Semaphore::new(max_concurrent_tasks)),
            next_task_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Initialize the AI player
    pub async fn initialize(&self) -> GameLogicResult<()> {
        // Set initial strategic goals based on personality
        let initial_goals = self.determine_initial_goals();
        {
            let mut goals = self.strategy_goals.write()
                .map_err(|e| GameLogicError::Threading(format!("Failed to acquire strategy goals lock: {}", e)))?;
            *goals = initial_goals;
        }

        // Start the main AI decision loop
        self.start_decision_loop().await?;
        
        // Start task execution system
        self.start_task_executor().await?;

        Ok(())
    }

    /// Main AI update loop
    pub async fn update(&self, delta_time: f32) -> GameLogicResult<()> {
        // Update game state snapshot
        self.update_game_state().await?;
        
        // Evaluate threats
        self.evaluate_threats().await?;
        
        // Make strategic decisions
        self.make_strategic_decisions().await?;
        
        // Execute tactical actions
        self.execute_tactical_actions().await?;
        
        // Update performance metrics
        self.update_metrics(delta_time).await?;

        Ok(())
    }

    /// Start the main decision-making loop
    async fn start_decision_loop(&self) -> GameLogicResult<()> {
        let context = Arc::clone(&self.context);
        let goals = Arc::clone(&self.strategy_goals);
        let difficulty = self.difficulty;
        let personality = self.personality;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100)); // 10 FPS decision making
            
            loop {
                interval.tick().await;
                
                // Perform high-level strategic planning
                if let Err(e) = Self::strategic_planning_cycle(
                    &context,
                    &goals,
                    difficulty,
                    personality,
                ).await {
                    log::error!("Strategic planning error: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Start the task execution system
    async fn start_task_executor(&self) -> GameLogicResult<()> {
        let task_receiver = Arc::clone(&self.task_receiver);
        let active_tasks = Arc::clone(&self.active_tasks);
        let task_handles = Arc::clone(&self.task_handles);
        let semaphore = Arc::clone(&self.task_semaphore);

        tokio::spawn(async move {
            let mut receiver = task_receiver.lock().await;
            
            while let Some(message) = receiver.recv().await {
                match message {
                    AiTaskMessage::ExecuteTask { task_id, task, response } => {
                        let permit = semaphore.acquire().await.unwrap();
                        let task_clone = task.clone();
                        let active_tasks_clone = Arc::clone(&active_tasks);
                        let mut handles = task_handles.lock().await;

                        // Add task to active tasks
                        {
                            let mut tasks = active_tasks_clone.write().await;
                            tasks.insert(task_id, task);
                        }

                        // Spawn task execution
                        handles.spawn(async move {
                            let start_time = Instant::now();
                            let result = Self::execute_task_impl(task_clone).await;
                            let execution_time = start_time.elapsed();
                            
                            // Send result back
                            let _ = response.send(result.clone());
                            
                            // Remove from active tasks
                            {
                                let mut tasks = active_tasks_clone.write().await;
                                tasks.remove(&task_id);
                            }
                            
                            drop(permit);
                            
                            TaskExecutionResult {
                                task_id,
                                result,
                                execution_time,
                            }
                        });
                    },
                    AiTaskMessage::CancelTask { task_id } => {
                        let mut tasks = active_tasks.write().await;
                        tasks.remove(&task_id);
                    },
                    AiTaskMessage::UpdatePriority { task_id, new_priority: _ } => {
                        // Update task priority in queue
                        // Implementation would depend on priority queue structure
                    },
                }
            }
        });

        Ok(())
    }

    /// Strategic planning cycle
    async fn strategic_planning_cycle(
        context: &Arc<RwLock<DecisionContext>>,
        goals: &Arc<RwLock<Vec<StrategyGoal>>>,
        difficulty: AiDifficulty,
        personality: AiPersonality,
    ) -> GameLogicResult<()> {
        let context_snapshot = {
            let ctx = context.read()
                .map_err(|e| GameLogicError::Threading(format!("Failed to read context: {}", e)))?;
            ctx.clone()
        };

        // Analyze current situation
        let situation_analysis = Self::analyze_situation(&context_snapshot, difficulty).await?;
        
        // Determine new strategic goals
        let new_goals = Self::determine_strategic_goals(
            &situation_analysis,
            personality,
            &context_snapshot,
        ).await?;
        
        // Update goals
        {
            let mut current_goals = goals.write()
                .map_err(|e| GameLogicError::Threading(format!("Failed to write goals: {}", e)))?;
            *current_goals = new_goals;
        }

        Ok(())
    }

    /// Execute a task implementation
    async fn execute_task_impl(task: AiTask) -> TaskResult {
        match task {
            AiTask::BuildStructure { structure_type, location, priority: _ } => {
                // Simulate building construction
                tokio::time::sleep(Duration::from_millis(100)).await;
                log::info!("Building {} at {:?}", structure_type, location);
                TaskResult::Success
            },
            AiTask::ProduceUnits { unit_type, quantity, priority: _ } => {
                // Simulate unit production
                tokio::time::sleep(Duration::from_millis(50 * quantity as u64)).await;
                log::info!("Producing {} units of {}", quantity, unit_type);
                TaskResult::Success
            },
            AiTask::AttackTarget { target, force_size, priority: _ } => {
                // Simulate attack coordination
                tokio::time::sleep(Duration::from_millis(200)).await;
                log::info!("Attacking target {} with {} units", target, force_size);
                TaskResult::Success
            },
            AiTask::DefendLocation { location, force_size, priority: _ } => {
                // Simulate defense setup
                tokio::time::sleep(Duration::from_millis(150)).await;
                log::info!("Defending location {:?} with {} units", location, force_size);
                TaskResult::Success
            },
            AiTask::Scout { area, unit_count, priority: _ } => {
                // Simulate scouting
                tokio::time::sleep(Duration::from_millis(300)).await;
                log::info!("Scouting area {:?} with {} units", area, unit_count);
                TaskResult::Success
            },
            AiTask::GatherResources { resource_site, harvester_count, priority: _ } => {
                // Simulate resource gathering
                tokio::time::sleep(Duration::from_millis(100)).await;
                log::info!("Gathering resources at {:?} with {} harvesters", resource_site, harvester_count);
                TaskResult::Success
            },
            AiTask::Research { research_type, priority: _ } => {
                // Simulate research
                tokio::time::sleep(Duration::from_millis(500)).await;
                log::info!("Researching {}", research_type);
                TaskResult::Success
            },
            AiTask::UseSpecialPower { power_type, target, location, priority: _ } => {
                // Simulate special power usage
                tokio::time::sleep(Duration::from_millis(50)).await;
                log::info!("Using special power {} on {:?} at {:?}", power_type, target, location);
                TaskResult::Success
            },
        }
    }

    /// Submit a task for execution
    pub async fn submit_task(&self, task: AiTask) -> GameLogicResult<TaskResult> {
        let task_id = {
            let mut next_id = self.next_task_id.lock().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let (response_sender, response_receiver) = oneshot::channel();
        
        self.task_sender.send(AiTaskMessage::ExecuteTask {
            task_id,
            task,
            response: response_sender,
        }).map_err(|e| GameLogicError::Threading(format!("Failed to send task: {}", e)))?;

        response_receiver.await
            .map_err(|e| GameLogicError::Threading(format!("Failed to receive task result: {}", e)))
    }

    /// Update game state snapshot
    async fn update_game_state(&self) -> GameLogicResult<()> {
        let frame = TheGameLogic::get_frame();
        let game_time = Duration::from_secs_f32(frame as f32 / LOGICFRAMES_PER_SECOND as f32);

        let mut owned_objects = HashSet::new();
        let mut owned_positions = Vec::new();
        let mut known_enemies = HashMap::new();

        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned());

        if let Some(player) = player_arc.as_ref().and_then(|arc| arc.read().ok()) {
            owned_objects.extend(player.get_all_objects());
            for obj_id in &player.get_all_objects() {
                if let Some(pos) =
                    OBJECT_REGISTRY.with_object(*obj_id, |obj_guard| *obj_guard.get_position())
                {
                    owned_positions.push(pos);
                }
            }
        }

        let now = Instant::now();
        // Host path: dual-world factory empty — no enemy residual scan.
        if !OBJECT_REGISTRY.is_empty() {
        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
        let obj_arc = match OBJECT_REGISTRY.get_object(obj_id) {
            Some(v) => v,
            None => continue,
        };
            let Ok(obj_guard) = obj_arc.read() else { continue };
            if obj_guard.is_destroyed() {
                continue;
            }

            let Some(owner_id) = obj_guard.get_controlling_player_id() else { continue };
            if owner_id as u32 == self.player_id {
                continue;
            }

            let is_enemy = match player_arc.as_ref().and_then(|arc| arc.read().ok()) {
                Some(player_guard) => {
                    let list = player_list().read().ok();
                    let other = list.and_then(|list| list.get_player(owner_id as i32)).and_then(|p| p.read().ok());
                    matches!(other.as_ref().map(|p| player_guard.get_relationship(p)), Some(crate::common::Relationship::Enemies))
                }
                None => true,
            };

            if !is_enemy {
                continue;
            }

            let movement_pattern = if obj_guard.is_moving() {
                MovementPattern::Erratic
            } else {
                MovementPattern::Static
            };

            known_enemies.insert(obj_guard.get_id(), EnemyInfo {
                object_id: obj_guard.get_id(),
                position: *obj_guard.get_position(),
                unit_type: obj_guard.get_template_name().to_string(),
                health_estimate: obj_guard.get_health(),
                last_seen: now,
                movement_pattern,
            });
        }
        }

        let enemy_positions: Vec<Coord3D> = known_enemies.values().map(|info| info.position).collect();

        let mut resource_sites = Vec::new();
        // Host path: dual-world factory empty — no resource residual.
        if !OBJECT_REGISTRY.is_empty() {
        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
        let obj_arc = match OBJECT_REGISTRY.get_object(obj_id) {
            Some(v) => v,
            None => continue,
        };
            let Ok(obj_guard) = obj_arc.read() else { continue };
            if !(obj_guard.is_kind_of(KindOf::ResourceNode)
                || obj_guard.is_kind_of(KindOf::SupplySource)
                || obj_guard.is_kind_of(KindOf::SupplySourceOnPreview)) {
                continue;
            }

            let pos = *obj_guard.get_position();
            let mut nearest_enemy = None;
            for enemy_pos in &enemy_positions {
                let dist = (*enemy_pos - pos).length();
                nearest_enemy = Some(nearest_enemy.map_or(dist, |best| best.min(dist)));
            }

            let safety_level = nearest_enemy
                .map(|dist| (dist / 300.0).clamp(0.0, 1.0))
                .unwrap_or(1.0);
            let contested = nearest_enemy.map(|dist| dist < 200.0).unwrap_or(false);

            let resource_type = if obj_guard.is_kind_of(KindOf::SupplySource) {
                "Supply".to_string()
            } else if obj_guard.is_kind_of(KindOf::ResourceNode) {
                "ResourceNode".to_string()
            } else {
                obj_guard.get_template_name().to_string()
            };

            resource_sites.push(ResourceSite {
                position: pos,
                resource_type,
                amount_remaining: obj_guard.get_health().max(0.0) as i32,
                contested,
                safety_level,
            });
        }
        }

        let mut strategic_points = Vec::new();
        for site in &resource_sites {
            let mut control_status = ControlStatus::Neutral;
            if site.contested {
                control_status = ControlStatus::Contested;
            } else {
                let friendly_near = owned_positions.iter().any(|pos| (*pos - site.position).length() < 200.0);
                let enemy_near = enemy_positions.iter().any(|pos| (*pos - site.position).length() < 200.0);
                control_status = if friendly_near {
                    ControlStatus::Friendly
                } else if enemy_near {
                    ControlStatus::Enemy
                } else {
                    ControlStatus::Neutral
                };
            }

            strategic_points.push(StrategyPoint {
                position: site.position,
                point_type: StrategyPointType::ResourceCluster,
                control_status,
                strategic_value: (site.amount_remaining as f32 / 10000.0).clamp(0.1, 1.0),
            });
        }

        // Host path: empty dual-world registry → map_control from owned count only.
        let total_objects = if OBJECT_REGISTRY.is_empty() {
            owned_objects.len().max(1) as f32
        } else {
            OBJECT_REGISTRY.get_all_object_ids().len().max(1) as f32
        };
        let map_control = owned_objects.len() as f32 / total_objects;

        let new_snapshot = GameStateSnapshot {
            game_time,
            owned_objects,
            known_enemies,
            resource_sites,
            strategic_points,
            map_control,
        };

        let mut context = self.context.write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to write context: {}", e)))?;
        context.game_state = new_snapshot;
        if let Some(player) = player_arc.and_then(|arc| arc.read().ok()) {
            context.resources = ResourceState {
                money: player.get_money().get_money(),
                income_rate: player.get_money().get_income_rate(),
                power: player.get_energy().get_power() as i32,
                supply_used: 0,
                supply_max: 0,
            };
        }

        Ok(())
    }

    /// Evaluate current threats
    async fn evaluate_threats(&self) -> GameLogicResult<()> {
        let previous_overall = self.context.read()
            .ok()
            .map(|ctx| ctx.threat_assessment.overall_threat)
            .unwrap_or(0.0);

        let context = self.context.read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read context: {}", e)))?;
        let owned_objects = context.game_state.owned_objects.clone();
        let enemies = context.game_state.known_enemies.clone();
        drop(context);

        let mut own_strength = 0.0f32;
        for obj_id in owned_objects {
            if let Some(health) =
                OBJECT_REGISTRY.with_object(obj_id, |obj_guard| obj_guard.get_health())
            {
                own_strength += health;
            }
        }

        let enemy_strength: f32 = enemies.values().map(|info| info.health_estimate.max(0.0)).sum();
        let overall_threat = if own_strength + enemy_strength <= f32::EPSILON {
            0.0
        } else {
            (enemy_strength / (own_strength + enemy_strength)).clamp(0.0, 1.0)
        };

        let mut immediate_threats = Vec::new();
        let owned_positions: Vec<Coord3D> = OBJECT_REGISTRY
            .get_all_object_ids()
            .into_iter()
            .filter_map(|id| {
                OBJECT_REGISTRY.with_object(id, |g| {
                    (
                        g.get_controlling_player_id(),
                        *g.get_position(),
                    )
                })
            })
            .filter(|(owner, _)| owner.map(|id| id as u32 == self.player_id).unwrap_or(false))
            .map(|(_, pos)| pos)
            .collect();

        for enemy in enemies.values() {
            let mut nearest = None;
            for pos in &owned_positions {
                let dist = (*pos - enemy.position).length();
                nearest = Some(nearest.map_or(dist, |best| best.min(dist)));
            }

            let threat_radius = 250.0;
            if let Some(distance) = nearest {
                if distance < threat_radius {
                    let threat_level = (1.0 - (distance / threat_radius)).clamp(0.1, 1.0);
                    let response = if threat_level > 0.7 {
                        ThreatResponse::CounterAttack
                    } else {
                        ThreatResponse::Reinforce
                    };
                    immediate_threats.push(ThreatInfo {
                        threat_object: enemy.object_id,
                        position: enemy.position,
                        threat_level,
                        detection_time: Instant::now(),
                        recommended_response: response,
                    });
                }
            }
        }

        let threat_assessment = ThreatAssessment {
            overall_threat,
            immediate_threats,
            enemy_strength,
            own_strength,
            threat_trend: overall_threat - previous_overall,
        };

        let mut context = self.context.write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to write context: {}", e)))?;
        context.threat_assessment = threat_assessment;

        Ok(())
    }

    /// Make strategic decisions based on current context
    async fn make_strategic_decisions(&self) -> GameLogicResult<()> {
        let context = {
            let ctx = self.context.read()
                .map_err(|e| GameLogicError::Threading(format!("Failed to read context: {}", e)))?;
            ctx.clone()
        };

        // Generate tasks based on current situation and goals
        let tasks = self.generate_tasks_from_strategy(&context).await?;
        
        // Submit high-priority tasks
        for task in tasks.into_iter().take(5) { // Limit concurrent high-level tasks
            tokio::spawn({
                let ai = self.clone_for_task();
                async move {
                    if let Err(e) = ai.submit_task(task).await {
                        log::error!("Failed to submit strategic task: {}", e);
                    }
                }
            });
        }

        Ok(())
    }

    /// Execute tactical actions
    async fn execute_tactical_actions(&self) -> GameLogicResult<()> {
        // Handle immediate tactical responses
        let context = {
            let ctx = self.context.read()
                .map_err(|e| GameLogicError::Threading(format!("Failed to read context: {}", e)))?;
            ctx.clone()
        };

        // Respond to immediate threats
        for threat in &context.threat_assessment.immediate_threats {
            match &threat.recommended_response {
                ThreatResponse::CounterAttack => {
                    let task = AiTask::AttackTarget {
                        target: threat.threat_object,
                        force_size: 5,
                        priority: 0.9,
                    };
                    tokio::spawn({
                        let ai = self.clone_for_task();
                        async move {
                            if let Err(e) = ai.submit_task(task).await {
                                log::error!("Failed to submit counter-attack task: {}", e);
                            }
                        }
                    });
                },
                _ => {
                    // Handle other response types
                },
            }
        }

        Ok(())
    }

    /// Update performance metrics
    async fn update_metrics(&self, _delta_time: f32) -> GameLogicResult<()> {
        let active_count = self.active_tasks.read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read active tasks: {}", e)))?
            .len() as f32;

        let context = self.context.read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read context: {}", e)))?;

        let resource_efficiency = if context.resources.income_rate > 0.0 {
            (context.resources.money as f32 / (context.resources.income_rate * 60.0).max(1.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let combat_effectiveness = (1.0 - context.threat_assessment.overall_threat).clamp(0.0, 1.0);
        let strategic_score = (context.game_state.map_control + resource_efficiency + combat_effectiveness) / 3.0;

        let new_metrics = AiMetrics {
            decisions_per_second: active_count.max(1.0),
            avg_decision_time: Duration::from_millis((1000.0 / active_count.max(1.0)) as u64),
            task_success_rate: if active_count == 0.0 { 1.0 } else { 0.8 },
            resource_efficiency,
            combat_effectiveness,
            strategic_score,
        };

        let mut metrics = self.metrics.write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to write metrics: {}", e)))?;
        *metrics = new_metrics;

        Ok(())
    }

    /// Helper methods
    fn determine_initial_goals(&self) -> Vec<StrategyGoal> {
        match self.personality {
            AiPersonality::Aggressive => vec![
                StrategyGoal::MilitaryBuildup,
                StrategyGoal::OffensiveAction,
                StrategyGoal::EconomicExpansion,
            ],
            AiPersonality::Defensive => vec![
                StrategyGoal::DefensiveAction,
                StrategyGoal::EconomicExpansion,
                StrategyGoal::TechnologicalAdvancement,
            ],
            AiPersonality::Economic => vec![
                StrategyGoal::EconomicExpansion,
                StrategyGoal::TerritorialExpansion,
                StrategyGoal::TechnologicalAdvancement,
            ],
            _ => vec![
                StrategyGoal::EconomicExpansion,
                StrategyGoal::MilitaryBuildup,
                StrategyGoal::TechnologicalAdvancement,
            ],
        }
    }

    fn create_initial_context() -> DecisionContext {
        DecisionContext {
            game_state: GameStateSnapshot {
                game_time: Duration::from_secs(0),
                owned_objects: HashSet::new(),
                known_enemies: HashMap::new(),
                resource_sites: Vec::new(),
                strategic_points: Vec::new(),
                map_control: 0.0,
            },
            resources: ResourceState {
                money: 10000,
                income_rate: 0.0,
                power: 0,
                supply_used: 0,
                supply_max: 0,
            },
            threat_assessment: ThreatAssessment {
                overall_threat: 0.0,
                immediate_threats: Vec::new(),
                enemy_strength: 0.0,
                own_strength: 0.0,
                threat_trend: 0.0,
            },
            objectives: Vec::new(),
            urgency: 0.0,
        }
    }

    async fn analyze_situation(
        context: &DecisionContext,
        _difficulty: AiDifficulty,
    ) -> GameLogicResult<SituationAnalysis> {
        let economic_pressure = if context.resources.income_rate <= 0.0 {
            1.0
        } else {
            (1.0 - (context.resources.money as f32 / (context.resources.income_rate * 30.0 + 1.0))).clamp(0.0, 1.0)
        };

        let expansion_opportunity = (1.0 - context.game_state.map_control).clamp(0.0, 1.0);
        let threat_level = context.threat_assessment.overall_threat;

        Ok(SituationAnalysis {
            threat_level,
            economic_pressure,
            expansion_opportunity,
        })
    }

    async fn determine_strategic_goals(
        analysis: &SituationAnalysis,
        personality: AiPersonality,
        _context: &DecisionContext,
    ) -> GameLogicResult<Vec<StrategyGoal>> {
        let mut goals = Vec::new();

        if analysis.economic_pressure > 0.4 {
            goals.push(StrategyGoal::EconomicExpansion);
        }
        if analysis.expansion_opportunity > 0.5 {
            goals.push(StrategyGoal::TerritorialExpansion);
        }
        if analysis.threat_level > 0.5 {
            goals.push(StrategyGoal::DefensiveAction);
            goals.push(StrategyGoal::MilitaryBuildup);
        }

        match personality {
            AiPersonality::Aggressive | AiPersonality::Rusher => {
                goals.push(StrategyGoal::OffensiveAction);
            }
            AiPersonality::Defensive | AiPersonality::Turtle => {
                goals.push(StrategyGoal::DefensiveAction);
            }
            AiPersonality::Economic => {
                goals.push(StrategyGoal::TechnologicalAdvancement);
                goals.push(StrategyGoal::EconomicExpansion);
            }
            AiPersonality::Balanced => {
                goals.push(StrategyGoal::MilitaryBuildup);
                goals.push(StrategyGoal::TechnologicalAdvancement);
            }
        }

        let mut seen = HashSet::new();
        goals.retain(|goal| seen.insert(goal.clone()));
        Ok(goals)
    }

    async fn generate_tasks_from_strategy(
        &self,
        context: &DecisionContext,
    ) -> GameLogicResult<Vec<AiTask>> {
        let mut tasks = Vec::new();

        let best_resource = context
            .game_state
            .resource_sites
            .iter()
            .filter(|site| site.safety_level > 0.3)
            .max_by(|a, b| a.amount_remaining.cmp(&b.amount_remaining));

        for goal in &context.objectives {
            match goal {
                StrategyGoal::EconomicExpansion => {
                    if let Some(site) = best_resource {
                        tasks.push(AiTask::GatherResources {
                            resource_site: site.position,
                            harvester_count: 3,
                            priority: 0.9,
                        });
                        tasks.push(AiTask::BuildStructure {
                            structure_type: "SupplyCenter".to_string(),
                            location: Some(site.position),
                            priority: 0.8,
                        });
                    } else {
                        tasks.push(AiTask::BuildStructure {
                            structure_type: "SupplyCenter".to_string(),
                            location: None,
                            priority: 0.7,
                        });
                    }
                }
                StrategyGoal::MilitaryBuildup => {
                    tasks.push(AiTask::ProduceUnits {
                        unit_type: "Infantry".to_string(),
                        quantity: 5,
                        priority: 0.8,
                    });
                }
                StrategyGoal::OffensiveAction => {
                    if let Some(enemy) = context.game_state.known_enemies.values().next() {
                        tasks.push(AiTask::AttackTarget {
                            target: enemy.object_id,
                            force_size: 6,
                            priority: 0.9,
                        });
                    }
                }
                StrategyGoal::DefensiveAction => {
                    if let Some(pos) = context.game_state.owned_objects.iter()
                        .filter_map(|id| {
                            OBJECT_REGISTRY.with_object(*id, |g| *g.get_position())
                        })
                        .next() {
                        tasks.push(AiTask::DefendLocation {
                            location: pos,
                            force_size: 4,
                            priority: 0.85,
                        });
                    }
                }
                StrategyGoal::TechnologicalAdvancement => {
                    tasks.push(AiTask::Research {
                        research_type: "Upgrade1".to_string(),
                        priority: 0.6,
                    });
                }
                StrategyGoal::TerritorialExpansion => {
                    if let Some(point) = context.game_state.strategic_points.iter().find(|p| matches!(p.control_status, ControlStatus::Neutral)) {
                        tasks.push(AiTask::Scout {
                            area: point.position,
                            unit_count: 2,
                            priority: 0.6,
                        });
                    }
                }
                StrategyGoal::Harassment => {
                    if let Some(enemy) = context.game_state.known_enemies.values().next() {
                        tasks.push(AiTask::AttackTarget {
                            target: enemy.object_id,
                            force_size: 2,
                            priority: 0.7,
                        });
                    }
                }
                StrategyGoal::AllySupport => {}
            }
        }

        Ok(tasks)
    }

    /// Create a clone suitable for task execution
    fn clone_for_task(&self) -> AsyncAiPlayerHandle {
        AsyncAiPlayerHandle {
            task_sender: self.task_sender.clone(),
            next_task_id: Arc::clone(&self.next_task_id),
        }
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> GameLogicResult<AiMetrics> {
        let metrics = self.metrics.read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read metrics: {}", e)))?;
        Ok(metrics.clone())
    }

    /// Get current strategic goals
    pub async fn get_strategic_goals(&self) -> GameLogicResult<Vec<StrategyGoal>> {
        let goals = self.strategy_goals.read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read goals: {}", e)))?;
        Ok(goals.clone())
    }
}

/// Lightweight handle for submitting tasks
#[derive(Clone)]
struct AsyncAiPlayerHandle {
    task_sender: mpsc::UnboundedSender<AiTaskMessage>,
    next_task_id: Arc<Mutex<u64>>,
}

impl AsyncAiPlayerHandle {
    async fn submit_task(&self, task: AiTask) -> GameLogicResult<TaskResult> {
        let task_id = {
            let mut next_id = self.next_task_id.lock().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let (response_sender, response_receiver) = oneshot::channel();
        
        self.task_sender.send(AiTaskMessage::ExecuteTask {
            task_id,
            task,
            response: response_sender,
        }).map_err(|e| GameLogicError::Threading(format!("Failed to send task: {}", e)))?;

        response_receiver.await
            .map_err(|e| GameLogicError::Threading(format!("Failed to receive task result: {}", e)))
    }
}

/// Situation analysis result
#[derive(Debug, Default)]
struct SituationAnalysis {
    threat_level: f32,
    economic_pressure: f32,
    expansion_opportunity: f32,
}

impl Default for AiMetrics {
    fn default() -> Self {
        Self {
            decisions_per_second: 0.0,
            avg_decision_time: Duration::from_millis(0),
            task_success_rate: 0.0,
            resource_efficiency: 0.0,
            combat_effectiveness: 0.0,
            strategic_score: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ai_player_creation() {
        let ai = AsyncAiPlayer::new(1, AiDifficulty::Normal, AiPersonality::Balanced);
        assert_eq!(ai.player_id, 1);
        assert_eq!(ai.difficulty, AiDifficulty::Normal);
        assert_eq!(ai.personality, AiPersonality::Balanced);
    }

    #[tokio::test]
    async fn test_task_submission() {
        let ai = AsyncAiPlayer::new(1, AiDifficulty::Normal, AiPersonality::Balanced);
        ai.initialize().await.unwrap();
        
        let task = AiTask::BuildStructure {
            structure_type: "TestBuilding".to_string(),
            location: None,
            priority: 0.5,
        };

        let result = ai.submit_task(task).await.unwrap();
        assert!(matches!(result, TaskResult::Success));
    }

    #[tokio::test]
    async fn test_metrics_access() {
        let ai = AsyncAiPlayer::new(1, AiDifficulty::Hard, AiPersonality::Aggressive);
        ai.initialize().await.unwrap();
        
        let metrics = ai.get_metrics().await.unwrap();
        assert!(metrics.decisions_per_second >= 0.0);
    }
}
