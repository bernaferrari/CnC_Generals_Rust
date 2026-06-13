////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_upgrade.rs
//! Author: Colin Day, March 2002 (Converted to Rust)
//! Desc:   Upgrade database

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ascii_string::AsciiString;
use crate::common::rts::special_power::AcademyClassificationType;

/// Result type for upgrade parsing operations
pub type UpgradeResult<T> = Result<T, UpgradeError>;

/// Errors that can occur during upgrade parsing
#[derive(Debug, Clone, PartialEq)]
pub enum UpgradeError {
    InvalidName,
    InvalidType,
    ParseError(String),
    CenterError(String),
    NotFound,
    AlreadyExists,
}

impl std::fmt::Display for UpgradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpgradeError::InvalidName => write!(f, "Invalid upgrade name"),
            UpgradeError::InvalidType => write!(f, "Invalid upgrade type"),
            UpgradeError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            UpgradeError::CenterError(msg) => write!(f, "Upgrade center error: {}", msg),
            UpgradeError::NotFound => write!(f, "Upgrade not found"),
            UpgradeError::AlreadyExists => write!(f, "Upgrade already exists"),
        }
    }
}

impl std::error::Error for UpgradeError {}

/// Upgrade categories
#[derive(Debug, Clone, PartialEq)]
pub enum UpgradeCategory {
    Weapon,
    Armor,
    Speed,
    Health,
    Range,
    Accuracy,
    Technology,
    Economic,
    Special,
    Custom(String),
}

impl UpgradeCategory {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "weapon" => Self::Weapon,
            "armor" => Self::Armor,
            "speed" => Self::Speed,
            "health" => Self::Health,
            "range" => Self::Range,
            "accuracy" => Self::Accuracy,
            "technology" => Self::Technology,
            "economic" => Self::Economic,
            "special" => Self::Special,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Weapon => "Weapon",
            Self::Armor => "Armor",
            Self::Speed => "Speed",
            Self::Health => "Health",
            Self::Range => "Range",
            Self::Accuracy => "Accuracy",
            Self::Technology => "Technology",
            Self::Economic => "Economic",
            Self::Special => "Special",
            Self::Custom(name) => name,
        }
    }
}

/// C++ UpgradeType enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeType {
    Player,
    Object,
}

impl UpgradeType {
    pub fn from_string(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_uppercase().as_str() {
            "PLAYER" => Ok(Self::Player),
            "OBJECT" => Ok(Self::Object),
            _ => Err(format!("Invalid upgrade type: {}", s)),
        }
    }
}

/// Upgrade effect types
#[derive(Debug, Clone, PartialEq)]
pub enum UpgradeEffect {
    AddAttribute(String),
    RemoveAttribute(String),
    ModifyAttribute {
        name: String,
        multiplier: f32,
        additive: f32,
    },
    ReplaceWeapon {
        old_weapon: String,
        new_weapon: String,
    },
    AddModule(String),
    RemoveModule(String),
    ChangeModel(String),
    ChangeTexture(String),
    Custom {
        effect_type: String,
        parameters: HashMap<String, String>,
    },
}

/// Upgrade requirement
#[derive(Debug, Clone)]
pub struct UpgradeRequirement {
    pub prerequisite_science: Vec<AsciiString>,
    pub required_objects: Vec<AsciiString>,
    pub forbidden_objects: Vec<AsciiString>,
    pub min_player_level: u32,
    pub cost: u32,
    pub research_time: f32,
}

impl Default for UpgradeRequirement {
    fn default() -> Self {
        Self {
            prerequisite_science: Vec::new(),
            required_objects: Vec::new(),
            forbidden_objects: Vec::new(),
            min_player_level: 0,
            cost: 0,
            research_time: 0.0,
        }
    }
}

