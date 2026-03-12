//! Poisoned Behavior Module
//! 
//! Behavior that reacts to poison damage by continuously damaging the object
//! further in an update loop. Converted from PoisonedBehavior.cpp/h.

use crate::common::{ObjectId, FrameNumber, HealthPoints, Percentage};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use thiserror::Error;

/// Sleep time for update modules
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateSleepTime {
    /// Update on every frame
    None,
    /// Sleep for the specified number of frames
    Frames(u32),
    /// Sleep forever (until explicitly woken)
    Forever,
}

/// Types of damage that can be dealt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageType {
    /// Regular damage
    Normal,
    /// Poison damage
    Poison,
    /// Unresistable damage
    Unresistable,
    /// Healing
    Healing,
}

/// Types of death
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathType {
    /// Normal death
    Normal,
    /// Death by poison
    Poisoned,
    /// No death
    None,
}

/// Mask for disabled states
pub type DisabledMask = u32;

/// All disabled states mask
pub const DISABLED_MASK_ALL: DisabledMask = 0xFFFFFFFF;

/// Tint status for visual effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TintStatus {
    /// Normal tinting
    Normal,
    /// Poisoned tinting (green)
    Poisoned,
}

/// Information about damage dealt or healing
#[derive(Debug, Clone)]
pub struct DamageInfo {
    /// Amount of damage/healing
    pub amount: HealthPoints,
    /// ID of the source object (or INVALID_OBJECT_ID if none)
    pub source_id: ObjectId,
    /// Type of damage being dealt
    pub damage_type: DamageType,
    /// Type of death this damage causes
    pub death_type: DeathType,
    /// Actual damage dealt after resistances
    pub actual_damage_dealt: HealthPoints,
    /// Override for damage effects
    pub damage_fx_override: Option<DamageType>,
}

impl DamageInfo {
    /// Creates new damage info
    pub fn new(
        amount: HealthPoints,
        source_id: ObjectId,
        damage_type: DamageType,
        death_type: DeathType,
    ) -> Self {
        Self {
            amount,
            source_id,
            damage_type,
            death_type,
            actual_damage_dealt: 0.0,
            damage_fx_override: None,
        }
    }
}

/// Configuration data for poisoned behavior module
#[derive(Debug, Clone)]
pub struct PoisonedBehaviorModuleData {
    /// How often poison damage is re-applied (in frames)
    pub poison_damage_interval: u32,
    /// How long after the last poison dose the object remains poisoned (in frames)
    pub poison_duration: u32,
}

impl Default for PoisonedBehaviorModuleData {
    fn default() -> Self {
        Self {
            poison_damage_interval: 0,
            poison_duration: 0,
        }
    }
}

/// Result type for poisoned behavior operations
pub type PoisonedBehaviorResult<T> = Result<T, PoisonedBehaviorError>;

/// Errors that can occur in poisoned behavior
#[derive(Debug, Error)]
pub enum PoisonedBehaviorError {
    /// Invalid object reference
    #[error("Invalid object ID: {0}")]
    InvalidObject(ObjectId),
    /// Module not initialized
    #[error("Module not initialized")]
    NotInitialized,
    /// Threading error
    #[error("Threading error: {0}")]
    Threading(String),
}

/// Trait for objects that can be damaged
pub trait DamageModuleInterface {
    /// Called when damage is dealt to the object
    fn on_damage(&mut self, damage_info: &mut DamageInfo) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Called when healing is applied to the object
    fn on_healing(&mut self, damage_info: &mut DamageInfo) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Called when body damage state changes
    fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        _old_state: BodyDamageType,
        _new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Default implementation does nothing
        Ok(())
    }
}

/// Body damage states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyDamageType {
    /// Pristine condition
    Pristine,
    /// Lightly damaged
    Light,
    /// Heavily damaged
    Heavy,
    /// Critically damaged
    Critical,
}

/// Trait for update modules
pub trait UpdateModule {
    /// Update the module for one frame
    fn update(&mut self) -> PoisonedBehaviorResult<UpdateSleepTime>;
    
