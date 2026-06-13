////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_particle_sys.rs
//! Author: Michael S. Booth, November 2001 (Converted to Rust)
//! Desc:   Parsing Particle System INI entries

use crate::common::ascii_string::AsciiString;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

const PARTICLE_PRIORITY_NAMES: &[&str] = &[
    "NONE",
    "WEAPON_EXPLOSION",
    "SCORCHMARK",
    "DUST_TRAIL",
    "BUILDUP",
    "DEBRIS_TRAIL",
    "UNIT_DAMAGE_FX",
    "DEATH_EXPLOSION",
    "SEMI_CONSTANT",
    "CONSTANT",
    "WEAPON_TRAIL",
    "AREA_EFFECT",
    "CRITICAL",
    "ALWAYS_RENDER",
];

const PARTICLE_SHADER_TYPE_NAMES: &[&str] =
    &["NONE", "ADDITIVE", "ALPHA", "ALPHA_TEST", "MULTIPLY"];
const PARTICLE_TYPE_NAMES: &[&str] = &[
    "NONE",
    "PARTICLE",
    "DRAWABLE",
    "STREAK",
    "VOLUME_PARTICLE",
    "SMUDGE",
];
const EMISSION_VELOCITY_TYPE_NAMES: &[&str] = &[
    "NONE",
    "ORTHO",
    "SPHERICAL",
    "HEMISPHERICAL",
    "CYLINDRICAL",
    "OUTWARD",
];
const EMISSION_VOLUME_TYPE_NAMES: &[&str] = &["NONE", "POINT", "LINE", "BOX", "SPHERE", "CYLINDER"];
const WIND_MOTION_NAMES: &[&str] = &["NONE", "Unused", "PingPong", "Circular"];

const CPP_PARTICLE_SYSTEM_FIELDS: &[&str] = &[
    "Priority",
    "IsOneShot",
    "Shader",
    "Type",
    "ParticleName",
    "AngleZ",
    "AngularRateZ",
    "AngularDamping",
    "VelocityDamping",
    "Gravity",
    "SlaveSystem",
    "SlavePosOffset",
    "PerParticleAttachedSystem",
    "Lifetime",
    "SystemLifetime",
    "Size",
    "StartSizeRate",
    "SizeRate",
    "SizeRateDamping",
    "Alpha1",
    "Alpha2",
    "Alpha3",
    "Alpha4",
    "Alpha5",
    "Alpha6",
    "Alpha7",
    "Alpha8",
    "Color1",
    "Color2",
    "Color3",
    "Color4",
    "Color5",
    "Color6",
    "Color7",
    "Color8",
    "ColorScale",
    "BurstDelay",
    "BurstCount",
    "InitialDelay",
    "DriftVelocity",
    "VelocityType",
    "VelOrthoX",
    "VelOrthoY",
    "VelOrthoZ",
    "VelSpherical",
    "VelHemispherical",
    "VelCylindricalRadial",
    "VelCylindricalNormal",
    "VelOutward",
    "VelOutwardOther",
    "VolumeType",
    "VolLineStart",
    "VolLineEnd",
    "VolBoxHalfSize",
    "VolSphereRadius",
    "VolCylinderRadius",
    "VolCylinderLength",
    "IsHollow",
    "IsGroundAligned",
    "IsEmitAboveGroundOnly",
    "IsParticleUpTowardsEmitter",
    "WindMotion",
    "WindAngleChangeMin",
    "WindAngleChangeMax",
    "WindPingPongStartAngleMin",
    "WindPingPongStartAngleMax",
    "WindPingPongEndAngleMin",
    "WindPingPongEndAngleMax",
];

/// Result type for particle system parsing operations
pub type ParticleSystemResult<T> = Result<T, ParticleSystemError>;

/// Errors that can occur during particle system parsing
#[derive(Debug, Clone, PartialEq)]
pub enum ParticleSystemError {
    InvalidName,
    TemplateNotFound,
    ParsingError(String),
    ManagerError(String),
}

impl std::fmt::Display for ParticleSystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParticleSystemError::InvalidName => write!(f, "Invalid particle system name"),
            ParticleSystemError::TemplateNotFound => {
                write!(f, "Particle system template not found")
            }
            ParticleSystemError::ParsingError(msg) => write!(f, "Parsing error: {}", msg),
            ParticleSystemError::ManagerError(msg) => write!(f, "Manager error: {}", msg),
        }
    }
}

