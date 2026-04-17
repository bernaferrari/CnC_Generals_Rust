//! Script Events System
//!
//! This module manages game events that can trigger scripts and conditions.

use super::{ScriptPriority, ScriptValue};
use crate::common::ICoord3D;
use crate::terrain::get_terrain_logic;
use crate::{GameLogicError, GameLogicResult};

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

/// Game event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GameEventType {
    // Unit events
    UnitCreated,
    UnitDestroyed,
    UnitDamaged,
    UnitMoved,
    UnitAttacked,
    UnitUpgraded,
    UnitPromoted,
    UnitEntersArea,
    UnitLeavesArea,
    UnitIdle,
    UnitGarrisoned,
    UnitEvacuated,

    // Building events
    BuildingConstructed,
    BuildingDestroyed,
    BuildingDamaged,
    BuildingCaptured,
    BuildingUpgraded,
    BuildingPowerChanged,

    // Player events
    PlayerDefeated,
    PlayerVictorious,
    PlayerResourceChanged,
    PlayerTechnologyResearched,
    PlayerUpgradeGranted,
    PlayerAllianceChanged,

    // Combat events
    CombatStarted,
    CombatEnded,
    WeaponFired,
    DamageDealt,
    CriticalHit,
    UnitKilled,

    // Special power events
    SpecialPowerActivated,
    SpecialPowerReady,
    SpecialPowerOnCooldown,

    // Map events
    AreaRevealed,
    AreaShrouded,
    WeatherChanged,
    TimeOfDayChanged,

    // Game state events
    GameStarted,
    GamePaused,
    GameResumed,
    GameEnded,
    VictoryConditionMet,
    ObjectiveCompleted,
    ObjectiveFailed,

    // Script events
    ScriptExecuted,
    ScriptFailed,
    VariableChanged,
    TimerExpired,

    // Custom events
    Custom(String),
}

/// Game event data
#[derive(Debug, Clone)]
pub struct GameEvent {
    /// Event type
    pub event_type: GameEventType,
    /// Event parameters
    pub parameters: HashMap<String, ScriptValue>,
    /// Timestamp when event occurred
    pub timestamp: Instant,
    /// Event priority
    pub priority: ScriptPriority,
    /// Source object (if applicable)
    pub source_object: Option<u32>,
    /// Target object (if applicable)
    pub target_object: Option<u32>,
    /// Player involved (if applicable)
    pub player_id: Option<u32>,
    /// Event description
    pub description: String,
}

/// Event filter for subscribers
#[derive(Debug, Clone)]
pub struct EventFilter {
    /// Event types to match
    pub event_types: Vec<GameEventType>,
    /// Player filter (None = all players)
    pub player_id: Option<u32>,
    /// Object filter (None = all objects)
    pub object_id: Option<u32>,
    /// Parameter filters
    pub parameter_filters: HashMap<String, ScriptValue>,
    /// Priority threshold
    pub min_priority: ScriptPriority,
}

/// Event subscriber trait
pub trait EventSubscriber: Send + Sync {
    /// Called when a matching event occurs
    fn on_event(&self, event: &GameEvent) -> GameLogicResult<()>;

    /// Get the event filter for this subscriber
    fn get_filter(&self) -> EventFilter;

    /// Get subscriber name for debugging
    fn get_name(&self) -> String;
}

/// Event history entry
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct EventHistoryEntry {
    event: GameEvent,
    processed: bool,
}

/// Game event manager
pub struct EventManager {
    /// Event queue
    event_queue: Arc<Mutex<VecDeque<GameEvent>>>,
    /// Event subscribers
    subscribers: Arc<RwLock<Vec<Box<dyn EventSubscriber>>>>,
    /// Event history (for lookups and debugging)
    event_history: Arc<RwLock<VecDeque<EventHistoryEntry>>>,
    /// Maximum history size
    max_history_size: usize,
    /// Event statistics
    statistics: Arc<RwLock<EventStatistics>>,
}

/// Event processing statistics
#[derive(Debug, Clone, Default)]
pub struct EventStatistics {
    /// Total events processed
    pub total_events: u64,
    /// Events processed by type
    pub events_by_type: HashMap<GameEventType, u64>,
    /// Average processing time per event
    pub avg_processing_time_ms: f64,
    /// Events in queue
    pub queued_events: u32,
    /// Active subscribers
    pub active_subscribers: u32,
    /// Failed events
    pub failed_events: u64,
}

impl GameEvent {
    /// Create a new game event
    pub fn new(event_type: GameEventType, description: String) -> Self {
        Self {
            event_type,
            parameters: HashMap::new(),
            timestamp: Instant::now(),
            priority: ScriptPriority::Normal,
            source_object: None,
            target_object: None,
            player_id: None,
            description,
        }
    }

    /// Builder pattern methods
    pub fn with_parameter(mut self, key: String, value: ScriptValue) -> Self {
        self.parameters.insert(key, value);
        self
    }