    /// Get the disabled types this module processes
    fn get_disabled_types_to_process(&self) -> DisabledMask {
        0 // By default, don't process disabled objects
    }
}

/// Mock drawable interface for visual effects
pub trait Drawable {
    /// Set tint status for visual effects
    fn set_tint_status(&mut self, status: TintStatus);
    
    /// Clear tint status
    fn clear_tint_status(&mut self, status: TintStatus);
}

/// Mock object interface
pub trait Object {
    /// Get the object's ID
    fn get_id(&self) -> ObjectId;
    
    /// Attempt to deal damage to this object
    fn attempt_damage(&mut self, damage_info: &mut DamageInfo) -> PoisonedBehaviorResult<()>;
    
    /// Check if the object is effectively dead
    fn is_effectively_dead(&self) -> bool;
    
    /// Get the object's drawable component
    fn get_drawable(&self) -> Option<&mut dyn Drawable>;
}

/// Mock game logic interface for frame tracking
pub trait GameLogic {
    /// Get the current game frame number
    fn get_frame(&self) -> FrameNumber;
}

/// Poisoned behavior module that handles poison effects over time
pub struct PoisonedBehavior {
    /// Configuration data
    config: PoisonedBehaviorModuleData,
    
    /// Frame when the next poison damage should be applied
    poison_damage_frame: FrameNumber,
    
    /// Frame when poison effects should stop
    poison_overall_stop_frame: FrameNumber,
    
    /// Amount of poison damage to deal
    poison_damage_amount: HealthPoints,
    
    /// Type of death caused by poison
    death_type: DeathType,
    
    /// Reference to the object this behavior is attached to
    object_id: ObjectId,
}

impl PoisonedBehavior {
    /// Creates a new poisoned behavior instance
    pub fn new(
        config: PoisonedBehaviorModuleData,
        object_id: ObjectId,
    ) -> Self {
        Self {
            config,
            poison_damage_frame: 0,
            poison_overall_stop_frame: 0,
            poison_damage_amount: 0.0,
            death_type: DeathType::Poisoned,
            object_id,
        }
    }
    
    /// Start the poison effects from the given damage info
    pub fn start_poisoned_effects(
        &mut self,
        damage_info: &DamageInfo,
        current_frame: FrameNumber,
        object: &mut dyn Object,
    ) -> PoisonedBehaviorResult<()> {
        // Store the damage amount dealt by the original poisoner
        self.poison_damage_amount = damage_info.actual_damage_dealt;
        
        // Set when poison effects should stop
        self.poison_overall_stop_frame = current_frame + self.config.poison_duration;
        
        // Set when the next poison damage should occur
        if self.poison_damage_frame != 0 {
            // If we're getting re-poisoned, don't reset the damage counter if already running
            self.poison_damage_frame = self.poison_damage_frame.min(current_frame + self.config.poison_damage_interval);
        } else {
            self.poison_damage_frame = current_frame + self.config.poison_damage_interval;
        }
        
        // Store the death type
        self.death_type = damage_info.input.death_type;
        
        // Apply visual effects
        if let Some(drawable) = object.get_drawable() {
            drawable.set_tint_status(TintStatus::Poisoned);
        }
        
        Ok(())
    }
    
    /// Stop all poison effects
    pub fn stop_poisoned_effects(
        &mut self,
        object: &mut dyn Object,
    ) -> PoisonedBehaviorResult<()> {
        self.poison_damage_frame = 0;
        self.poison_overall_stop_frame = 0;
        self.poison_damage_amount = 0.0;
        
        // Remove visual effects
        if let Some(drawable) = object.get_drawable() {
            drawable.clear_tint_status(TintStatus::Poisoned);
        }
        
        Ok(())
    }
    
