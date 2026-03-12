//! Transport AI Update Module - Troop transport and evacuation
//!
//! Handles AI for transport vehicles (APCs, Overlords, etc.) including:
//! - Loading units
//! - Transport to destination
//! - Unloading units
//! - Evacuation procedures
//! - Safety/escort behavior

use super::{
    AIModulePriority, AIModuleState, AIModuleType, AIUpdateContext, AIUpdateModuleTrait,
    AIUpdateResult,
};
use crate::ai::AiError;
use crate::common::{Coord3D, ObjectID, Real};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Idle,
    MovingToPickup,
    Loading,
    Transporting,
    Unloading,
    Evacuating,
}

#[derive(Debug)]
pub struct TransportAIUpdate {
    state: AIModuleState,
    transport_state: TransportState,

    pickup_location: Option<Coord3D>,
    dropoff_location: Option<Coord3D>,

    passengers: Vec<ObjectID>,
    max_capacity: usize,

    load_time: Real,
    loading_timer: Real,
}

impl TransportAIUpdate {
    pub fn new() -> Self {
        Self {
            state: AIModuleState::Idle,
            transport_state: TransportState::Idle,
            pickup_location: None,
            dropoff_location: None,
            passengers: Vec::new(),
            max_capacity: 8,
            load_time: 2.0,
            loading_timer: 0.0,
        }
    }

    pub fn set_transport_mission(&mut self, pickup: Coord3D, dropoff: Coord3D) {
        self.pickup_location = Some(pickup);
        self.dropoff_location = Some(dropoff);
        self.transport_state = TransportState::MovingToPickup;
    }

    pub fn is_full(&self) -> bool {
        self.passengers.len() >= self.max_capacity
    }

    pub fn is_empty(&self) -> bool {
        self.passengers.is_empty()
    }
}

impl AIUpdateModuleTrait for TransportAIUpdate {
    fn get_module_type(&self) -> AIModuleType {
        AIModuleType::Transport
    }

    fn get_priority(&self) -> AIModulePriority {
        AIModulePriority::Normal
    }

    fn get_state(&self) -> AIModuleState {
        self.state
    }

    fn init(&mut self, _context: &AIUpdateContext) -> AIUpdateResult<()> {
        self.state = AIModuleState::Idle;
        Ok(())
    }

    fn reset(&mut self) -> AIUpdateResult<()> {
        self.transport_state = TransportState::Idle;
        self.passengers.clear();
        Ok(())
    }

    fn update(&mut self, context: &mut AIUpdateContext) -> AIUpdateResult<()> {
        match self.transport_state {
            TransportState::MovingToPickup => {
                if !context.is_moving {
                    self.transport_state = TransportState::Loading;
                    self.loading_timer = 0.0;
                }
            }
            TransportState::Loading => {
                self.loading_timer += context.delta_time;
                if self.loading_timer >= self.load_time || self.is_full() {
                    self.transport_state = TransportState::Transporting;
                }
            }
            TransportState::Transporting => {
                if !context.is_moving {
                    self.transport_state = TransportState::Unloading;
                }
            }
            TransportState::Unloading => {
                self.passengers.clear();
                self.transport_state = TransportState::Idle;
            }
            _ => {}
        }
        Ok(())
    }

    fn should_update(&self, _context: &AIUpdateContext) -> bool {
        !matches!(self.transport_state, TransportState::Idle)
    }
}

impl Default for TransportAIUpdate {
    fn default() -> Self {
        Self::new()
    }
}
