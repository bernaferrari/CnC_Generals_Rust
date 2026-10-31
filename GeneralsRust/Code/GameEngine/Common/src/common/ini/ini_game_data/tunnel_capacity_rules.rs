//! GameData's tunnel capacity, parsed without publishing process-global state.
use super::{GlobalData, INI};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TunnelCapacityRules {
    pub max_capacity: i32,
}

impl TunnelCapacityRules {
    /// Apply one ordered application or map source using the GameData parser.
    pub fn overlay_ini_text(self, text: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.max_tunnel_capacity = self.max_capacity;
        let mut ini = INI::new();
        ini.with_inline_source(text, |ini| {
            loop {
                ini.read_line()?;
                if ini.is_eof() {
                    break;
                }
                if ini
                    .get_next_token_or_null()
                    .is_some_and(|key| key.eq_ignore_ascii_case("GameData"))
                {
                    super::parse_game_data_block(ini, &mut draft)?;
                }
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;
        Ok(Self {
            max_capacity: draft.max_tunnel_capacity,
        })
    }
}
