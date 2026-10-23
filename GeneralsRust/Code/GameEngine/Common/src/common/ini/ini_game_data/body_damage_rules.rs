//! Immutable receiving GameData ActiveBody classification definitions.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyDamageRules {
    damaged: f32,
    really_damaged: f32,
}
impl Default for BodyDamageRules {
    fn default() -> Self {
        Self::from_global_data(&GlobalData::new())
    }
}
impl BodyDamageRules {
    fn from_global_data(data: &GlobalData) -> Self {
        Self {
            damaged: data.unit_damaged_thresh,
            really_damaged: data.unit_really_damaged_thresh,
        }
    }
    /// Explicit historical standalone/tool policy, not application content admission.
    pub const fn standalone_retail() -> Self {
        Self {
            damaged: 0.7,
            really_damaged: 0.35,
        }
    }
    pub const fn thresholds(self) -> (f32, f32) {
        (self.damaged, self.really_damaged)
    }
    /// Save wire values preserve authored ordering/range and exact f32 bits.
    pub fn from_thresholds(damaged: f32, really_damaged: f32) -> Result<Self, String> {
        if !damaged.is_finite() || !really_damaged.is_finite() {
            return Err("non-finite Body damage thresholds".into());
        }
        Ok(Self {
            damaged,
            really_damaged,
        })
    }
    /// Uses the original field table, with a private draft and no GlobalData publication.
    pub fn overlay_ini_text(self, text: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.unit_damaged_thresh = self.damaged;
        draft.unit_really_damaged_thresh = self.really_damaged;
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
                    parse_game_data_block(ini, &mut draft)?;
                }
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;
        Self::from_thresholds(draft.unit_damaged_thresh, draft.unit_really_damaged_thresh)
    }
}
