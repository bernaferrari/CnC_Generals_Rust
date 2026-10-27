//! Water definitions owned by the application/session receiving an INI load.
use super::{
    TIME_OF_DAY_COUNT, TIME_OF_DAY_NAMES, TimeOfDay, WaterError, WaterResult, WaterSetting,
    WaterTransparencySetting, apply_transparency_properties, skybox_names,
};
use crate::common::ini::ini::{INI, INILoadType};
use std::collections::HashMap;

/// Original WaterSettings and WaterTransparency override chain for one world.
#[derive(Debug, Clone)]
pub struct WaterDefinitions {
    settings: [WaterSetting; TIME_OF_DAY_COUNT],
    transparency: WaterTransparencySetting,
    transparency_defined: bool,
}

impl Default for WaterDefinitions {
    fn default() -> Self {
        Self {
            settings: std::array::from_fn(|index| WaterSetting::new(TimeOfDay::from_index(index))),
            transparency: WaterTransparencySetting::new(),
            transparency_defined: false,
        }
    }
}

impl WaterDefinitions {
    /// Parse a detached draft, retaining the previous owner if parsing fails.
    pub fn overlay_ini_text(&self, text: &str, load_type: INILoadType) -> Result<Self, String> {
        let mut draft = self.clone();
        let mut ini = INI::new();
        ini.with_inline_source_and_load_type(text, load_type, |ini| {
            ini.parse_water_definitions_into(&mut draft)
        })
        .map_err(|error| error.to_string())?;
        Ok(draft)
    }

    pub fn setting(&self, time: TimeOfDay) -> &WaterSetting {
        &self.settings[time.as_index()]
    }

    pub fn transparency(&self) -> &WaterTransparencySetting {
        self.transparency.get_final_override()
    }

    pub fn presentation(&self) -> WaterPresentation {
        let transparency = self.transparency();
        WaterPresentation {
            settings: std::array::from_fn(|index| {
                let setting = &self.settings[index];
                WaterPresentationSetting {
                    surface_color: setting.surface_color,
                    transparent_diffuse_color: setting.transparent_diffuse_color,
                    water_repeat_count: setting.water_repeat_count,
                    texture_name: setting.texture_name.to_string(),
                }
            }),
            transparent_water_depth: transparency.transparent_water_depth,
            min_water_opacity: transparency.min_water_opacity,
            standing_water_color: transparency.standing_water_color,
            radar_water_color: transparency.radar_water_color,
            additive_blending: transparency.additive_blending,
            standing_water_texture: transparency.standing_water_texture.to_string(),
            original_skybox_textures: skybox_names(&self.transparency),
            skybox_textures: skybox_names(transparency),
        }
    }

    pub(crate) fn apply_setting(
        &mut self,
        name: &str,
        properties: HashMap<String, String>,
    ) -> WaterResult<()> {
        let index = TIME_OF_DAY_NAMES
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(name))
            .ok_or(WaterError::InvalidTimeOfDay)?;
        // INIWater.cpp updates the existing indexed record, including NONE.
        let mut updated = self.settings[index].clone();
        updated.update_from_properties(&properties)?;
        self.settings[index] = updated;
        Ok(())
    }

    pub(crate) fn apply_transparency(
        &mut self,
        load_type: INILoadType,
        properties: HashMap<String, String>,
    ) -> WaterResult<()> {
        if !self.transparency_defined {
            let mut updated = self.transparency.clone();
            apply_transparency_properties(&mut updated, &properties)?;
            self.transparency = updated;
            self.transparency_defined = true;
            return Ok(());
        }
        if load_type != INILoadType::CreateOverrides {
            return Err(WaterError::AlreadyExists);
        }
        // Each C++ override starts from the non-overloaded base, then joins
        // the final override; it is not a copy of the previous override.
        let mut next = self.transparency.clone();
        next.next_override = None;
        next.is_override = false;
        apply_transparency_properties(&mut next, &properties)?;
        next.mark_as_override();
        self.transparency
            .get_final_override_mut()
            .set_next_override(next);
        Ok(())
    }
}

/// Immutable values delivered to this world's terrain and radar presentation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WaterPresentation {
    pub settings: [WaterPresentationSetting; TIME_OF_DAY_COUNT],
    pub transparent_water_depth: f32,
    pub min_water_opacity: f32,
    pub standing_water_color: (f32, f32, f32),
    pub radar_water_color: (f32, f32, f32),
    pub additive_blending: bool,
    pub standing_water_texture: String,
    pub original_skybox_textures: [String; 5],
    pub skybox_textures: [String; 5],
}

impl Default for WaterPresentation {
    fn default() -> Self {
        WaterDefinitions::default().presentation()
    }
}

impl WaterPresentation {
    pub fn setting_at(&self, time_of_day: u8) -> &WaterPresentationSetting {
        self.settings
            .get(time_of_day as usize)
            .unwrap_or(&self.settings[0])
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WaterPresentationSetting {
    pub surface_color: (f32, f32, f32, f32),
    pub transparent_diffuse_color: (f32, f32, f32, f32),
    pub water_repeat_count: i32,
    pub texture_name: String,
}