impl std::error::Error for ParticleSystemError {}

/// Particle system template definition
#[derive(Debug, Clone)]
pub struct ParticleSystemTemplate {
    pub name: AsciiString,
    pub properties: HashMap<String, String>,
    pub priority: i32,
    pub is_one_shot: bool,
    pub max_particles: u32,
    pub lifetime: f32,
    pub creation_rate: f32,
    pub velocity: (f32, f32, f32), // x, y, z
    pub acceleration: (f32, f32, f32),
    pub size: f32,
    pub size_variation: f32,
    pub color: (f32, f32, f32, f32), // r, g, b, a
    pub texture_name: AsciiString,
    pub shader_type: AsciiString,
    pub is_emissive: bool,
    pub wind_motion: bool,
    pub gravity: f32,
}

impl ParticleSystemTemplate {
    pub fn new(name: AsciiString) -> Self {
        Self {
            name,
            properties: HashMap::new(),
            priority: 0,
            is_one_shot: false,
            max_particles: 1000,
            lifetime: 5.0,
            creation_rate: 10.0,
            velocity: (0.0, 0.0, 0.0),
            acceleration: (0.0, 0.0, 0.0),
            size: 1.0,
            size_variation: 0.0,
            color: (1.0, 1.0, 1.0, 1.0),
            texture_name: AsciiString::from(""),
            shader_type: AsciiString::from("Default"),
            is_emissive: false,
            wind_motion: false,
            gravity: 0.0,
        }
    }