    /// Calculate the sleep time until the next update
    fn calc_sleep_time(&self, current_frame: FrameNumber) -> UpdateSleepTime {
        if self.poison_overall_stop_frame == 0 || self.poison_overall_stop_frame == current_frame {
            return UpdateSleepTime::Forever;
        }
        
        // Return the minimum of the two times (next damage or stop time)
        let next_damage_frames = if self.poison_damage_frame > current_frame {
            self.poison_damage_frame - current_frame
        } else {
            0
        };
        
        let stop_frames = if self.poison_overall_stop_frame > current_frame {
            self.poison_overall_stop_frame - current_frame
        } else {
            0
        };
        
        if next_damage_frames == 0 && stop_frames == 0 {
            UpdateSleepTime::None
        } else if next_damage_frames == 0 {
            UpdateSleepTime::Frames(stop_frames)
        } else if stop_frames == 0 {
            UpdateSleepTime::Frames(next_damage_frames)
        } else {
            UpdateSleepTime::Frames(next_damage_frames.min(stop_frames))
        }
    }
}

impl UpdateModule for PoisonedBehavior {
    /// Update function - matches C++ PoisonedBehavior::update()
    /// Deals periodic poison damage and stops poison effects when duration expires.
    /// Matches C++ lines 84-120.
    fn update(&mut self) -> PoisonedBehaviorResult<UpdateSleepTime> {
        // Get current frame from game logic singleton
        // Matches C++ line 87: UnsignedInt now = TheGameLogic->getFrame();
        let current_frame = crate::game_logic::get_current_frame();

        // Matches C++ lines 89-94: Not poisoned check
        if self.poison_overall_stop_frame == 0 {
            // DEBUG_CRASH in C++ indicates this shouldn't happen
            return Ok(UpdateSleepTime::Forever);
        }

        // Check if it's time to deal poison damage
        // Matches C++ lines 96-108
        if self.poison_damage_frame != 0 && current_frame >= self.poison_damage_frame {
            // Create damage info for poison damage
            // Matches C++ lines 99-104
            let mut damage = DamageInfo::new(
                self.poison_damage_amount,
                crate::common::INVALID_OBJECT_ID, // INVALID_ID in C++
                DamageType::Unresistable, // DAMAGE_UNRESISTABLE - Not poison to avoid re-infection
                self.death_type,
            );
            damage.damage_fx_override = Some(DamageType::Poison); // DAMAGE_POISON for visual effects

            // Apply damage to object - matches C++ line 105: getObject()->attemptDamage(&damage);
            if let Some(object) = crate::game_logic::get_object_by_id(self.object_id) {
                if let Ok(mut obj) = object.try_lock() {
                    let _ = obj.attempt_damage(&mut damage);
                }
            }

            // Reset the damage timer - matches C++ line 107
            self.poison_damage_frame = current_frame + self.config.poison_damage_interval;
        }

        // Check if poison effects should stop
        // Matches C++ lines 112-117
        if self.poison_overall_stop_frame != 0
            && current_frame >= self.poison_overall_stop_frame
        {
            // Check if object is not effectively dead before stopping effects
            // Matches C++ line 114: !getObject()->isEffectivelyDead()
            let should_stop = if let Some(object) = crate::game_logic::get_object_by_id(self.object_id) {
                if let Ok(obj) = object.try_lock() {
                    !obj.is_effectively_dead()
                } else {
                    true
                }
            } else {
                true
            };

            if should_stop {
                // Stop poison effects - matches C++ line 116
                if let Some(object) = crate::game_logic::get_object_by_id(self.object_id) {
                    if let Ok(mut obj) = object.try_lock() {
                        let _ = self.stop_poisoned_effects(&mut *obj);
                    }
                }
            }
        }

        // Matches C++ line 119: return calcSleepTime();
        Ok(self.calc_sleep_time(current_frame))
    }
    
    fn get_disabled_types_to_process(&self) -> DisabledMask {
        DISABLED_MASK_ALL // Process even when disabled (poison continues)
    }
}

