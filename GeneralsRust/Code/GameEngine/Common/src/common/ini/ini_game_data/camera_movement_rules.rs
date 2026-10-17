//! Original immutable GameData movement switch; no publication or Options poll.
use super::{GlobalData, INI, parse_game_data_block};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CameraMovementRules {
    pub disable_camera_movement: bool,
}
impl CameraMovementRules {
    pub fn overlay_ini_text(self, contents: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.disable_camera_movement = self.disable_camera_movement;
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
        Ok(Self {
            disable_camera_movement: draft.disable_camera_movement,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_movement_definition_preserves_bool_overlay_and_unrelated_fields() {
        let base = CameraMovementRules::default();
        assert!(!base.disable_camera_movement);
        let map = base
            .overlay_ini_text("GameData\n DisableCameraMovements = Yes\n End\n")
            .unwrap();
        assert!(map.disable_camera_movement);
        assert!(
            map.overlay_ini_text("GameData\n MaxCameraHeight = 600\n End\n")
                .unwrap()
                .disable_camera_movement
        );
        assert!(
            !map.overlay_ini_text("GameData\n DisableCameraMovements = No\n End\n")
                .unwrap()
                .disable_camera_movement
        );
        assert!(!base.disable_camera_movement);
    }
}
