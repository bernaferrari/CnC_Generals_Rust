//! Wander AI Update Module - Idle movement behavior
//!
//! Matches C++ WanderAIUpdate.cpp: if idle, issue a small random move.

use super::{
    AIModulePriority, AIModuleState, AIModuleType, AIUpdateContext, AIUpdateModuleTrait,
    AIUpdateResult,
};
use crate::ai::AiError;
use crate::common::{CommandSourceType, Coord3D, ObjectID, Real};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::registry::OBJECT_REGISTRY;
use rand::Rng;

#[derive(Debug)]
pub struct WanderAIUpdate {
    state: AIModuleState,
}

impl WanderAIUpdate {
    pub fn new() -> Self {
        Self {
            state: AIModuleState::Idle,
        }
    }

    fn get_object_position(&self, object_id: ObjectID) -> Option<Coord3D> {
        let obj = OBJECT_REGISTRY.get_object(object_id)?;
        let guard = obj.read().ok()?;
        Some(*guard.get_position())
    }

    fn issue_wander_move(&self, object_id: ObjectID) {
        let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(guard) = obj.read() else {
            return;
        };
        let Some(ai) = guard.get_ai_update_interface() else {
            return;
        };
        let mut rng = rand::thread_rng();
        let dx: Real = rng.gen_range(5.0..=50.0);
        let dy: Real = rng.gen_range(5.0..=50.0);
        let pos = guard.get_position();
        let dest = Coord3D::new(pos.x + dx, pos.y + dy, pos.z);
        ai.ai_move_to_position(&dest, false, CommandSourceType::FromAi);
    }
}

impl AIUpdateModuleTrait for WanderAIUpdate {
    fn get_module_type(&self) -> AIModuleType {
        AIModuleType::Wander
    }

    fn get_priority(&self) -> AIModulePriority {
        AIModulePriority::Low
    }

    fn get_state(&self) -> AIModuleState {
        self.state
    }

    fn init(&mut self, _context: &AIUpdateContext) -> AIUpdateResult<()> {
        self.state = AIModuleState::Idle;
        Ok(())
    }

    fn reset(&mut self) -> AIUpdateResult<()> {
        self.state = AIModuleState::Idle;
        Ok(())
    }

    fn update(&mut self, context: &mut AIUpdateContext) -> AIUpdateResult<()> {
        let Some(pos) = self.get_object_position(context.object_id) else {
            return Ok(());
        };
        context.position = pos;

        let Some(obj) = OBJECT_REGISTRY.get_object(context.object_id) else {
            return Ok(());
        };
        let Ok(guard) = obj.read() else {
            return Ok(());
        };
        let Some(ai) = guard.get_ai_update_interface() else {
            return Ok(());
        };
        if ai.is_idle() {
            self.issue_wander_move(context.object_id);
        }
        Ok(())
    }

    fn should_update(&self, _context: &AIUpdateContext) -> bool {
        true
    }
}

impl Default for WanderAIUpdate {
    fn default() -> Self {
        Self::new()
    }
}
