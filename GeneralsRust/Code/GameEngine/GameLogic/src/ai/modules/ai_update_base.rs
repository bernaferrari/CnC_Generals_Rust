//! AI Update Base Module - Foundation for all AI update modules
//!
//! This module provides the base trait and common functionality for all
//! specialized AI update modules. It defines the interface that each
//! AI module must implement.
//!
//! Author: Converted from C++ original by Michael S. Booth

use super::{AIModulePriority, AIModuleState, AIModuleType, AIUpdateContext};
use crate::ai::AiError;
use crate::common::{Coord3D, ObjectID, Real};

/// Result type for AI update operations
pub type AIUpdateResult<T> = Result<T, AiError>;

/// Base trait for all AI update modules
pub trait AIUpdateModuleTrait: Send + Sync {
    /// Get module type
    fn get_module_type(&self) -> AIModuleType;

    /// Get module priority for execution order
    fn get_priority(&self) -> AIModulePriority;

    /// Get current module state
    fn get_state(&self) -> AIModuleState;

    /// Initialize module for a specific object
    fn init(&mut self, context: &AIUpdateContext) -> AIUpdateResult<()>;

    /// Reset module state
    fn reset(&mut self) -> AIUpdateResult<()>;

    /// Main update method called each frame
    fn update(&mut self, context: &mut AIUpdateContext) -> AIUpdateResult<()>;

    /// Check if module should be active for this object
    fn should_update(&self, context: &AIUpdateContext) -> bool;

    /// Called when module becomes active
    fn on_activate(&mut self) -> AIUpdateResult<()> {
        Ok(())
    }

    /// Called when module is deactivated
    fn on_deactivate(&mut self) -> AIUpdateResult<()> {
        Ok(())
    }

    /// Called when object takes damage
    fn on_damage_received(
        &mut self,
        _damage_amount: f32,
        _attacker: Option<ObjectID>,
    ) -> AIUpdateResult<()> {
        Ok(())
    }

    /// Called when object reaches destination
    fn on_destination_reached(&mut self) -> AIUpdateResult<()> {
        Ok(())
    }

    /// Called when object is blocked
    fn on_blocked(&mut self) -> AIUpdateResult<()> {
        Ok(())
    }
}

/// Base AI Update module implementation
#[derive(Debug)]
pub struct AIUpdateModule {
    module_type: AIModuleType,
    priority: AIModulePriority,
    state: AIModuleState,
    enabled: bool,
    last_update_frame: u32,
    update_interval: u32, // Frames between updates
}

impl AIUpdateModule {
    pub fn new(module_type: AIModuleType, priority: AIModulePriority) -> Self {
        Self {
            module_type,
            priority,
            state: AIModuleState::Idle,
            enabled: true,
            last_update_frame: 0,
            update_interval: 1, // Update every frame by default
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_update_interval(&mut self, frames: u32) {
        self.update_interval = frames.max(1);
    }

    pub fn should_update_this_frame(&self, current_frame: u32) -> bool {
        self.enabled && (current_frame - self.last_update_frame >= self.update_interval)
    }

    pub fn mark_updated(&mut self, current_frame: u32) {
        self.last_update_frame = current_frame;
    }

    pub fn set_state(&mut self, state: AIModuleState) {
        self.state = state;
    }
}

impl AIUpdateModuleTrait for AIUpdateModule {
    fn get_module_type(&self) -> AIModuleType {
        self.module_type
    }

    fn get_priority(&self) -> AIModulePriority {
        self.priority
    }

    fn get_state(&self) -> AIModuleState {
        self.state
    }

    fn init(&mut self, _context: &AIUpdateContext) -> AIUpdateResult<()> {
        self.state = AIModuleState::Idle;
        self.enabled = true;
        Ok(())
    }

    fn reset(&mut self) -> AIUpdateResult<()> {
        self.state = AIModuleState::Idle;
        self.last_update_frame = 0;
        Ok(())
    }

    fn update(&mut self, context: &mut AIUpdateContext) -> AIUpdateResult<()> {
        if !self.should_update_this_frame(context.current_frame) {
            return Ok(());
        }

        self.mark_updated(context.current_frame);
        self.state = AIModuleState::Active;

        // Base implementation does nothing
        Ok(())
    }

    fn should_update(&self, context: &AIUpdateContext) -> bool {
        self.enabled && self.should_update_this_frame(context.current_frame)
    }
}

/// AI Module Manager - manages multiple AI modules for an object
pub struct AIModuleManager {
    modules: Vec<Box<dyn AIUpdateModuleTrait>>,
    active_module: Option<usize>,
}

impl AIModuleManager {
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
            active_module: None,
        }
    }

    pub fn add_module(&mut self, module: Box<dyn AIUpdateModuleTrait>) {
        self.modules.push(module);
        self.sort_modules_by_priority();
    }

    pub fn init_all(&mut self, context: &AIUpdateContext) -> AIUpdateResult<()> {
        for module in &mut self.modules {
            module.init(context)?;
        }
        Ok(())
    }

    pub fn reset_all(&mut self) -> AIUpdateResult<()> {
        for module in &mut self.modules {
            module.reset()?;
        }
        self.active_module = None;
        Ok(())
    }

    pub fn update_all(&mut self, context: &mut AIUpdateContext) -> AIUpdateResult<()> {
        for (i, module) in self.modules.iter_mut().enumerate() {
            if module.should_update(context) {
                module.update(context)?;
                self.active_module = Some(i);
            }
        }
        Ok(())
    }

    pub fn get_active_module(&self) -> Option<&dyn AIUpdateModuleTrait> {
        self.active_module
            .and_then(|i| self.modules.get(i).map(|m| m.as_ref()))
    }

    fn sort_modules_by_priority(&mut self) {
        self.modules.sort_by_key(|m| m.get_priority());
    }
}

impl Default for AIModuleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_update_module_creation() {
        let module = AIUpdateModule::new(AIModuleType::Base, AIModulePriority::Normal);
        assert_eq!(module.get_module_type(), AIModuleType::Base);
        assert_eq!(module.get_priority(), AIModulePriority::Normal);
        assert_eq!(module.get_state(), AIModuleState::Idle);
        assert!(module.is_enabled());
    }

    #[test]
    fn test_ai_update_interval() {
        let mut module = AIUpdateModule::new(AIModuleType::Base, AIModulePriority::Normal);
        module.set_update_interval(5);

        assert!(!module.should_update_this_frame(0));
        module.mark_updated(0);

        assert!(!module.should_update_this_frame(3));
        assert!(module.should_update_this_frame(5));
    }

    #[test]
    fn test_ai_module_manager() {
        let mut manager = AIModuleManager::new();

        let module1 = Box::new(AIUpdateModule::new(
            AIModuleType::Dozer,
            AIModulePriority::High,
        ));
        let module2 = Box::new(AIUpdateModule::new(
            AIModuleType::Wander,
            AIModulePriority::Low,
        ));

        manager.add_module(module1);
        manager.add_module(module2);

        // High priority module should come first after sorting
        assert_eq!(manager.modules[0].get_priority(), AIModulePriority::High);
        assert_eq!(manager.modules[1].get_priority(), AIModulePriority::Low);
    }
}
