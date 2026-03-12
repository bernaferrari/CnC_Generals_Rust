//! Behavior Integration System
//!
//! This module provides integration between the legacy C++ style behavior modules
//! and the new async behavior framework, allowing seamless interoperability.

use super::{
    advanced_behavior_system::{
        AdvancedBehavior, BehaviorContext, BehaviorEvent, BehaviorManager, BehaviorOutcome,
        BehaviorPriority, BehaviorState,
    },
    formation_behavior::{FormationBehavior, FormationConfig},
    stealth_behavior::{StealthBehavior, StealthConfig},
};

use crate::common::{ModuleData, LOGICFRAMES_PER_SECOND};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::object::{Object, ObjectId};
use crate::GameLogicResult;
use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// Integrated behavior system that manages both legacy and modern behaviors
#[derive(Debug)]
pub struct IntegratedBehaviorSystem {
    /// Behavior managers per object
    behavior_managers: HashMap<ObjectId, BehaviorManager>,

    /// Legacy behavior registry
    legacy_registry: super::BehaviorModuleRegistry,

    /// Object to behavior mappings
    object_behaviors: HashMap<ObjectId, Vec<String>>,

    /// Behavior configurations
    behavior_configs: HashMap<String, BehaviorConfiguration>,
}

/// Configuration for different behavior types
#[derive(Debug, Clone)]
pub enum BehaviorConfiguration {
    Stealth(StealthConfig),
    Formation(FormationConfig),
    Legacy(Arc<dyn ModuleData>),
}

/// Bridge adapter that wraps legacy behaviors to work with the async system
#[derive(Debug)]
pub struct LegacyBehaviorAdapter {
    behavior_name: String,
    legacy_behavior: Arc<Mutex<Box<dyn crate::modules::BehaviorModuleInterface>>>,
    last_update: std::time::Instant,
}

impl IntegratedBehaviorSystem {
    /// Create a new integrated behavior system
    pub fn new() -> Self {
        Self {
            behavior_managers: HashMap::new(),
            legacy_registry: super::BehaviorModuleRegistry::new(),
            object_behaviors: HashMap::new(),
            behavior_configs: HashMap::new(),
        }
    }

    /// Register a behavior configuration
    pub fn register_behavior_config(&mut self, name: String, config: BehaviorConfiguration) {
        self.behavior_configs.insert(name, config);
    }

    /// Helper to create context from object
    fn create_context(object: &Object) -> BehaviorContext {
        let position = *object.get_position();
        let mut velocity = [0.0, 0.0, 0.0];
        if let Some(physics) = object.get_physics() {
            if let Ok(mut guard) = physics.lock() {
                let vel = guard.get_velocity();
                velocity = [vel.x, vel.y, vel.z];
            }
        }
        BehaviorContext {
            object_id: object.get_id(),
            current_frame: TheGameLogic::get_frame(),
            delta_time: 1.0 / LOGICFRAMES_PER_SECOND as f32,
            position: [position.x, position.y, position.z],
            velocity,
            health: object.get_health(),
            max_health: object.get_max_health(),
            shared_data: Arc::new(RwLock::new(HashMap::new())), // Should persist?
        }
    }

    /// Add a behavior to an object
    pub async fn add_behavior_to_object(
        &mut self,
        object_id: ObjectId,
        behavior_name: &str,
        object: &mut Object,
    ) -> GameLogicResult<()> {
        // Get behavior configuration
        let config = self.behavior_configs.get(behavior_name).ok_or_else(|| {
            crate::GameLogicError::Configuration(format!("Unknown behavior: {}", behavior_name))
        })?;

        let context = Self::create_context(object);

        let behavior: Box<dyn AdvancedBehavior> = match config {
            BehaviorConfiguration::Stealth(stealth_config) => {
                Box::new(StealthBehavior::with_config(stealth_config.clone()))
            }

            BehaviorConfiguration::Formation(formation_config) => {
                Box::new(FormationBehavior::with_config(formation_config.clone()))
            }

            BehaviorConfiguration::Legacy(module_data) => {
                // Create legacy behavior adapter
                Box::new(self.create_legacy_adapter(behavior_name, module_data.clone(), object)?)
            }
        };

        // Now get manager (mutable borrow)
        let manager = self
            .behavior_managers
            .entry(object_id)
            .or_insert_with(|| BehaviorManager::new(object_id));

        manager
            .add_behavior(behavior_name.to_string(), behavior, object, &context)
            .await?;

        // Track behavior assignment
        self.object_behaviors
            .entry(object_id)
            .or_default()
            .push(behavior_name.to_string());

        Ok(())
    }

