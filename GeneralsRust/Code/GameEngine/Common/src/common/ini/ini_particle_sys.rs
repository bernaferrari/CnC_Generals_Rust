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
        vec![
            ("Priority", |value| {
                value
                    .parse::<i32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse priority: {}", e))
            }),
            ("IsOneShot", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse bool: {}", e))
            }),
            ("MaxParticles", |value| {
                value
                    .parse::<u32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse max particles: {}", e))
            }),
            ("Lifetime", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse lifetime: {}", e))
            }),
            ("CreationRate", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse creation rate: {}", e))
            }),
            ("Size", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse size: {}", e))
            }),
            ("SizeVariation", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse size variation: {}", e))
            }),
            ("Texture", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("Shader", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("IsEmissive", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse emissive: {}", e))
            }),
            ("WindMotion", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse wind motion: {}", e))
            }),
            ("Gravity", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse gravity: {}", e))
            }),
        ]
    }

    /// Update template properties from parsed data
    pub fn update_from_properties(&mut self, properties: &HashMap<String, String>) {
        for (key, value) in properties {
            match key.as_str() {
                "Priority" => {
                    if let Ok(priority) = value.parse::<i32>() {
                        self.priority = priority;
                    }
                }
                "IsOneShot" => {
                    if let Ok(is_one_shot) = parse_bool(value) {
                        self.is_one_shot = is_one_shot;
                    }
                }
                "MaxParticles" => {
                    if let Ok(max_particles) = value.parse::<u32>() {
                        self.max_particles = max_particles;
                    }
                }
                "Lifetime" => {
                    if let Ok(lifetime) = value.parse::<f32>() {
                        self.lifetime = lifetime;
                    }
                }
                "CreationRate" => {
                    if let Ok(rate) = value.parse::<f32>() {
                        self.creation_rate = rate;
                    }
                }
                "Size" => {
                    if let Ok(size) = value.parse::<f32>() {
                        self.size = size;
                    }
                }
                "SizeVariation" => {
                    if let Ok(variation) = value.parse::<f32>() {
                        self.size_variation = variation;
                    }
                }
                "Texture" => {
                    self.texture_name = AsciiString::from(value);
                }
                "Shader" => {
                    self.shader_type = AsciiString::from(value);
                }
                "IsEmissive" => {
                    if let Ok(emissive) = parse_bool(value) {
                        self.is_emissive = emissive;
                    }
                }
                "WindMotion" => {
                    if let Ok(wind) = parse_bool(value) {
                        self.wind_motion = wind;
                    }
                }
                "Gravity" => {
                    if let Ok(gravity) = value.parse::<f32>() {
                        self.gravity = gravity;
                    }
                }
                _ => {
                    // Store unknown properties for later processing
                    self.properties.insert(key.clone(), value.clone());
                }
            }
        }
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn is_valid(&self) -> bool {
        !self.name.is_empty() && self.max_particles > 0 && self.lifetime > 0.0
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
        template.update_from_properties(&properties);

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
        properties.insert("MaxParticles".to_string(), "2000".to_string());
        properties.insert("Lifetime".to_string(), "10.5".to_string());
        properties.insert("IsOneShot".to_string(), "true".to_string());

        template.update_from_properties(&properties);

        assert_eq!(template.max_particles, 2000);
        assert_eq!(template.lifetime, 10.5);
        assert!(template.is_one_shot);
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

        let result = ParticleSystemTokenParser::parse_property_line("MaxParticles = 1000");
        assert!(result.is_ok());
        let (key, value) = result.unwrap();
        assert_eq!(key, "MaxParticles");
        assert_eq!(value, "1000");
    }
}
