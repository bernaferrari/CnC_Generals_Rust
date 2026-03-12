////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_special_power.rs
//! Author: Colin Day, April 2002 (Converted to Rust)
//! Desc:   Special Power INI database

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ascii_string::AsciiString;

/// Result type for special power parsing operations
pub type SpecialPowerResult<T> = Result<T, SpecialPowerError>;

/// Errors that can occur during special power parsing
#[derive(Debug, Clone, PartialEq)]
pub enum SpecialPowerError {
    InvalidName,
    InvalidType,
    ParseError(String),
    StoreError(String),
    NotFound,
}

impl std::fmt::Display for SpecialPowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecialPowerError::InvalidName => write!(f, "Invalid special power name"),
            SpecialPowerError::InvalidType => write!(f, "Invalid special power type"),
            SpecialPowerError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            SpecialPowerError::StoreError(msg) => write!(f, "Store error: {}", msg),
            SpecialPowerError::NotFound => write!(f, "Special power not found"),
        }
    }
}

impl std::error::Error for SpecialPowerError {}

/// Special power types
#[derive(Debug, Clone, PartialEq)]
pub enum SpecialPowerType {
    Airstrike,
    Artillery,
    Heal,
    Repair,
    Nuke,
    EMP,
    SpyDrone,
    Radar,
    Superweapon,
    Support,
    Custom(String),
}

impl SpecialPowerType {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "airstrike" => Self::Airstrike,
            "artillery" => Self::Artillery,
            "heal" => Self::Heal,
            "repair" => Self::Repair,
            "nuke" => Self::Nuke,
            "emp" => Self::EMP,
            "spydrone" => Self::SpyDrone,
            "radar" => Self::Radar,
            "superweapon" => Self::Superweapon,
            "support" => Self::Support,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Airstrike => "Airstrike",
            Self::Artillery => "Artillery",
            Self::Heal => "Heal",
            Self::Repair => "Repair",
            Self::Nuke => "Nuke",
            Self::EMP => "EMP",
            Self::SpyDrone => "SpyDrone",
            Self::Radar => "Radar",
            Self::Superweapon => "Superweapon",
            Self::Support => "Support",
            Self::Custom(name) => name,
        }
    }
}

/// Special power definition
#[derive(Debug, Clone)]
pub struct SpecialPowerTemplate {
    pub name: AsciiString,
    pub power_type: SpecialPowerType,
    pub prerequisite_science: Vec<AsciiString>,
    pub required_science: Vec<AsciiString>,
    pub recharge_time: f32,
    pub init_charge_time: f32,
    pub cost: u32,
    pub range: f32,
    pub radius: f32,
    pub damage: f32,
    pub shared_sync_group: AsciiString,
    pub view_object_name: AsciiString,
    pub view_object_duration: f32,
    pub icon_name: AsciiString,
    pub button_border_type: AsciiString,
    pub description: AsciiString,
    pub sound_effect: AsciiString,
    pub flags: u32,
    pub properties: HashMap<String, String>,
}