    /// Remove a behavior from an object
    pub async fn remove_behavior_from_object(
        &mut self,
        object_id: ObjectId,
        behavior_name: &str,
        object: &mut Object, // Added object argument to match manager signature
    ) -> GameLogicResult<()> {
        let context = Self::create_context(object);

        if let Some(manager) = self.behavior_managers.get_mut(&object_id) {
            manager
                .remove_behavior(behavior_name, object, &context)
                .await?;
        }

        // Update tracking
        if let Some(behaviors) = self.object_behaviors.get_mut(&object_id) {
            behaviors.retain(|name| name != behavior_name);
            if behaviors.is_empty() {
                self.object_behaviors.remove(&object_id);
            }
        }

        Ok(())
    }

    /// Update all behaviors for all objects
    pub async fn update_all_behaviors(
        &mut self,
        objects: &mut HashMap<ObjectId, Object>,
    ) -> GameLogicResult<()> {
        // Iterate through all objects that have behaviors
        let manager_ids: Vec<ObjectId> = self.behavior_managers.keys().cloned().collect();

        for object_id in manager_ids {
            if let Some(object) = objects.get_mut(&object_id) {
                if let Some(manager) = self.behavior_managers.get_mut(&object_id) {
                    let context = Self::create_context(object);
                    if let Err(e) = manager.update(object, &context).await {
                        log::error!("Error updating behaviors for object {}: {:?}", object_id, e);
                    }
                }
            } else {
                // Object no longer exists, should clean up manager
                self.behavior_managers.remove(&object_id);
            }
        }

        Ok(())
    }

    /// Handle events for specific objects
    pub async fn handle_event(
        &mut self,
        object_id: ObjectId,
        event: &str,
        data: &[u8],
        object: &mut Object,
    ) -> GameLogicResult<()> {
        if let Some(manager) = self.behavior_managers.get_mut(&object_id) {
            let context = Self::create_context(object);

            // Convert raw event string/data to BehaviorEvent
            // This is a naive conversion, real implementation would be more robust
            let behavior_event = BehaviorEvent::Custom {
                event_type: event.to_string(),
                data: HashMap::new(), // Todo: Parse data
            };

            manager
                .handle_event(&behavior_event, object, &context)
                .await?;
        }
        Ok(())
    }

    /// Get behavior statistics
    pub fn get_behavior_metrics(
        &self,
        object_id: ObjectId,
    ) -> Option<HashMap<String, super::advanced_behavior_system::BehaviorStatistics>> {
        // The original method returned behavior metrics map
        // But BehaviorManager::get_statistics returns a single struct
        // We might want to return that struct
        self.behavior_managers.get(&object_id).map(|m| {
            let mut map = HashMap::new();
            map.insert("total".to_string(), m.get_statistics().clone());
            map
        })
    }

    /// Create a legacy behavior adapter
    fn create_legacy_adapter(
        &self,
        behavior_name: &str,
        module_data: Arc<dyn ModuleData>,
        object: &Object,
    ) -> Result<LegacyBehaviorAdapter, crate::GameLogicError> {
        let thing = crate::object::registry::OBJECT_REGISTRY
            .get_object(object.get_id())
            .ok_or_else(|| {
                crate::GameLogicError::Configuration(
                    "Object not registered for legacy adapter".to_string(),
                )
            })?;

        let behavior = self
            .legacy_registry
            .create_behavior(behavior_name, thing, module_data)
            .map_err(|e| {
                crate::GameLogicError::Configuration(format!(
                    "Legacy behavior create failed: {}",
                    e
                ))
            })?;

        Ok(LegacyBehaviorAdapter {
            behavior_name: behavior_name.to_string(),
            legacy_behavior: Arc::new(Mutex::new(behavior)),
            last_update: std::time::Instant::now(),
        })
    }

