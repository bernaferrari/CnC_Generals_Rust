//! Candidate effects belong to one complete map attempt, including early failure.
use super::*;

impl GameLogic {
    pub(super) fn load_map_with_definition_source_and_progress<F>(
        &mut self,
        map_name: &str,
        definition_source: Option<&str>,
        map_script_difficulty: Option<gamelogic::player::GameDifficulty>,
        water_delivery: Option<&mut dyn FnMut(&gamelogic::terrain_water::WaterGridState)>,
        mut native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
        report_progress: F,
    ) -> bool
    where
        F: FnMut(f32, &str),
    {
        if let Some(native) = native.as_mut() {
            native.discard_attempt();
        }
        let accepted = self.load_map_contents_with_definition_source_and_progress(
            map_name,
            definition_source,
            map_script_difficulty,
            water_delivery,
            native
                .as_mut()
                .map(|native| &mut **native as &mut dyn crate::game_logic::NativeMapAdmission),
            report_progress,
        );
        if !accepted && let Some(native) = native.as_mut() {
            native.discard_attempt();
        }
        accepted
    }
}
