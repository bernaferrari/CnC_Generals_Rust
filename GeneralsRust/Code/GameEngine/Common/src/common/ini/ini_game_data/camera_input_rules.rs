//! Receiving immutable LookAt/GameData input coefficients. User selection is separate.
use super::{GlobalData, INI, parse_game_data_block, scroll_factor_from_preferences};
use crate::common::user_preferences::UserPreferences;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraInputRules {
    pub horizontal_scroll_speed_factor: f32,
    pub vertical_scroll_speed_factor: f32,
    pub keyboard_default_scroll_factor: f32,
    pub keyboard_camera_rotate_speed: f32,
}
impl Default for CameraInputRules {
    fn default() -> Self {
        // GlobalData.cpp618-619,955,1028; no runtime global observation.
        Self {
            horizontal_scroll_speed_factor: 1.0,
            vertical_scroll_speed_factor: 1.0,
            keyboard_default_scroll_factor: 0.5,
            keyboard_camera_rotate_speed: 0.1,
        }
    }
}
impl CameraInputRules {
    pub fn overlay_ini_text(self, text: &str) -> Result<Self, String> {
        self.overlay_ini_text_with_parse_observation(text)
            .map(|(rules, _)| rules)
    }
    pub fn overlay_ini_text_with_parse_observation(
        self,
        text: &str,
    ) -> Result<(Self, bool), String> {
        let mut parsed_game_data = false;
        let mut draft = GlobalData::new();
        draft.horizontal_scroll_speed_factor = self.horizontal_scroll_speed_factor;
        draft.vertical_scroll_speed_factor = self.vertical_scroll_speed_factor;
        draft.keyboard_default_scroll_factor = self.keyboard_default_scroll_factor;
        draft.keyboard_camera_rotate_speed = self.keyboard_camera_rotate_speed;
        let mut ini = INI::new();
        ini.with_inline_source(text, |ini| {
            loop {
                ini.read_line()?;
                if ini.is_eof() {
                    break;
                }
                if ini
                    .get_next_token_or_null()
                    .is_some_and(|k| k.eq_ignore_ascii_case("GameData"))
                {
                    parse_game_data_block(ini, &mut draft)?;
                    parsed_game_data = true;
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        Ok((
            Self {
                horizontal_scroll_speed_factor: draft.horizontal_scroll_speed_factor,
                vertical_scroll_speed_factor: draft.vertical_scroll_speed_factor,
                keyboard_default_scroll_factor: draft.keyboard_default_scroll_factor,
                keyboard_camera_rotate_speed: draft.keyboard_camera_rotate_speed,
            },
            parsed_game_data,
        ))
    }
    /// GlobalData::parseGameDataDefinition reapplies Options after each definition.
    /// Missing ScrollFactor preserves the current receiving GameData default.
    pub fn keyboard_scroll_factor(self, preferences: &UserPreferences) -> f32 {
        scroll_factor_from_preferences(preferences, self.keyboard_default_scroll_factor)
    }
}
#[cfg(test)]
#[path = "camera_input_rules_tests.rs"]
mod tests;
