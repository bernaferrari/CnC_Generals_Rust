//! GameData values consumed by ActiveBody's synchronous auto fire/smoke update.
use super::{GlobalData, INI, parse_game_data_block};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyAutoParticleRules {
    pub fire_small_prefix: String,
    pub fire_small_system: String,
    pub fire_small_max: i32,
    pub fire_medium_prefix: String,
    pub fire_medium_system: String,
    pub fire_medium_max: i32,
    pub fire_large_prefix: String,
    pub fire_large_system: String,
    pub fire_large_max: i32,
    pub smoke_small_prefix: String,
    pub smoke_small_system: String,
    pub smoke_small_max: i32,
    pub smoke_medium_prefix: String,
    pub smoke_medium_system: String,
    pub smoke_medium_max: i32,
    pub smoke_large_prefix: String,
    pub smoke_large_system: String,
    pub smoke_large_max: i32,
    pub aflame_prefix: String,
    pub aflame_system: String,
    pub aflame_max: i32,
}
impl Default for BodyAutoParticleRules {
    fn default() -> Self {
        Self::from_global_data(&GlobalData::new())
    }
}
impl BodyAutoParticleRules {
    pub fn from_global_data(data: &GlobalData) -> Self {
        Self {
            fire_small_prefix: data.auto_fire_particle_small_prefix.clone(),
            fire_small_system: data.auto_fire_particle_small_system.clone(),
            fire_small_max: data.auto_fire_particle_small_max,
            fire_medium_prefix: data.auto_fire_particle_medium_prefix.clone(),
            fire_medium_system: data.auto_fire_particle_medium_system.clone(),
            fire_medium_max: data.auto_fire_particle_medium_max,
            fire_large_prefix: data.auto_fire_particle_large_prefix.clone(),
            fire_large_system: data.auto_fire_particle_large_system.clone(),
            fire_large_max: data.auto_fire_particle_large_max,
            smoke_small_prefix: data.auto_smoke_particle_small_prefix.clone(),
            smoke_small_system: data.auto_smoke_particle_small_system.clone(),
            smoke_small_max: data.auto_smoke_particle_small_max,
            smoke_medium_prefix: data.auto_smoke_particle_medium_prefix.clone(),
            smoke_medium_system: data.auto_smoke_particle_medium_system.clone(),
            smoke_medium_max: data.auto_smoke_particle_medium_max,
            smoke_large_prefix: data.auto_smoke_particle_large_prefix.clone(),
            smoke_large_system: data.auto_smoke_particle_large_system.clone(),
            smoke_large_max: data.auto_smoke_particle_large_max,
            aflame_prefix: data.auto_aflame_particle_prefix.clone(),
            aflame_system: data.auto_aflame_particle_system.clone(),
            aflame_max: data.auto_aflame_particle_max,
        }
    }
    /// Apply ordered GameData blocks to a private draft, without publishing to Common GlobalData.
    pub fn overlay_ini_text(self, contents: &str) -> Result<Self, String> {
        let mut draft = GlobalData::new();
        draft.auto_fire_particle_small_prefix = self.fire_small_prefix;
        draft.auto_fire_particle_small_system = self.fire_small_system;
        draft.auto_fire_particle_small_max = self.fire_small_max;
        draft.auto_fire_particle_medium_prefix = self.fire_medium_prefix;
        draft.auto_fire_particle_medium_system = self.fire_medium_system;
        draft.auto_fire_particle_medium_max = self.fire_medium_max;
        draft.auto_fire_particle_large_prefix = self.fire_large_prefix;
        draft.auto_fire_particle_large_system = self.fire_large_system;
        draft.auto_fire_particle_large_max = self.fire_large_max;
        draft.auto_smoke_particle_small_prefix = self.smoke_small_prefix;
        draft.auto_smoke_particle_small_system = self.smoke_small_system;
        draft.auto_smoke_particle_small_max = self.smoke_small_max;
        draft.auto_smoke_particle_medium_prefix = self.smoke_medium_prefix;
        draft.auto_smoke_particle_medium_system = self.smoke_medium_system;
        draft.auto_smoke_particle_medium_max = self.smoke_medium_max;
        draft.auto_smoke_particle_large_prefix = self.smoke_large_prefix;
        draft.auto_smoke_particle_large_system = self.smoke_large_system;
        draft.auto_smoke_particle_large_max = self.smoke_large_max;
        draft.auto_aflame_particle_prefix = self.aflame_prefix;
        draft.auto_aflame_particle_system = self.aflame_system;
        draft.auto_aflame_particle_max = self.aflame_max;
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
        Ok(Self::from_global_data(&draft))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_and_solo_gamedata_overlays_are_ordered_and_owner_local() {
        let base = BodyAutoParticleRules::default()
            .overlay_ini_text("GameData\nAutoFireParticleSmallSystem = BaseFire\nAutoFireParticleSmallMax = 4\nEnd")
            .unwrap();
        let map = base.clone()
            .overlay_ini_text("GameData\nAutoFireParticleSmallSystem = MapFire\nEnd\nGameData\nAutoFireParticleSmallMax = 6\nEnd")
            .unwrap();
        let peer = base
            .overlay_ini_text("GameData\nAutoFireParticleSmallSystem = PeerFire\nEnd")
            .unwrap();
        assert_eq!(map.fire_small_system, "MapFire");
        assert_eq!(map.fire_small_max, 6);
        assert_eq!(peer.fire_small_system, "PeerFire");
        assert_eq!(peer.fire_small_max, 4);
    }
}
