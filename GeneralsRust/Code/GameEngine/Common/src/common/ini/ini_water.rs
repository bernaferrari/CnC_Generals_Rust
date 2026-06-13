////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_water.rs
//! Author: Colin Day, December 2001 (Converted to Rust)
//! Desc:   Water settings

use crate::common::ascii_string::AsciiString;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

const CPP_WATER_SETTING_FIELDS: &[&str] = &[
    "SkyTexture",
    "WaterTexture",
    "Vertex00Color",
    "Vertex10Color",
    "Vertex01Color",
    "Vertex11Color",
    "DiffuseColor",
    "TransparentDiffuseColor",
    "UScrollPerMS",
    "VScrollPerMS",
    "SkyTexelsPerUnit",
    "WaterRepeatCount",
];

const CPP_WATER_TRANSPARENCY_FIELDS: &[&str] = &[
    "TransparentWaterDepth",
    "TransparentWaterMinOpacity",
    "StandingWaterColor",
    "StandingWaterTexture",
    "AdditiveBlending",
    "RadarWaterColor",
    "SkyboxTextureN",
    "SkyboxTextureE",
    "SkyboxTextureS",
    "SkyboxTextureW",
    "SkyboxTextureT",
];

/// Result type for water parsing operations
pub type WaterResult<T> = Result<T, WaterError>;

/// Errors that can occur during water parsing
#[derive(Debug, Clone, PartialEq)]
pub enum WaterError {
    InvalidName,
    InvalidTimeOfDay,
    InvalidData,
    ParseError(String),
    NotFound,
    AlreadyExists,
}

impl std::fmt::Display for WaterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaterError::InvalidName => write!(f, "Invalid water setting name"),
            WaterError::InvalidTimeOfDay => write!(f, "Invalid time of day"),
            WaterError::InvalidData => write!(f, "Invalid water data"),
            WaterError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            WaterError::NotFound => write!(f, "Water setting not found"),
            WaterError::AlreadyExists => write!(f, "Water setting already exists"),
        }
    }
}

impl std::error::Error for WaterError {}

/// Time of day enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeOfDay {
    Morning,
    Noon,
    Afternoon,
    Evening,
    Night,
    Invalid,
}

impl TimeOfDay {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "morning" => Self::Morning,
            "noon" => Self::Noon,
            "afternoon" => Self::Afternoon,
            "evening" => Self::Evening,
            "night" => Self::Night,
            _ => Self::Invalid,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Morning => "Morning",
            Self::Noon => "Noon",
            Self::Afternoon => "Afternoon",
            Self::Evening => "Evening",
            Self::Night => "Night",
            Self::Invalid => "Invalid",
        }
    }

    pub fn as_index(&self) -> usize {
        match self {
            Self::Morning => 0,
            Self::Noon => 1,
            Self::Afternoon => 2,
            Self::Evening => 3,
            Self::Night => 4,
            Self::Invalid => usize::MAX,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Morning,
            1 => Self::Noon,
            2 => Self::Afternoon,
            3 => Self::Evening,
            4 => Self::Night,
            _ => Self::Invalid,
        }
    }
}

/// Time of day names for parsing
pub const TIME_OF_DAY_NAMES: &[&str] = &["Morning", "Noon", "Afternoon", "Evening", "Night"];

/// Water rendering settings for different times of day
#[derive(Debug, Clone)]
pub struct WaterSetting {
    pub time_of_day: TimeOfDay,
    pub sky_texture_name: AsciiString,
    pub surface_color: (f32, f32, f32, f32),    // RGBA
    pub depth_color: (f32, f32, f32, f32),      // RGBA
    pub reflection_color: (f32, f32, f32, f32), // RGBA
    pub vertex00_color: (f32, f32, f32, f32),
    pub vertex10_color: (f32, f32, f32, f32),
    pub vertex01_color: (f32, f32, f32, f32),
    pub vertex11_color: (f32, f32, f32, f32),
    pub transparent_diffuse_color: (f32, f32, f32, f32),
    pub u_scroll_per_ms: f32,
    pub v_scroll_per_ms: f32,
    pub sky_texels_per_unit: f32,
    pub water_repeat_count: i32,
    pub wave_amplitude: f32,
    pub wave_frequency: f32,
    pub wave_speed: f32,
    pub transparency: f32,
    pub reflection_intensity: f32,
    pub refraction_intensity: f32,
    pub foam_color: (f32, f32, f32, f32), // RGBA
    pub caustic_intensity: f32,
    pub normal_map_scale: f32,
    pub texture_name: AsciiString,
    pub normal_map_name: AsciiString,
    pub environment_map_name: AsciiString,
    pub properties: HashMap<String, String>,
}