/// Upgrade template definition
#[derive(Debug, Clone)]
pub struct UpgradeTemplate {
    pub name: AsciiString,
    pub display_name: AsciiString,
    pub description: AsciiString,
    pub upgrade_mask: u128,
    pub upgrade_type: UpgradeType,
    pub category: UpgradeCategory,
    pub requirements: UpgradeRequirement,
    pub effects: Vec<UpgradeEffect>,
    pub icon_name: AsciiString,
    pub button_image: AsciiString,
    pub research_sound: AsciiString,
    pub unit_specific_sound: AsciiString,
    pub sound_effect: AsciiString,
    pub academy_classification_type: AcademyClassificationType,
    pub is_purchasable: bool,
    pub is_stackable: bool,
    pub max_stack_count: u32,
    pub affects_all_of_type: bool,
    pub affects_existing_objects: bool,
    pub properties: HashMap<String, String>,
}

impl UpgradeTemplate {
    pub fn new(name: AsciiString) -> Self {
        Self {
            name,
            display_name: AsciiString::from(""),
            description: AsciiString::from(""),
            upgrade_mask: 0,
            upgrade_type: UpgradeType::Player,
            category: UpgradeCategory::Custom("Unknown".to_string()),
            requirements: UpgradeRequirement::default(),
            effects: Vec::new(),
            icon_name: AsciiString::from(""),
            button_image: AsciiString::from(""),
            research_sound: AsciiString::from(""),
            unit_specific_sound: AsciiString::from(""),
            sound_effect: AsciiString::from(""),
            academy_classification_type: AcademyClassificationType::None,
            is_purchasable: true,
            is_stackable: false,
            max_stack_count: 1,
            affects_all_of_type: false,
            affects_existing_objects: true,
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
            ("DisplayName", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("Type", |value| {
                UpgradeType::from_string(value)
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse upgrade type: {}", e))
            }),
            ("BuildCost", |value| {
                value
                    .parse::<u32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse build cost: {}", e))
            }),
            ("BuildTime", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse build time: {}", e))
            }),
            ("ButtonImage", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("ResearchSound", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("UnitSpecificSound", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("AcademyClassify", |value| {
                parse_academy_classification(value)
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse academy classification: {}", e))
            }),
        ]
    }

    /// Update template from properties
    pub fn update_from_properties(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> UpgradeResult<()> {
        for (key, value) in properties {
            match key.as_str() {
                "DisplayName" => {
                    self.display_name = AsciiString::from(value);
                }
                "Type" => {
                    self.upgrade_type =
                        UpgradeType::from_string(value).map_err(UpgradeError::ParseError)?;
                }
                "BuildCost" => {
                    self.requirements.cost = value.parse::<u32>().map_err(|e| {
                        UpgradeError::ParseError(format!("Invalid build cost '{}': {}", value, e))
                    })?;
                }
                "BuildTime" => {
                    self.requirements.research_time = value.parse::<f32>().map_err(|e| {
                        UpgradeError::ParseError(format!("Invalid build time '{}': {}", value, e))
                    })?;
                }
                "ButtonImage" => {
                    self.button_image = AsciiString::from(value);
                }
                "ResearchSound" => {
                    self.research_sound = AsciiString::from(value);
                }
                "UnitSpecificSound" => {
                    self.unit_specific_sound = AsciiString::from(value);
                }
                "AcademyClassify" => {
                    self.academy_classification_type =
                        parse_academy_classification(value).map_err(UpgradeError::ParseError)?;
                }
                _ => {
                    return Err(UpgradeError::ParseError(format!(
                        "Unknown upgrade field '{}'",
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

    pub fn get_upgrade_mask(&self) -> u128 {
        self.upgrade_mask
    }

    pub fn is_valid(&self) -> bool {
        !self.name.is_empty()
    }

    pub fn can_be_researched(&self) -> bool {
        self.is_purchasable && self.requirements.cost > 0
    }

    pub fn add_effect(&mut self, effect: UpgradeEffect) {
        self.effects.push(effect);
    }

    pub fn has_prerequisite_science(&self, science: &AsciiString) -> bool {
        self.requirements.prerequisite_science.contains(science)
    }

    pub fn requires_object(&self, object: &AsciiString) -> bool {
        self.requirements.required_objects.contains(object)
    }
}

/// Upgrade center - manages all upgrade templates and research
#[derive(Debug)]
pub struct UpgradeCenter {
    templates: HashMap<String, UpgradeTemplate>,
    template_order: Vec<String>,
    next_template_mask_bit: usize,
    researched_upgrades: HashMap<String, u32>, // Name -> stack count
}

impl UpgradeCenter {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            template_order: Vec::new(),
            next_template_mask_bit: 0,
            researched_upgrades: HashMap::new(),
        }
    }

    fn assign_new_template_mask(&mut self, template: &mut UpgradeTemplate) {
        if self.next_template_mask_bit >= 128 {
            panic!("Can't have over 128 types of Upgrades and have a Bitfield function.");
        }

        template.upgrade_mask = 1u128 << self.next_template_mask_bit;
        self.next_template_mask_bit += 1;
    }

    /// Find a template by name
    pub fn find_template(&self, name: &AsciiString) -> Option<&UpgradeTemplate> {
        self.templates.get(name.as_str())
    }

    /// Find a mutable template by name
    pub fn find_template_mut(&mut self, name: &AsciiString) -> Option<&mut UpgradeTemplate> {
        self.templates.get_mut(name.as_str())
    }

    /// Create a new template
    pub fn new_template(&mut self, name: AsciiString) -> &mut UpgradeTemplate {
        let key = name.as_str().to_string();
        let mut template = UpgradeTemplate::new(name.clone());
        if let Some(existing) = self.templates.get(&key) {
            template.upgrade_mask = existing.upgrade_mask;
        } else {
            self.assign_new_template_mask(&mut template);
            self.template_order.insert(0, key.clone());
        }
        self.templates.insert(key, template);
        self.templates.get_mut(name.as_str()).unwrap()
    }

    /// Get or create a template
    pub fn get_or_create_template(&mut self, name: &AsciiString) -> &mut UpgradeTemplate {
        if !self.templates.contains_key(name.as_str()) {
            self.new_template(name.clone());
        }
        self.templates.get_mut(name.as_str()).unwrap()
    }

    /// Register a template
    pub fn register_template(&mut self, mut template: UpgradeTemplate) {
        let name = template.name.as_str().to_string();
        if let Some(existing) = self.templates.get(&name) {
            template.upgrade_mask = existing.upgrade_mask;
        } else {
            self.assign_new_template_mask(&mut template);
            self.template_order.insert(0, name.clone());
        }
        self.templates.insert(name, template);
    }

    /// Get all template names
    pub fn get_template_names(&self) -> Vec<&String> {
        self.template_order
            .iter()
            .filter(|name| self.templates.contains_key(name.as_str()))
            .collect()
    }

    /// Get templates by category
    pub fn get_templates_by_category(&self, category: &UpgradeCategory) -> Vec<&UpgradeTemplate> {
        self.template_order
            .iter()
            .filter_map(|name| self.templates.get(name.as_str()))
            .filter(|t| &t.category == category)
            .collect()
    }

    /// Research an upgrade
    pub fn research_upgrade(&mut self, name: &AsciiString) -> UpgradeResult<()> {
        let template = self.find_template(name).ok_or(UpgradeError::NotFound)?;

        if !template.can_be_researched() {
            return Err(UpgradeError::ParseError(
                "Upgrade cannot be researched".to_string(),
            ));
        }

        let current_count = *self.researched_upgrades.get(name.as_str()).unwrap_or(&0);

        if !template.is_stackable && current_count > 0 {
            return Err(UpgradeError::AlreadyExists);
        }

        if current_count >= template.max_stack_count {
            return Err(UpgradeError::ParseError(
                "Max stack count reached".to_string(),
            ));
        }

        self.researched_upgrades
            .insert(name.as_str().to_string(), current_count + 1);
        Ok(())
    }

    /// Check if an upgrade is researched
    pub fn is_upgrade_researched(&self, name: &AsciiString) -> bool {
        self.researched_upgrades.contains_key(name.as_str())
    }

    /// Get research count for an upgrade
    pub fn get_research_count(&self, name: &AsciiString) -> u32 {
        *self.researched_upgrades.get(name.as_str()).unwrap_or(&0)
    }

    /// Remove a template
    pub fn remove_template(&mut self, name: &AsciiString) -> bool {
        let removed = self.templates.remove(name.as_str()).is_some();
        if removed {
            self.template_order
                .retain(|template_name| template_name != name.as_str());
        }
        removed
    }

    /// Clear all templates
    pub fn clear(&mut self) {
        self.templates.clear();
        self.template_order.clear();
        self.next_template_mask_bit = 0;
        self.researched_upgrades.clear();
    }

    /// Get template count
    pub fn get_template_count(&self) -> usize {
        self.templates.len()
    }

    /// Parse upgrade definition - equivalent to original parseUpgradeDefinition
    pub fn parse_upgrade_definition(name: AsciiString) -> UpgradeResult<()> {
        // In the original C++, this would delegate to UpgradeCenter::parseUpgradeDefinition
        println!("Parsing upgrade definition for: {}", name.as_str());
        Ok(())
    }
}

impl Default for UpgradeCenter {
    fn default() -> Self {
        Self::new()
    }
}

/// Global upgrade center instance
static UPGRADE_CENTER: OnceCell<RwLock<UpgradeCenter>> = OnceCell::new();

/// Initialize the global upgrade center
pub fn initialize_upgrade_center() {
    if UPGRADE_CENTER.get().is_none() {
        let _ = UPGRADE_CENTER.set(RwLock::new(UpgradeCenter::new()));
    }
}

/// Get a reference to the global upgrade center
pub fn get_upgrade_center() -> Option<RwLockReadGuard<'static, UpgradeCenter>> {
    UPGRADE_CENTER
        .get()
        .map(|center| center.read().expect("UpgradeCenter poisoned"))
}

pub fn get_upgrade_center_mut() -> Option<RwLockWriteGuard<'static, UpgradeCenter>> {
    UPGRADE_CENTER
        .get()
        .map(|center| center.write().expect("UpgradeCenter poisoned"))
}

/// Parse a boolean value from string
pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

pub fn parse_academy_classification(value: &str) -> Result<AcademyClassificationType, String> {
    match value.trim().to_ascii_uppercase().as_str() {
        "ACT_NONE" => Ok(AcademyClassificationType::None),
        "ACT_UPGRADE_RADAR" => Ok(AcademyClassificationType::UpgradeRadar),
        "ACT_SUPERPOWER" => Ok(AcademyClassificationType::Superpower),
        _ => Err(format!(
            "token {} is not a valid member of the academy classification list",
            value
        )),
    }
}

/// INI parsing functions for upgrades
pub struct IniUpgrade;

impl IniUpgrade {
    /// Parse upgrade definition - equivalent to INI::parseUpgradeDefinition
    pub fn parse_upgrade_definition(name: AsciiString) -> UpgradeResult<()> {
        // Validate name
        if name.is_empty() {
            return Err(UpgradeError::InvalidName);
        }

        // Initialize upgrade center if needed
        initialize_upgrade_center();

        // Delegate to UpgradeCenter
        UpgradeCenter::parse_upgrade_definition(name)
    }

    /// Parse a complete upgrade block from INI data
    pub fn parse_upgrade_block(
        name: AsciiString,
        properties: HashMap<String, String>,
    ) -> UpgradeResult<UpgradeTemplate> {
        // Validate name
        if name.is_empty() {
            return Err(UpgradeError::InvalidName);
        }

        // Create template
        let mut template = UpgradeTemplate::new(name);

        // Update template from properties
        template.update_from_properties(&properties)?;

        // Validate template
        if !template.is_valid() {
            return Err(UpgradeError::ParseError(
                "Invalid upgrade template configuration".to_string(),
            ));
        }

        Ok(template)
    }

    /// Register an upgrade template
    pub fn register_template(template: UpgradeTemplate) -> UpgradeResult<()> {
        initialize_upgrade_center();

        let mut center = get_upgrade_center_mut()
            .ok_or_else(|| UpgradeError::CenterError("Center not initialized".to_string()))?;

        center.register_template(template);
        Ok(())
    }

    /// Find an upgrade template by name
    pub fn find_template_by_name(name: &AsciiString) -> Option<UpgradeTemplate> {
        if let Some(center) = get_upgrade_center() {
            center.find_template(name).cloned()
        } else {
            None
        }
    }

    /// Research an upgrade
    pub fn research_upgrade(name: &AsciiString) -> UpgradeResult<()> {
        initialize_upgrade_center();

        let mut center = get_upgrade_center_mut()
            .ok_or_else(|| UpgradeError::CenterError("Center not initialized".to_string()))?;

        center.research_upgrade(name)
    }

    /// Validate upgrade name format
    pub fn validate_name(name: &AsciiString) -> bool {
        !name.is_empty() && name.len() < 128 // Reasonable length limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upgrade_category_parsing() {
        assert_eq!(
            UpgradeCategory::from_string("weapon"),
            UpgradeCategory::Weapon
        );
        assert_eq!(
            UpgradeCategory::from_string("ARMOR"),
            UpgradeCategory::Armor
        );
        assert_eq!(
            UpgradeCategory::from_string("CustomType"),
            UpgradeCategory::Custom("CustomType".to_string())
        );
    }

    #[test]
    fn test_upgrade_template_creation() {
        let name = AsciiString::from("TestUpgrade");
        let template = UpgradeTemplate::new(name.clone());

        assert_eq!(template.name, name);
        assert!(template.is_purchasable);
        assert!(!template.is_stackable);
        assert_eq!(template.max_stack_count, 1);
        assert!(template.is_valid());
    }

    #[test]
    fn test_upgrade_center() {
        let mut center = UpgradeCenter::new();
        let name = AsciiString::from("TestUpgrade");

        // Create new template
        let template = center.new_template(name.clone());
        template.category = UpgradeCategory::Weapon;
        template.requirements.cost = 500;

        // Find template
        let found = center.find_template(&name);
        assert!(found.is_some());
        assert_eq!(found.unwrap().requirements.cost, 500);
        assert!(matches!(found.unwrap().category, UpgradeCategory::Weapon));

        // Research upgrade
        let result = center.research_upgrade(&name);
        assert!(result.is_ok());
        assert!(center.is_upgrade_researched(&name));
        assert_eq!(center.get_research_count(&name), 1);

        // Count templates
        assert_eq!(center.get_template_count(), 1);
    }

    #[test]
    fn upgrade_center_assigns_cpp_style_unique_masks() {
        let mut center = UpgradeCenter::new();

        let first_mask = center
            .new_template(AsciiString::from("MaskUpgradeA"))
            .get_upgrade_mask();
        let second_mask = center
            .new_template(AsciiString::from("MaskUpgradeB"))
            .get_upgrade_mask();
        let third_mask = center
            .new_template(AsciiString::from("MaskUpgradeC"))
            .get_upgrade_mask();

        assert_eq!(first_mask, 1u128 << 0);
        assert_eq!(second_mask, 1u128 << 1);
        assert_eq!(third_mask, 1u128 << 2);
    }

    #[test]
    fn upgrade_center_preserves_mask_when_template_is_reparsed() {
        let mut center = UpgradeCenter::new();
        let name = AsciiString::from("ReparsedUpgrade");
        let original_mask = center.new_template(name.clone()).get_upgrade_mask();

        let mut replacement = UpgradeTemplate::new(name.clone());
        replacement.requirements.cost = 1000;
        center.register_template(replacement);

        let reparsed = center.find_template(&name).unwrap();
        assert_eq!(reparsed.get_upgrade_mask(), original_mask);
        assert_eq!(reparsed.requirements.cost, 1000);

        let next_mask = center
            .new_template(AsciiString::from("AfterReparsedUpgrade"))
            .get_upgrade_mask();
        assert_eq!(next_mask, 1u128 << 1);
    }

    #[test]
    fn upgrade_center_enumerates_in_cpp_list_order() {
        let mut center = UpgradeCenter::new();

        let mut first = UpgradeTemplate::new(AsciiString::from("FirstUpgrade"));
        first.category = UpgradeCategory::Weapon;
        let mut second = UpgradeTemplate::new(AsciiString::from("SecondUpgrade"));
        second.category = UpgradeCategory::Armor;
        let mut third = UpgradeTemplate::new(AsciiString::from("ThirdUpgrade"));
        third.category = UpgradeCategory::Weapon;

        center.register_template(first);
        center.register_template(second);
        center.register_template(third);

        let names: Vec<&str> = center
            .get_template_names()
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(names, vec!["ThirdUpgrade", "SecondUpgrade", "FirstUpgrade"]);

        let weapon_names: Vec<&str> = center
            .get_templates_by_category(&UpgradeCategory::Weapon)
            .into_iter()
            .map(|template| template.name.as_str())
            .collect();
        assert_eq!(weapon_names, vec!["ThirdUpgrade", "FirstUpgrade"]);

        let mut replacement = UpgradeTemplate::new(AsciiString::from("SecondUpgrade"));
        replacement.category = UpgradeCategory::Weapon;
        center.register_template(replacement);

        let names_after_override: Vec<&str> = center
            .get_template_names()
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(
            names_after_override,
            vec!["ThirdUpgrade", "SecondUpgrade", "FirstUpgrade"]
        );
    }

    #[test]
    fn test_upgrade_effects() {
        let mut template = UpgradeTemplate::new(AsciiString::from("TestUpgrade"));

        template.add_effect(UpgradeEffect::AddAttribute("VETERANCY_BONUS".to_string()));
        template.add_effect(UpgradeEffect::ModifyAttribute {
            name: "Damage".to_string(),
            multiplier: 1.25,
            additive: 0.0,
        });

        assert_eq!(template.effects.len(), 2);

        if let UpgradeEffect::AddAttribute(attr) = &template.effects[0] {
            assert_eq!(attr, "VETERANCY_BONUS");
        } else {
            panic!("Wrong effect type");
        }
    }

    #[test]
    fn test_template_properties_update() {
        let mut template = UpgradeTemplate::new(AsciiString::from("Test"));
        let mut properties = HashMap::new();
        properties.insert("Type".to_string(), "OBJECT".to_string());
        properties.insert("BuildCost".to_string(), "1000".to_string());
        properties.insert("BuildTime".to_string(), "12.5".to_string());
        properties.insert("ButtonImage".to_string(), "SSRadar".to_string());
        properties.insert("ResearchSound".to_string(), "UpgradeStarted".to_string());
        properties.insert("UnitSpecificSound".to_string(), "UnitUpgrade".to_string());
        properties.insert(
            "AcademyClassify".to_string(),
            "ACT_UPGRADE_RADAR".to_string(),
        );

        template.update_from_properties(&properties).unwrap();

        assert_eq!(template.upgrade_type, UpgradeType::Object);
        assert_eq!(template.requirements.cost, 1000);
        assert_eq!(template.requirements.research_time, 12.5);
        assert_eq!(template.button_image.as_str(), "SSRadar");
        assert_eq!(template.research_sound.as_str(), "UpgradeStarted");
        assert_eq!(template.unit_specific_sound.as_str(), "UnitUpgrade");
        assert_eq!(
            template.academy_classification_type,
            AcademyClassificationType::UpgradeRadar
        );
    }

    #[test]
    fn upgrade_block_rejects_fields_outside_cpp_parse_table() {
        for field in [
            "Description",
            "Category",
            "PrerequisiteScience",
            "RequiredObjects",
            "Cost",
            "ResearchTime",
            "IconName",
            "SoundEffect",
            "IsPurchasable",
            "IsStackable",
            "MaxStackCount",
            "AffectsAllOfType",
            "AffectsExistingObjects",
            "UnknownField",
        ] {
            let mut props = HashMap::new();
            props.insert(field.to_string(), "1".to_string());
            assert!(
                IniUpgrade::parse_upgrade_block(AsciiString::from("BadField"), props).is_err(),
                "{} should be rejected because C++ UpgradeTemplate does not parse it",
                field
            );
        }
    }

    #[test]
    fn upgrade_block_accepts_cpp_field_table_fields() {
        let mut props = HashMap::new();
        props.insert(
            "DisplayName".to_string(),
            "CONTROLBAR:UpgradeRadar".to_string(),
        );
        props.insert("Type".to_string(), "PLAYER".to_string());
        props.insert("BuildTime".to_string(), "15.0".to_string());
        props.insert("BuildCost".to_string(), "500".to_string());
        props.insert("ButtonImage".to_string(), "SSRadar".to_string());
        props.insert(
            "ResearchSound".to_string(),
            "UpgradeRadarComplete".to_string(),
        );
        props.insert(
            "UnitSpecificSound".to_string(),
            "UnitSpecificUpgrade".to_string(),
        );
        props.insert(
            "AcademyClassify".to_string(),
            "ACT_UPGRADE_RADAR".to_string(),
        );

        let template =
            IniUpgrade::parse_upgrade_block(AsciiString::from("Upgrade_AmericaRadar"), props)
                .unwrap();

        assert_eq!(template.display_name.as_str(), "CONTROLBAR:UpgradeRadar");
        assert_eq!(template.upgrade_type, UpgradeType::Player);
        assert_eq!(template.requirements.research_time, 15.0);
        assert_eq!(template.requirements.cost, 500);
        assert_eq!(template.button_image.as_str(), "SSRadar");
        assert_eq!(template.research_sound.as_str(), "UpgradeRadarComplete");
        assert_eq!(template.unit_specific_sound.as_str(), "UnitSpecificUpgrade");
        assert_eq!(
            template.academy_classification_type,
            AcademyClassificationType::UpgradeRadar
        );
    }

    #[test]
    fn test_upgrade_type_and_academy_parsing() {
        assert_eq!(UpgradeType::from_string("PLAYER"), Ok(UpgradeType::Player));
        assert_eq!(UpgradeType::from_string("object"), Ok(UpgradeType::Object));
        assert!(UpgradeType::from_string("GLOBAL").is_err());

        assert_eq!(
            parse_academy_classification("ACT_SUPERPOWER"),
            Ok(AcademyClassificationType::Superpower)
        );
        assert!(parse_academy_classification("SUPERWEAPON").is_err());
    }

    #[test]
    fn upgrade_block_rejects_invalid_cpp_field_values() {
        let mut props = HashMap::new();
        props.insert("Type".to_string(), "GLOBAL".to_string());
        assert!(IniUpgrade::parse_upgrade_block(AsciiString::from("BadType"), props).is_err());

        let mut props = HashMap::new();
        props.insert("BuildCost".to_string(), "expensive".to_string());
        assert!(IniUpgrade::parse_upgrade_block(AsciiString::from("BadCost"), props).is_err());

        let mut props = HashMap::new();
        props.insert("BuildTime".to_string(), "soon".to_string());
        assert!(IniUpgrade::parse_upgrade_block(AsciiString::from("BadTime"), props).is_err());

        let mut props = HashMap::new();
        props.insert("AcademyClassify".to_string(), "SUPERWEAPON".to_string());
        assert!(IniUpgrade::parse_upgrade_block(AsciiString::from("BadAcademy"), props).is_err());
    }

    #[test]
    fn test_stackable_research() {
        let mut center = UpgradeCenter::new();
        let name = AsciiString::from("StackableUpgrade");

        let template = center.new_template(name.clone());
        template.is_stackable = true;
        template.max_stack_count = 3;
        template.requirements.cost = 100;

        // Research multiple times
        assert!(center.research_upgrade(&name).is_ok());
        assert_eq!(center.get_research_count(&name), 1);

        assert!(center.research_upgrade(&name).is_ok());
        assert_eq!(center.get_research_count(&name), 2);

        assert!(center.research_upgrade(&name).is_ok());
        assert_eq!(center.get_research_count(&name), 3);

        // Should fail on 4th attempt
        assert!(center.research_upgrade(&name).is_err());
    }

    #[test]
    fn test_prerequisite_science() {
        let mut template = UpgradeTemplate::new(AsciiString::from("TestUpgrade"));
        template
            .requirements
            .prerequisite_science
            .push(AsciiString::from("SCIENCE_AdvancedWeapons"));
        template
            .requirements
            .required_objects
            .push(AsciiString::from("AmericaWarFactory"));

        assert!(template.has_prerequisite_science(&AsciiString::from("SCIENCE_AdvancedWeapons")));
        assert!(!template.has_prerequisite_science(&AsciiString::from("SCIENCE_BasicWeapons")));
        assert!(template.requires_object(&AsciiString::from("AmericaWarFactory")));
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
        assert!(IniUpgrade::validate_name(&AsciiString::from("ValidName")));
        assert!(!IniUpgrade::validate_name(&AsciiString::from("")));
    }
}