    pub fn with_priority(mut self, priority: ScriptPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_source_object(mut self, object_id: u32) -> Self {
        self.source_object = Some(object_id);
        self
    }

    pub fn with_target_object(mut self, object_id: u32) -> Self {
        self.target_object = Some(object_id);
        self
    }

    pub fn with_player(mut self, player_id: u32) -> Self {
        self.player_id = Some(player_id);
        self
    }

    /// Get parameter value
    pub fn get_parameter(&self, key: &str) -> Option<&ScriptValue> {
        self.parameters.get(key)
    }

    /// Get parameter as integer
    pub fn get_int_parameter(&self, key: &str) -> Option<i64> {
        match self.parameters.get(key) {
            Some(ScriptValue::Int(i)) => Some(*i),
            Some(ScriptValue::Float(f)) => Some(*f as i64),
            _ => None,
        }
    }

    /// Get parameter as float
    pub fn get_float_parameter(&self, key: &str) -> Option<f64> {
        match self.parameters.get(key) {
            Some(ScriptValue::Float(f)) => Some(*f),
            Some(ScriptValue::Int(i)) => Some(*i as f64),
            _ => None,
        }
    }

    /// Get parameter as string
    pub fn get_string_parameter(&self, key: &str) -> Option<&String> {
        match self.parameters.get(key) {
            Some(ScriptValue::String(s)) => Some(s),
            _ => None,
        }
    }
}

impl EventFilter {
    /// Create a new event filter
    pub fn new() -> Self {
        Self {
            event_types: Vec::new(),
            player_id: None,
            object_id: None,
            parameter_filters: HashMap::new(),
            min_priority: ScriptPriority::Low,
        }
    }

    /// Filter by event type
    pub fn with_event_type(mut self, event_type: GameEventType) -> Self {
        self.event_types.push(event_type);
        self
    }

    /// Filter by event types
    pub fn with_event_types(mut self, event_types: Vec<GameEventType>) -> Self {
        self.event_types.extend(event_types);
        self
    }

    /// Filter by player
    pub fn with_player(mut self, player_id: u32) -> Self {
        self.player_id = Some(player_id);
        self
    }

    /// Filter by object
    pub fn with_object(mut self, object_id: u32) -> Self {
        self.object_id = Some(object_id);
        self
    }

    /// Filter by parameter
    pub fn with_parameter_filter(mut self, key: String, value: ScriptValue) -> Self {
        self.parameter_filters.insert(key, value);
        self
    }

    /// Filter by minimum priority
    pub fn with_min_priority(mut self, priority: ScriptPriority) -> Self {
        self.min_priority = priority;
        self
    }

    /// Check if event matches this filter
    pub fn matches(&self, event: &GameEvent) -> bool {
        // Check event type
        if !self.event_types.is_empty() && !self.event_types.contains(&event.event_type) {
            return false;
        }

        // Check player
        if let Some(filter_player) = self.player_id {
            if event.player_id != Some(filter_player) {
                return false;
            }
        }

        // Check object
        if let Some(filter_object) = self.object_id {
            if event.source_object != Some(filter_object)
                && event.target_object != Some(filter_object)
            {
                return false;
            }
        }

        // Check priority
        if event.priority < self.min_priority {
            return false;
        }

        // Check parameters
        for (key, expected_value) in &self.parameter_filters {
            if let Some(actual_value) = event.parameters.get(key) {
                if actual_value != expected_value {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

impl EventManager {
    /// Create a new event manager
    pub fn new() -> Self {
        Self {
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
            subscribers: Arc::new(RwLock::new(Vec::new())),
            event_history: Arc::new(RwLock::new(VecDeque::new())),
            max_history_size: 10000,
            statistics: Arc::new(RwLock::new(EventStatistics::default())),
        }
    }

    /// Fire an event
    pub async fn fire_event(&self, event: GameEvent) -> GameLogicResult<()> {
        log::debug!(
            "Firing event: {:?} - {}",
            event.event_type,
            event.description
        );

        // Add to queue
        {
            let mut queue = self.event_queue.lock().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire event queue lock: {}", e))
            })?;
            queue.push_back(event.clone());
        }

        // Update statistics
        {
            let mut stats = self.statistics.write().map_err(|e| {
                GameLogicError::Threading(format!("Failed to write statistics: {}", e))
            })?;
            stats.queued_events += 1;
        }

        Ok(())
    }

    /// Fire an event synchronously.
    ///
    /// This is equivalent to `fire_event(...).await` and exists primarily for
    /// scripting integrations that must expose a synchronous API (e.g. Rhai).
    pub fn fire_event_sync(&self, event: GameEvent) -> GameLogicResult<()> {
        log::debug!(
            "Firing event: {:?} - {}",
            event.event_type,
            event.description
        );

        {
            let mut queue = self.event_queue.lock().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire event queue lock: {}", e))
            })?;
            queue.push_back(event.clone());
        }

        {
            let mut stats = self.statistics.write().map_err(|e| {
                GameLogicError::Threading(format!("Failed to write statistics: {}", e))
            })?;
            stats.queued_events += 1;
        }

        Ok(())
    }

    /// Process all queued events
    pub async fn process_events(&self) -> GameLogicResult<()> {
        let events_to_process = {
            let mut queue = self.event_queue.lock().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire event queue lock: {}", e))
            })?;
            let events: Vec<_> = queue.drain(..).collect();
            events
        };

        if events_to_process.is_empty() {
            return Ok(());
        }

        // Sort events by priority
        let mut sorted_events = events_to_process;
        sorted_events.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Process each event
        for event in sorted_events {
            let start_time = Instant::now();

            if let Err(e) = self.process_single_event(&event).await {
                log::error!("Failed to process event {:?}: {}", event.event_type, e);

                // Update failure statistics
                let mut stats = self.statistics.write().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to write statistics: {}", e))
                })?;
                stats.failed_events += 1;
            }

