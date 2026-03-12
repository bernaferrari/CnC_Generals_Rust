//! Turret AI Update Module - Base defense turret behavior
//!
//! Handles AI for stationary defense turrets including:
//! - Target scanning
//! - Threat prioritization
//! - Firing control
//! - Arc of fire management
//! - Power state management

use super::{
    AIModulePriority, AIModuleState, AIModuleType, AIUpdateContext, AIUpdateModuleTrait,
    AIUpdateResult,
};
use crate::ai::AiError;
use crate::common::{Coord3D, ObjectID, Real};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurretState {
    Idle,
    Scanning,
    Tracking,
    Firing,
    Reloading,
    PoweredDown,
}

#[derive(Debug)]
pub struct TurretAIUpdate {
    state: AIModuleState,
    turret_state: TurretState,

    current_target: Option<ObjectID>,
    potential_targets: Vec<ObjectID>,

    scan_radius: Real,
    fire_arc: Real, // Degrees
    current_rotation: Real,

    ammo_count: i32,
    max_ammo: i32,
    reload_time: Real,
    reload_timer: Real,

    scan_interval: u32,
    last_scan: u32,

    powered: bool,
}

impl TurretAIUpdate {
    pub fn new() -> Self {
        Self {
            state: AIModuleState::Idle,
            turret_state: TurretState::Idle,
            current_target: None,
            potential_targets: Vec::new(),
            scan_radius: 300.0,
            fire_arc: 360.0,
            current_rotation: 0.0,
            ammo_count: 100,
            max_ammo: 100,
            reload_time: 3.0,
            reload_timer: 0.0,
            scan_interval: 15, // Scan every 0.5 seconds
            last_scan: 0,
            powered: true,
        }
    }

    pub fn set_powered(&mut self, powered: bool) {
        self.powered = powered;
        if !powered {
            self.turret_state = TurretState::PoweredDown;
        }
    }

    pub fn set_scan_radius(&mut self, radius: Real) {
        self.scan_radius = radius;
    }

    fn scan_for_targets(&mut self, context: &AIUpdateContext) -> AIUpdateResult<()> {
        if context.current_frame - self.last_scan < self.scan_interval {
            return Ok(());
        }

        self.last_scan = context.current_frame;

        // Would scan for enemies in radius
        // For now, simulate target acquisition
        self.potential_targets.clear();

        if !self.potential_targets.is_empty() {
            self.turret_state = TurretState::Tracking;
        }

        Ok(())
    }

    fn select_best_target(&mut self) -> Option<ObjectID> {
        // Would prioritize targets based on:
        // - Threat level
        // - Distance
        // - Unit type
        // - Within fire arc
        self.potential_targets.first().copied()
    }

    fn track_target(&mut self, _context: &AIUpdateContext) -> AIUpdateResult<()> {
        if let Some(_target) = self.current_target {
            // Would calculate angle to target and rotate turret
            // Check if within fire arc
            if self.can_fire_at_target() {
                self.turret_state = TurretState::Firing;
            }
        } else {
            self.turret_state = TurretState::Scanning;
        }
        Ok(())
    }

    fn fire_at_target(&mut self) -> AIUpdateResult<()> {
        if self.ammo_count > 0 {
            self.ammo_count -= 1;

            if self.ammo_count == 0 {
                self.turret_state = TurretState::Reloading;
                self.reload_timer = 0.0;
            }
        }
        Ok(())
    }

    fn reload(&mut self, delta_time: Real) -> AIUpdateResult<()> {
        self.reload_timer += delta_time;

        if self.reload_timer >= self.reload_time {
            self.ammo_count = self.max_ammo;
            self.reload_timer = 0.0;
            self.turret_state = TurretState::Scanning;
        }

        Ok(())
    }

    fn can_fire_at_target(&self) -> bool {
        // Check if target is within fire arc
        // For now, always return true
        true
    }
}

impl AIUpdateModuleTrait for TurretAIUpdate {
    fn get_module_type(&self) -> AIModuleType {
        AIModuleType::Turret
    }

    fn get_priority(&self) -> AIModulePriority {
        AIModulePriority::Critical // Defense is critical
    }

    fn get_state(&self) -> AIModuleState {
        self.state
    }

    fn init(&mut self, _context: &AIUpdateContext) -> AIUpdateResult<()> {
        self.state = AIModuleState::Active;
        self.turret_state = TurretState::Scanning;
        Ok(())
    }

    fn reset(&mut self) -> AIUpdateResult<()> {
        self.turret_state = TurretState::Idle;
        self.current_target = None;
        self.potential_targets.clear();
        Ok(())
    }

    fn update(&mut self, context: &mut AIUpdateContext) -> AIUpdateResult<()> {
        if !self.powered {
            return Ok(());
        }

        match self.turret_state {
            TurretState::Idle | TurretState::Scanning => {
                self.scan_for_targets(context)?;
            }
            TurretState::Tracking => {
                self.track_target(context)?;
            }
            TurretState::Firing => {
                self.fire_at_target()?;

                // Check if target still valid
                if self.current_target.is_none() {
                    self.turret_state = TurretState::Scanning;
                }
            }
            TurretState::Reloading => {
                self.reload(context.delta_time)?;
            }
            TurretState::PoweredDown => {
                // Do nothing
            }
        }

        Ok(())
    }

    fn should_update(&self, _context: &AIUpdateContext) -> bool {
        self.powered
    }
}

impl Default for TurretAIUpdate {
    fn default() -> Self {
        Self::new()
    }
}
