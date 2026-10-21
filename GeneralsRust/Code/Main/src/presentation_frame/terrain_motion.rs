use super::*;

/// Read-only script/physics input to the receiving terrain renderer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresentationTerrainMotion {
    pub paused: bool,
    pub gravity: f32,
    pub breeze: Option<PresentationTreeBreeze>,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresentationTreeBreeze {
    pub version: i32,
    pub lean: f32,
    pub intensity: f32,
    pub direction: [f32; 2],
    pub randomness: f32,
    pub period: i32,
}
impl Default for PresentationTerrainMotion {
    fn default() -> Self {
        Self {
            paused: false,
            gravity: -1.0,
            breeze: None,
        }
    }
}
impl PresentationTerrainMotion {
    pub(crate) fn from_logic(logic: &GameLogic) -> Self {
        let engine = logic.owned_script_engine.as_ref();
        let breeze = engine.map(|engine| {
            let info = engine.get_breeze_info();
            PresentationTreeBreeze {
                version: i32::from(info.breeze_version),
                lean: info.lean,
                intensity: info.intensity,
                direction: info.direction_vec,
                randomness: info.randomness,
                period: i32::from(info.breeze_period.max(1)),
            }
        });
        Self {
            paused: logic.is_paused() || engine.is_some_and(|engine| engine.is_time_frozen()),
            gravity: logic.physics_definitions.effective.gravity,
            breeze,
        }
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn native(
        self,
    ) -> Result<game_client::terrain::terrain_visual::TerrainVisualMotion, String> {
        let info = self
            .breeze
            .ok_or("native terrain requires its receiving ScriptEngine observation")?;
        Ok(game_client::terrain::terrain_visual::TerrainVisualMotion {
            paused: self.paused,
            gravity: self.gravity,
            breeze: game_client::terrain::BreezeInfo {
                breeze_version: info.version,
                lean: info.lean,
                intensity: info.intensity,
                direction_vec: glam::Vec2::from(info.direction),
                randomness: info.randomness,
                breeze_period: info.period,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receiving_terrain_motion_observes_only_its_script_and_physics_owner() {
        let mut first = GameLogic::new();
        let mut second = GameLogic::new();
        let first_engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
        first_engine.set_breeze_info(0.25, 0.5, 0.75, 17, 0.125);
        first.install_owned_script_engine(first_engine);
        let mut second_engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
        second_engine.set_breeze_info(1.25, 2.5, 3.75, 29, 0.625);
        second_engine.set_time_frozen_debug(true);
        second.install_owned_script_engine(second_engine);
        first.physics_definitions.effective.gravity = -0.25;
        second.physics_definitions.effective.gravity = -2.0;
        let baseline = PresentationTerrainMotion::from_logic(&first);
        let foreign = PresentationTerrainMotion::from_logic(&second);
        assert!(!baseline.paused);
        assert!(foreign.paused);
        assert_eq!(baseline.gravity, -0.25);
        assert_eq!(foreign.gravity, -2.0);
        assert_eq!(baseline.breeze.unwrap().intensity, 0.5);
        assert_eq!(foreign.breeze.unwrap().intensity, 2.5);
        second
            .owned_script_engine
            .as_ref()
            .unwrap()
            .set_breeze_info(2.0, 9.0, 8.0, 41, 0.0);
        second.set_paused(true);
        drop(second);
        assert_eq!(PresentationTerrainMotion::from_logic(&first), baseline);
        first.set_paused(true);
        assert!(PresentationTerrainMotion::from_logic(&first).paused);
    }
    #[test]
    #[cfg(feature = "game_client")]
    fn absent_receiving_script_observation_is_not_an_ambient_default() {
        assert!(PresentationTerrainMotion::default().native().is_err());
    }
}