            // Add to history
            self.add_to_history(event, true).await?;

            // Update timing statistics
            let processing_time = start_time.elapsed();
            self.update_timing_statistics(processing_time).await?;
        }

        Ok(())
    }

    /// Process a single event
    async fn process_single_event(&self, event: &GameEvent) -> GameLogicResult<()> {
        let subscribers = self
            .subscribers
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read subscribers: {}", e)))?;

        // Find matching subscribers
        let matching_subscribers: Vec<_> = subscribers
            .iter()
            .filter(|subscriber| subscriber.get_filter().matches(event))
            .collect();

        // Notify subscribers
        for subscriber in matching_subscribers {
            if let Err(e) = subscriber.on_event(event) {
                log::warn!(
                    "Subscriber '{}' failed to process event: {}",
                    subscriber.get_name(),
                    e
                );
            }
        }

        // Update statistics
        let mut stats = self
            .statistics
            .write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to write statistics: {}", e)))?;

        stats.total_events += 1;
        stats.queued_events = stats.queued_events.saturating_sub(1);

        let count = stats
            .events_by_type
            .entry(event.event_type.clone())
            .or_insert(0);
        *count += 1;

        Ok(())
    }

    /// Add event subscriber
    pub async fn subscribe(&self, subscriber: Box<dyn EventSubscriber>) -> GameLogicResult<()> {
        log::info!("Adding event subscriber: {}", subscriber.get_name());

        let mut subscribers = self.subscribers.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to write subscribers: {}", e))
        })?;

        subscribers.push(subscriber);

        // Update statistics
        let mut stats = self
            .statistics
            .write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to write statistics: {}", e)))?;
        stats.active_subscribers = subscribers.len() as u32;

        Ok(())
    }

    /// Remove all subscribers matching a filter
    pub async fn unsubscribe_by_name(&self, name: &str) -> GameLogicResult<usize> {
        let mut subscribers = self.subscribers.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to write subscribers: {}", e))
        })?;

        let original_len = subscribers.len();
        subscribers.retain(|subscriber| subscriber.get_name() != name);
        let removed_count = original_len - subscribers.len();

        if removed_count > 0 {
            log::info!("Removed {} subscribers with name '{}'", removed_count, name);

            // Update statistics
            let mut stats = self.statistics.write().map_err(|e| {
                GameLogicError::Threading(format!("Failed to write statistics: {}", e))
            })?;
            stats.active_subscribers = subscribers.len() as u32;
        }

        Ok(removed_count)
    }

    /// Get events from history matching criteria
    pub async fn query_history(
        &self,
        filter: &EventFilter,
        max_results: usize,
    ) -> GameLogicResult<Vec<GameEvent>> {
        let history = self
            .event_history
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read history: {}", e)))?;

        let matching_events: Vec<_> = history
            .iter()
            .rev() // Most recent first
            .filter(|entry| filter.matches(&entry.event))
            .take(max_results)
            .map(|entry| entry.event.clone())
            .collect();

        Ok(matching_events)
    }

    /// Check if event occurred recently
    pub async fn has_recent_event(
        &self,
        filter: &EventFilter,
        within_seconds: f64,
    ) -> GameLogicResult<bool> {
        let history = self
            .event_history
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read history: {}", e)))?;

        let cutoff_time = Instant::now() - std::time::Duration::from_secs_f64(within_seconds);

        let found = history
            .iter()
            .rev() // Most recent first
            .any(|entry| entry.event.timestamp >= cutoff_time && filter.matches(&entry.event));

        Ok(found)
    }

    /// Get current statistics
    pub async fn get_statistics(&self) -> GameLogicResult<EventStatistics> {
        let stats = self
            .statistics
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read statistics: {}", e)))?;
        Ok(stats.clone())
    }

    /// Clear event history
    pub async fn clear_history(&self) -> GameLogicResult<()> {
        let mut history = self
            .event_history
            .write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to write history: {}", e)))?;

        let cleared_count = history.len();
        history.clear();

        log::info!("Cleared {} events from history", cleared_count);
        Ok(())
    }

    /// Add event to history
    async fn add_to_history(&self, event: GameEvent, processed: bool) -> GameLogicResult<()> {
        let mut history = self
            .event_history
            .write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to write history: {}", e)))?;

        // Add new entry
        history.push_back(EventHistoryEntry { event, processed });

        // Trim history if too large
        while history.len() > self.max_history_size {
            history.pop_front();
        }

        Ok(())
    }

    /// Update timing statistics
    async fn update_timing_statistics(
        &self,
        processing_time: std::time::Duration,
    ) -> GameLogicResult<()> {
        let mut stats = self
            .statistics
            .write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to write statistics: {}", e)))?;

        let processing_time_ms = processing_time.as_secs_f64() * 1000.0;

        // Simple moving average
        if stats.avg_processing_time_ms == 0.0 {
            stats.avg_processing_time_ms = processing_time_ms;
        } else {
            stats.avg_processing_time_ms =
                (stats.avg_processing_time_ms * 0.95) + (processing_time_ms * 0.05);
        }

        Ok(())
    }
}

