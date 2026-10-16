//! Immutable receiving definition; kept separate from frozen CameraContentRules V5.
use super::{GlobalData, INI, parse_game_data_block};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CameraClipRules {
    pub draw_entire_terrain: bool,
}
impl CameraClipRules {
    pub fn overlay_ini_text(self, contents: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.draw_entire_terrain = self.draw_entire_terrain;
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
            draw_entire_terrain: draft.draw_entire_terrain,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_clip_definition_parser_applies_original_bool_and_keeps_peer() {
        let base = CameraClipRules::default();
        let peer = base;
        let map = base
            .overlay_ini_text("GameData\n DrawEntireTerrain = Yes\n End\n")
            .unwrap();
        assert!(map.draw_entire_terrain);
        assert_eq!(peer, base);
        assert!(
            !map.overlay_ini_text("GameData\n DrawEntireTerrain = No\n End\n")
                .unwrap()
                .draw_entire_terrain
        );
    }
}