impl SpecialPowerTemplate {
    pub fn new(name: AsciiString) -> Self {
        Self {
            name,
            power_type: SpecialPowerType::Custom("Unknown".to_string()),
            prerequisite_science: Vec::new(),
            required_science: Vec::new(),
            recharge_time: 30.0,
            init_charge_time: 0.0,
            cost: 0,
            range: 100.0,
            radius: 50.0,
            damage: 0.0,
            shared_sync_group: AsciiString::from(""),
            view_object_name: AsciiString::from(""),
            view_object_duration: 0.0,
            icon_name: AsciiString::from(""),
            button_border_type: AsciiString::from(""),
            description: AsciiString::from(""),
            sound_effect: AsciiString::from(""),
            flags: 0,
            properties: HashMap::new(),
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
            ("Type", |value| {
                Ok(Box::new(SpecialPowerType::from_string(value)) as Box<dyn std::any::Any>)
            }),
            ("PrerequisiteScience", |value| {
                let sciences: Vec<AsciiString> = value
                    .split_whitespace()
                    .map(|s| AsciiString::from(s))
                    .collect();
                Ok(Box::new(sciences) as Box<dyn std::any::Any>)
            }),
            ("RequiredScience", |value| {
                let sciences: Vec<AsciiString> = value
                    .split_whitespace()
                    .map(|s| AsciiString::from(s))
                    .collect();
                Ok(Box::new(sciences) as Box<dyn std::any::Any>)
            }),
            ("RechargeTime", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse recharge time: {}", e))
            }),
            ("InitChargeTime", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse init charge time: {}", e))
            }),
            ("Cost", |value| {
                value
                    .parse::<u32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse cost: {}", e))
            }),
            ("Range", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse range: {}", e))
            }),
            ("Radius", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse radius: {}", e))
            }),
            ("Damage", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse damage: {}", e))
            }),
            ("SharedSyncGroup", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("ViewObjectName", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("ViewObjectDuration", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse view object duration: {}", e))
            }),
            ("IconName", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("ButtonBorderType", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("Description", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("SoundEffect", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("Flags", |value| {
                value
                    .parse::<u32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse flags: {}", e))
            }),
        ]
    }

    /// Update template from properties
    pub fn update_from_properties(&mut self, properties: &HashMap<String, String>) {
        for (key, value) in properties {
            match key.as_str() {
                "Type" => {
                    self.power_type = SpecialPowerType::from_string(value);
                }
                "PrerequisiteScience" => {
                    self.prerequisite_science = value
                        .split_whitespace()
                        .map(|s| AsciiString::from(s))
                        .collect();
                }
                "RequiredScience" => {
                    self.required_science = value
                        .split_whitespace()
                        .map(|s| AsciiString::from(s))
                        .collect();
                }
                "RechargeTime" => {
                    if let Ok(time) = value.parse::<f32>() {
                        self.recharge_time = time;
                    }
                }
                "InitChargeTime" => {
                    if let Ok(time) = value.parse::<f32>() {
                        self.init_charge_time = time;
                    }
                }
                "Cost" => {
                    if let Ok(cost) = value.parse::<u32>() {
                        self.cost = cost;
                    }
                }
                "Range" => {
                    if let Ok(range) = value.parse::<f32>() {
                        self.range = range;
                    }
                }
                "Radius" => {
                    if let Ok(radius) = value.parse::<f32>() {
                        self.radius = radius;
                    }
                }
                "Damage" => {
                    if let Ok(damage) = value.parse::<f32>() {
                        self.damage = damage;
                    }
                }
                "SharedSyncGroup" => {
                    self.shared_sync_group = AsciiString::from(value);
                }
                "ViewObjectName" => {
                    self.view_object_name = AsciiString::from(value);
                }
                "ViewObjectDuration" => {
                    if let Ok(duration) = value.parse::<f32>() {
                        self.view_object_duration = duration;
                    }
                }
                "IconName" => {
                    self.icon_name = AsciiString::from(value);
                }
                "ButtonBorderType" => {
                    self.button_border_type = AsciiString::from(value);
                }
                "Description" => {
                    self.description = AsciiString::from(value);
                }
                "SoundEffect" => {
                    self.sound_effect = AsciiString::from(value);
                }
                "Flags" => {
                    if let Ok(flags) = value.parse::<u32>() {
                        self.flags = flags;
                    }
                }
                _ => {
                    // Store unknown properties
                    self.properties.insert(key.clone(), value.clone());
                }
            }
        }
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn is_valid(&self) -> bool {
        !self.name.is_empty() && self.recharge_time > 0.0
    }

    pub fn is_superweapon(&self) -> bool {
        matches!(
            self.power_type,
            SpecialPowerType::Superweapon | SpecialPowerType::Nuke
        )
    }

    pub fn has_prerequisite_science(&self, science: &AsciiString) -> bool {
        self.prerequisite_science.contains(science) || self.required_science.contains(science)
    }
}

/// Special power store - manages all special power templates
#[derive(Debug)]
pub struct SpecialPowerStore {
    templates: HashMap<String, SpecialPowerTemplate>,
}

impl SpecialPowerStore {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Find a template by name
    pub fn find_template(&self, name: &AsciiString) -> Option<&SpecialPowerTemplate> {
        self.templates.get(name.as_str())
    }

    /// Find a mutable template by name
    pub fn find_template_mut(&mut self, name: &AsciiString) -> Option<&mut SpecialPowerTemplate> {
        self.templates.get_mut(name.as_str())
    }

    /// Create a new template
    pub fn new_template(&mut self, name: AsciiString) -> &mut SpecialPowerTemplate {
        let template = SpecialPowerTemplate::new(name.clone());
        self.templates.insert(name.as_str().to_string(), template);
        self.templates.get_mut(name.as_str()).unwrap()
    }

    /// Get or create a template
    pub fn get_or_create_template(&mut self, name: &AsciiString) -> &mut SpecialPowerTemplate {
        if !self.templates.contains_key(name.as_str()) {
            self.new_template(name.clone());
        }
        self.templates.get_mut(name.as_str()).unwrap()
    }

    /// Register a template
    pub fn register_template(&mut self, template: SpecialPowerTemplate) {
        let name = template.name.as_str().to_string();
        self.templates.insert(name, template);
    }

    /// Get all template names
    pub fn get_template_names(&self) -> Vec<&String> {
        self.templates.keys().collect()
    }

    /// Get templates by type
    pub fn get_templates_by_type(
        &self,
        power_type: &SpecialPowerType,
    ) -> Vec<&SpecialPowerTemplate> {
        self.templates
            .values()
            .filter(|t| &t.power_type == power_type)
            .collect()
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

    /// Parse special power definition - equivalent to original parseSpecialPowerDefinition
    pub fn parse_special_power_definition(name: AsciiString) -> SpecialPowerResult<()> {
        // In the original C++, this would delegate to SpecialPowerStore::parseSpecialPowerDefinition
        println!("Parsing special power definition for: {}", name.as_str());
        Ok(())
    }
}

impl Default for SpecialPowerStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global special power store instance
static SPECIAL_POWER_STORE: OnceCell<RwLock<SpecialPowerStore>> = OnceCell::new();

fn special_power_store_cell() -> &'static RwLock<SpecialPowerStore> {
    SPECIAL_POWER_STORE.get_or_init(|| RwLock::new(SpecialPowerStore::new()))
}

fn special_power_store_mut() -> RwLockWriteGuard<'static, SpecialPowerStore> {
    special_power_store_cell()
        .write()
        .expect("SpecialPowerStore poisoned")
}

fn special_power_store() -> RwLockReadGuard<'static, SpecialPowerStore> {
    special_power_store_cell()
        .read()
        .expect("SpecialPowerStore poisoned")
}

/// Initialize the global special power store
pub fn initialize_special_power_store() {
    let _ = special_power_store_cell();
}

/// Get a reference to the global special power store
pub fn get_special_power_store() -> Option<RwLockReadGuard<'static, SpecialPowerStore>> {
    Some(special_power_store())
}

