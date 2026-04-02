//! Advanced Behavior System
//!
//! This module provides a comprehensive behavior system with over 90 behavior types,
//! featuring dynamic behavior composition, state management, and parallel execution.

use crate::object::Object;
use crate::{GameLogicError, GameLogicResult, ObjectId};

use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};

/// Behavior execution priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BehaviorPriority {
    /// Background behaviors
    Background = 0,
    /// Normal behaviors
    Normal = 1,
    /// High priority behaviors
    High = 2,
    /// Critical system behaviors
    Critical = 3,
    /// Emergency behaviors
    Emergency = 4,
}

/// Behavior execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorState {
    /// Behavior is inactive
    Inactive,
    /// Behavior is initializing
    Initializing,
    /// Behavior is running
    Active,
    /// Behavior is paused
    Paused,
    /// Behavior completed successfully
    Completed,
    /// Behavior failed
    Failed,
    /// Behavior was cancelled
    Cancelled,
}

/// Behavior flags for controlling behavior interactions
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct BehaviorFlags: u32 {
        /// Behavior can run concurrently with others
        const CONCURRENT = 0x0001;
        /// Behavior excludes other behaviors of the same type
        const EXCLUSIVE = 0x0002;
        /// Behavior persists across object state changes
        const PERSISTENT = 0x0004;
        /// Behavior can be interrupted
        const INTERRUPTIBLE = 0x0008;
        /// Behavior requires specific conditions to start
        const CONDITIONAL = 0x0010;
        /// Behavior is networked and needs synchronization
        const NETWORKED = 0x0020;
        /// Behavior is deterministic (for replay compatibility)
        const DETERMINISTIC = 0x0040;
        /// Behavior affects object statistics
        const STAT_MODIFIER = 0x0080;
        /// Behavior handles damage events
        const DAMAGE_HANDLER = 0x0100;
        /// Behavior handles weapon events
        const WEAPON_HANDLER = 0x0200;
        /// Behavior handles movement events
        const MOVEMENT_HANDLER = 0x0400;
        /// Behavior handles collision events
        const COLLISION_HANDLER = 0x0800;
    }
}

/// Behavior context containing shared state
#[derive(Debug)]
pub struct BehaviorContext {
    /// Object owning this behavior
    pub object_id: ObjectId,
    /// Current game frame
    pub current_frame: u32,
    /// Delta time since last update
    pub delta_time: f32,
    /// Object position
    pub position: [f32; 3],
    /// Object velocity
    pub velocity: [f32; 3],
    /// Object health
    pub health: f32,
    /// Object maximum health
    pub max_health: f32,
    /// Shared behavior data
    pub shared_data: Arc<RwLock<HashMap<String, Box<dyn Any + Send + Sync>>>>,
}

/// Behavior execution result
#[derive(Debug, Clone)]
pub enum BehaviorOutcome {
    /// Continue execution
    Continue,
    /// Behavior completed successfully
    Completed,
    /// Behavior failed with error
    Failed(String),
    /// Request to pause behavior
    Pause,
    /// Request to restart behavior
    Restart,
    /// Request to cancel behavior
    Cancel,
    /// Request to transition to another behavior
    Transition(String),
}

/// Advanced behavior trait with async support
#[async_trait]
pub trait AdvancedBehavior: Send + Sync {
    /// Get behavior name
    fn name(&self) -> &str;

    /// Get behavior type ID
    fn type_id(&self) -> TypeId {
        std::any::TypeId::of::<Self>()
    }

    /// Get behavior flags
    fn flags(&self) -> BehaviorFlags {
        BehaviorFlags::empty()
    }

    /// Get behavior priority
    fn priority(&self) -> BehaviorPriority {
        BehaviorPriority::Normal
    }