/// Built-in event creation helpers
impl EventManager {
    /// Fire unit created event
    pub async fn fire_unit_created(
        &self,
        unit_id: u32,
        unit_type: String,
        player_id: u32,
        position: [f32; 3],
    ) -> GameLogicResult<()> {
        let event = GameEvent::new(
            GameEventType::UnitCreated,
            format!(
                "Unit {} ({}) created for player {}",
                unit_id, unit_type, player_id
            ),
        )
        .with_source_object(unit_id)
        .with_player(player_id)
        .with_parameter("unit_type".to_string(), ScriptValue::String(unit_type))
        .with_parameter("position".to_string(), ScriptValue::Coord3D(position));

        self.fire_event(event).await
    }

    /// Fire unit destroyed event
    pub async fn fire_unit_destroyed(
        &self,
        unit_id: u32,
        unit_type: String,
        player_id: u32,
        killer_id: Option<u32>,
    ) -> GameLogicResult<()> {
        let mut event = GameEvent::new(
            GameEventType::UnitDestroyed,
            format!("Unit {} ({}) destroyed", unit_id, unit_type),
        )
        .with_source_object(unit_id)
        .with_player(player_id)
        .with_parameter("unit_type".to_string(), ScriptValue::String(unit_type));

        if let Some(killer) = killer_id {
            event = event
                .with_target_object(killer)
                .with_parameter("killer_id".to_string(), ScriptValue::ObjectId(killer));
        }

        self.fire_event(event).await
    }

    /// Fire player defeated event
    pub async fn fire_player_defeated(
        &self,
        player_id: u32,
        reason: String,
    ) -> GameLogicResult<()> {
        let event = GameEvent::new(
            GameEventType::PlayerDefeated,
            format!("Player {} defeated: {}", player_id, reason),
        )
        .with_player(player_id)
        .with_parameter("reason".to_string(), ScriptValue::String(reason))
        .with_priority(ScriptPriority::High);

        self.fire_event(event).await
    }

    /// Fire objective completed event
    pub async fn fire_objective_completed(
        &self,
        player_id: u32,
        objective_id: String,
        objective_name: String,
    ) -> GameLogicResult<()> {
        let event = GameEvent::new(
            GameEventType::ObjectiveCompleted,
            format!(
                "Objective '{}' completed by player {}",
                objective_name, player_id
            ),
        )
        .with_player(player_id)
        .with_parameter(
            "objective_id".to_string(),
            ScriptValue::String(objective_id),
        )
        .with_parameter(
            "objective_name".to_string(),
            ScriptValue::String(objective_name),
        )
        .with_priority(ScriptPriority::High);

        self.fire_event(event).await
    }
}

impl Default for EventManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Named object tracker for script system
pub struct NamedObjectTracker {
    /// Map from object name to object ID
    name_to_id: Arc<RwLock<HashMap<String, u32>>>,
    /// Map from object ID to name
    id_to_name: Arc<RwLock<HashMap<u32, String>>>,
    /// Names that have existed at least once (used for ScriptConditions::didUnitExist parity)
    name_history: Arc<RwLock<HashSet<String>>>,
}

impl NamedObjectTracker {
    /// Create a new named object tracker
    pub fn new() -> Self {
        Self {
            name_to_id: Arc::new(RwLock::new(HashMap::new())),
            id_to_name: Arc::new(RwLock::new(HashMap::new())),
            name_history: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Register a named object
    pub fn register_named_object(&self, name: String, object_id: u32) -> GameLogicResult<()> {
        let mut name_map = self.name_to_id.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire name map lock: {}", e))
        })?;

        let mut id_map = self.id_to_name.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire ID map lock: {}", e))
        })?;

        // Remove old mapping if object was previously named
        if let Some(old_name) = id_map.insert(object_id, name.clone()) {
            name_map.remove(&old_name);
        }

        // Add new mapping
        name_map.insert(name.clone(), object_id);
        if let Ok(mut history) = self.name_history.write() {
            history.insert(name.clone());
        }
        log::debug!("Registered named object: {} -> {}", name, object_id);

        Ok(())
    }

    /// Unregister an object
    pub fn unregister_object(&self, object_id: u32) -> GameLogicResult<()> {
        let mut id_map = self.id_to_name.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire ID map lock: {}", e))
        })?;

        if let Some(name) = id_map.remove(&object_id) {
            let mut name_map = self.name_to_id.write().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire name map lock: {}", e))
            })?;
            name_map.remove(&name);
            log::debug!("Unregistered named object: {} (ID: {})", name, object_id);
        }

        Ok(())
    }

    /// Get object ID by name
    pub fn get_object_id(&self, name: &str) -> GameLogicResult<Option<u32>> {
        let name_map = self.name_to_id.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire name map lock: {}", e))
        })?;

        Ok(name_map.get(name).copied())
    }

    /// Get object name by ID
    pub fn get_object_name(&self, object_id: u32) -> GameLogicResult<Option<String>> {
        let id_map = self.id_to_name.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire ID map lock: {}", e))
        })?;

        Ok(id_map.get(&object_id).cloned())
    }

    /// Check if object is named
    pub fn is_named(&self, object_id: u32) -> GameLogicResult<bool> {
        let id_map = self.id_to_name.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire ID map lock: {}", e))
        })?;

        Ok(id_map.contains_key(&object_id))
    }

    /// Get all named objects
    pub fn get_all_named_objects(&self) -> GameLogicResult<Vec<(String, u32)>> {
        let name_map = self.name_to_id.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire name map lock: {}", e))
        })?;

        Ok(name_map
            .iter()
            .map(|(name, id)| (name.clone(), *id))
            .collect())
    }

    /// Clear all tracked named objects and history.
    pub fn clear(&self) -> GameLogicResult<()> {
        let mut name_map = self.name_to_id.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire name map lock: {}", e))
        })?;
        let mut id_map = self.id_to_name.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire ID map lock: {}", e))
        })?;
        let mut history = self.name_history.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire name history lock: {}", e))
        })?;

        name_map.clear();
        id_map.clear();
        history.clear();
        Ok(())
    }

    /// Check whether a named object has ever been registered.
    pub fn did_object_exist(&self, name: &str) -> GameLogicResult<bool> {
        let history = self.name_history.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire name history lock: {}", e))
        })?;
        Ok(history.contains(name))
    }
}

