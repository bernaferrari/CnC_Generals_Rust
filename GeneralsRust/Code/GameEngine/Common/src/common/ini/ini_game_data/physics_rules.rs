//! CPP GlobalData.cpp808–810 and GameData field-table immutable Physics projection.
use super::{GlobalData, INI, parse_game_data_block, runtime_global_data};

/// Immutable effective Physics definitions from the receiving GameData content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsContentRules {
    pub gravity: f32,
    pub ground_stiffness: f32,
    pub structure_stiffness: f32,
}
impl PhysicsContentRules {
    pub fn from_global_data(data: &runtime_global_data::GlobalData) -> Self {
        Self {
            gravity: data.gravity,
            ground_stiffness: data.ground_stiffness,
            structure_stiffness: data.structure_stiffness,
        }
    }
    pub fn validate(self) -> Result<Self, String> {
        if !self.gravity.is_finite()
            || !self.ground_stiffness.is_finite()
            || !self.structure_stiffness.is_finite()
        {
            return Err("nonfinite Physics content rules".into());
        }
        Ok(self)
    }
    /// Reuse the original field table and acceleration conversion privately.
    pub fn overlay_ini_text(self, contents: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.gravity = self.gravity;
        draft.ground_stiffness = self.ground_stiffness;
        draft.structure_stiffness = self.structure_stiffness;
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
            gravity: draft.gravity,
            ground_stiffness: draft.ground_stiffness,
            structure_stiffness: draft.structure_stiffness,
        }
        .validate()
    }
}