    /// Get all behaviors for an object
    pub fn get_object_behaviors(&self, object_id: ObjectId) -> Vec<String> {
        self.object_behaviors
            .get(&object_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if an object has a specific behavior
    pub fn has_behavior(&self, object_id: ObjectId, behavior_name: &str) -> bool {
        self.object_behaviors
            .get(&object_id)
            .map(|behaviors| behaviors.contains(&behavior_name.to_string()))
            .unwrap_or(false)
    }
}

#[async_trait]
impl AdvancedBehavior for LegacyBehaviorAdapter {
    fn name(&self) -> &str {
        &self.behavior_name
    }

    fn priority(&self) -> BehaviorPriority {
        BehaviorPriority::Normal
    }

    // fn flags() -> Default

    async fn initialize(
        &mut self,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        log::info!(
            "Legacy behavior adapter initialized: {}",
            self.behavior_name
        );
        Ok(())
    }

    async fn update(
        &mut self,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<BehaviorOutcome> {
        let now = std::time::Instant::now();
        // let delta_time = now.duration_since(self.last_update);
        self.last_update = now;

        let legacy_behavior = self.legacy_behavior.clone();
        let mut behavior_guard = legacy_behavior
            .lock()
            .map_err(|e| crate::GameLogicError::System(format!("Failed to lock: {}", e)))?;

        if let Some(update_module) = behavior_guard.get_update() {
            match update_module.update() {
                Ok(_sleep_time) => Ok(BehaviorOutcome::Continue),
                Err(e) => {
                    log::error!("Legacy behavior update failed: {}", e);
                    Ok(BehaviorOutcome::Failed(e.to_string()))
                }
            }
        } else {
            Ok(BehaviorOutcome::Continue)
        }
    }

    async fn cleanup(
        &mut self,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        log::info!("Legacy behavior adapter cleanup: {}", self.behavior_name);
        Ok(())
    }

    async fn handle_event(
        &mut self,
        event: &BehaviorEvent,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        let legacy_behavior = self.legacy_behavior.clone();
        let mut behavior_guard = legacy_behavior
            .lock()
            .map_err(|e| crate::GameLogicError::System(format!("Failed to lock: {}", e)))?;

        match event {
            BehaviorEvent::DamageReceived { amount, .. } => {
                if let Some(damage_module) = behavior_guard.get_damage() {
                    // Create minimal DamageInfo
                    let mut damage_info = DamageInfo::new();
                    damage_info.input.amount = *amount;
                    damage_info.sync_from_input();
                    damage_module.on_damage(&mut damage_info).map_err(|e| {
                        crate::GameLogicError::Execution(format!(
                            "Legacy damage handler failed: {}",
                            e
                        ))
                    })?;
                }
            }
            // ... other events ...
            _ => {}
        }

        Ok(())
    }
}

/// Configuration builder for common behavior setups
pub struct BehaviorConfigurationBuilder {
    configs: HashMap<String, BehaviorConfiguration>,
}

impl BehaviorConfigurationBuilder {
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    pub fn add_stealth_behavior(mut self, name: &str, config: StealthConfig) -> Self {
        self.configs
            .insert(name.to_string(), BehaviorConfiguration::Stealth(config));
        self
    }

    pub fn add_formation_behavior(mut self, name: &str, config: FormationConfig) -> Self {
        self.configs
            .insert(name.to_string(), BehaviorConfiguration::Formation(config));
        self
    }

    pub fn add_legacy_behavior(mut self, name: &str, module_data: Arc<dyn ModuleData>) -> Self {
        self.configs
            .insert(name.to_string(), BehaviorConfiguration::Legacy(module_data));
        self
    }

    pub fn build(self) -> HashMap<String, BehaviorConfiguration> {
        self.configs
    }
}

pub struct BehaviorFactory;

impl BehaviorFactory {
    pub fn create_basic_stealth() -> StealthConfig {
        StealthConfig {
            stealth_delay: 2.0,
            unstealth_delay: 1.0,
            moving_detection_radius: 150.0,
            stationary_detection_radius: 75.0,
            broken_by_attacking: true,
            broken_by_damage: true,
            ..Default::default()
        }
    }

    pub fn create_advanced_stealth() -> StealthConfig {
        StealthConfig {
            stealth_delay: 1.0,
            unstealth_delay: 0.5,
            moving_detection_radius: 100.0,
            stationary_detection_radius: 50.0,
            can_stealth_while_moving: true,
            broken_by_attacking: false,
            requires_power: true,
            power_consumption: 5.0,
            ..Default::default()
        }
    }

    pub fn create_line_formation() -> FormationConfig {
        FormationConfig {
            formation_type: super::formation_behavior::FormationType::Line,
            unit_spacing: 75.0,
            max_units: 8,
            maintain_during_movement: true,
            cohesion_strength: 0.8,
            ..Default::default()
        }
    }

    pub fn create_combat_formation() -> FormationConfig {
        FormationConfig {
            formation_type: super::formation_behavior::FormationType::Wedge,
            unit_spacing: 100.0,
            max_units: 12,
            maintain_during_combat: true,
            maintain_during_movement: true,
            cohesion_strength: 0.9,
            max_dispersion_distance: 150.0,
            ..Default::default()
        }
    }
}

// Tests commented out
// #[cfg(test)]
// mod tests { ... }
