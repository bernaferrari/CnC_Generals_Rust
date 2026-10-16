//! Immutable camera GameData inputs, parsed privately for one receiving session.
use super::{GlobalData, INI, parse_game_data_block, runtime_global_data};
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CameraContentRules {
    pub camera_height: f32,
    pub camera_pitch: f32,
    pub camera_yaw: f32,
    pub min_camera_height: f32,
    pub max_camera_height: f32,
    pub enforce_max_camera_height: bool,
    pub camera_adjust_speed: f32,
    pub scroll_amount_cutoff: f32,
    pub use_camera_in_replay: bool,
}
impl Default for CameraContentRules {
    fn default() -> Self {
        Self::from_global_data(&runtime_global_data::GlobalData::default())
    }
}
impl CameraContentRules {
    pub fn from_global_data(data: &runtime_global_data::GlobalData) -> Self {
        Self {
            camera_height: data.camera_height,
            camera_pitch: data.camera_pitch,
            camera_yaw: data.camera_yaw,
            min_camera_height: data.min_camera_height,
            max_camera_height: data.max_camera_height,
            enforce_max_camera_height: data.enforce_max_camera_height,
            camera_adjust_speed: data.camera_adjust_speed,
            scroll_amount_cutoff: data.scroll_amount_cutoff,
            use_camera_in_replay: data.use_camera_in_replay,
        }
    }
    pub fn validate(self) -> Result<Self, String> {
        if !(self.camera_height.is_finite()
            && self.camera_pitch.is_finite()
            && self.camera_yaw.is_finite()
            && self.min_camera_height.is_finite()
            && self.max_camera_height.is_finite()
            && self.camera_adjust_speed.is_finite()
            && self.scroll_amount_cutoff.is_finite())
        {
            return Err("nonfinite camera content rules".into());
        }
        Ok(self)
    }
    /// Original Common GameData parser; no global publication or lookup.
    pub fn overlay_ini_text(self, contents: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.camera_height = self.camera_height;
        draft.camera_pitch = self.camera_pitch;
        draft.camera_yaw = self.camera_yaw;
        draft.min_camera_height = self.min_camera_height;
        draft.max_camera_height = self.max_camera_height;
        draft.enforce_max_camera_height = self.enforce_max_camera_height;
        draft.camera_adjust_speed = self.camera_adjust_speed;
        draft.scroll_amount_cutoff = self.scroll_amount_cutoff;
        draft.use_camera_in_replay = self.use_camera_in_replay;
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
        .map_err(|e| e.to_string())?;
        Self {
            camera_height: draft.camera_height,
            camera_pitch: draft.camera_pitch,
            camera_yaw: draft.camera_yaw,
            min_camera_height: draft.min_camera_height,
            max_camera_height: draft.max_camera_height,
            enforce_max_camera_height: draft.enforce_max_camera_height,
            camera_adjust_speed: draft.camera_adjust_speed,
            scroll_amount_cutoff: draft.scroll_amount_cutoff,
            use_camera_in_replay: draft.use_camera_in_replay,
        }
        .validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_camera_overlay_uses_original_gamedata_fields_and_keeps_peer_immutable() {
        let base = CameraContentRules::default();
        let peer = base;
        let mapped=base.overlay_ini_text("GameData\n MinCameraHeight = 111\n MaxCameraHeight = 777\n CameraHeight = 333\n End\n").unwrap();
        assert_eq!(mapped.min_camera_height, 111.);
        assert_eq!(mapped.max_camera_height, 777.);
        assert_eq!(mapped.camera_height, 333.);
        assert_eq!(base, peer);
        assert_eq!(mapped.camera_pitch, base.camera_pitch);
    }
}
