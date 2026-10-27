//! One session's baseline/effective lighting. Parser drafts carry values only.
pub(crate) use game_engine::common::ini::ini_game_data::LightingContentRules;
use game_engine::common::ini::ini_game_data::{TIME_OF_DAY_FIRST, TerrainLighting, TimeOfDay};
#[derive(Debug, Clone, Default)]
pub(crate) struct LightingDefinitions {
    pub(crate) base: LightingContentRules,
    pub(crate) effective: LightingContentRules,
}
/// Exact LightingData payload after version expansion; no native side effects.
#[derive(Debug, Clone)]
pub struct MapLighting {
    pub time_of_day: TimeOfDay,
    pub terrain: [[[f32; 9]; 3]; 4],
    pub objects: [[[f32; 9]; 3]; 4],
}
fn authored(row: [f32; 9]) -> TerrainLighting {
    use game_engine::common::ini::ini_game_data::{Coord3D, RGBColor};
    TerrainLighting {
        ambient: RGBColor::new(row[0], row[1], row[2]),
        diffuse: RGBColor::new(row[3], row[4], row[5]),
        light_pos: Coord3D::new(row[6], row[7], row[8]),
    }
}
impl super::GameLogic {
    pub(crate) fn admit_lighting_rules_base(&mut self, rules: LightingContentRules) {
        self.lighting_definitions = LightingDefinitions {
            base: rules.clone(),
            effective: rules,
        };
    }
    pub(crate) fn lighting_rules_baseline(&self) -> &LightingContentRules {
        &self.lighting_definitions.base
    }
    pub(crate) fn lighting_rules(&self) -> &LightingContentRules {
        &self.lighting_definitions.effective
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn native_lighting_time_of_day(&self) -> Option<game_client::system::TimeOfDay> {
        use game_client::system::TimeOfDay as Native;
        Some(match self.lighting_rules().time_of_day {
            TimeOfDay::Morning => Native::Morning,
            TimeOfDay::Afternoon => Native::Afternoon,
            TimeOfDay::Evening => Native::Evening,
            TimeOfDay::Night => Native::Night,
            TimeOfDay::Invalid => return None,
        })
    }
    pub(crate) fn set_receiving_time_of_day(&mut self, time: TimeOfDay) {
        self.lighting_definitions.effective.time_of_day = time;
    }
    /// A rejected companion draft never partially mutates the receiving owner.
    pub(crate) fn admit_lighting_companion_sources(
        &mut self,
        texts: &[String],
    ) -> Result<(), String> {
        let mut rules = self.lighting_definitions.base.clone();
        for text in texts {
            rules = rules.overlay_ini_text(text)?;
            rules.water_definitions = rules
                .water_definitions
                .overlay_ini_text(text, game_engine::common::ini::INILoadType::CreateOverrides)?;
        }
        self.lighting_definitions.effective = rules;
        Ok(())
    }
    pub(crate) fn apply_map_lighting(&mut self, lighting: &MapLighting) {
        let rules = &mut self.lighting_definitions.effective;
        rules.time_of_day = lighting.time_of_day;
        for tod in 0..4 {
            for slot in 0..3 {
                rules.terrain[tod + TIME_OF_DAY_FIRST][slot] =
                    authored(lighting.terrain[tod][slot]);
                rules.objects[tod + TIME_OF_DAY_FIRST][slot] =
                    authored(lighting.objects[tod][slot]);
            }
        }
    }
}

/// Data-only payload for the shared snapshot coordinator's versioned LITE slot.
/// The application baseline remains admitted from content; this is current state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct LightingContinuationV1 {
    pub(crate) version: u32,
    time_of_day: u8,
    terrain: [[[f32; 9]; 3]; 5],
    objects: [[[f32; 9]; 3]; 5],
    num_global_lights: i32,
    infantry_light_scale: [f32; 5],
    infantry_override: f32,
    #[serde(default)]
    force_models_to_follow_time_of_day: Option<bool>,
    force_models_to_follow_weather: bool,
    use_draw_module_lod: bool,
    use_tree_sway: bool,
}
fn row(light: &TerrainLighting) -> [f32; 9] {
    [
        light.ambient.r,
        light.ambient.g,
        light.ambient.b,
        light.diffuse.r,
        light.diffuse.g,
        light.diffuse.b,
        light.light_pos.x,
        light.light_pos.y,
        light.light_pos.z,
    ]
}
impl super::GameLogic {
    pub(crate) fn capture_lighting_continuation(&self) -> LightingContinuationV1 {
        let rules = self.lighting_rules();
        LightingContinuationV1 {
            version: 1,
            time_of_day: rules.time_of_day as u8,
            terrain: std::array::from_fn(|tod| {
                std::array::from_fn(|i| row(&rules.terrain[tod][i]))
            }),
            objects: std::array::from_fn(|tod| {
                std::array::from_fn(|i| row(&rules.objects[tod][i]))
            }),
            num_global_lights: rules.num_global_lights,
            infantry_light_scale: rules.infantry_light_scale,
            infantry_override: rules.infantry_override,
            force_models_to_follow_time_of_day: Some(rules.force_models_to_follow_time_of_day),
            force_models_to_follow_weather: rules.force_models_to_follow_weather,
            use_draw_module_lod: rules.use_draw_module_lod,
            use_tree_sway: rules.use_tree_sway,
        }
    }
    pub(crate) fn validate_lighting_continuation(
        &self,
        saved: &LightingContinuationV1,
    ) -> Result<LightingContentRules, String> {
        if saved.version != 1 {
            return Err("unsupported receiving lighting continuation version".into());
        }
        let time = match saved.time_of_day {
            0 => TimeOfDay::Invalid,
            1 => TimeOfDay::Morning,
            2 => TimeOfDay::Afternoon,
            3 => TimeOfDay::Evening,
            4 => TimeOfDay::Night,
            _ => return Err("invalid receiving lighting time-of-day".into()),
        };
        if saved
            .terrain
            .iter()
            .chain(saved.objects.iter())
            .flatten()
            .flatten()
            .any(|f| !f.is_finite())
            || saved.infantry_light_scale.iter().any(|f| !f.is_finite())
            || !saved.infantry_override.is_finite()
        {
            return Err("non-finite receiving lighting continuation".into());
        }
        let mut rules = self.lighting_rules().clone();
        rules.time_of_day = time;
        rules.terrain =
            std::array::from_fn(|tod| std::array::from_fn(|i| authored(saved.terrain[tod][i])));
        rules.objects =
            std::array::from_fn(|tod| std::array::from_fn(|i| authored(saved.objects[tod][i])));
        rules.num_global_lights = saved.num_global_lights;
        rules.infantry_light_scale = saved.infantry_light_scale;
        rules.infantry_override = saved.infantry_override;
        rules.force_models_to_follow_time_of_day = saved
            .force_models_to_follow_time_of_day
            .unwrap_or(rules.force_models_to_follow_time_of_day);
        rules.force_models_to_follow_weather = saved.force_models_to_follow_weather;
        rules.use_draw_module_lod = saved.use_draw_module_lod;
        rules.use_tree_sway = saved.use_tree_sway;
        Ok(rules)
    }
    pub(crate) fn restore_lighting_continuation(
        &mut self,
        saved: &LightingContinuationV1,
    ) -> Result<(), String> {
        let rules = self.validate_lighting_continuation(saved)?;
        self.lighting_definitions.effective = rules;
        Ok(())
    }

    /// Commit rules that were fully validated while the WorldSnapshot was
    /// still a read-only candidate. This commit is infallible and belongs
    /// before restored Objects are reconstructed from the snapshot.
    pub(crate) fn install_validated_lighting_rules(&mut self, rules: LightingContentRules) {
        self.lighting_definitions.effective = rules;
    }
}
