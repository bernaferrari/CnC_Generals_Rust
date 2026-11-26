//! Observe completed host step identities before the next step can reuse them.

use super::{GameWorldShadow, presentation_entity_count_from_shadow};
use crate::game_logic::GameLogic;

impl GameWorldShadow {
    pub(crate) fn observe_completed_host_step(&mut self, logic: &GameLogic) -> usize {
        self.sync_host_step_identities(logic);
        presentation_entity_count_from_shadow(self)
    }
}

/// Fixtures can inspect the mirror immediately after completing a phase.
#[cfg(test)]
pub(crate) fn run_owned_host_boundary(
    shadow: Option<&mut GameWorldShadow>,
    logic: &mut GameLogic,
) -> usize {
    logic.complete_presentation_observations();
    if let Some(shadow) = shadow {
        shadow.sync_from_host(logic);
        presentation_entity_count_from_shadow(shadow)
    } else {
        0
    }
}
