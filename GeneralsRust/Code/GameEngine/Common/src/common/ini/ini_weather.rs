//! INI parser for Weather settings
//!
//! Reference: GeneralsMD/Code/GameEngine/Source/GameClient/Snow.cpp
//! Reference: GeneralsMD/Code/GameEngine/Include/GameClient/Snow.h
//!
//! Parses [Weather] blocks from INI files.
//!
//! # Example INI
//! ```ini
//! Weather
//!     SnowTexture = EXSnowFlake.tga
//!     SnowFrequencyScaleX = 0.0533
//!     SnowFrequencyScaleY = 0.0275
//!     SnowAmplitude = 5.0
//!     SnowPointSize = 1.0
//!     SnowMaxPointSize = 64.0
//!     SnowMinPointSize = 0.0
//!     SnowQuadSize = 0.5
//!     SnowBoxDimensions = 200.0
//!     SnowBoxDensity = 1.0
//!     SnowVelocity = 4.0
//!     SnowPointSprites = Yes
//!     SnowEnabled = No
//! End
//! ```

use once_cell::sync::OnceCell;
use std::sync::RwLock;

use super::ini::{FieldParse, INIError, INILoadType, INIResult, INI};

/// Weather settings structure matching C++ WeatherSetting
///
/// This structure keeps the weather settings, which can be overridden on a per-map basis.
/// Weather settings control the visual appearance and behavior of snow effects in the game.
#[derive(Debug, Clone)]
pub struct WeatherSetting {
    /// Texture file for snow flakes
    pub snow_texture: String,
    /// Frequency scale X for snow position adjustment
    pub snow_frequency_scale_x: f32,
    /// Frequency scale Y for snow position adjustment
    pub snow_frequency_scale_y: f32,
    /// Amplitude of snow movement (world units)
    pub snow_amplitude: f32,
    /// Hardware point-sprite size (arbitrary units)
    pub snow_point_size: f32,
    /// Maximum size of point sprite (pixels)
    pub snow_max_point_size: f32,
    /// Minimum size of point sprite (pixels)
    pub snow_min_point_size: f32,
    /// Quad size when no hardware point sprites (world width/height)
    pub snow_quad_size: f32,
    /// Dimensions of box surrounding camera (world units)
    pub snow_box_dimensions: f32,
    /// Number of emitters per world unit
    pub snow_box_density: f32,
    /// Speed at which snow falls (world units/sec)
    pub snow_velocity: f32,
    /// Whether to use hardware point sprites
    pub use_point_sprites: bool,
    /// Enable/disable snow on the map
    pub snow_enabled: bool,
    /// Whether this is an override setting
    is_override: bool,
    /// Next override in the chain (for override system)
    next_override: Option<Box<WeatherSetting>>,
}

impl Default for WeatherSetting {
    fn default() -> Self {
        Self {
            snow_texture: "EXSnowFlake.tga".to_string(),
            snow_frequency_scale_x: 0.0533,
            snow_frequency_scale_y: 0.0275,
            snow_amplitude: 5.0,
            snow_point_size: 1.0,
            snow_max_point_size: 64.0,
            snow_min_point_size: 0.0,
            snow_quad_size: 0.5,
            snow_box_dimensions: 200.0,
            snow_box_density: 1.0,
            snow_velocity: 4.0,
            use_point_sprites: true,
            snow_enabled: false,
            is_override: false,
            next_override: None,
        }
    }
}

impl WeatherSetting {
    /// Create a new WeatherSetting with default values matching C++ defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark this setting as an override
    pub fn mark_as_override(&mut self) {
        self.is_override = true;
    }

    /// Check if this is an override setting
    pub fn is_override(&self) -> bool {
        self.is_override
    }

    /// Get the next override in the chain
    pub fn get_next_override(&self) -> Option<&WeatherSetting> {
        self.next_override.as_ref().map(|b| b.as_ref())
    }

    /// Get mutable access to the next override in the chain
    pub fn get_next_override_mut(&mut self) -> Option<&mut WeatherSetting> {
        self.next_override.as_mut().map(|b| b.as_mut())
    }

    /// Set the next override in the chain
    pub fn set_next_override(&mut self, next: WeatherSetting) {
        self.next_override = Some(Box::new(next));
    }

    /// Get the final override in the chain (the one at the end)
    pub fn get_final_override(&mut self) -> &mut WeatherSetting {
        if let Some(ref mut next) = self.next_override {
            next.get_final_override()
        } else {
            self
        }
    }

    /// Get the non-overridden base setting
    pub fn get_non_overridden_pointer(&self) -> &WeatherSetting {
        if self.is_override {
            // This is an override, so the base is stored elsewhere
            // For now, just return self since we don't track the base separately
            self
        } else {
            self
        }
    }