pub fn get_special_power_store_mut() -> Option<RwLockWriteGuard<'static, SpecialPowerStore>> {
    Some(special_power_store_mut())
}

/// INI parsing functions for special powers
pub struct IniSpecialPower;

impl IniSpecialPower {
    /// Parse special power definition - equivalent to INI::parseSpecialPowerDefinition
    pub fn parse_special_power_definition(name: AsciiString) -> SpecialPowerResult<()> {
        // Validate name
        if name.is_empty() {
            return Err(SpecialPowerError::InvalidName);
        }

        // Initialize store if needed
        initialize_special_power_store();

        // Delegate to SpecialPowerStore
        SpecialPowerStore::parse_special_power_definition(name)
    }

    /// Parse a complete special power block from INI data
    pub fn parse_special_power_block(
        name: AsciiString,
        properties: HashMap<String, String>,
    ) -> SpecialPowerResult<SpecialPowerTemplate> {
        // Validate name
        if name.is_empty() {
            return Err(SpecialPowerError::InvalidName);
        }

        // Create template
        let mut template = SpecialPowerTemplate::new(name);

        // Update template from properties
        template.update_from_properties(&properties);

        // Validate template
        if !template.is_valid() {
            return Err(SpecialPowerError::ParseError(
                "Invalid special power template configuration".to_string(),
            ));
        }

        Ok(template)
    }

