//! One driving match owns the AI queue and player phase.

use super::super::GameLogic;

pub(in super::super) struct PathfindAiRules {
    pub repulsed_distance: f32,
    pub wall_height: f32,
}

impl GameLogic {
    /// C++ AI.cpp:332-343, after object modules and before BuildAssistant.
    /// Compatibility registries are never consulted to choose another driver.
    pub(in super::super) fn update_match_ai(&mut self) {
        self.reissue_pending_moves();
        self.process_pathfind_queue();
        let current_time = self.sim_time_seconds;
        crate::ai::AIManager::update_owned(self, current_time);
    }

    /// Map overrides and fallback definitions come from the driving instance.
    /// Copy only the two pathfinding values; no guard escapes into callbacks.
    pub(in super::super) fn pathfind_ai_rules(&self) -> PathfindAiRules {
        let data = self.ai_definitions.data();
        PathfindAiRules {
            repulsed_distance: data.repulsed_distance,
            wall_height: data.wall_height,
        }
    }
}