/// Area definition for trigger regions
#[derive(Debug, Clone)]
pub struct TriggerArea {
    /// Area name
    pub name: String,
    /// Center position
    pub center: [f32; 3],
    /// Radius for circular areas
    pub radius: Option<f32>,
    /// Rectangular bounds (min_x, min_y, max_x, max_y)
    pub bounds: Option<[f32; 4]>,
    /// Is area active
    pub active: bool,
    /// Player filter (None = all players)
    pub player_filter: Option<u32>,
    /// Polygon trigger name for terrain-backed areas
    pub polygon_name: Option<String>,
}

impl TriggerArea {
    /// Create a new circular trigger area
    pub fn new_circular(name: String, center: [f32; 3], radius: f32) -> Self {
        Self {
            name,
            center,
            radius: Some(radius),
            bounds: None,
            active: true,
            player_filter: None,
            polygon_name: None,
        }
    }

    /// Create a new rectangular trigger area
    pub fn new_rectangular(
        name: String,
        center: [f32; 3],
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
    ) -> Self {
        Self {
            name,
            center,
            radius: None,
            bounds: Some([min_x, min_y, max_x, max_y]),
            active: true,
            player_filter: None,
            polygon_name: None,
        }
    }

    /// Check if position is inside the area
    pub fn contains_position(&self, position: [f32; 3]) -> bool {
        if !self.active {
            return false;
        }

        if let Some(radius) = self.radius {
            // Circular area
            let dx = position[0] - self.center[0];
            let dy = position[1] - self.center[1];
            let distance_sq = dx * dx + dy * dy;
            distance_sq <= radius * radius
        } else if let Some([min_x, min_y, max_x, max_y]) = self.bounds {
            // Rectangular area
            position[0] >= min_x
                && position[0] <= max_x
                && position[1] >= min_y
                && position[1] <= max_y
        } else {
            false
        }
    }
}

/// Area tracker for enter/exit events
pub struct AreaTracker {
    /// Registered trigger areas
    areas: Arc<RwLock<HashMap<String, TriggerArea>>>,
    /// Objects in each area (area_name -> set of object IDs)
    objects_in_areas: Arc<RwLock<HashMap<String, HashSet<u32>>>>,
    /// Last frame when an object entered an area (C++ Object::didEnter equivalent support)
    last_enter_frame: Arc<RwLock<HashMap<(String, u32), u32>>>,
    /// Last frame when an object exited an area (C++ Object::didExit equivalent support)
    last_exit_frame: Arc<RwLock<HashMap<(String, u32), u32>>>,
}