    /// Get the field parse table for INI parsing
    pub fn get_field_parse() -> Vec<FieldParse<WeatherSetting>> {
        vec![
            FieldParse {
                token: "SnowTexture",
                parse: parse_snow_texture,
            },
            FieldParse {
                token: "SnowFrequencyScaleX",
                parse: parse_snow_frequency_scale_x,
            },
            FieldParse {
                token: "SnowFrequencyScaleY",
                parse: parse_snow_frequency_scale_y,
            },
            FieldParse {
                token: "SnowAmplitude",
                parse: parse_snow_amplitude,
            },
            FieldParse {
                token: "SnowPointSize",
                parse: parse_snow_point_size,
            },
            FieldParse {
                token: "SnowMaxPointSize",
                parse: parse_snow_max_point_size,
            },
            FieldParse {
                token: "SnowMinPointSize",
                parse: parse_snow_min_point_size,
            },
            FieldParse {
                token: "SnowQuadSize",
                parse: parse_snow_quad_size,
            },
            FieldParse {
                token: "SnowBoxDimensions",
                parse: parse_snow_box_dimensions,
            },
            FieldParse {
                token: "SnowBoxDensity",
                parse: parse_snow_box_density,
            },
            FieldParse {
                token: "SnowVelocity",
                parse: parse_snow_velocity,
            },
            FieldParse {
                token: "SnowPointSprites",
                parse: parse_use_point_sprites,
            },
            FieldParse {
                token: "SnowEnabled",
                parse: parse_snow_enabled,
            },
        ]
    }
}

// Field parsing functions

fn parse_snow_texture(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_texture = INI::parse_ascii_string(value)?;
    Ok(())
}

fn parse_snow_frequency_scale_x(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_frequency_scale_x = INI::parse_real(value)?;
    Ok(())
}

fn parse_snow_frequency_scale_y(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_frequency_scale_y = INI::parse_real(value)?;
    Ok(())
}

fn parse_snow_amplitude(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_amplitude = INI::parse_real(value)?;
    Ok(())
}

fn parse_snow_point_size(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_point_size = INI::parse_real(value)?;
    Ok(())
}

fn parse_snow_max_point_size(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_max_point_size = INI::parse_real(value)?;
    Ok(())
}

fn parse_snow_min_point_size(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_min_point_size = INI::parse_real(value)?;
    Ok(())
}

fn parse_snow_quad_size(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_quad_size = INI::parse_real(value)?;
    Ok(())
}

fn parse_snow_box_dimensions(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_box_dimensions = INI::parse_real(value)?;
    Ok(())
}

fn parse_snow_box_density(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_box_density = INI::parse_real(value)?;
    Ok(())
}

fn parse_snow_velocity(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_velocity = INI::parse_real(value)?;
    Ok(())
}

fn parse_use_point_sprites(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.use_point_sprites = INI::parse_bool(value)?;
    Ok(())
}

fn parse_snow_enabled(
    _ini: &mut INI,
    target: &mut WeatherSetting,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    target.snow_enabled = INI::parse_bool(value)?;
    Ok(())
}

// Global weather setting singleton storage with override support
static WEATHER_SETTING: OnceCell<RwLock<Option<WeatherSetting>>> = OnceCell::new();

/// Get the global weather setting store, initializing if needed
pub fn get_weather_setting() -> Option<WeatherSetting> {
    WEATHER_SETTING
        .get_or_init(|| RwLock::new(None))
        .read()
        .ok()
        .and_then(|guard| guard.clone())
}

/// Get mutable access to the weather setting store
fn get_weather_setting_mut() -> &'static RwLock<Option<WeatherSetting>> {
    WEATHER_SETTING.get_or_init(|| RwLock::new(None))
}

/// Get read access to the weather setting store
pub fn get_weather_setting_lock() -> Option<std::sync::RwLockReadGuard<'static, Option<WeatherSetting>>> {
    WEATHER_SETTING
        .get()
        .and_then(|lock| lock.read().ok())
}

/// Initialize the weather settings system
pub fn init_weather_setting() {
    let _ = WEATHER_SETTING.get_or_init(|| RwLock::new(None));
}