impl WaterSetting {
    pub fn new(time_of_day: TimeOfDay) -> Self {
        Self {
            time_of_day,
            sky_texture_name: AsciiString::from(""),
            surface_color: (0.2, 0.4, 0.8, 0.8),
            depth_color: (0.0, 0.2, 0.4, 1.0),
            reflection_color: (0.8, 0.8, 1.0, 0.5),
            vertex00_color: (0.0, 0.0, 0.0, 0.0),
            vertex10_color: (0.0, 0.0, 0.0, 0.0),
            vertex01_color: (0.0, 0.0, 0.0, 0.0),
            vertex11_color: (0.0, 0.0, 0.0, 0.0),
            transparent_diffuse_color: (0.0, 0.0, 0.0, 0.0),
            u_scroll_per_ms: 0.0,
            v_scroll_per_ms: 0.0,
            sky_texels_per_unit: 0.0,
            water_repeat_count: 0,
            wave_amplitude: 0.5,
            wave_frequency: 2.0,
            wave_speed: 1.0,
            transparency: 0.7,
            reflection_intensity: 0.8,
            refraction_intensity: 0.3,
            foam_color: (1.0, 1.0, 1.0, 0.8),
            caustic_intensity: 0.5,
            normal_map_scale: 1.0,
            texture_name: AsciiString::from(""),
            normal_map_name: AsciiString::from(""),
            environment_map_name: AsciiString::from(""),
            properties: HashMap::new(),
        }
    }

    /// Get the field parse table for this water setting
    pub fn get_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        CPP_WATER_SETTING_FIELDS
            .iter()
            .map(|field| {
                (
                    *field,
                    parse_cpp_water_field_for_table
                        as fn(&str) -> Result<Box<dyn std::any::Any>, String>,
                )
            })
            .collect()
    }

    /// Update water setting from properties
    pub fn update_from_properties(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> WaterResult<()> {
        for (key, value) in properties {
            match key.as_str() {
                "SkyTexture" => {
                    self.sky_texture_name = AsciiString::from(value);
                }
                "WaterTexture" => {
                    self.texture_name = AsciiString::from(value);
                }
                "Vertex00Color" => {
                    self.vertex00_color = parse_color_rgba_for_water(key, value)?;
                }
                "Vertex10Color" => {
                    self.vertex10_color = parse_color_rgba_for_water(key, value)?;
                }
                "Vertex01Color" => {
                    self.vertex01_color = parse_color_rgba_for_water(key, value)?;
                }
                "Vertex11Color" => {
                    self.vertex11_color = parse_color_rgba_for_water(key, value)?;
                }
                "DiffuseColor" => {
                    self.surface_color = parse_color_rgba_for_water(key, value)?;
                }
                "TransparentDiffuseColor" => {
                    self.transparent_diffuse_color = parse_color_rgba_for_water(key, value)?;
                }
                "UScrollPerMS" => {
                    self.u_scroll_per_ms = parse_f32_field(key, value)?;
                }
                "VScrollPerMS" => {
                    self.v_scroll_per_ms = parse_f32_field(key, value)?;
                }
                "SkyTexelsPerUnit" => {
                    self.sky_texels_per_unit = parse_f32_field(key, value)?;
                }
                "WaterRepeatCount" => {
                    self.water_repeat_count = parse_i32_field(key, value)?;
                }
                _ => {
                    return Err(WaterError::ParseError(format!(
                        "Unknown water setting field '{}'",
                        key
                    )));
                }
            }
        }

        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        self.time_of_day != TimeOfDay::Invalid
    }
}

