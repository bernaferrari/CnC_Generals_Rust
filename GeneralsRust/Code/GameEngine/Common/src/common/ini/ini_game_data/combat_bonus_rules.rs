//! GameData combat bonuses owned by one session and its map overrides.
use super::{DIFFICULTY_COUNT, GlobalData, INI, LEVEL_COUNT, PLAYERTYPE_COUNT, WeaponBonusEntry};

#[derive(Debug, Clone)]
pub struct CombatBonusRules {
    pub weapon_entries: Vec<WeaponBonusEntry>,
    pub solo_health: [[f32; DIFFICULTY_COUNT]; PLAYERTYPE_COUNT],
    pub veterancy_health: [f32; LEVEL_COUNT],
}

impl Default for CombatBonusRules {
    fn default() -> Self {
        let defaults = GlobalData::new();
        Self {
            weapon_entries: defaults.weapon_bonus_entries,
            solo_health: defaults.solo_player_health_bonus_for_difficulty,
            veterancy_health: defaults.health_bonus,
        }
    }
}

impl CombatBonusRules {
    /// Use the same GameData parser and ordering as application admission,
    /// writing into a private draft that cannot publish another world's data.
    pub fn overlay_ini_text(self, text: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.weapon_bonus_entries = self.weapon_entries;
        draft.solo_player_health_bonus_for_difficulty = self.solo_health;
        draft.health_bonus = self.veterancy_health;
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
            weapon_entries: draft.weapon_bonus_entries,
            solo_health: draft.solo_player_health_bonus_for_difficulty,
            veterancy_health: draft.health_bonus,
        })
    }
}
