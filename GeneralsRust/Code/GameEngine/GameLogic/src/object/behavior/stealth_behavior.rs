//! Stealth Behavior Implementation
//!
//! Manages stealth capabilities, detection, and visibility states.
//! Derived from C++ StealthUpdate module.

use super::advanced_behavior_system::{
    AdvancedBehavior, BehaviorContext, BehaviorEvent, BehaviorOutcome, BehaviorPriority,
    BehaviorState,
};
use crate::common::*;
use crate::object::{Object, ObjectId};
use crate::GameLogicResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Stealth configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StealthConfig {
    /// Time before re-entering stealth (seconds)
    pub stealth_delay: f32,
    /// Time to transition between states (seconds)
    pub unstealth_delay: f32,
    /// Detection radius when moving
    pub moving_detection_radius: f32,
    /// Detection radius when stationary
    pub stationary_detection_radius: f32,
    /// Can unit remain stealthed while moving?
    pub can_stealth_while_moving: bool,
    /// Does attacking break stealth?
    pub broken_by_attacking: bool,
    /// Does taking damage break stealth?
    pub broken_by_damage: bool,
    /// Does the unit require power to be stealthed?
    pub requires_power: bool,
    /// Power consumption while stealthed
    pub power_consumption: f32,
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            stealth_delay: 2.0,
            unstealth_delay: 1.0,
            moving_detection_radius: 100.0,
            stationary_detection_radius: 50.0,
            can_stealth_while_moving: true,
            broken_by_attacking: true,
            broken_by_damage: true,
            requires_power: false,
            power_consumption: 0.0,
        }
    }
}

/// Current state of stealth
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StealthState {
    Visible,
    TransitioningToStealth,
    Stealthed,
    TransitioningToVisible,
    Detected,
}

/// Stealth Behavior implementation
#[derive(Debug)]
pub struct StealthBehavior {
    config: StealthConfig,
    stealth_state: StealthState,
    state_transition_time: Option<Instant>,
    last_movement_time: Option<Instant>,
    last_attack_time: Option<Instant>,
    last_damage_time: Option<Instant>,
    detection_tags: HashMap<ObjectId, Instant>,
    power_drain_accumulator: f32,
    forced_visible_until: Option<Instant>,
}

impl StealthBehavior {
    pub fn new() -> Self {
        Self::with_config(StealthConfig::default())
    }

    pub fn with_config(config: StealthConfig) -> Self {
        Self {
            config,
            stealth_state: StealthState::Visible,
            state_transition_time: None,
            last_movement_time: None,
            last_attack_time: None,
            last_damage_time: None,
            detection_tags: HashMap::new(),
            power_drain_accumulator: 0.0,
            forced_visible_until: None,
        }
    }

    fn is_moving(&self, object: &Object) -> bool {
        object
            .get_physics()
            .and_then(|physics| physics.lock().ok().map(|guard| guard.get_velocity()))
            .map(|vel| (vel.x * vel.x + vel.y * vel.y + vel.z * vel.z) > 0.01)
            .unwrap_or(false)
    }

    async fn update_power(&mut self, object: &mut Object, delta_time: f32) -> bool {
        if !self.config.requires_power {
            return true;
        }

        self.power_drain_accumulator += delta_time;
        if self.power_drain_accumulator >= 1.0 {
            self.power_drain_accumulator = 0.0;
        }

        !object.is_disabled_by_type(DisabledType::DisabledUnderpowered)
            && !object.is_disabled_by_type(DisabledType::DisabledScriptUnderpowered)
    }