impl DamageModuleInterface for PoisonedBehavior {
    /// Called when damage is dealt - matches C++ PoisonedBehavior::onDamage()
    /// Matches C++ lines 67-71
    fn on_damage(&mut self, damage_info: &mut DamageInfo) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Matches C++ line 69: if(damageInfo->in.m_damageType == DAMAGE_POISON)
        if damage_info.input.damage_type == DamageType::Poison {
            // Get current frame from game logic singleton
            let current_frame = crate::game_logic::get_current_frame();

            // Get object reference and start poison effects
            // Matches C++ line 70: startPoisonedEffects(damageInfo);
            if let Some(object) = crate::game_logic::get_object_by_id(self.object_id) {
                if let Ok(mut obj) = object.try_lock() {
                    self.start_poisoned_effects(damage_info, current_frame, &mut *obj)?;
                }
            }
        }
        Ok(())
    }

    /// Called when healing is applied - matches C++ PoisonedBehavior::onHealing()
    /// Matches C++ lines 75-80
    fn on_healing(&mut self, _damage_info: &mut DamageInfo) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Matches C++ line 77: stopPoisonedEffects();
        if let Some(object) = crate::game_logic::get_object_by_id(self.object_id) {
            if let Ok(mut obj) = object.try_lock() {
                self.stop_poisoned_effects(&mut *obj)?;
            }
        }
        // Matches C++ line 79: setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER);
        // Wake frame management is handled by the module system
        Ok(())
    }
}

/// Thread-safe wrapper for poisoned behavior
pub struct PoisonedBehaviorSync {
    inner: Arc<RwLock<PoisonedBehavior>>,
}

impl PoisonedBehaviorSync {
    /// Creates a new thread-safe poisoned behavior
    pub fn new(config: PoisonedBehaviorModuleData, object_id: ObjectId) -> Self {
        Self {
            inner: Arc::new(RwLock::new(PoisonedBehavior::new(config, object_id))),
        }
    }
    
    /// Update the behavior (thread-safe)
    pub fn update(&self) -> PoisonedBehaviorResult<UpdateSleepTime> {
        self.inner
            .write()
            .map_err(|e| PoisonedBehaviorError::Threading(e.to_string()))?
            .update()
    }
    
    /// Handle damage (thread-safe)
    pub fn on_damage(&self, damage_info: &mut DamageInfo) -> PoisonedBehaviorResult<()> {
        self.inner
            .write()
            .map_err(|e| PoisonedBehaviorError::Threading(e.to_string()))?
            .on_damage(damage_info)
            .map_err(|e| PoisonedBehaviorError::Threading(e.to_string()))?;
        Ok(())
    }
    
    /// Handle healing (thread-safe)
    pub fn on_healing(&self, damage_info: &mut DamageInfo) -> PoisonedBehaviorResult<()> {
        self.inner
            .write()
            .map_err(|e| PoisonedBehaviorError::Threading(e.to_string()))?
            .on_healing(damage_info)
            .map_err(|e| PoisonedBehaviorError::Threading(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_poisoned_behavior_creation() {
        let config = PoisonedBehaviorModuleData::default();
        let behavior = PoisonedBehavior::new(config, 1);
        assert_eq!(behavior.object_id, 1);
        assert_eq!(behavior.poison_damage_frame, 0);
        assert_eq!(behavior.poison_overall_stop_frame, 0);
    }
    
    #[test]
    fn test_thread_safe_wrapper() {
        let config = PoisonedBehaviorModuleData::default();
        let behavior = PoisonedBehaviorSync::new(config, 1);
        
        // Should be able to update without issues
        let result = behavior.update();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), UpdateSleepTime::Forever);
    }
    
    #[test]
    fn test_damage_info_creation() {
        let damage = DamageInfo::new(
            10.0,
            123,
            DamageType::Poison,
            DeathType::Poisoned,
        );
        
        assert_eq!(damage.amount, 10.0);
        assert_eq!(damage.source_id, 123);
        assert_eq!(damage.damage_type, DamageType::Poison);
        assert_eq!(damage.death_type, DeathType::Poisoned);
        assert_eq!(damage.actual_damage_dealt, 0.0);
    }
    
    #[test]
    fn test_sleep_time_calculation() {
        let config = PoisonedBehaviorModuleData {
            poison_damage_interval: 30,
            poison_duration: 150,
        };
        let behavior = PoisonedBehavior::new(config, 1);
        
        // When not poisoned, should sleep forever
        let sleep_time = behavior.calc_sleep_time(100);
        assert_eq!(sleep_time, UpdateSleepTime::Forever);
    }
}