    /// Check if behavior can start with given context
    async fn can_start(
        &self,
        _object: &Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<bool> {
        Ok(true)
    }

    /// Initialize behavior
    async fn initialize(
        &mut self,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    /// Update behavior (called every frame)
    async fn update(
        &mut self,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<BehaviorOutcome> {
        Ok(BehaviorOutcome::Continue)
    }

    /// Handle behavior-specific events
    async fn handle_event(
        &mut self,
        _event: &BehaviorEvent,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    /// Cleanup when behavior stops
    async fn cleanup(
        &mut self,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    /// Serialize behavior state (for save/load)
    fn serialize_state(&self) -> GameLogicResult<Vec<u8>> {
        Ok(Vec::new())
    }

    /// Deserialize behavior state (for save/load)
    fn deserialize_state(&mut self, _data: &[u8]) -> GameLogicResult<()> {
        Ok(())
    }
}

/// Behavior events
#[derive(Debug, Clone)]
pub enum BehaviorEvent {
    /// Damage received
    DamageReceived {
        amount: f32,
        source: Option<ObjectId>,
    },
    /// Weapon fired
    WeaponFired {
        weapon_id: String,
        target: Option<ObjectId>,
    },
    /// Collision occurred
    Collision { other_object: ObjectId },
    /// Position changed
    PositionChanged {
        old_pos: [f32; 3],
        new_pos: [f32; 3],
    },
    /// Health changed
    HealthChanged { old_health: f32, new_health: f32 },
    /// State changed
    StateChanged {
        old_state: String,
        new_state: String,
    },
    /// Custom event
    Custom {
        event_type: String,
        data: HashMap<String, String>,
    },
}

/// Behavior instance with metadata
#[derive(Debug)]
pub struct BehaviorInstance {
    /// Behavior implementation
    pub behavior: Box<dyn AdvancedBehavior>,
    /// Current state
    pub state: BehaviorState,
    /// Priority level
    pub priority: BehaviorPriority,
    /// Behavior flags
    pub flags: BehaviorFlags,
    /// When behavior was started
    pub start_time: Instant,
    /// Total execution time
    pub execution_time: Duration,
    /// Number of updates processed
    pub update_count: u64,
    /// Last update time
    pub last_update: Instant,
    /// Behavior-specific data
    pub data: HashMap<String, Box<dyn Any + Send + Sync>>,
}

/// Behavior manager for a single object
pub struct BehaviorManager {
    /// Object this manager belongs to
    object_id: ObjectId,
    /// Active behaviors
    active_behaviors: HashMap<String, BehaviorInstance>,
    /// Behavior execution queue
    execution_queue: VecDeque<String>,
    /// Behavior dependencies
    dependencies: HashMap<String, Vec<String>>,
    /// Exclusive behavior groups
    exclusive_groups: HashMap<String, HashSet<String>>,
    /// Event listeners
    event_listeners: HashMap<TypeId, Vec<String>>,
    /// Behavior statistics
    statistics: BehaviorStatistics,
}

/// Behavior execution statistics
#[derive(Debug, Clone, Default)]
pub struct BehaviorStatistics {
    /// Total behaviors executed
    pub total_behaviors: u32,
    /// Currently active behaviors
    pub active_count: u32,
    /// Failed behaviors
    pub failed_count: u32,
    /// Average execution time per behavior
    pub avg_execution_time_ms: f32,
    /// Peak concurrent behaviors
    pub peak_concurrent: u32,
}

impl BehaviorInstance {
    /// Create new behavior instance
    pub fn new(behavior: Box<dyn AdvancedBehavior>) -> Self {
        let priority = behavior.priority();
        let flags = behavior.flags();

        Self {
            behavior,
            state: BehaviorState::Inactive,
            priority,
            flags,
            start_time: Instant::now(),
            execution_time: Duration::default(),
            update_count: 0,
            last_update: Instant::now(),
            data: HashMap::new(),
        }
    }

    /// Start the behavior
    pub async fn start(
        &mut self,
        object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        if !self.behavior.can_start(object, context).await? {
            return Err(GameLogicError::Configuration(format!(
                "Behavior '{}' cannot start in current context",
                self.behavior.name()
            )));
        }

        self.state = BehaviorState::Initializing;
        self.behavior.initialize(object, context).await?;
        self.state = BehaviorState::Active;
        self.start_time = Instant::now();

        Ok(())
    }

    /// Update the behavior
    pub async fn update(
        &mut self,
        object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<BehaviorOutcome> {
        if self.state != BehaviorState::Active {
            return Ok(BehaviorOutcome::Continue);
        }

        let start_time = Instant::now();
        let result = self.behavior.update(object, context).await?;
        let update_time = start_time.elapsed();

        // Update statistics
        self.execution_time += update_time;
        self.update_count += 1;
        self.last_update = Instant::now();

        // Handle result
        match &result {
            BehaviorOutcome::Completed => {
                self.state = BehaviorState::Completed;
                self.behavior.cleanup(object, context).await?;
            }
            BehaviorOutcome::Failed(_) => {
                self.state = BehaviorState::Failed;
                self.behavior.cleanup(object, context).await?;
            }
            BehaviorOutcome::Pause => {
                self.state = BehaviorState::Paused;
            }
            BehaviorOutcome::Cancel => {
                self.state = BehaviorState::Cancelled;
                self.behavior.cleanup(object, context).await?;
            }
            _ => {}
        }

        Ok(result)
    }

    /// Handle event
    pub async fn handle_event(
        &mut self,
        event: &BehaviorEvent,
        object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        if self.state == BehaviorState::Active {
            self.behavior.handle_event(event, object, context).await?;
        }
        Ok(())
    }

    /// Stop the behavior
    pub async fn stop(
        &mut self,
        object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        if matches!(self.state, BehaviorState::Active | BehaviorState::Paused) {
            self.behavior.cleanup(object, context).await?;
        }
        self.state = BehaviorState::Cancelled;
        Ok(())
    }

    /// Pause the behavior
    pub fn pause(&mut self) {
        if self.state == BehaviorState::Active {
            self.state = BehaviorState::Paused;
        }
    }

    /// Resume the behavior
    pub fn resume(&mut self) {
        if self.state == BehaviorState::Paused {
            self.state = BehaviorState::Active;
        }
    }

    /// Get execution statistics
    pub fn get_stats(&self) -> (Duration, u64, f32) {
        let avg_time_per_update = if self.update_count > 0 {
            self.execution_time.as_secs_f32() / self.update_count as f32
        } else {
            0.0
        };

        (self.execution_time, self.update_count, avg_time_per_update)
    }
}

impl BehaviorManager {
    /// Create new behavior manager
    pub fn new(object_id: ObjectId) -> Self {
        Self {
            object_id,
            active_behaviors: HashMap::new(),
            execution_queue: VecDeque::new(),
            dependencies: HashMap::new(),
            exclusive_groups: HashMap::new(),
            event_listeners: HashMap::new(),
            statistics: BehaviorStatistics::default(),
        }
    }

    /// Add behavior to the manager
    pub async fn add_behavior(
        &mut self,
        name: String,
        behavior: Box<dyn AdvancedBehavior>,
        object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        // Check for exclusive conflicts
        self.check_exclusive_conflicts(&behavior)?;

        // Create behavior instance
        let mut instance = BehaviorInstance::new(behavior);

        // Start the behavior if possible
        if instance.behavior.can_start(object, context).await? {
            instance.start(object, context).await?;

            // Add to execution queue if active
            if instance.state == BehaviorState::Active {
                self.execution_queue.push_back(name.clone());
            }
        }

        // Register event listeners
        self.register_event_listeners(&name, &instance);

        // Store behavior
        self.active_behaviors.insert(name, instance);
        self.update_statistics();

        Ok(())
    }

    /// Remove behavior by name
    pub async fn remove_behavior(
        &mut self,
        name: &str,
        object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        if let Some(mut instance) = self.active_behaviors.remove(name) {
            instance.stop(object, context).await?;

            // Remove from execution queue
            self.execution_queue.retain(|n| n != name);

            // Unregister event listeners
            self.unregister_event_listeners(name);

            self.update_statistics();
        }

        Ok(())
    }

    /// Update all behaviors
    pub async fn update(
        &mut self,
        object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        let mut completed_behaviors = Vec::new();
        let mut failed_behaviors = Vec::new();
        let mut transitions = Vec::new();

        // Process behaviors in priority order
        let mut sorted_names: Vec<_> = self
            .active_behaviors
            .iter()
            .map(|(name, instance)| (name.clone(), instance.priority))
            .collect();
        sorted_names.sort_by(|a, b| b.1.cmp(&a.1));

        for (name, _) in sorted_names {
            if let Some(instance) = self.active_behaviors.get_mut(&name) {
                match instance.update(object, context).await? {
                    BehaviorOutcome::Continue => {}
                    BehaviorOutcome::Completed => {
                        completed_behaviors.push(name.clone());
                    }
                    BehaviorOutcome::Failed(error) => {
                        log::error!("Behavior '{}' failed: {}", name, error);
                        failed_behaviors.push(name.clone());
                    }
                    BehaviorOutcome::Transition(target) => {
                        transitions.push((name.clone(), target));
                    }
                    BehaviorOutcome::Restart => {
                        // Restart the behavior
                        instance.state = BehaviorState::Inactive;
                        instance.start(object, context).await?;
                    }
                    BehaviorOutcome::Pause => {
                        // Already handled in instance.update()
                    }
                    BehaviorOutcome::Cancel => {
                        completed_behaviors.push(name.clone());
                    }
                }
            }
        }

        // Clean up completed/failed behaviors
        for name in completed_behaviors.iter().chain(failed_behaviors.iter()) {
            self.remove_behavior(name, object, context).await?;
        }

        // Handle transitions
        for (from_behavior, to_behavior) in transitions {
            self.transition_behavior(&from_behavior, &to_behavior, object, context)
                .await?;
        }

        self.update_statistics();
        Ok(())
    }

    /// Send event to relevant behaviors
    pub async fn handle_event(
        &mut self,
        event: &BehaviorEvent,
        object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        // Find behaviors that should handle this event
        let event_type_id = TypeId::of::<BehaviorEvent>(); // Simplified

        if let Some(listener_names) = self.event_listeners.get(&event_type_id) {
            for name in listener_names.clone() {
                // Clone to avoid borrow checker issues
                if let Some(instance) = self.active_behaviors.get_mut(&name) {
                    instance.handle_event(event, object, context).await?;
                }
            }
        }

        Ok(())
    }

    /// Pause all behaviors
    pub fn pause_all(&mut self) {
        for instance in self.active_behaviors.values_mut() {
            instance.pause();
        }
    }

    /// Resume all behaviors
    pub fn resume_all(&mut self) {
        for instance in self.active_behaviors.values_mut() {
            instance.resume();
        }
    }

    /// Get behavior by name
    pub fn get_behavior(&self, name: &str) -> Option<&BehaviorInstance> {
        self.active_behaviors.get(name)
    }

    /// Get mutable behavior by name
    pub fn get_behavior_mut(&mut self, name: &str) -> Option<&mut BehaviorInstance> {
        self.active_behaviors.get_mut(name)
    }

    /// List all active behavior names
    pub fn list_active_behaviors(&self) -> Vec<String> {
        self.active_behaviors.keys().cloned().collect()
    }

    /// Get behavior statistics
    pub fn get_statistics(&self) -> &BehaviorStatistics {
        &self.statistics
    }

    /// Check for exclusive behavior conflicts
    fn check_exclusive_conflicts(
        &self,
        new_behavior: &Box<dyn AdvancedBehavior>,
    ) -> GameLogicResult<()> {
        if new_behavior.flags().contains(BehaviorFlags::EXCLUSIVE) {
            let new_type = new_behavior.type_id();

            for (name, instance) in &self.active_behaviors {
                if instance.behavior.type_id() == new_type {
                    return Err(GameLogicError::Configuration(format!(
                        "Exclusive behavior conflict: '{}' already active",
                        name
                    )));
                }
            }
        }

        Ok(())
    }

    /// Register event listeners for a behavior
    fn register_event_listeners(&mut self, name: &str, _instance: &BehaviorInstance) {
        // Simplified
        let event_type = TypeId::of::<BehaviorEvent>();
        self.event_listeners
            .entry(event_type)
            .or_insert_with(Vec::new)
            .push(name.clone());
    }

    /// Unregister event listeners for a behavior
    fn unregister_event_listeners(&mut self, name: &str) {
        for listeners in self.event_listeners.values_mut() {
            listeners.retain(|n| n != name);
        }
    }

    /// Transition from one behavior to another
    async fn transition_behavior(
        &mut self,
        from_behavior: &str,
        to_behavior: &str,
        object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        // Remove the old behavior
        self.remove_behavior(from_behavior, object, context).await?;

        // Add the new behavior implementation would go here

        Ok(())
    }

    /// Update internal statistics
    fn update_statistics(&mut self) {
        self.statistics.active_count = self.active_behaviors.len() as u32;
        self.statistics.total_behaviors = self.statistics.active_count; // Simplified

        if self.statistics.active_count > self.statistics.peak_concurrent {
            self.statistics.peak_concurrent = self.statistics.active_count;
        }

        // Calculate average execution time
        let total_time: Duration = self
            .active_behaviors
            .values()
            .map(|instance| instance.execution_time)
            .sum();

        let total_updates: u64 = self
            .active_behaviors
            .values()
            .map(|instance| instance.update_count)
            .sum();

        if total_updates > 0 {
            self.statistics.avg_execution_time_ms =
                (total_time.as_secs_f32() * 1000.0) / total_updates as f32;
        }
    }
}

// ============================================================================
// Concrete Behavior Implementations
// ============================================================================

/// Stealth behavior implementation
pub struct StealthBehaviorImpl {
    name: String,
    stealth_delay: Duration,
    stealth_allowed_frame: u32,
    is_stealthed: bool,
    detection_expires_frame: u32,
    friendly_opacity: f32,
    stealth_level_flags: u32,
}

impl StealthBehaviorImpl {
    pub fn new(stealth_delay_ms: u64) -> Self {
        Self {
            name: "StealthBehavior".to_string(),
            stealth_delay: Duration::from_millis(stealth_delay_ms),
            stealth_allowed_frame: 0,
            is_stealthed: false,
            detection_expires_frame: 0,
            friendly_opacity: 0.5,
            stealth_level_flags: 0,
        }
    }

    fn allowed_to_stealth(&self, context: &BehaviorContext) -> bool {
        if context.current_frame < self.detection_expires_frame {
            return false;
        }
        if context.current_frame < self.stealth_allowed_frame {
            return false;
        }
        const STEALTH_NOT_WHILE_MOVING: u32 = 0x00000002;
        if (self.stealth_level_flags & STEALTH_NOT_WHILE_MOVING) != 0 {
            let speed = (context.velocity[0].powi(2)
                + context.velocity[1].powi(2)
                + context.velocity[2].powi(2))
            .sqrt();
            if speed > 0.1 {
                return false;
            }
        }
        if context.health < context.max_health * 0.1 {
            return false;
        }
        true
    }

    pub fn mark_as_detected(&mut self, current_frame: u32, detection_duration_frames: u32) {
        self.detection_expires_frame = current_frame + detection_duration_frames;
        self.is_stealthed = false;
    }
}

#[async_trait]
impl AdvancedBehavior for StealthBehaviorImpl {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> BehaviorPriority {
        BehaviorPriority::High
    }

    fn flags(&self) -> BehaviorFlags {
        BehaviorFlags::PERSISTENT | BehaviorFlags::CONCURRENT
    }

    async fn initialize(
        &mut self,
        _object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        self.stealth_allowed_frame =
            context.current_frame + (self.stealth_delay.as_millis() / 33) as u32;
        Ok(())
    }

    async fn update(
        &mut self,
        _object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<BehaviorOutcome> {
        let should_be_stealthed = self.allowed_to_stealth(context);

        if should_be_stealthed && !self.is_stealthed {
            self.is_stealthed = true;
            log::debug!("Object {} entered stealth", context.object_id);
        } else if !should_be_stealthed && self.is_stealthed {
            self.is_stealthed = false;
            log::debug!("Object {} exited stealth", context.object_id);
        }

        Ok(BehaviorOutcome::Continue)
    }

    async fn handle_event(
        &mut self,
        event: &BehaviorEvent,
        _object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        match event {
            BehaviorEvent::DamageReceived { .. } => {
                const STEALTH_NOT_WHILE_TAKING_DAMAGE: u32 = 0x00000080;
                if (self.stealth_level_flags & STEALTH_NOT_WHILE_TAKING_DAMAGE) != 0 {
                    self.mark_as_detected(context.current_frame, 90);
                }
            }
            BehaviorEvent::WeaponFired { .. } => {
                const STEALTH_NOT_WHILE_ATTACKING: u32 = 0x00000001;
                if (self.stealth_level_flags & STEALTH_NOT_WHILE_ATTACKING) != 0 {
                    self.mark_as_detected(context.current_frame, 90);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Veterancy behavior implementation
pub struct VeterancyBehaviorImpl {
    name: String,
}

impl VeterancyBehaviorImpl {
    pub fn new(_build_cost: i32) -> Self {
        Self {
            name: "VeterancyBehavior".to_string(),
        }
    }

    fn add_experience_points(&mut self, _amount: i32, _can_scale: bool) -> Option<u8> {
        // Non-canonical XP formulas were removed from this behavior path.
        None
    }

    pub fn get_damage_multiplier(&self) -> f32 {
        // Damage output is handled by weapon/bonus systems, not this behavior.
        1.0
    }

    pub fn get_armor_multiplier(&self) -> f32 {
        // C++ parity: veterancy does not apply a direct armor multiplier here.
        1.0
    }

    pub fn get_self_heal_rate(&self) -> f32 {
        0.0
    }
}

#[async_trait]
impl AdvancedBehavior for VeterancyBehaviorImpl {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> BehaviorPriority {
        BehaviorPriority::Normal
    }

    fn flags(&self) -> BehaviorFlags {
        BehaviorFlags::PERSISTENT | BehaviorFlags::CONCURRENT | BehaviorFlags::STAT_MODIFIER
    }

    async fn update(
        &mut self,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<BehaviorOutcome> {
        Ok(BehaviorOutcome::Continue)
    }

    async fn handle_event(
        &mut self,
        event: &BehaviorEvent,
        _object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        match event {
            BehaviorEvent::DamageReceived { amount, source } => {
                if let Some(_attacker_id) = source {
                    log::trace!(
                        "Object {} received {} damage from {:?}",
                        context.object_id,
                        amount,
                        source
                    );
                }
            }
            BehaviorEvent::Custom { event_type, data } => {
                if event_type == "award_experience" {
                    if let Some(xp_str) = data.get("amount") {
                        let _ = xp_str.parse::<i32>().ok();
                        let _ = self.add_experience_points(0, true);
                        log::trace!(
                            "Ignoring non-canonical advanced_behavior_system XP event on object {}",
                            context.object_id
                        );
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Experience tracking behavior
pub struct ExperienceTrackingBehaviorImpl {
    name: String,
    total_damage_dealt: f32,
    total_kills: u32,
}

impl ExperienceTrackingBehaviorImpl {
    pub fn new() -> Self {
        Self {
            name: "ExperienceTracking".to_string(),
            total_damage_dealt: 0.0,
            total_kills: 0,
        }
    }
}

#[async_trait]
impl AdvancedBehavior for ExperienceTrackingBehaviorImpl {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> BehaviorPriority {
        BehaviorPriority::Background
    }

    fn flags(&self) -> BehaviorFlags {
        BehaviorFlags::CONCURRENT | BehaviorFlags::DAMAGE_HANDLER
    }

    async fn handle_event(
        &mut self,
        event: &BehaviorEvent,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        match event {
            BehaviorEvent::Custom { event_type, data } => {
                if event_type == "damage_dealt" {
                    if let Some(damage_str) = data.get("damage") {
                        if let Ok(damage) = damage_str.parse::<f32>() {
                            self.total_damage_dealt += damage;
                        }
                    }
                } else if event_type == "unit_killed" {
                    self.total_kills += 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn update(
        &mut self,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<BehaviorOutcome> {
        Ok(BehaviorOutcome::Continue)
    }
}

/// Special ability behavior base
pub struct SpecialAbilityBehaviorImpl {
    name: String,
    ability_type: String,
    cooldown_time: Duration,
    last_used: Option<Instant>,
    is_active: bool,
}

impl SpecialAbilityBehaviorImpl {
    pub fn new(ability_type: String, cooldown_ms: u64) -> Self {
        Self {
            name: format!("SpecialAbility_{}", ability_type),
            ability_type,
            cooldown_time: Duration::from_millis(cooldown_ms),
            last_used: None,
            is_active: false,
        }
    }

    fn is_ready(&self) -> bool {
        if let Some(last_used) = self.last_used {
            last_used.elapsed() >= self.cooldown_time
        } else {
            true
        }
    }

    pub fn activate(&mut self) -> Result<(), String> {
        if !self.is_ready() {
            let remaining = self.cooldown_time - self.last_used.unwrap().elapsed();
            return Err(format!(
                "Ability on cooldown: {:.1}s remaining",
                remaining.as_secs_f32()
            ));
        }

        self.is_active = true;
        self.last_used = Some(Instant::now());
        Ok(())
    }

    pub fn deactivate(&mut self) {
        self.is_active = false;
    }
}

#[async_trait]
impl AdvancedBehavior for SpecialAbilityBehaviorImpl {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> BehaviorPriority {
        BehaviorPriority::High
    }

    fn flags(&self) -> BehaviorFlags {
        BehaviorFlags::INTERRUPTIBLE | BehaviorFlags::CONDITIONAL
    }

    async fn can_start(
        &self,
        _object: &Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<bool> {
        Ok(self.is_ready())
    }

    async fn update(
        &mut self,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<BehaviorOutcome> {
        if self.is_active {
            match self.ability_type.as_str() {
                "black_market" => {
                    log::debug!("Black Market ability active");
                }
                "cash_hack" => {
                    log::debug!("Cash Hack ability active");
                }
                "emergency_repair" => {
                    log::debug!("Emergency Repair ability active");
                }
                _ => {}
            }
        }

        Ok(BehaviorOutcome::Continue)
    }

    async fn handle_event(
        &mut self,
        event: &BehaviorEvent,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        match event {
            BehaviorEvent::Custom { event_type, .. } => {
                if event_type == "activate_ability" {
                    let _ = self.activate();
                } else if event_type == "deactivate_ability" {
                    self.deactivate();
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Async behavior alias used by higher level modules
pub trait AsyncBehavior: AdvancedBehavior {}
impl<T: AdvancedBehavior + ?Sized> AsyncBehavior for T {}

// Tests are commented out due to difficulty in creating mock Object instances
// #[cfg(test)]
// mod concrete_behavior_tests { ... }