/// Water transparency setting
#[derive(Debug, Clone)]
pub struct WaterTransparencySetting {
    pub transparent_water_depth: f32,
    pub min_water_opacity: f32,
    pub standing_water_color: (f32, f32, f32),
    pub radar_water_color: (f32, f32, f32),
    pub additive_blending: bool,
    pub standing_water_texture: AsciiString,
    pub skybox_texture_n: AsciiString, // North
    pub skybox_texture_e: AsciiString, // East
    pub skybox_texture_s: AsciiString, // South
    pub skybox_texture_w: AsciiString, // West
    pub skybox_texture_t: AsciiString, // Top
    pub water_transparency: f32,
    pub is_override: bool,
    pub next_override: Option<Box<WaterTransparencySetting>>,
    pub properties: HashMap<String, String>,
}

impl WaterTransparencySetting {
    pub fn new() -> Self {
        Self {
            transparent_water_depth: 3.0,
            min_water_opacity: 1.0,
            standing_water_color: (1.0, 1.0, 1.0),
            radar_water_color: (140.0, 140.0, 255.0),
            additive_blending: false,
            standing_water_texture: AsciiString::from("TWWater01.tga"),
            skybox_texture_n: AsciiString::from("TSMorningN.tga"),
            skybox_texture_e: AsciiString::from("TSMorningE.tga"),
            skybox_texture_s: AsciiString::from("TSMorningS.tga"),
            skybox_texture_w: AsciiString::from("TSMorningW.tga"),
            skybox_texture_t: AsciiString::from("TSMorningT.tga"),
            water_transparency: 0.8,
            is_override: false,
            next_override: None,
            properties: HashMap::new(),
        }
    }

    /// Get the field parse table for this transparency setting
    pub fn get_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        CPP_WATER_TRANSPARENCY_FIELDS
            .iter()
            .map(|field| {
                (
                    *field,
                    parse_cpp_water_field_for_table
                        as fn(&str) -> Result<Box<dyn std::any::Any>, String>,
                )
            })
            .collect()
    }

    pub fn mark_as_override(&mut self) {
        self.is_override = true;
    }

    pub fn set_next_override(&mut self, override_setting: WaterTransparencySetting) {
        self.next_override = Some(Box::new(override_setting));
    }

    pub fn get_final_override(&self) -> &WaterTransparencySetting {
        if let Some(ref next) = self.next_override {
            next.get_final_override()
        } else {
            self
        }
    }

    pub fn get_final_override_mut(&mut self) -> &mut WaterTransparencySetting {
        if let Some(ref mut next) = self.next_override {
            next.get_final_override_mut()
        } else {
            self
        }
    }
}

impl Default for WaterTransparencySetting {
    fn default() -> Self {
        Self::new()
    }
}

/// Global water settings array (indexed by time of day)
static WATER_SETTINGS: OnceCell<[Arc<RwLock<WaterSetting>>; 5]> = OnceCell::new();
static WATER_TRANSPARENCY: OnceCell<Arc<RwLock<WaterTransparencySetting>>> = OnceCell::new();

/// Initialize water settings for all times of day
pub fn initialize_water_settings() {
    WATER_SETTINGS.get_or_init(|| {
        [
            Arc::new(RwLock::new(WaterSetting::new(TimeOfDay::from_index(0)))),
            Arc::new(RwLock::new(WaterSetting::new(TimeOfDay::from_index(1)))),
            Arc::new(RwLock::new(WaterSetting::new(TimeOfDay::from_index(2)))),
            Arc::new(RwLock::new(WaterSetting::new(TimeOfDay::from_index(3)))),
            Arc::new(RwLock::new(WaterSetting::new(TimeOfDay::from_index(4)))),
        ]
    });

    WATER_TRANSPARENCY.get_or_init(|| Arc::new(RwLock::new(WaterTransparencySetting::new())));
}

/// Get water setting for a specific time of day
pub fn get_water_setting(time_of_day: TimeOfDay) -> Option<Arc<RwLock<WaterSetting>>> {
    if time_of_day == TimeOfDay::Invalid {
        return None;
    }

    let settings = WATER_SETTINGS.get()?;
    Some(Arc::clone(&settings[time_of_day.as_index()]))
}

/// Get the water transparency setting
pub fn get_water_transparency() -> Option<Arc<RwLock<WaterTransparencySetting>>> {
    WATER_TRANSPARENCY.get().cloned()
}

