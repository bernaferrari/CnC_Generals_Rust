//! GameWorld sole-tick for factory doors under PRODUCTION_AUTHORITY.
//!
//! C++ `ProductionUpdate::updateDoors` (`ProductionUpdate.cpp:513`) advances
//! OPENING → WAITING_OPEN → WAITING_TO_CLOSE → CLOSING once per logic frame
//! when the phase deadline is reached. Host `Object::tick_production_door`
//! uses INI DoorOpeningTime / DoorWaitOpenTime / DoorCloseTime.

use super::*;
use crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration;
use gamelogic::world::WorldMutation;

impl GameWorldShadow {
    /// Advance production door phases once per logic frame (C++ updateDoors).
    pub fn tick_production_doors(&mut self, now: u32) -> usize {
        if !gameworld_production_authority_enabled() {
            return 0;
        }
        let mut n = 0usize;
        let mut updates: Vec<(gamelogic::world::entities::EntityId, u8, u32, bool)> = Vec::new();
        let ids: Vec<gamelogic::world::entities::EntityId> =
            self.host_to_entity.values().copied().collect();
        for eid in ids {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            if ent.production_door_phase == 0 {
                continue;
            }
            if now < ent.production_door_phase_end_frame {
                continue;
            }
            let hold = ent.production_door_hold_open;
            let name = ent.template_name();
            let (next_phase, next_end) = match ent.production_door_phase {
                1 => (2, now.saturating_add(producer_door_phase_duration(name, 2))),
                2 if hold => (2, now.saturating_add(producer_door_phase_duration(name, 2))),
                2 => (3, now.saturating_add(1)),
                3 if hold => (3, now.saturating_add(1)),
                3 => (4, now.saturating_add(producer_door_phase_duration(name, 4))),
                4 if hold => (2, now.saturating_add(producer_door_phase_duration(name, 2))),
                4 => (0, 0),
                _ => (0, 0),
            };
            if next_phase == ent.production_door_phase
                && next_end == ent.production_door_phase_end_frame
            {
                continue;
            }
            updates.push((eid, next_phase, next_end, hold));
            n += 1;
        }
        for (target, production_door_phase, production_door_phase_end_frame, hold) in updates {
            self.world.queue_mutation(WorldMutation::SetProductionDoor {
                target,
                production_door_phase,
                production_door_phase_end_frame,
                production_door_hold_open: hold,
            });
        }
        if n > 0 {
            let _ = self.world.apply_pending_mutations();
        }
        n
    }
}