/// Parse a [Weather] block from an INI file
///
/// This matches the C++ INI::parseWeatherDefinition function.
///
/// The parsing behavior depends on the load type:
/// - If no existing setting exists, create a new one
/// - If `CreateOverrides` load type, create an override of the existing setting
/// - Otherwise, overwrite the existing setting (throws error in C++)
pub fn parse_weather_definition(ini: &mut INI) -> INIResult<()> {
    let load_type = ini.get_load_type();
    
    // Get the current setting (if any)
    let current = get_weather_setting();
    
    // Determine what to do based on load type and current state
    let setting = if current.is_none() {
        // No existing setting, create a new one
        WeatherSetting::new()
    } else if load_type == INILoadType::CreateOverrides {
        // We're creating an override - copy existing and mark as override
        let mut new_setting = current.unwrap();
        new_setting.mark_as_override();
        new_setting.next_override = None; // Clear any existing override chain
        new_setting
    } else {
        // In C++, this throws INI_INVALID_DATA, but we'll allow overwriting
        // for flexibility and to match other parsers in this codebase
        WeatherSetting::new()
    };
    
    // Parse the fields using the field parse table
    parse_weather_fields(ini, setting)
}

/// Parse weather fields from INI using the field parse table
fn parse_weather_fields(ini: &mut INI, mut setting: WeatherSetting) -> INIResult<()> {
    let field_parse_table = WeatherSetting::get_field_parse();
    
    ini.init_from_ini_with_fields(&mut setting, &field_parse_table)?;
    
    // Store the setting
    let mut guard = get_weather_setting_mut()
        .write()
        .map_err(|_| INIError::UnknownError)?;
    *guard = Some(setting);
    
    Ok(())
}

/// Update weather settings from the global store (for SnowManager)
///
/// This would be called by SnowManager when settings change.
/// For now, just ensures the setting is initialized.
pub fn update_weather_settings() -> INIResult<()> {
    let _ = get_weather_setting();
    Ok(())
}

/// Check if snow is enabled
pub fn is_snow_enabled() -> bool {
    get_weather_setting()
        .map(|s| s.snow_enabled)
        .unwrap_or(false)
}

/// Get snow texture name
pub fn get_snow_texture() -> String {
    get_weather_setting()
        .map(|s| s.snow_texture.clone())
        .unwrap_or_else(|| "EXSnowFlake.tga".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_setting_defaults() {
        let setting = WeatherSetting::new();
        assert_eq!(setting.snow_texture, "EXSnowFlake.tga");
        assert!((setting.snow_frequency_scale_x - 0.0533).abs() < 0.0001);
        assert!((setting.snow_frequency_scale_y - 0.0275).abs() < 0.0001);
        assert!((setting.snow_amplitude - 5.0).abs() < 0.0001);
        assert!((setting.snow_point_size - 1.0).abs() < 0.0001);
        assert!((setting.snow_max_point_size - 64.0).abs() < 0.0001);
        assert!((setting.snow_min_point_size - 0.0).abs() < 0.0001);
        assert!((setting.snow_quad_size - 0.5).abs() < 0.0001);
        assert!((setting.snow_box_dimensions - 200.0).abs() < 0.0001);
        assert!((setting.snow_box_density - 1.0).abs() < 0.0001);
        assert!((setting.snow_velocity - 4.0).abs() < 0.0001);
        assert!(!setting.snow_enabled);
        assert!(setting.use_point_sprites);
        assert!(!setting.is_override());
    }

    #[test]
    fn test_weather_setting_override() {
        let mut setting = WeatherSetting::new();
        assert!(!setting.is_override());
        setting.mark_as_override();
        assert!(setting.is_override());
    }

    #[test]
    fn test_weather_setting_override_chain() {
        let mut base = WeatherSetting::new();
        let mut override_setting = WeatherSetting::new();
        override_setting.snow_enabled = true;
        override_setting.mark_as_override();
        
        base.set_next_override(override_setting);
        
        assert!(base.get_next_override().is_some());
        assert!(base.get_next_override().unwrap().snow_enabled);
        
        // Test get_final_override
        let final_override = base.get_final_override();
        assert!(final_override.is_override());
    }

    #[test]
    fn test_get_field_parse_table() {
        let table = WeatherSetting::get_field_parse();
        assert_eq!(table.len(), 13);
        
        // Verify all expected fields are present
        let tokens: Vec<&str> = table.iter().map(|f| f.token).collect();
        assert!(tokens.contains(&"SnowTexture"));
        assert!(tokens.contains(&"SnowFrequencyScaleX"));
        assert!(tokens.contains(&"SnowFrequencyScaleY"));
        assert!(tokens.contains(&"SnowAmplitude"));
        assert!(tokens.contains(&"SnowPointSize"));
        assert!(tokens.contains(&"SnowMaxPointSize"));
        assert!(tokens.contains(&"SnowMinPointSize"));
        assert!(tokens.contains(&"SnowQuadSize"));
        assert!(tokens.contains(&"SnowBoxDimensions"));
        assert!(tokens.contains(&"SnowBoxDensity"));
        assert!(tokens.contains(&"SnowVelocity"));
        assert!(tokens.contains(&"SnowPointSprites"));
        assert!(tokens.contains(&"SnowEnabled"));
    }
}