impl AreaTracker {
    /// Create a new area tracker
    pub fn new() -> Self {
        Self {
            areas: Arc::new(RwLock::new(HashMap::new())),
            objects_in_areas: Arc::new(RwLock::new(HashMap::new())),
            last_enter_frame: Arc::new(RwLock::new(HashMap::new())),
            last_exit_frame: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a trigger area
    pub fn register_area(&self, area: TriggerArea) -> GameLogicResult<()> {
        let mut areas = self.areas.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire areas lock: {}", e))
        })?;

        let area_name = area.name.clone();
        areas.insert(area_name.clone(), area);

        // Initialize empty set for this area
        let mut objects_in_areas = self.objects_in_areas.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire objects in areas lock: {}", e))
        })?;
        objects_in_areas.insert(area_name.clone(), HashSet::new());

        log::debug!("Registered trigger area: {}", area_name);
        Ok(())
    }

    /// Register a polygon trigger area by name (terrain-backed).
    pub fn register_polygon_area(&self, area_name: &str) -> GameLogicResult<()> {
        let area = TriggerArea {
            name: area_name.to_string(),
            center: [0.0, 0.0, 0.0],
            radius: None,
            bounds: None,
            active: true,
            player_filter: None,
            polygon_name: Some(area_name.to_string()),
        };
        self.register_area(area)
    }

    /// Unregister a trigger area
    pub fn unregister_area(&self, area_name: &str) -> GameLogicResult<()> {
        let mut areas = self.areas.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire areas lock: {}", e))
        })?;

        areas.remove(area_name);

        let mut objects_in_areas = self.objects_in_areas.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire objects in areas lock: {}", e))
        })?;
        objects_in_areas.remove(area_name);

        log::debug!("Unregistered trigger area: {}", area_name);
        Ok(())
    }

    fn collect_area_events(
        &self,
        object_id: u32,
        position: [f32; 3],
    ) -> GameLogicResult<Vec<GameEvent>> {
        let frame = crate::helpers::TheGameLogic::get_frame() as u32;
        let areas = self.areas.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire areas lock: {}", e))
        })?;

        let mut objects_in_areas = self.objects_in_areas.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire objects in areas lock: {}", e))
        })?;

        let terrain = get_terrain_logic().read().ok();
        let mut events = Vec::new();

        for (area_name, area) in areas.iter() {
            let was_inside = objects_in_areas
                .get(area_name)
                .map(|set| set.contains(&object_id))
                .unwrap_or(false);

            let is_inside = if !area.active {
                false
            } else if let Some(polygon_name) = area.polygon_name.as_deref() {
                if let Some(terrain_guard) = terrain.as_ref() {
                    if let Some(trigger) = terrain_guard.get_trigger_area_by_name(polygon_name) {
                        let point = ICoord3D::new(
                            position[0] as i32,
                            position[1] as i32,
                            position[2] as i32,
                        );
                        trigger.point_in_trigger_int(&point)
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                area.contains_position(position)
            };

            if is_inside && !was_inside {
                objects_in_areas
                    .entry(area_name.clone())
                    .or_insert_with(HashSet::new)
                    .insert(object_id);

                if let Ok(mut enter_frames) = self.last_enter_frame.write() {
                    enter_frames.insert((area_name.clone(), object_id), frame);
                }

                if let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) {
                    // Avoid self-deadlock when called from Object::set_position while that object
                    // is already write-locked by the caller.
                    if let Ok(obj_guard) = obj_arc.try_read() {
                        if let Some(team_arc) = obj_guard.get_team() {
                            if let Ok(mut team_guard) = team_arc.write() {
                                team_guard.set_entered_exited();
                            }
                        }
                    }
                }

                let event = GameEvent::new(
                    GameEventType::UnitEntersArea,
                    format!("Object {} entered area {}", object_id, area_name),
                )
                .with_source_object(object_id)
                .with_parameter(
                    "area_name".to_string(),
                    ScriptValue::String(area_name.clone()),
                )
                .with_parameter("frame".to_string(), ScriptValue::Int(frame as i64))
                .with_parameter("position".to_string(), ScriptValue::Coord3D(position));

                events.push(event);
            } else if !is_inside && was_inside {
                if let Some(set) = objects_in_areas.get_mut(area_name) {
                    set.remove(&object_id);
                }

                if let Ok(mut exit_frames) = self.last_exit_frame.write() {
                    exit_frames.insert((area_name.clone(), object_id), frame);
                }

                if let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) {
                    // Avoid self-deadlock when called from Object::set_position while that object
                    // is already write-locked by the caller.
                    if let Ok(obj_guard) = obj_arc.try_read() {
                        if let Some(team_arc) = obj_guard.get_team() {
                            if let Ok(mut team_guard) = team_arc.write() {
                                team_guard.set_entered_exited();
                            }
                        }
                    }
                }

                let event = GameEvent::new(
                    GameEventType::UnitLeavesArea,
                    format!("Object {} exited area {}", object_id, area_name),
                )
                .with_source_object(object_id)
                .with_parameter(
                    "area_name".to_string(),
                    ScriptValue::String(area_name.clone()),
                )
                .with_parameter("frame".to_string(), ScriptValue::Int(frame as i64))
                .with_parameter("position".to_string(), ScriptValue::Coord3D(position));

                events.push(event);
            }
        }

        Ok(events)
    }

    /// Update object position and fire enter/exit events
    pub async fn update_object_position(
        &self,
        object_id: u32,
        position: [f32; 3],
        event_manager: &EventManager,
    ) -> GameLogicResult<()> {
        for event in self.collect_area_events(object_id, position)? {
            event_manager.fire_event(event).await?;
        }
        Ok(())
    }

    /// Update object position and fire enter/exit events synchronously.
    pub fn update_object_position_sync(
        &self,
        object_id: u32,
        position: [f32; 3],
        event_manager: &EventManager,
    ) -> GameLogicResult<()> {
        for event in self.collect_area_events(object_id, position)? {
            event_manager.fire_event_sync(event)?;
        }
        Ok(())
    }

    /// Remove object from all areas (call when object is destroyed)
    pub fn remove_object(&self, object_id: u32) -> GameLogicResult<()> {
        let mut objects_in_areas = self.objects_in_areas.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire objects in areas lock: {}", e))
        })?;

        for set in objects_in_areas.values_mut() {
            set.remove(&object_id);
        }

        if let Ok(mut enter_frames) = self.last_enter_frame.write() {
            enter_frames.retain(|(_, id), _| *id != object_id);
        }
        if let Ok(mut exit_frames) = self.last_exit_frame.write() {
            exit_frames.retain(|(_, id), _| *id != object_id);
        }

        Ok(())
    }

    pub fn get_last_enter_frame(&self, area_name: &str, object_id: u32) -> Option<u32> {
        let guard = self.last_enter_frame.read().ok()?;
        guard.get(&(area_name.to_string(), object_id)).copied()
    }

    pub fn get_last_exit_frame(&self, area_name: &str, object_id: u32) -> Option<u32> {
        let guard = self.last_exit_frame.read().ok()?;
        guard.get(&(area_name.to_string(), object_id)).copied()
    }

    /// Get all objects in an area
    pub fn get_objects_in_area(&self, area_name: &str) -> GameLogicResult<Vec<u32>> {
        let objects_in_areas = self.objects_in_areas.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire objects in areas lock: {}", e))
        })?;

        Ok(objects_in_areas
            .get(area_name)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default())
    }

    /// Check if a trigger area is registered.
    pub fn has_area(&self, area_name: &str) -> GameLogicResult<bool> {
        let areas = self.areas.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire areas lock: {}", e))
        })?;
        Ok(areas.contains_key(area_name))
    }

    /// Check if object is in area
    pub fn is_object_in_area(&self, object_id: u32, area_name: &str) -> GameLogicResult<bool> {
        let objects_in_areas = self.objects_in_areas.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire objects in areas lock: {}", e))
        })?;

        Ok(objects_in_areas
            .get(area_name)
            .map(|set| set.contains(&object_id))
            .unwrap_or(false))
    }
}