/// Clear any map-generated water transparency overrides.
///
/// Matches C++ GameLogic::reset() lines 465-466:
///   WaterTransparencySetting *wt = TheWaterTransparency.getNonOverloadedPointer();
///   TheWaterTransparency = wt->deleteOverrides();
pub fn clear_water_transparency_overrides() {
    if let Some(transparency) = WATER_TRANSPARENCY.get() {
        if let Ok(mut guard) = transparency.write() {
            guard.next_override = None;
        }
    }
}

/// Parse RGBA color from string (format: R G B A or R,G,B,A)
pub fn parse_color_rgba(value: &str) -> Result<(f32, f32, f32, f32), String> {
    let parts: Vec<&str> = if value.contains(',') {
        value.split(',').collect()
    } else {
        value.split_whitespace().collect()
    };

    match parts.len() {
        3 => {
            let r = parts[0]
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("Invalid red component: {}", parts[0]))?;
            let g = parts[1]
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("Invalid green component: {}", parts[1]))?;
            let b = parts[2]
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("Invalid blue component: {}", parts[2]))?;
            Ok((r, g, b, 1.0))
        }
        4 => {
            let r = parts[0]
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("Invalid red component: {}", parts[0]))?;
            let g = parts[1]
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("Invalid green component: {}", parts[1]))?;
            let b = parts[2]
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("Invalid blue component: {}", parts[2]))?;
            let a = parts[3]
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("Invalid alpha component: {}", parts[3]))?;
            Ok((r, g, b, a))
        }
        _ => Err(format!("Invalid color format: {}", value)),
    }
}

pub fn parse_color_rgb(value: &str) -> Result<(f32, f32, f32), String> {
    let (r, g, b, _) = parse_color_rgba(value)?;
    Ok((r, g, b))
}

fn parse_cpp_water_field_for_table(value: &str) -> Result<Box<dyn std::any::Any>, String> {
    Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
}

fn parse_color_rgba_for_water(field_name: &str, value: &str) -> WaterResult<(f32, f32, f32, f32)> {
    parse_color_rgba(value).map_err(|e| {
        WaterError::ParseError(format!("Invalid {} color '{}': {}", field_name, value, e))
    })
}

fn parse_color_rgb_for_water(field_name: &str, value: &str) -> WaterResult<(f32, f32, f32)> {
    parse_color_rgb(value).map_err(|e| {
        WaterError::ParseError(format!("Invalid {} color '{}': {}", field_name, value, e))
    })
}

fn parse_f32_field(field_name: &str, value: &str) -> WaterResult<f32> {
    value.parse::<f32>().map_err(|e| {
        WaterError::ParseError(format!("Invalid {} value '{}': {}", field_name, value, e))
    })
}

fn parse_i32_field(field_name: &str, value: &str) -> WaterResult<i32> {
    value.parse::<i32>().map_err(|e| {
        WaterError::ParseError(format!("Invalid {} value '{}': {}", field_name, value, e))
    })
}