    async fn update_detection(&mut self, _object: &mut Object) -> GameLogicResult<bool> {
        // Remove expired tags
        let now = Instant::now();
        self.detection_tags.retain(|_, expiry| *expiry > now);

        if !self.detection_tags.is_empty() {
            return Ok(true);
        }

        if let Some(forced_visible) = self.forced_visible_until {
            if forced_visible > now {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::DefaultThingTemplate;
    use std::sync::Arc;

    #[tokio::test]
    async fn required_power_blocks_stealth_when_object_is_underpowered() {
        let template = Arc::new(DefaultThingTemplate::default());
        let object = Object::new(template, ObjectStatusMaskType::none(), None)
            .expect("test object should construct");
        let mut object = object
            .write()
            .expect("test object lock should be available");
        object.set_disabled(DisabledType::DisabledUnderpowered);

        let mut behavior = StealthBehavior::with_config(StealthConfig {
            requires_power: true,
            ..StealthConfig::default()
        });

        assert!(!behavior.update_power(&mut object, 1.0).await);
    }

    #[tokio::test]
    async fn required_power_blocks_stealth_when_script_underpowered() {
        let template = Arc::new(DefaultThingTemplate::default());
        let object = Object::new(template, ObjectStatusMaskType::none(), None)
            .expect("test object should construct");
        let mut object = object
            .write()
            .expect("test object lock should be available");
        object.set_disabled(DisabledType::DisabledScriptUnderpowered);

        let mut behavior = StealthBehavior::with_config(StealthConfig {
            requires_power: true,
            ..StealthConfig::default()
        });

        assert!(!behavior.update_power(&mut object, 1.0).await);
    }

    #[tokio::test]
    async fn optional_power_ignores_underpowered_disable() {
        let template = Arc::new(DefaultThingTemplate::default());
        let object = Object::new(template, ObjectStatusMaskType::none(), None)
            .expect("test object should construct");
        let mut object = object
            .write()
            .expect("test object lock should be available");
        object.set_disabled(DisabledType::DisabledUnderpowered);

        let mut behavior = StealthBehavior::with_config(StealthConfig {
            requires_power: false,
            ..StealthConfig::default()
        });

        assert!(behavior.update_power(&mut object, 1.0).await);
    }
}

#[async_trait]
impl AdvancedBehavior for StealthBehavior {
    fn name(&self) -> &str {
        "Stealth"
    }

    fn priority(&self) -> BehaviorPriority {
        BehaviorPriority::High
    }

    async fn initialize(
        &mut self,
        object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        object.enable_stealth_capability(true);
        self.stealth_state = StealthState::TransitioningToStealth;
        self.state_transition_time = Some(Instant::now());
        log::info!(
            "Stealth behavior initialized for object {}",
            object.get_id()
        );
        Ok(())
    }

    async fn update(
        &mut self,
        object: &mut Object,
        context: &BehaviorContext,
    ) -> GameLogicResult<BehaviorOutcome> {
        // Update timers
        if self.is_moving(object) {
            self.last_movement_time = Some(Instant::now());
        }

        // Check power
        if !self.update_power(object, context.delta_time).await {
            self.stealth_state = StealthState::Visible;
            object
                .set_stealth_visibility(1.0)
                .await
                .map_err(crate::GameLogicError::ModuleError)?;
            return Ok(BehaviorOutcome::Continue);
        }

        // Check detection
        let is_detected = self.update_detection(object).await?;

        // State Machine
        match self.stealth_state {
            StealthState::Visible => {
                if !is_detected {
                    let can_stealth = if !self.config.can_stealth_while_moving {
                        if let Some(last_move) = self.last_movement_time {
                            last_move.elapsed().as_secs_f32() > 0.5
                        } else {
                            true
                        }
                    } else {
                        true
                    };

                    if can_stealth {
                        self.stealth_state = StealthState::TransitioningToStealth;
                        self.state_transition_time = Some(Instant::now());
                    }
                }
            }
            StealthState::TransitioningToStealth => {
                if let Some(start_time) = self.state_transition_time {
                    let progress =
                        start_time.elapsed().as_secs_f32() / self.config.stealth_delay.max(0.1);
                    if progress >= 1.0 {
                        self.stealth_state = StealthState::Stealthed;
                        object
                            .set_stealth_visibility(0.0)
                            .await
                            .map_err(crate::GameLogicError::ModuleError)?;
                        log::debug!("Entered stealth mode");
                    } else {
                        object
                            .set_stealth_visibility(1.0 - progress)
                            .await
                            .map_err(crate::GameLogicError::ModuleError)?;
                    }
                }
            }
            StealthState::Stealthed => {
                if is_detected {
                    self.stealth_state = StealthState::TransitioningToVisible;
                    self.state_transition_time = Some(Instant::now());
                } else if !self.config.can_stealth_while_moving && self.is_moving(object) {
                    self.stealth_state = StealthState::TransitioningToVisible;
                    self.state_transition_time = Some(Instant::now());
                }
                // Handle attack/damage breaking stealth
                if self.config.broken_by_attacking {
                    if let Some(last_attack) = self.last_attack_time {
                        if last_attack.elapsed().as_secs_f32() < 2.0 {
                            self.stealth_state = StealthState::TransitioningToVisible;
                            self.state_transition_time = Some(Instant::now());
                        }
                    }
                }
            }
            StealthState::TransitioningToVisible => {
                if let Some(start_time) = self.state_transition_time {
                    let progress =
                        start_time.elapsed().as_secs_f32() / self.config.unstealth_delay.max(0.1);
                    if progress >= 1.0 {
                        self.stealth_state = StealthState::Visible; // Or Detect depending on reason
                        if is_detected {
                            self.stealth_state = StealthState::Detected;
                        }
                        object
                            .set_stealth_visibility(1.0)
                            .await
                            .map_err(crate::GameLogicError::ModuleError)?;
                    } else {
                        object
                            .set_stealth_visibility(progress)
                            .await
                            .map_err(crate::GameLogicError::ModuleError)?;
                    }
                }
            }
            StealthState::Detected => {
                if !is_detected {
                    // Can return to stealth after delay
                    // Use forced_visible_until logic or simply transition back
                    self.stealth_state = StealthState::Visible;
                }
            }
        }

        Ok(BehaviorOutcome::Continue)
    }

    async fn cleanup(
        &mut self,
        object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        // Ensure unit is visible when behavior is removed
        object
            .set_stealth_visibility(1.0)
            .await
            .map_err(crate::GameLogicError::ModuleError)?;
        object.enable_stealth_capability(false);
        log::info!("Stealth behavior cleanup completed");
        Ok(())
    }

    async fn handle_event(
        &mut self,
        event: &BehaviorEvent,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        match event {
            BehaviorEvent::DamageReceived { .. } => {
                if self.config.broken_by_damage {
                    self.last_damage_time = Some(Instant::now());
                    if self.stealth_state == StealthState::Stealthed {
                        self.stealth_state = StealthState::TransitioningToVisible;
                        self.state_transition_time = Some(Instant::now());
                    }
                }
            }
            BehaviorEvent::WeaponFired { .. } => {
                self.last_attack_time = Some(Instant::now());
                // Detection logic handled in update loop based on time
            }
            _ => {}
        }
        Ok(())
    }
}