impl Default for NamedObjectTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for AreaTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSubscriber {
        name: String,
        filter: EventFilter,
        received_events: Arc<Mutex<Vec<GameEvent>>>,
    }

    impl TestSubscriber {
        fn new(name: String, filter: EventFilter) -> Self {
            Self {
                name,
                filter,
                received_events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_received_events(&self) -> Vec<GameEvent> {
            self.received_events.lock().unwrap().clone()
        }
    }

    impl EventSubscriber for TestSubscriber {
        fn on_event(&self, event: &GameEvent) -> GameLogicResult<()> {
            let mut events = self.received_events.lock().unwrap();
            events.push(event.clone());
            Ok(())
        }

        fn get_filter(&self) -> EventFilter {
            self.filter.clone()
        }

        fn get_name(&self) -> String {
            self.name.clone()
        }
    }

    #[tokio::test]
    async fn test_event_manager_creation() {
        let manager = EventManager::new();
        let stats = manager.get_statistics().await.unwrap();
        assert_eq!(stats.total_events, 0);
        assert_eq!(stats.active_subscribers, 0);
    }

    #[tokio::test]
    async fn test_event_firing_and_processing() {
        let manager = EventManager::new();

        let event = GameEvent::new(GameEventType::UnitCreated, "Test unit created".to_string());

        manager.fire_event(event).await.unwrap();
        manager.process_events().await.unwrap();

        let stats = manager.get_statistics().await.unwrap();
        assert_eq!(stats.total_events, 1);
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let manager = EventManager::new();

        let filter = EventFilter::new().with_event_type(GameEventType::UnitCreated);

        let subscriber = TestSubscriber::new("test_sub".to_string(), filter);
        let subscriber_events = subscriber.received_events.clone();

        manager.subscribe(Box::new(subscriber)).await.unwrap();

        let event = GameEvent::new(GameEventType::UnitCreated, "Test unit created".to_string());

        manager.fire_event(event).await.unwrap();
        manager.process_events().await.unwrap();

        let received = subscriber_events.lock().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].event_type, GameEventType::UnitCreated);
    }

    #[tokio::test]
    async fn test_event_filtering() {
        let filter = EventFilter::new()
            .with_event_type(GameEventType::UnitCreated)
            .with_player(1);

        let matching_event =
            GameEvent::new(GameEventType::UnitCreated, "Test".to_string()).with_player(1);

        let non_matching_event =
            GameEvent::new(GameEventType::UnitDestroyed, "Test".to_string()).with_player(1);

        let wrong_player_event =
            GameEvent::new(GameEventType::UnitCreated, "Test".to_string()).with_player(2);

        assert!(filter.matches(&matching_event));
        assert!(!filter.matches(&non_matching_event));
        assert!(!filter.matches(&wrong_player_event));
    }

    #[tokio::test]
    async fn test_event_history_query() {
        let manager = EventManager::new();

        let event1 =
            GameEvent::new(GameEventType::UnitCreated, "Unit 1 created".to_string()).with_player(1);

        let event2 = GameEvent::new(GameEventType::UnitDestroyed, "Unit 2 destroyed".to_string())
            .with_player(2);

        manager.fire_event(event1).await.unwrap();
        manager.fire_event(event2).await.unwrap();
        manager.process_events().await.unwrap();

        let filter = EventFilter::new().with_player(1);
        let history = manager.query_history(&filter, 10).await.unwrap();

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].event_type, GameEventType::UnitCreated);
    }

    #[tokio::test]
    async fn test_built_in_event_helpers() {
        let manager = EventManager::new();

        manager
            .fire_unit_created(123, "Tank".to_string(), 1, [100.0, 200.0, 0.0])
            .await
            .unwrap();

        manager.process_events().await.unwrap();

        let stats = manager.get_statistics().await.unwrap();
        assert_eq!(stats.total_events, 1);
        assert_eq!(
            stats.events_by_type.get(&GameEventType::UnitCreated),
            Some(&1)
        );
    }

    #[tokio::test]
    async fn test_named_object_tracker() {
        let tracker = NamedObjectTracker::new();

        // Register a named object
        tracker
            .register_named_object("MyTank".to_string(), 123)
            .unwrap();

        // Retrieve by name
        let id = tracker.get_object_id("MyTank").unwrap();
        assert_eq!(id, Some(123));

        // Retrieve by ID
        let name = tracker.get_object_name(123).unwrap();
        assert_eq!(name, Some("MyTank".to_string()));

        // Check if named
        assert!(tracker.is_named(123).unwrap());
        assert!(!tracker.is_named(999).unwrap());

        // Unregister
        tracker.unregister_object(123).unwrap();
        assert_eq!(tracker.get_object_id("MyTank").unwrap(), None);
    }

    #[tokio::test]
    async fn test_area_tracker_circular() {
        let tracker = AreaTracker::new();
        let manager = EventManager::new();

        // Register a circular area
        let area = TriggerArea::new_circular("TestArea".to_string(), [100.0, 100.0, 0.0], 50.0);
        tracker.register_area(area).unwrap();

        // Object starts outside the area
        let object_id = 123;
        let position_outside = [200.0, 200.0, 0.0];
        tracker
            .update_object_position(object_id, position_outside, &manager)
            .await
            .unwrap();

        // No events yet
        let stats = manager.get_statistics().await.unwrap();
        assert_eq!(stats.total_events, 0);

        // Move object inside the area
        let position_inside = [110.0, 110.0, 0.0];
        tracker
            .update_object_position(object_id, position_inside, &manager)
            .await
            .unwrap();

        // Process events
        manager.process_events().await.unwrap();

        // Should have 1 enter event
        let stats = manager.get_statistics().await.unwrap();
        assert_eq!(stats.total_events, 1);
        assert_eq!(
            stats.events_by_type.get(&GameEventType::UnitEntersArea),
            Some(&1)
        );

        // Verify object is in area
        assert!(tracker.is_object_in_area(object_id, "TestArea").unwrap());

        // Move object out of area
        tracker
            .update_object_position(object_id, position_outside, &manager)
            .await
            .unwrap();

        // Process events
        manager.process_events().await.unwrap();

        // Should have 1 enter + 1 exit event
        let stats = manager.get_statistics().await.unwrap();
        assert_eq!(stats.total_events, 2);
        assert_eq!(
            stats.events_by_type.get(&GameEventType::UnitLeavesArea),
            Some(&1)
        );

        // Object should no longer be in area
        assert!(!tracker.is_object_in_area(object_id, "TestArea").unwrap());
    }

    #[tokio::test]
    async fn test_area_tracker_rectangular() {
        let tracker = AreaTracker::new();
        let manager = EventManager::new();

        // Register a rectangular area
        let area = TriggerArea::new_rectangular(
            "RectArea".to_string(),
            [100.0, 100.0, 0.0],
            50.0,  // min_x
            50.0,  // min_y
            150.0, // max_x
            150.0, // max_y
        );
        tracker.register_area(area).unwrap();

        let object_id = 456;

        // Position inside rectangle
        let pos_inside = [100.0, 100.0, 0.0];
        tracker
            .update_object_position(object_id, pos_inside, &manager)
            .await
            .unwrap();
        manager.process_events().await.unwrap();

        let stats = manager.get_statistics().await.unwrap();
        assert_eq!(stats.total_events, 1);
        assert!(tracker.is_object_in_area(object_id, "RectArea").unwrap());

        // Get objects in area
        let objects_in_area = tracker.get_objects_in_area("RectArea").unwrap();
        assert_eq!(objects_in_area.len(), 1);
        assert_eq!(objects_in_area[0], object_id);
    }

    #[tokio::test]
    async fn test_area_tracker_remove_object() {
        let tracker = AreaTracker::new();
        let manager = EventManager::new();

        let area = TriggerArea::new_circular("RemoveTest".to_string(), [0.0, 0.0, 0.0], 100.0);
        tracker.register_area(area).unwrap();

        let object_id = 789;

        // Add object to area
        let pos_inside = [10.0, 10.0, 0.0];
        tracker
            .update_object_position(object_id, pos_inside, &manager)
            .await
            .unwrap();
        assert!(tracker.is_object_in_area(object_id, "RemoveTest").unwrap());

        // Remove object (simulating destruction)
        tracker.remove_object(object_id).unwrap();
        assert!(!tracker.is_object_in_area(object_id, "RemoveTest").unwrap());
    }

    #[tokio::test]
    async fn test_named_object_tracker_rename() {
        let tracker = NamedObjectTracker::new();

        // Register object with first name
        tracker
            .register_named_object("FirstName".to_string(), 100)
            .unwrap();
        assert_eq!(tracker.get_object_id("FirstName").unwrap(), Some(100));

        // Rename object (register with new name)
        tracker
            .register_named_object("SecondName".to_string(), 100)
            .unwrap();

        // Old name should not resolve
        assert_eq!(tracker.get_object_id("FirstName").unwrap(), None);

        // New name should resolve
        assert_eq!(tracker.get_object_id("SecondName").unwrap(), Some(100));
    }

    #[tokio::test]
    async fn test_named_object_tracker_all_objects() {
        let tracker = NamedObjectTracker::new();

        tracker
            .register_named_object("Tank1".to_string(), 1)
            .unwrap();
        tracker
            .register_named_object("Tank2".to_string(), 2)
            .unwrap();
        tracker
            .register_named_object("Building".to_string(), 3)
            .unwrap();

        let all_objects = tracker.get_all_named_objects().unwrap();
        assert_eq!(all_objects.len(), 3);

        // Verify all names are present
        let names: Vec<String> = all_objects.iter().map(|(name, _)| name.clone()).collect();
        assert!(names.contains(&"Tank1".to_string()));
        assert!(names.contains(&"Tank2".to_string()));
        assert!(names.contains(&"Building".to_string()));
    }
}
