//! The GameData values consumed by the W3D power-meter callbacks.
use super::{GlobalData, INI, parse_game_data_block};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PowerBarRules {
    pub base: i32,
    pub intervals: f32,
    pub yellow_range: i32,
}

impl Default for PowerBarRules {
    fn default() -> Self {
        let data = GlobalData::new();
        Self {
            base: data.power_bar_base,
            intervals: data.power_bar_intervals,
            yellow_range: data.power_bar_yellow_range,
        }
    }
}

impl PowerBarRules {
    /// Standalone Core clients can still use their own GlobalData adapter.
    pub fn from_global_data(data: &GlobalData) -> Self {
        Self {
            base: data.power_bar_base,
            intervals: data.power_bar_intervals,
            yellow_range: data.power_bar_yellow_range,
        }
    }

    /// Apply ordered GameData, Map.ini, and Solo.ini blocks without publishing
    /// the parser's temporary GlobalData to any other session.
    pub fn overlay_ini_text(self, contents: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.power_bar_base = self.base;
        draft.power_bar_intervals = self.intervals;
        draft.power_bar_yellow_range = self.yellow_range;
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
        Ok(Self {
            base: draft.power_bar_base,
            intervals: draft.power_bar_intervals,
            yellow_range: draft.power_bar_yellow_range,
        })
    }
}
