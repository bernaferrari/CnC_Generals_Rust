//! Observe completed host step identities before the next step can reuse them.

use super::{GameWorldShadow, presentation_entity_count_from_shadow};
use crate::game_logic::GameLogic;

impl GameWorldShadow {
    pub(crate) fn observe_completed_host_step(&mut self, logic: &GameLogic) -> usize {
        self.sync_host_step_identities(logic);
        presentation_entity_count_from_shadow(self)
    }
}
