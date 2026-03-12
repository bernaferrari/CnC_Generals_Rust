//! Jet AI Update Module - Fighter jet and aircraft behavior
//!
//! Handles AI for jets and aircraft including:
//! - Target acquisition
//! - Attack runs
//! - Return to airfield
//! - Refueling
//! - Evasive maneuvers

use super::{
    AIModulePriority, AIModuleState, AIModuleType, AIUpdateContext, AIUpdateModuleTrait,
    AIUpdateResult,
};
use crate::ai::AiError;
use crate::common::{Coord3D, ObjectID, Real};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JetState {
    Idle,
    TakingOff,
    Searching,
    AttackRun,
    Returning,
    Landing,
    Refueling,
    Evading,
}

#[derive(Debug)]
pub struct JetAIUpdate {
    state: AIModuleState,
    jet_state: JetState,

    airfield: Option<ObjectID>,
    attack_target: Option<ObjectID>,

    fuel_level: f32,
    max_fuel: f32,
    fuel_consumption_rate: f32,

    ammo_count: i32,
    max_ammo: i32,

    last_attack_run: u32,
    attack_cooldown: u32,
}

impl JetAIUpdate {
    pub fn new() -> Self {
        Self {
            state: AIModuleState::Idle,
            jet_state: JetState::Idle,
            airfield: None,
            attack_target: None,
            fuel_level: 100.0,
            max_fuel: 100.0,
            fuel_consumption_rate: 1.0,
            ammo_count: 4,
            max_ammo: 4,
            last_attack_run: 0,
            attack_cooldown: 180, // 6 seconds
        }
    }

    pub fn set_airfield(&mut self, airfield: ObjectID) {
        self.airfield = Some(airfield);
    }

    pub fn set_attack_target(&mut self, target: ObjectID) {
        self.attack_target = Some(target);
        self.jet_state = JetState::AttackRun;
    }

    pub fn needs_refuel(&self) -> bool {
        self.fuel_level < self.max_fuel * 0.3
    }

    pub fn needs_rearm(&self) -> bool {
        self.ammo_count == 0
    }

    fn consume_fuel(&mut self, delta_time: Real) {
        self.fuel_level = (self.fuel_level - self.fuel_consumption_rate * delta_time).max(0.0);
    }
}

impl AIUpdateModuleTrait for JetAIUpdate {
    fn get_module_type(&self) -> AIModuleType {
        AIModuleType::Jet
    }

    fn get_priority(&self) -> AIModulePriority {
        AIModulePriority::High
    }

    fn get_state(&self) -> AIModuleState {
        self.state
    }

    fn init(&mut self, _context: &AIUpdateContext) -> AIUpdateResult<()> {
        self.state = AIModuleState::Idle;
        Ok(())
    }

    fn reset(&mut self) -> AIUpdateResult<()> {
        self.jet_state = JetState::Idle;
        self.fuel_level = self.max_fuel;
        self.ammo_count = self.max_ammo;
        Ok(())
    }

    fn update(&mut self, context: &mut AIUpdateContext) -> AIUpdateResult<()> {
        self.consume_fuel(context.delta_time);

        // Check if need to return to base
        if self.needs_refuel() || self.needs_rearm() {
            if !matches!(
                self.jet_state,
                JetState::Returning | JetState::Landing | JetState::Refueling
            ) {
                self.jet_state = JetState::Returning;
            }
        }

        match self.jet_state {
            JetState::TakingOff => {
                if !context.is_moving {
                    self.jet_state = JetState::Searching;
                }
            }
            JetState::Searching => {
                if let Some(_target) = self.attack_target {
                    self.jet_state = JetState::AttackRun;
                }
            }
            JetState::AttackRun => {
                if self.ammo_count > 0 {
                    self.ammo_count -= 1;
                    self.last_attack_run = context.current_frame;
                }

                if self.ammo_count == 0 || self.attack_target.is_none() {
                    self.jet_state = JetState::Returning;
                }
            }
            JetState::Returning => {
                if !context.is_moving {
                    self.jet_state = JetState::Landing;
                }
            }
            JetState::Landing => {
                self.jet_state = JetState::Refueling;
            }
            JetState::Refueling => {
                self.fuel_level = (self.fuel_level + 10.0 * context.delta_time).min(self.max_fuel);
                if self.fuel_level >= self.max_fuel {
                    self.ammo_count = self.max_ammo;
                    self.jet_state = JetState::Idle;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn should_update(&self, _context: &AIUpdateContext) -> bool {
        true
    }

    fn on_damage_received(
        &mut self,
        _damage: f32,
        _attacker: Option<ObjectID>,
    ) -> AIUpdateResult<()> {
        if !matches!(self.jet_state, JetState::Returning | JetState::Landing) {
            self.jet_state = JetState::Evading;
        }
        Ok(())
    }
}

impl Default for JetAIUpdate {
    fn default() -> Self {
        Self::new()
    }
}