fn parse_bool_field(field_name: &str, value: &str) -> WaterResult<bool> {
    parse_bool(value).map_err(|e| {
        WaterError::ParseError(format!("Invalid {} value '{}': {}", field_name, value, e))
    })
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Ok(true),
        "no" | "false" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

/// Compare strings ignoring case
pub fn stricmp(a: &str, b: &str) -> i32 {
    a.to_lowercase().cmp(&b.to_lowercase()) as i32
}

/// INI parsing functions for water
pub struct IniWater;

impl IniWater {
    /// Parse water setting definition - equivalent to INI::parseWaterSettingDefinition
    pub fn parse_water_setting_definition(name: AsciiString) -> WaterResult<()> {
        // Validate name
        if name.is_empty() {
            return Err(WaterError::InvalidName);
        }

        // Initialize water settings if needed
        initialize_water_settings();

        // Find the time of day that matches the name
        let mut time_of_day_index = TimeOfDay::Invalid;
        for (index, &time_name) in TIME_OF_DAY_NAMES.iter().enumerate() {
            if stricmp(time_name, name.as_str()) == 0 {
                time_of_day_index = TimeOfDay::from_index(index);
                break;
            }
        }

        // Check for no time of day match
        if time_of_day_index == TimeOfDay::Invalid {
            return Err(WaterError::InvalidTimeOfDay);
        }

        // Get the water setting to load based on time of day
        if let Some(water_setting_lock) = get_water_setting(time_of_day_index) {
            let _guard = water_setting_lock.write().expect("Water setting poisoned");
            // In the original C++, this would call:
            // ini->initFromINI(waterSetting, waterSetting->getFieldParse());
            println!("Parsing water setting definition for: {}", name.as_str());
        } else {
            return Err(WaterError::NotFound);
        }

        Ok(())
    }

    /// Parse water transparency definition - equivalent to INI::parseWaterTransparencyDefinition
    pub fn parse_water_transparency_definition(load_type: IniLoadType) -> WaterResult<()> {
        initialize_water_settings();

        let transparency_lock = get_water_transparency().ok_or(WaterError::NotFound)?;

        match load_type {
            IniLoadType::Overwrite => {
                let _guard = transparency_lock
                    .write()
                    .expect("Water transparency poisoned");
                // Just update the existing setting
            }
            IniLoadType::CreateOverrides => {
                let mut transparency = transparency_lock
                    .write()
                    .expect("Water transparency poisoned");
                // Create override (simplified version)
                let mut override_setting = WaterTransparencySetting::new();
                override_setting.mark_as_override();

                // In the real implementation, this would properly handle the override chain
                println!("Creating water transparency override");
                transparency.set_next_override(override_setting);
            }
            _ => {
                return Err(WaterError::InvalidData);
            }
        }

        // In the original C++, this would call:
        // ini->initFromINI(waterTrans, TheWaterTransparency->getFieldParse());
        println!("Parsing water transparency definition");

        // Handle skybox texture replacement logic would go here
        // This is complex and involves the terrain visual system

        Ok(())
    }

    /// Parse a complete water setting block from INI data
    pub fn parse_water_setting_block(
        name: AsciiString,
        properties: HashMap<String, String>,
    ) -> WaterResult<WaterSetting> {
        // Find time of day
        let mut time_of_day = TimeOfDay::Invalid;
        for (index, &time_name) in TIME_OF_DAY_NAMES.iter().enumerate() {
            if stricmp(time_name, name.as_str()) == 0 {
                time_of_day = TimeOfDay::from_index(index);
                break;
            }
        }

        if time_of_day == TimeOfDay::Invalid {
            return Err(WaterError::InvalidTimeOfDay);
        }

        // Create water setting
        let mut water_setting = WaterSetting::new(time_of_day);

        // Update setting from properties
        water_setting.update_from_properties(&properties)?;

        Ok(water_setting)
    }

    /// Parse a water transparency block from INI data
    pub fn parse_water_transparency_block(
        properties: HashMap<String, String>,
    ) -> WaterResult<WaterTransparencySetting> {
        let mut transparency_setting = WaterTransparencySetting::new();

        // Update setting from properties
        for (key, value) in properties {
            match key.as_str() {
                "TransparentWaterDepth" => {
                    transparency_setting.transparent_water_depth = parse_f32_field(&key, &value)?;
                }
                "TransparentWaterMinOpacity" => {
                    transparency_setting.min_water_opacity = parse_f32_field(&key, &value)?;
                }
                "StandingWaterColor" => {
                    transparency_setting.standing_water_color =
                        parse_color_rgb_for_water(&key, &value)?;
                }
                "StandingWaterTexture" => {
                    transparency_setting.standing_water_texture = AsciiString::from(&value);
                }
                "AdditiveBlending" => {
                    transparency_setting.additive_blending = parse_bool_field(&key, &value)?;
                }
                "RadarWaterColor" => {
                    transparency_setting.radar_water_color =
                        parse_color_rgb_for_water(&key, &value)?;
                }
                "SkyboxTextureN" => {
                    transparency_setting.skybox_texture_n = AsciiString::from(&value);
                }
                "SkyboxTextureE" => {
                    transparency_setting.skybox_texture_e = AsciiString::from(&value);
                }
                "SkyboxTextureS" => {
                    transparency_setting.skybox_texture_s = AsciiString::from(&value);
                }
                "SkyboxTextureW" => {
                    transparency_setting.skybox_texture_w = AsciiString::from(&value);
                }
                "SkyboxTextureT" => {
                    transparency_setting.skybox_texture_t = AsciiString::from(&value);
                }
                _ => {
                    return Err(WaterError::ParseError(format!(
                        "Unknown water transparency field '{}'",
                        key
                    )));
                }
            }
        }

        Ok(transparency_setting)
    }

    /// Validate water setting name format
    pub fn validate_name(name: &AsciiString) -> bool {
        !name.is_empty()
            && TIME_OF_DAY_NAMES
                .iter()
                .any(|&time_name| stricmp(time_name, name.as_str()) == 0)
    }

    /// Get all available time of day names
    pub fn get_time_of_day_names() -> Vec<&'static str> {
        TIME_OF_DAY_NAMES.to_vec()
    }
}