    /// Get the field parse table for this template
    pub fn get_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        CPP_PARTICLE_SYSTEM_FIELDS
            .iter()
            .map(|field| {
                (
                    *field,
                    parse_cpp_particle_field_for_table
                        as fn(&str) -> Result<Box<dyn std::any::Any>, String>,
                )
            })
            .collect()
    }

    /// Update template properties from parsed data
    pub fn update_from_properties(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> ParticleSystemResult<()> {
        for (key, value) in properties {
            match key.as_str() {
                "Priority" => {
                    self.priority = parse_enum_index(key, value, PARTICLE_PRIORITY_NAMES)? as i32;
                }
                "IsOneShot" => {
                    self.is_one_shot =
                        parse_bool(value).map_err(ParticleSystemError::ParsingError)?;
                }
                "Shader" => {
                    parse_enum_index(key, value, PARTICLE_SHADER_TYPE_NAMES)?;
                    self.shader_type = AsciiString::from(value);
                }
                "Type" => {
                    parse_enum_index(key, value, PARTICLE_TYPE_NAMES)?;
                    self.properties.insert(key.clone(), value.clone());
                }
                "ParticleName" => {
                    self.texture_name = AsciiString::from(value);
                }
                "AngleZ"
                | "AngularRateZ"
                | "AngularDamping"
                | "VelocityDamping"
                | "StartSizeRate"
                | "SizeRate"
                | "SizeRateDamping"
                | "ColorScale"
                | "BurstDelay"
                | "InitialDelay"
                | "VelOrthoX"
                | "VelOrthoY"
                | "VelOrthoZ"
                | "VelSpherical"
                | "VelHemispherical"
                | "VelCylindricalRadial"
                | "VelCylindricalNormal"
                | "VelOutward"
                | "VelOutwardOther" => {
                    parse_random_variable_field(key, value)?;
                    self.properties.insert(key.clone(), value.clone());
                }
                "Lifetime" => {
                    self.lifetime = parse_random_variable_field(key, value)?.0;
                }
                "Size" => {
                    self.size = parse_random_variable_field(key, value)?.0;
                }
                "BurstCount" => {
                    self.creation_rate = parse_random_variable_field(key, value)?.0;
                }
                "SystemLifetime" => {
                    parse_u32_field(key, value)?;
                    self.properties.insert(key.clone(), value.clone());
                }
                "Gravity"
                | "VolSphereRadius"
                | "VolCylinderRadius"
                | "VolCylinderLength"
                | "WindAngleChangeMin"
                | "WindAngleChangeMax"
                | "WindPingPongStartAngleMin"
                | "WindPingPongStartAngleMax"
                | "WindPingPongEndAngleMin"
                | "WindPingPongEndAngleMax" => {
                    let parsed = parse_f32_field(key, value)?;
                    if key == "Gravity" {
                        self.gravity = parsed;
                    } else {
                        self.properties.insert(key.clone(), value.clone());
                    }
                }
                "SlaveSystem" | "PerParticleAttachedSystem" => {
                    self.properties.insert(key.clone(), value.clone());
                }
                "SlavePosOffset" | "DriftVelocity" | "VolLineStart" | "VolLineEnd"
                | "VolBoxHalfSize" => {
                    parse_coord3d_field(key, value)?;
                    self.properties.insert(key.clone(), value.clone());
                }
                "VelocityType" => {
                    parse_enum_index(key, value, EMISSION_VELOCITY_TYPE_NAMES)?;
                    self.properties.insert(key.clone(), value.clone());
                }
                "VolumeType" => {
                    parse_enum_index(key, value, EMISSION_VOLUME_TYPE_NAMES)?;
                    self.properties.insert(key.clone(), value.clone());
                }
                "IsHollow"
                | "IsGroundAligned"
                | "IsEmitAboveGroundOnly"
                | "IsParticleUpTowardsEmitter" => {
                    parse_bool(value).map_err(ParticleSystemError::ParsingError)?;
                    self.properties.insert(key.clone(), value.clone());
                }
                "WindMotion" => {
                    parse_enum_index(key, value, WIND_MOTION_NAMES)?;
                    self.properties.insert(key.clone(), value.clone());
                }
                "Alpha1" | "Alpha2" | "Alpha3" | "Alpha4" | "Alpha5" | "Alpha6" | "Alpha7"
                | "Alpha8" => {
                    parse_random_keyframe_field(key, value)?;
                    self.properties.insert(key.clone(), value.clone());
                }
                "Color1" | "Color2" | "Color3" | "Color4" | "Color5" | "Color6" | "Color7"
                | "Color8" => {
                    parse_rgb_color_keyframe_field(key, value)?;
                    self.properties.insert(key.clone(), value.clone());
                }
                _ => {
                    return Err(ParticleSystemError::ParsingError(format!(
                        "Unknown particle system field '{}'",
                        key
                    )));
                }
            }
        }

        Ok(())
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn is_valid(&self) -> bool {
        !self.name.is_empty()
    }
}

/// Particle system manager - manages templates and instances
#[derive(Debug)]
pub struct ParticleSystemManager {
    templates: HashMap<String, ParticleSystemTemplate>,
}

impl ParticleSystemManager {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Find a template by name
    pub fn find_template(&self, name: &AsciiString) -> Option<&ParticleSystemTemplate> {
        self.templates.get(name.as_str())
    }

    /// Find a mutable template by name
    pub fn find_template_mut(&mut self, name: &AsciiString) -> Option<&mut ParticleSystemTemplate> {
        self.templates.get_mut(name.as_str())
    }

    /// Create a new template
    pub fn new_template(&mut self, name: AsciiString) -> &mut ParticleSystemTemplate {
        let template = ParticleSystemTemplate::new(name.clone());
        self.templates.insert(name.as_str().to_string(), template);
        self.templates.get_mut(name.as_str()).unwrap()
    }

    /// Get or create a template
    pub fn get_or_create_template(&mut self, name: &AsciiString) -> &mut ParticleSystemTemplate {
        if !self.templates.contains_key(name.as_str()) {
            self.new_template(name.clone());
        }
        self.templates.get_mut(name.as_str()).unwrap()
    }

    /// Get all template names
    pub fn get_template_names(&self) -> Vec<&String> {
        self.templates.keys().collect()
    }

    /// Remove a template
    pub fn remove_template(&mut self, name: &AsciiString) -> bool {
        self.templates.remove(name.as_str()).is_some()
    }

    /// Clear all templates
    pub fn clear(&mut self) {
        self.templates.clear();
    }

    /// Get template count
    pub fn get_template_count(&self) -> usize {
        self.templates.len()
    }
}

impl Default for ParticleSystemManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global particle system manager instance (thread-safe)
static PARTICLE_SYSTEM_MANAGER: OnceCell<Arc<RwLock<ParticleSystemManager>>> = OnceCell::new();

/// Ensure the particle system manager exists and return a handle to it
pub fn ensure_particle_system_manager() -> Arc<RwLock<ParticleSystemManager>> {
    PARTICLE_SYSTEM_MANAGER
        .get_or_init(|| Arc::new(RwLock::new(ParticleSystemManager::new())))
        .clone()
}

/// Initialize (or reinitialize) the global particle system manager
pub fn initialize_particle_system_manager() {
    ensure_particle_system_manager();
}

/// Get a handle to the global particle system manager if initialized
pub fn get_particle_system_manager() -> Option<Arc<RwLock<ParticleSystemManager>>> {
    PARTICLE_SYSTEM_MANAGER.get().cloned()
}

/// Parse a boolean value from string
pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

fn parse_i32_field(field_name: &str, value: &str) -> ParticleSystemResult<i32> {
    value.parse::<i32>().map_err(|e| {
        ParticleSystemError::ParsingError(format!(
            "Invalid {} value '{}': {}",
            field_name, value, e
        ))
    })
}

fn parse_u32_field(field_name: &str, value: &str) -> ParticleSystemResult<u32> {
    value.parse::<u32>().map_err(|e| {
        ParticleSystemError::ParsingError(format!(
            "Invalid {} value '{}': {}",
            field_name, value, e
        ))
    })
}

fn parse_f32_field(field_name: &str, value: &str) -> ParticleSystemResult<f32> {
    value.parse::<f32>().map_err(|e| {
        ParticleSystemError::ParsingError(format!(
            "Invalid {} value '{}': {}",
            field_name, value, e
        ))
    })
}

fn parse_cpp_particle_field_for_table(value: &str) -> Result<Box<dyn std::any::Any>, String> {
    Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
}

fn parse_enum_index(field_name: &str, value: &str, names: &[&str]) -> ParticleSystemResult<usize> {
    names
        .iter()
        .position(|name| *name == value.trim())
        .ok_or_else(|| {
            ParticleSystemError::ParsingError(format!(
                "Invalid {} value '{}': not in C++ enum table",
                field_name, value
            ))
        })
}

fn parse_random_variable_field(field_name: &str, value: &str) -> ParticleSystemResult<(f32, f32)> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if !(parts.len() == 2 || parts.len() == 3) {
        return Err(ParticleSystemError::ParsingError(format!(
            "Invalid {} random variable '{}': expected low high [distribution]",
            field_name, value
        )));
    }

    let low = parse_f32_token(field_name, parts[0])?;
    let high = parse_f32_token(field_name, parts[1])?;
    if let Some(distribution) = parts.get(2) {
        parse_enum_index(
            field_name,
            distribution,
            &[
                "CONSTANT",
                "UNIFORM",
                "GAUSSIAN",
                "TRIANGULAR",
                "LOW_BIAS",
                "HIGH_BIAS",
            ],
        )?;
    }
    Ok((low, high))
}

