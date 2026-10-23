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
        self.update_match_ai_with_effects(&mut Default::default())
    }

    pub(in super::super) fn update_match_ai_with_effects(
        &mut self,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // AIStates.cpp:2052-2085 updates the selected object goal during
        // object AI; AI.cpp:332 then consumes its path queue. Re-read the
        // driving object here so queued work sees this frame's goal pose.
        let object_ids: Vec<_> = self.objects.registered_keys().copied().collect();
        self.refresh_movement_goal_objects(&object_ids);
        self.reissue_pending_moves();
        self.process_pathfind_queue();
        let current_time = self.sim_time_seconds;
        crate::ai::AIManager::update_owned_with_effects(self, current_time, ui);
    }
    // Explicit UI-only adapter for standalone callers; native roots lend both domains.
    pub(in super::super) fn update_match_ai_with_ui(
        &mut self,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) {
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            audio: None,
            terrain: None,
            ui: ui
                .as_mut()
                .map(|ui| &mut **ui as &mut dyn crate::game_logic::ScriptUiEffects),
            view: None,
        };
        self.update_match_ai_with_effects(&mut effects)
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
