//! Command queue is a view; GameWorld mutations are last-writer while shadow live.
//!
//! Host `process_commands` still executes C++ side effects (able-to-attack,
//! AI state). While a coupled shadow session is live, attack/move commands
//! also write `SetAttackTarget` / `SetMoveTarget` / `PushAiDecision` so a
//! queued attack lands in GameWorld in the same tick.

use super::*;
use crate::command_system::{CommandType, GameCommand};
use crate::game_logic::GameLogic;
use gamelogic::world::WorldMutation;

const AI_KIND_ATTACK: u8 = 0;
const AI_KIND_MOVE: u8 = 2;

impl GameWorldShadow {
    /// Peek `command_queue` (do not pop) and write GW mutations. Fail-closed
    /// when shadow is not coupled or a selected unit is unmapped.
    pub fn ingest_command_queue_view(&mut self, logic: &GameLogic) -> usize {
        if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
            return 0;
        }
        let mut n = 0usize;
        for cmd in logic.command_queue.iter() {
            n = n.saturating_add(self.queue_command_mutations(cmd));
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    fn queue_command_mutations(&mut self, cmd: &GameCommand) -> usize {
        match &cmd.command_type {
            CommandType::Attack { target_id } | CommandType::AttackObject { target_id } => {
                self.queue_attack_commands(cmd, *target_id)
            }
            CommandType::Move { destination } | CommandType::MoveTo { destination, .. } => {
                self.queue_move_commands(cmd, [destination.x, destination.y, destination.z])
            }
            _ => 0,
        }
    }

    fn queue_attack_commands(
        &mut self,
        cmd: &GameCommand,
        target_host: crate::game_logic::ObjectId,
    ) -> usize {
        let Some(target) = self.entity_for_host(target_host) else {
            return 0;
        };
        let mut n = 0usize;
        for unit in &cmd.selected_units {
            let Some(attacker) = self.entity_for_host(*unit) else {
                continue;
            };
            self.world.queue_mutation(WorldMutation::SetAttackTarget {
                attacker,
                target: Some(target),
            });
            self.world.queue_mutation(WorldMutation::PushAiDecision {
                host_object: unit.0,
                kind: AI_KIND_ATTACK,
                target_host: target_host.0,
                destination: None,
                ai_state_ordinal: 0,
            });
            n = n.saturating_add(1);
        }
        n
    }

    fn queue_move_commands(&mut self, cmd: &GameCommand, destination: [f32; 3]) -> usize {
        let mut n = 0usize;
        for unit in &cmd.selected_units {
            let Some(eid) = self.entity_for_host(*unit) else {
                continue;
            };
            self.world.queue_mutation(WorldMutation::SetMoveTarget {
                unit: eid,
                destination: Some(destination),
            });
            self.world.queue_mutation(WorldMutation::PushAiDecision {
                host_object: unit.0,
                kind: AI_KIND_MOVE,
                target_host: 0,
                destination: Some(destination),
                ai_state_ordinal: 0,
            });
            n = n.saturating_add(1);
        }
        n
    }
}