fn parse_random_keyframe_field(field_name: &str, value: &str) -> ParticleSystemResult<()> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(ParticleSystemError::ParsingError(format!(
            "Invalid {} keyframe '{}': expected low high frame",
            field_name, value
        )));
    }
    parse_f32_token(field_name, parts[0])?;
    parse_f32_token(field_name, parts[1])?;
    parse_u32_field(field_name, parts[2])?;
    Ok(())
}

fn parse_rgb_color_keyframe_field(field_name: &str, value: &str) -> ParticleSystemResult<()> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.len() != 4 {
        return Err(ParticleSystemError::ParsingError(format!(
            "Invalid {} color keyframe '{}': expected R:nnn G:nnn B:nnn frame",
            field_name, value
        )));
    }
    parse_labeled_f32(field_name, parts[0], "R")?;
    parse_labeled_f32(field_name, parts[1], "G")?;
    parse_labeled_f32(field_name, parts[2], "B")?;
    parse_u32_field(field_name, parts[3])?;
    Ok(())
}

fn parse_coord3d_field(field_name: &str, value: &str) -> ParticleSystemResult<()> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(ParticleSystemError::ParsingError(format!(
            "Invalid {} coord '{}': expected X Y Z",
            field_name, value
        )));
    }

    parse_coord_component(field_name, parts[0], "X")?;
    parse_coord_component(field_name, parts[1], "Y")?;
    parse_coord_component(field_name, parts[2], "Z")?;
    Ok(())
}

fn parse_coord_component(field_name: &str, token: &str, label: &str) -> ParticleSystemResult<f32> {
    if token.contains(':') {
        parse_labeled_f32(field_name, token, label)
    } else {
        parse_f32_token(field_name, token)
    }
}

