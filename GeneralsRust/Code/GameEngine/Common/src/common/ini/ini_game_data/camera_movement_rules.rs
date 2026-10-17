//! Original immutable GameData movement switch; no publication or Options poll.
use super::{GlobalData, INI, parse_game_data_block};
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraMovementRules {
    pub disable_camera_movement: bool,
    /// C++ GlobalData constructor default is zero; map overrides are unsaved.
    pub partition_cell_size: f32,
}
impl CameraMovementRules {
    pub fn overlay_ini_text(self, contents: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.disable_camera_movement = self.disable_camera_movement;
        draft.partition_cell_size = self.partition_cell_size;
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
            partition_cell_size: draft.partition_cell_size,
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

#[cfg(test)]
mod partition_tests {
    use super::*;
    #[test]
    fn receiving_partition_definition_preserves_cpp_zero_and_ordered_overlay() {
        let base = CameraMovementRules::default();
        assert_eq!(base.partition_cell_size, 0.0);
        assert_eq!(
            base.overlay_ini_text("GameData\n MaxCameraHeight = 600\n End\n")
                .unwrap()
                .partition_cell_size,
            0.0
        );
        let map = base
            .overlay_ini_text("GameData\n PartitionCellSize = 40\n End\n")
            .unwrap();
        assert_eq!(map.partition_cell_size, 40.0);
        assert_eq!(
            map.overlay_ini_text("GameData\n DisableCameraMovements = Yes\n End\n")
                .unwrap()
                .partition_cell_size,
            40.0
        );
        assert_eq!(
            map.overlay_ini_text("GameData\n PartitionCellSize = 80\n End\n")
                .unwrap()
                .partition_cell_size,
            80.0
        );
    }
}
