//! Immutable W3DView::shake inputs, admitted by one receiving session.
//! Original GlobalData.cpp416-423/851-858; no global publication or polling.
use super::{GlobalData, INI, parse_game_data_block, runtime_global_data};
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CameraShakeRules {
    pub shake_subtle_intensity: f32,
    pub shake_normal_intensity: f32,
    pub shake_strong_intensity: f32,
    pub shake_severe_intensity: f32,
    pub shake_cine_extreme_intensity: f32,
    pub shake_cine_insane_intensity: f32,
    pub max_shake_intensity: f32,
    pub max_shake_range: f32,
}
impl Default for CameraShakeRules {
    fn default() -> Self {
        Self::from_global_data(&runtime_global_data::GlobalData::default())
    }
}
impl CameraShakeRules {
    pub fn from_global_data(data: &runtime_global_data::GlobalData) -> Self {
        Self {
            shake_subtle_intensity: data.shake_subtle_intensity,
            shake_normal_intensity: data.shake_normal_intensity,
            shake_strong_intensity: data.shake_strong_intensity,
            shake_severe_intensity: data.shake_severe_intensity,
            shake_cine_extreme_intensity: data.shake_cine_extreme_intensity,
            shake_cine_insane_intensity: data.shake_cine_insane_intensity,
            max_shake_intensity: data.max_shake_intensity,
            max_shake_range: data.max_shake_range,
        }
    }
    pub fn validate(self) -> Result<Self, String> {
        if [
            self.shake_subtle_intensity,
            self.shake_normal_intensity,
            self.shake_strong_intensity,
            self.shake_severe_intensity,
            self.shake_cine_extreme_intensity,
            self.shake_cine_insane_intensity,
            self.max_shake_intensity,
            self.max_shake_range,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return Err("nonfinite camera shake rules".into());
        }
        // Finite signed intensity/range values retain the original arithmetic.
        // In particular, zero range is not silently replaced with a floor.
        Ok(self)
    }
    /// The canonical GameData parser over a private draft. Unrelated settings
    /// are discarded, and no global configuration owner is published.
    pub fn overlay_ini_text(self, contents: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.shake_subtle_intensity = self.shake_subtle_intensity;
        draft.shake_normal_intensity = self.shake_normal_intensity;
        draft.shake_strong_intensity = self.shake_strong_intensity;
        draft.shake_severe_intensity = self.shake_severe_intensity;
        draft.shake_cine_extreme_intensity = self.shake_cine_extreme_intensity;
        draft.shake_cine_insane_intensity = self.shake_cine_insane_intensity;
        draft.max_shake_intensity = self.max_shake_intensity;
        draft.max_shake_range = self.max_shake_range;
        let mut ini = INI::new();
        ini.with_inline_source(contents, |ini| {
            loop {
                ini.read_line()?;
                if ini.is_eof() {
                    break;
                }
                if ini
                    .get_next_token_or_null()
                    .is_some_and(|key| key.eq_ignore_ascii_case("GameData"))
                {
                    parse_game_data_block(ini, &mut draft)?;
                }
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;
        Self {
            shake_subtle_intensity: draft.shake_subtle_intensity,
            shake_normal_intensity: draft.shake_normal_intensity,
            shake_strong_intensity: draft.shake_strong_intensity,
            shake_severe_intensity: draft.shake_severe_intensity,
            shake_cine_extreme_intensity: draft.shake_cine_extreme_intensity,
            shake_cine_insane_intensity: draft.shake_cine_insane_intensity,
            max_shake_intensity: draft.max_shake_intensity,
            max_shake_range: draft.max_shake_range,
        }
        .validate()
    }
}

#[cfg(test)]
#[path = "camera_shake_rules_tests.rs"]
mod tests;