fn parse_labeled_f32(field_name: &str, token: &str, label: &str) -> ParticleSystemResult<f32> {
    let (actual_label, value) = token.split_once(':').ok_or_else(|| {
        ParticleSystemError::ParsingError(format!(
            "Invalid {} token '{}': expected {}:value",
            field_name, token, label
        ))
    })?;
    if actual_label != label {
        return Err(ParticleSystemError::ParsingError(format!(
            "Invalid {} token '{}': expected {} label",
            field_name, token, label
        )));
    }
    parse_f32_token(field_name, value)
}

fn parse_f32_token(field_name: &str, token: &str) -> ParticleSystemResult<f32> {
    token.parse::<f32>().map_err(|e| {
        ParticleSystemError::ParsingError(format!(
            "Invalid {} value '{}': {}",
            field_name, token, e
        ))
    })
}

/// INI parsing functions for particle systems
pub struct IniParticleSys;

impl IniParticleSys {
    /// Parse particle system definition - equivalent to INI::parseParticleSystemDefinition
    pub fn parse_particle_system_definition(name: AsciiString) -> ParticleSystemResult<()> {
        // Validate name
        if name.is_empty() {
            return Err(ParticleSystemError::InvalidName);
        }

        // Initialize manager if needed
        initialize_particle_system_manager();

        let manager_handle = ensure_particle_system_manager();

        // Find existing template or create new one
        {
            let mut manager = manager_handle.write();
            if manager.find_template(&name).is_some() {
                manager
                    .find_template_mut(&name)
                    .expect("Particle template should exist");
            } else {
                manager.new_template(name.clone());
            }
        }

        // In the original C++, this would call:
        // ini->initFromINI(sysTemplate, sysTemplate->getFieldParse());
        println!("Parsing particle system definition for: {}", name.as_str());

        Ok(())
    }

    /// Parse a complete particle system block from INI data
    pub fn parse_particle_system_block(
        name: AsciiString,
        properties: HashMap<String, String>,
    ) -> ParticleSystemResult<ParticleSystemTemplate> {
        // Validate name
        if name.is_empty() {
            return Err(ParticleSystemError::InvalidName);
        }

        // Create template
        let mut template = ParticleSystemTemplate::new(name);

        // Update template from properties
        template.update_from_properties(&properties)?;

        // Validate template
        if !template.is_valid() {
            return Err(ParticleSystemError::ParsingError(
                "Invalid particle system template configuration".to_string(),
            ));
        }

        Ok(template)
    }

    /// Register a particle system template
    pub fn register_template(template: ParticleSystemTemplate) -> ParticleSystemResult<()> {
        initialize_particle_system_manager();

        let manager_handle = ensure_particle_system_manager();

        let name = template.name.clone();
        manager_handle
            .write()
            .templates
            .insert(name.as_str().to_string(), template);

        println!("Registered particle system template: {}", name.as_str());
        Ok(())
    }

    /// Find a particle system template by name
    pub fn find_template_by_name(name: &AsciiString) -> Option<ParticleSystemTemplate> {
        get_particle_system_manager()
            .and_then(|manager| manager.read().find_template(name).cloned())
    }

    /// Validate particle system name format
    pub fn validate_name(name: &AsciiString) -> bool {
        !name.is_empty() && name.len() < 128 // Reasonable length limit
    }
}

/// Token parser for extracting particle system data from INI tokens
pub struct ParticleSystemTokenParser;

