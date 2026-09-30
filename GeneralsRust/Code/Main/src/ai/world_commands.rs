//! Synchronous host execution adapters for strategic AI.
//!
//! Keep command side effects on the driving GameLogic and apply them immediately, as
//! C++ AIPlayer does. These adapters do not buffer decisions or select an active world.
use super::*;

impl GameLogic {
    /// Record C++ `AIAttackMoveState` / `AIInternalMoveToState::onEnter` on the
    /// crate `AiStateMachine` (move/attack only; does not run the 48-state graph).
    pub(super) fn dispatch_ai_attack_move(
        &mut self,
        unit_id: ObjectId,
        dest: Vec3,
        focus: Option<ObjectId>,
    ) {
        let dest = gamelogic::common::types::Coord3D::new(dest.x, dest.y, dest.z);
        let _ = gamelogic::ai::state_machine::dispatch_host_move_attack(
            &mut self.host_move_attack_machines,
            unit_id.0,
            gamelogic::ai::state_machine::HostMoveAttackKind::AttackMoveTo,
            Some(dest),
            focus.map(|id| id.0),
        );
    }

    /// Drain the world-owned TeamFactory's pre-destruction notifications before AI phases.
    pub(super) fn take_ai_team_destroy_notifications(&mut self) -> Vec<(u32, String)> {
        match self.team_factory.lock() {
            Ok(mut factory) => factory.take_host_pre_team_destroy_requests(),
            Err(poisoned) => poisoned.into_inner().take_host_pre_team_destroy_requests(),
        }
    }
}