/// Load types for INI parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IniLoadType {
    Overwrite,
    CreateOverrides,
    Multifile,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_of_day_parsing() {
        assert_eq!(TimeOfDay::from_string("morning"), TimeOfDay::Morning);
        assert_eq!(TimeOfDay::from_string("NOON"), TimeOfDay::Noon);
        assert_eq!(TimeOfDay::from_string("invalid"), TimeOfDay::Invalid);
    }

    #[test]
    fn test_time_of_day_indexing() {
        assert_eq!(TimeOfDay::Morning.as_index(), 0);
        assert_eq!(TimeOfDay::Night.as_index(), 4);
        assert_eq!(TimeOfDay::from_index(2), TimeOfDay::Afternoon);
        assert_eq!(TimeOfDay::from_index(999), TimeOfDay::Invalid);
    }

    #[test]
    fn test_water_setting_creation() {
        let setting = WaterSetting::new(TimeOfDay::Morning);

        assert_eq!(setting.time_of_day, TimeOfDay::Morning);
        assert_eq!(setting.wave_amplitude, 0.5);
        assert_eq!(setting.transparency, 0.7);
        assert!(setting.is_valid());
    }

    #[test]
    fn test_water_transparency_setting() {
        let mut transparency = WaterTransparencySetting::new();
        transparency.skybox_texture_n = AsciiString::from("skybox_n.tga");
        transparency.water_transparency = 0.9;

        assert_eq!(transparency.skybox_texture_n.as_str(), "skybox_n.tga");
        assert_eq!(transparency.water_transparency, 0.9);
        assert!(!transparency.is_override);
        assert_eq!(transparency.transparent_water_depth, 3.0);
        assert_eq!(transparency.min_water_opacity, 1.0);
        assert_eq!(transparency.standing_water_color, (1.0, 1.0, 1.0));
        assert_eq!(transparency.radar_water_color, (140.0, 140.0, 255.0));
        assert_eq!(
            transparency.standing_water_texture.as_str(),
            "TWWater01.tga"
        );
        assert_eq!(transparency.skybox_texture_t.as_str(), "TSMorningT.tga");
        assert!(!transparency.additive_blending);
    }

    #[test]
    fn test_water_properties_update() {
        let mut setting = WaterSetting::new(TimeOfDay::Evening);
        let mut properties = HashMap::new();
        properties.insert("WaterTexture".to_string(), "WaterEvening.tga".to_string());
        properties.insert("DiffuseColor".to_string(), "0.1 0.3 0.7 0.8".to_string());
        properties.insert("WaterRepeatCount".to_string(), "12".to_string());

        setting.update_from_properties(&properties).unwrap();

        assert_eq!(setting.texture_name.as_str(), "WaterEvening.tga");
        assert_eq!(setting.surface_color, (0.1, 0.3, 0.7, 0.8));
        assert_eq!(setting.water_repeat_count, 12);
    }

    #[test]
    fn water_setting_accepts_cpp_field_table_fields() {
        let mut properties = HashMap::new();
        properties.insert("SkyTexture".to_string(), "SkyMorning.tga".to_string());
        properties.insert("WaterTexture".to_string(), "Water01.tga".to_string());
        properties.insert("Vertex00Color".to_string(), "1 2 3 4".to_string());
        properties.insert("Vertex10Color".to_string(), "5 6 7 8".to_string());
        properties.insert("Vertex01Color".to_string(), "9 10 11 12".to_string());
        properties.insert("Vertex11Color".to_string(), "13 14 15 16".to_string());
        properties.insert("DiffuseColor".to_string(), "17 18 19 20".to_string());
        properties.insert(
            "TransparentDiffuseColor".to_string(),
            "21 22 23 24".to_string(),
        );
        properties.insert("UScrollPerMS".to_string(), "0.001".to_string());
        properties.insert("VScrollPerMS".to_string(), "0.002".to_string());
        properties.insert("SkyTexelsPerUnit".to_string(), "0.5".to_string());
        properties.insert("WaterRepeatCount".to_string(), "8".to_string());

        let setting =
            IniWater::parse_water_setting_block(AsciiString::from("Morning"), properties).unwrap();

        assert_eq!(setting.sky_texture_name.as_str(), "SkyMorning.tga");
        assert_eq!(setting.texture_name.as_str(), "Water01.tga");
        assert_eq!(setting.vertex00_color, (1.0, 2.0, 3.0, 4.0));
        assert_eq!(setting.vertex10_color, (5.0, 6.0, 7.0, 8.0));
        assert_eq!(setting.vertex01_color, (9.0, 10.0, 11.0, 12.0));
        assert_eq!(setting.vertex11_color, (13.0, 14.0, 15.0, 16.0));
        assert_eq!(setting.surface_color, (17.0, 18.0, 19.0, 20.0));
        assert_eq!(setting.transparent_diffuse_color, (21.0, 22.0, 23.0, 24.0));
        assert_eq!(setting.u_scroll_per_ms, 0.001);
        assert_eq!(setting.v_scroll_per_ms, 0.002);
        assert_eq!(setting.sky_texels_per_unit, 0.5);
        assert_eq!(setting.water_repeat_count, 8);
        assert!(setting.properties.is_empty());
    }

    #[test]
    fn water_setting_rejects_fields_outside_cpp_parse_table() {
        for field in [
            "SurfaceColor",
            "Texture",
            "DepthColor",
            "ReflectionColor",
            "WaveAmplitude",
            "WaveFrequency",
            "WaveSpeed",
            "Transparency",
            "NormalMap",
            "EnvironmentMap",
            "UnknownField",
        ] {
            let mut properties = HashMap::new();
            properties.insert(field.to_string(), "1".to_string());
            assert!(
                IniWater::parse_water_setting_block(AsciiString::from("Morning"), properties)
                    .is_err(),
                "{} should be rejected because C++ WaterSetting does not parse it",
                field
            );
        }
    }

    #[test]
    fn water_setting_rejects_malformed_cpp_field_values() {
        let mut properties = HashMap::new();
        properties.insert("DiffuseColor".to_string(), "not-a-color".to_string());
        assert!(
            IniWater::parse_water_setting_block(AsciiString::from("Morning"), properties).is_err()
        );

        let mut properties = HashMap::new();
        properties.insert("UScrollPerMS".to_string(), "fast".to_string());
        assert!(
            IniWater::parse_water_setting_block(AsciiString::from("Morning"), properties).is_err()
        );

        let mut properties = HashMap::new();
        properties.insert("WaterRepeatCount".to_string(), "many".to_string());
        assert!(
            IniWater::parse_water_setting_block(AsciiString::from("Morning"), properties).is_err()
        );
    }

    #[test]
    fn water_transparency_accepts_cpp_field_table_fields() {
        let mut properties = HashMap::new();
        properties.insert("TransparentWaterDepth".to_string(), "9.5".to_string());
        properties.insert("TransparentWaterMinOpacity".to_string(), "0.35".to_string());
        properties.insert("StandingWaterColor".to_string(), "0.2 0.3 0.4".to_string());
        properties.insert(
            "StandingWaterTexture".to_string(),
            "Standing.tga".to_string(),
        );
        properties.insert("AdditiveBlending".to_string(), "Yes".to_string());
        properties.insert("RadarWaterColor".to_string(), "10 20 30".to_string());
        properties.insert("SkyboxTextureN".to_string(), "North.tga".to_string());
        properties.insert("SkyboxTextureE".to_string(), "East.tga".to_string());
        properties.insert("SkyboxTextureS".to_string(), "South.tga".to_string());
        properties.insert("SkyboxTextureW".to_string(), "West.tga".to_string());
        properties.insert("SkyboxTextureT".to_string(), "Top.tga".to_string());

        let setting = IniWater::parse_water_transparency_block(properties).unwrap();

        assert_eq!(setting.transparent_water_depth, 9.5);
        assert_eq!(setting.min_water_opacity, 0.35);
        assert_eq!(setting.standing_water_color, (0.2, 0.3, 0.4));
        assert_eq!(setting.standing_water_texture.as_str(), "Standing.tga");
        assert!(setting.additive_blending);
        assert_eq!(setting.radar_water_color, (10.0, 20.0, 30.0));
        assert_eq!(setting.skybox_texture_n.as_str(), "North.tga");
        assert_eq!(setting.skybox_texture_e.as_str(), "East.tga");
        assert_eq!(setting.skybox_texture_s.as_str(), "South.tga");
        assert_eq!(setting.skybox_texture_w.as_str(), "West.tga");
        assert_eq!(setting.skybox_texture_t.as_str(), "Top.tga");
        assert!(setting.properties.is_empty());
    }

    #[test]
    fn water_transparency_rejects_fields_outside_cpp_parse_table() {
        for field in ["WaterTransparency", "UnknownField"] {
            let mut properties = HashMap::new();
            properties.insert(field.to_string(), "1".to_string());
            assert!(
                IniWater::parse_water_transparency_block(properties).is_err(),
                "{} should be rejected because C++ WaterTransparencySetting does not parse it",
                field
            );
        }
    }

    #[test]
    fn water_transparency_rejects_malformed_cpp_field_values() {
        let mut properties = HashMap::new();
        properties.insert("TransparentWaterDepth".to_string(), "deep".to_string());
        assert!(IniWater::parse_water_transparency_block(properties).is_err());

        let mut properties = HashMap::new();
        properties.insert("StandingWaterColor".to_string(), "blue".to_string());
        assert!(IniWater::parse_water_transparency_block(properties).is_err());

        let mut properties = HashMap::new();
        properties.insert("AdditiveBlending".to_string(), "maybe".to_string());
        assert!(IniWater::parse_water_transparency_block(properties).is_err());
    }

    #[test]
    fn test_parse_color_rgba() {
        assert_eq!(
            parse_color_rgba("1.0 0.5 0.0 0.8"),
            Ok((1.0, 0.5, 0.0, 0.8))
        );
        assert_eq!(parse_color_rgba("1.0,0.5,0.0"), Ok((1.0, 0.5, 0.0, 1.0)));
        assert_eq!(
            parse_color_rgba("255 128 64 128"),
            Ok((255.0, 128.0, 64.0, 128.0))
        );

        assert!(parse_color_rgba("1.0").is_err());
        assert!(parse_color_rgba("1.0 0.5").is_err());
        assert!(parse_color_rgba("invalid").is_err());
    }

    #[test]
    fn test_stricmp() {
        assert_eq!(stricmp("Morning", "morning"), 0);
        assert_eq!(stricmp("NOON", "noon"), 0);
        assert!(stricmp("afternoon", "evening") != 0);
    }

    #[test]
    fn test_validate_name() {
        assert!(IniWater::validate_name(&AsciiString::from("Morning")));
        assert!(IniWater::validate_name(&AsciiString::from("Noon")));
        assert!(IniWater::validate_name(&AsciiString::from("Night")));
        assert!(!IniWater::validate_name(&AsciiString::from("Invalid")));
        assert!(!IniWater::validate_name(&AsciiString::from("")));
    }

    #[test]
    fn test_override_chain() {
        let mut base = WaterTransparencySetting::new();
        base.water_transparency = 0.5;

        let mut override_setting = WaterTransparencySetting::new();
        override_setting.water_transparency = 0.8;
        override_setting.mark_as_override();

        base.set_next_override(override_setting);

        assert!(base.next_override.is_some());
        assert!(base.get_final_override().is_override);
        assert_eq!(base.get_final_override().water_transparency, 0.8);
    }

    #[test]
    fn test_time_of_day_names() {
        let names = IniWater::get_time_of_day_names();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"Morning"));
        assert!(names.contains(&"Night"));
    }
}