impl ParticleSystemTokenParser {
    /// Extract the next particle system name token
    pub fn get_next_name(token: &str) -> ParticleSystemResult<AsciiString> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(ParticleSystemError::InvalidName);
        }

        Ok(AsciiString::from(trimmed))
    }

    /// Parse a property line (key = value)
    pub fn parse_property_line(line: &str) -> ParticleSystemResult<(String, String)> {
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            Ok((key, value))
        } else {
            Err(ParticleSystemError::ParsingError(format!(
                "Invalid property line format: {}",
                line
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_system_template_creation() {
        let name = AsciiString::from("TestParticleSystem");
        let template = ParticleSystemTemplate::new(name.clone());

        assert_eq!(template.name, name);
        assert_eq!(template.max_particles, 1000);
        assert_eq!(template.lifetime, 5.0);
        assert!(!template.is_one_shot);
        assert!(template.is_valid());
    }

    #[test]
    fn test_particle_system_manager() {
        let mut manager = ParticleSystemManager::new();
        let name = AsciiString::from("TestSystem");

        // Create new template
        let template = manager.new_template(name.clone());
        template.max_particles = 500;

        // Find template
        let found = manager.find_template(&name);
        assert!(found.is_some());
        assert_eq!(found.unwrap().max_particles, 500);

        // Count templates
        assert_eq!(manager.get_template_count(), 1);
    }

    #[test]
    fn test_template_properties_update() {
        let mut template = ParticleSystemTemplate::new(AsciiString::from("Test"));
        let mut properties = HashMap::new();
        properties.insert("Priority".to_string(), "WEAPON_EXPLOSION".to_string());
        properties.insert("Lifetime".to_string(), "10.5 12.5".to_string());
        properties.insert("Size".to_string(), "2.0 4.0".to_string());
        properties.insert("BurstCount".to_string(), "1.0 3.0".to_string());
        properties.insert("ParticleName".to_string(), "EXSmoke.tga".to_string());
        properties.insert("Shader".to_string(), "ALPHA".to_string());
        properties.insert("IsOneShot".to_string(), "Yes".to_string());

        template.update_from_properties(&properties).unwrap();

        assert_eq!(template.lifetime, 10.5);
        assert_eq!(template.size, 2.0);
        assert_eq!(template.creation_rate, 1.0);
        assert_eq!(template.texture_name.as_str(), "EXSmoke.tga");
        assert_eq!(template.shader_type.as_str(), "ALPHA");
        assert!(template.is_one_shot);
    }

    #[test]
    fn particle_block_rejects_invalid_parsed_field_values() {
        let mut properties = HashMap::new();
        properties.insert("Priority".to_string(), "urgent".to_string());
        assert!(IniParticleSys::parse_particle_system_block(
            AsciiString::from("BadPriority"),
            properties
        )
        .is_err());

        let mut properties = HashMap::new();
        properties.insert("Lifetime".to_string(), "many".to_string());
        assert!(IniParticleSys::parse_particle_system_block(
            AsciiString::from("BadLifetime"),
            properties,
        )
        .is_err());

        let mut properties = HashMap::new();
        properties.insert("Alpha1".to_string(), "0.0 1.0".to_string());
        assert!(IniParticleSys::parse_particle_system_block(
            AsciiString::from("BadAlpha"),
            properties
        )
        .is_err());

        let mut properties = HashMap::new();
        properties.insert("IsOneShot".to_string(), "maybe".to_string());
        assert!(IniParticleSys::parse_particle_system_block(
            AsciiString::from("BadIsOneShot"),
            properties
        )
        .is_err());
    }

    #[test]
    fn particle_block_accepts_real_cpp_field_table_fields() {
        let mut properties = HashMap::new();
        properties.insert("Priority".to_string(), "WEAPON_EXPLOSION".to_string());
        properties.insert("IsOneShot".to_string(), "No".to_string());
        properties.insert("Shader".to_string(), "ALPHA".to_string());
        properties.insert("Type".to_string(), "PARTICLE".to_string());
        properties.insert("ParticleName".to_string(), "EXSmokNew1.tga".to_string());
        properties.insert("AngleZ".to_string(), "0.00 0.25".to_string());
        properties.insert("AngularRateZ".to_string(), "-0.01 0.01".to_string());
        properties.insert("AngularDamping".to_string(), "0.99 0.99".to_string());
        properties.insert("VelocityDamping".to_string(), "0.99 0.98".to_string());
        properties.insert("Gravity".to_string(), "0.01".to_string());
        properties.insert("Lifetime".to_string(), "60.00 60.00".to_string());
        properties.insert("SystemLifetime".to_string(), "0".to_string());
        properties.insert("Size".to_string(), "5.00 5.00".to_string());
        properties.insert("StartSizeRate".to_string(), "0.00 0.00".to_string());
        properties.insert("SizeRate".to_string(), "3.00 3.00".to_string());
        properties.insert("SizeRateDamping".to_string(), "0.95 0.95".to_string());
        properties.insert("Alpha1".to_string(), "0.00 0.00 0".to_string());
        properties.insert("Color1".to_string(), "R:255 G:255 B:255 0".to_string());
        properties.insert("ColorScale".to_string(), "0.00 0.00".to_string());
        properties.insert("BurstDelay".to_string(), "40.00 40.00".to_string());
        properties.insert("BurstCount".to_string(), "0.00 2.00".to_string());
        properties.insert("InitialDelay".to_string(), "20.00 20.00".to_string());
        properties.insert(
            "DriftVelocity".to_string(),
            "X:0.00 Y:0.00 Z:0.00".to_string(),
        );
        properties.insert("VelocityType".to_string(), "OUTWARD".to_string());
        properties.insert("VelOutward".to_string(), "0.00 0.00".to_string());
        properties.insert("VelOutwardOther".to_string(), "0.00 0.00".to_string());
        properties.insert("VolumeType".to_string(), "SPHERE".to_string());
        properties.insert("VolSphereRadius".to_string(), "4.00".to_string());
        properties.insert("IsHollow".to_string(), "No".to_string());
        properties.insert("IsGroundAligned".to_string(), "No".to_string());
        properties.insert("IsEmitAboveGroundOnly".to_string(), "No".to_string());
        properties.insert("IsParticleUpTowardsEmitter".to_string(), "No".to_string());
        properties.insert("WindMotion".to_string(), "Unused".to_string());
        properties.insert("WindAngleChangeMin".to_string(), "0.149924".to_string());
        properties.insert("WindAngleChangeMax".to_string(), "0.449946".to_string());
        properties.insert(
            "WindPingPongStartAngleMin".to_string(),
            "0.000000".to_string(),
        );
        properties.insert(
            "WindPingPongStartAngleMax".to_string(),
            "0.785398".to_string(),
        );
        properties.insert(
            "WindPingPongEndAngleMin".to_string(),
            "5.497787".to_string(),
        );
        properties.insert(
            "WindPingPongEndAngleMax".to_string(),
            "6.283185".to_string(),
        );

        let template = IniParticleSys::parse_particle_system_block(
            AsciiString::from("TsingMaTrailSmoke"),
            properties,
        )
        .unwrap();

        assert_eq!(template.priority, 1);
        assert!(!template.is_one_shot);
        assert_eq!(template.shader_type.as_str(), "ALPHA");
        assert_eq!(template.texture_name.as_str(), "EXSmokNew1.tga");
        assert_eq!(template.lifetime, 60.0);
        assert_eq!(template.size, 5.0);
        assert_eq!(template.gravity, 0.01);
    }

    #[test]
    fn particle_block_rejects_fields_outside_cpp_parse_table() {
        for field in [
            "MaxParticles",
            "CreationRate",
            "SizeVariation",
            "Texture",
            "IsEmissive",
            "UnknownField",
        ] {
            let mut properties = HashMap::new();
            properties.insert(field.to_string(), "1".to_string());
            assert!(
                IniParticleSys::parse_particle_system_block(
                    AsciiString::from("BadField"),
                    properties
                )
                .is_err(),
                "{} should be rejected because C++ ParticleSystemTemplate does not parse it",
                field
            );
        }
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("true"), Ok(true));
        assert_eq!(parse_bool("TRUE"), Ok(true));
        assert_eq!(parse_bool("yes"), Ok(true));
        assert_eq!(parse_bool("1"), Ok(true));

        assert_eq!(parse_bool("false"), Ok(false));
        assert_eq!(parse_bool("FALSE"), Ok(false));
        assert_eq!(parse_bool("no"), Ok(false));
        assert_eq!(parse_bool("0"), Ok(false));

        assert!(parse_bool("invalid").is_err());
    }

    #[test]
    fn test_validate_name() {
        assert!(IniParticleSys::validate_name(&AsciiString::from(
            "ValidName"
        )));
        assert!(!IniParticleSys::validate_name(&AsciiString::from("")));
    }

    #[test]
    fn test_token_parser() {
        assert!(ParticleSystemTokenParser::get_next_name("TestSystem").is_ok());
        assert!(ParticleSystemTokenParser::get_next_name("  SpacedName  ").is_ok());
        assert!(ParticleSystemTokenParser::get_next_name("").is_err());

        let result = ParticleSystemTokenParser::parse_property_line("SystemLifetime = 1000");
        assert!(result.is_ok());
        let (key, value) = result.unwrap();
        assert_eq!(key, "SystemLifetime");
        assert_eq!(value, "1000");
    }
}