    /// Register a special power template
    pub fn register_template(template: SpecialPowerTemplate) -> SpecialPowerResult<()> {
        initialize_special_power_store();

        let mut store = get_special_power_store_mut()
            .ok_or_else(|| SpecialPowerError::StoreError("Store not initialized".to_string()))?;

        store.register_template(template);
        Ok(())
    }

    /// Find a special power template by name
    pub fn find_template_by_name(name: &AsciiString) -> Option<SpecialPowerTemplate> {
        if let Some(store) = get_special_power_store() {
            store.find_template(name).cloned()
        } else {
            None
        }
    }

    /// Validate special power name format
    pub fn validate_name(name: &AsciiString) -> bool {
        !name.is_empty() && name.len() < 128 // Reasonable length limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_power_type_parsing() {
        assert_eq!(
            SpecialPowerType::from_string("airstrike"),
            SpecialPowerType::Airstrike
        );
        assert_eq!(
            SpecialPowerType::from_string("NUKE"),
            SpecialPowerType::Nuke
        );
        assert_eq!(
            SpecialPowerType::from_string("CustomPower"),
            SpecialPowerType::Custom("CustomPower".to_string())
        );
    }

    #[test]
    fn test_special_power_template_creation() {
        let name = AsciiString::from("TestSpecialPower");
        let template = SpecialPowerTemplate::new(name.clone());

        assert_eq!(template.name, name);
        assert_eq!(template.recharge_time, 30.0);
        assert_eq!(template.cost, 0);
        assert!(template.is_valid());
    }

    #[test]
    fn test_special_power_store() {
        let mut store = SpecialPowerStore::new();
        let name = AsciiString::from("TestPower");

        // Create new template
        let template = store.new_template(name.clone());
        template.power_type = SpecialPowerType::Airstrike;
        template.cost = 1000;

        // Find template
        let found = store.find_template(&name);
        assert!(found.is_some());
        assert_eq!(found.unwrap().cost, 1000);
        assert!(matches!(
            found.unwrap().power_type,
            SpecialPowerType::Airstrike
        ));

        // Count templates
        assert_eq!(store.get_template_count(), 1);
    }

    #[test]
    fn test_template_properties_update() {
        let mut template = SpecialPowerTemplate::new(AsciiString::from("Test"));
        let mut properties = HashMap::new();
        properties.insert("Type".to_string(), "Nuke".to_string());
        properties.insert("Cost".to_string(), "5000".to_string());
        properties.insert("RechargeTime".to_string(), "120.0".to_string());
        properties.insert("Damage".to_string(), "1000.0".to_string());

        template.update_from_properties(&properties);

        assert!(matches!(template.power_type, SpecialPowerType::Nuke));
        assert_eq!(template.cost, 5000);
        assert_eq!(template.recharge_time, 120.0);
        assert_eq!(template.damage, 1000.0);
        assert!(template.is_superweapon());
    }

    #[test]
    fn test_prerequisite_science() {
        let mut template = SpecialPowerTemplate::new(AsciiString::from("TestPower"));
        template
            .prerequisite_science
            .push(AsciiString::from("SCIENCE_NuclearReactor"));
        template
            .required_science
            .push(AsciiString::from("SCIENCE_AdvancedWeapons"));

        assert!(template.has_prerequisite_science(&AsciiString::from("SCIENCE_NuclearReactor")));
        assert!(template.has_prerequisite_science(&AsciiString::from("SCIENCE_AdvancedWeapons")));
        assert!(!template.has_prerequisite_science(&AsciiString::from("SCIENCE_BasicWeapons")));
    }

    #[test]
    fn test_validate_name() {
        assert!(IniSpecialPower::validate_name(&AsciiString::from(
            "ValidName"
        )));
        assert!(!IniSpecialPower::validate_name(&AsciiString::from("")));
    }
}
