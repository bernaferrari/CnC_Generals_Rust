//! Wander AI Update Module - Idle movement behavior
//!
//! Matches C++ WanderAIUpdate.cpp: if idle, issue a small random move.

use super::{
    AIModulePriority, AIModuleState, AIModuleType, AIUpdateContext, AIUpdateModuleTrait,
    AIUpdateResult,
};
use crate::ai::AiError;
use crate::common::{CommandSourceType, Coord3D, ObjectID, Real};
use crate::helpers::get_game_logic_random_value_real;
use crate::modules::AIUpdateInterfaceExt;
use crate::object::registry::OBJECT_REGISTRY;

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
        OBJECT_REGISTRY.with_object(object_id, |guard| *guard.get_position())
    }

    fn issue_wander_move(&self, object_id: ObjectID) {
        let Some((ai, pos)) = OBJECT_REGISTRY
            .with_object(object_id, |guard| {
                guard
                    .get_ai_update_interface()
                    .map(|ai| (ai, *guard.get_position()))
            })
            .flatten()
        else {
            return;
        };
        let dx: Real = get_game_logic_random_value_real(5.0, 50.0);
        let dy: Real = get_game_logic_random_value_real(5.0, 50.0);
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

        let Some(is_idle) = OBJECT_REGISTRY
            .with_object(context.object_id, |guard| {
                guard.get_ai_update_interface().map(|ai| ai.is_idle())
            })
            .flatten()
        else {
            return Ok(());
        };
        if is_idle {
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
