////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_weapon.rs
//! Author: Colin Day, November 2001 (Converted to Rust)
//! Desc:   Parsing Weapon INI entries

use crate::common::ascii_string::AsciiString;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Result type for weapon parsing operations
pub type WeaponResult<T> = Result<T, WeaponError>;

/// Errors that can occur during weapon parsing
#[derive(Debug, Clone, PartialEq)]
pub enum WeaponError {
    InvalidName,
    InvalidType,
    ParseError(String),
    StoreError(String),
    NotFound,
    AlreadyExists,
}

impl std::fmt::Display for WeaponError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeaponError::InvalidName => write!(f, "Invalid weapon name"),
            WeaponError::InvalidType => write!(f, "Invalid weapon type"),
            WeaponError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            WeaponError::StoreError(msg) => write!(f, "Weapon store error: {}", msg),
            WeaponError::NotFound => write!(f, "Weapon not found"),
            WeaponError::AlreadyExists => write!(f, "Weapon already exists"),
        }
    }
}

impl std::error::Error for WeaponError {}

/// Weapon damage types
#[derive(Debug, Clone, PartialEq)]
pub enum DamageType {
    Physical,
    Explosive,
    Fire,
    Chemical,
    Electrical,
    Radiation,
    Laser,
    Plasma,
    Kinetic,
    Armor,
    Structure,
    Custom(String),
}

impl DamageType {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "physical" => Self::Physical,
            "explosive" => Self::Explosive,
            "fire" => Self::Fire,
            "chemical" => Self::Chemical,
            "electrical" => Self::Electrical,
            "radiation" => Self::Radiation,
            "laser" => Self::Laser,
            "plasma" => Self::Plasma,
            "kinetic" => Self::Kinetic,
            "armor" => Self::Armor,
            "structure" => Self::Structure,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Physical => "Physical",
            Self::Explosive => "Explosive",
            Self::Fire => "Fire",
            Self::Chemical => "Chemical",
            Self::Electrical => "Electrical",
            Self::Radiation => "Radiation",
            Self::Laser => "Laser",
            Self::Plasma => "Plasma",
            Self::Kinetic => "Kinetic",
            Self::Armor => "Armor",
            Self::Structure => "Structure",
            Self::Custom(name) => name,
        }
    }
}

/// Weapon attack types
#[derive(Debug, Clone, PartialEq)]
pub enum AttackType {
    Direct,
    Area,
    Projectile,
    Beam,
    Hitscan,
    Guided,
    Ballistic,
    Custom(String),
}

impl AttackType {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "direct" => Self::Direct,
            "area" => Self::Area,
            "projectile" => Self::Projectile,
            "beam" => Self::Beam,
            "hitscan" => Self::Hitscan,
            "guided" => Self::Guided,
            "ballistic" => Self::Ballistic,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Direct => "Direct",
            Self::Area => "Area",
            Self::Projectile => "Projectile",
            Self::Beam => "Beam",
            Self::Hitscan => "Hitscan",
            Self::Guided => "Guided",
            Self::Ballistic => "Ballistic",
            Self::Custom(name) => name,
        }
    }
}

/// Weapon firing effects
#[derive(Debug, Clone)]
pub struct FiringEffects {
    pub muzzle_flash: AsciiString,
    pub projectile_object: AsciiString,
    pub hit_effect: AsciiString,
    pub miss_effect: AsciiString,
    pub sound_effect: AsciiString,
    pub tracer_effect: AsciiString,
}

impl Default for FiringEffects {
    fn default() -> Self {
        Self {
            muzzle_flash: AsciiString::from(""),
            projectile_object: AsciiString::from(""),
            hit_effect: AsciiString::from(""),
            miss_effect: AsciiString::from(""),
            sound_effect: AsciiString::from(""),
            tracer_effect: AsciiString::from(""),
        }
    }
}

/// Weapon template definition
#[derive(Debug, Clone)]
pub struct WeaponTemplate {
    pub name: AsciiString,
    pub display_name: AsciiString,
    pub damage_type: DamageType,
    pub attack_type: AttackType,
    pub primary_damage: f32,
    pub secondary_damage: f32,
    pub damage_radius: f32,
    pub range: f32,
    pub min_range: f32,
    pub rate_of_fire: f32,
    pub reload_time: f32,
    pub accuracy: f32,
    pub projectile_speed: f32,
    pub projectile_count: u32,
    pub ammo_capacity: u32,
    pub penetration: f32,
    pub armor_piercing: f32,
    pub can_target_air: bool,
    pub can_target_ground: bool,
    pub can_target_water: bool,
    pub can_target_stealth: bool,
    pub can_fire_while_moving: bool,
    pub requires_los: bool, // Line of sight
    pub effects: FiringEffects,
    pub projectile_template: AsciiString,
    pub damage_fx_template: AsciiString,
    pub prerequisites: Vec<AsciiString>,
    pub properties: HashMap<String, String>,
}

impl WeaponTemplate {
    pub fn new(name: AsciiString) -> Self {
        Self {
            name,
            display_name: AsciiString::from(""),
            damage_type: DamageType::Physical,
            attack_type: AttackType::Direct,
            primary_damage: 10.0,
            secondary_damage: 0.0,
            damage_radius: 0.0,
            range: 100.0,
            min_range: 0.0,
            rate_of_fire: 1.0,
            reload_time: 1.0,
            accuracy: 1.0,
            projectile_speed: 500.0,
            projectile_count: 1,
            ammo_capacity: 0, // 0 = unlimited
            penetration: 0.0,
            armor_piercing: 1.0,
            can_target_air: true,
            can_target_ground: true,
            can_target_water: false,
            can_target_stealth: false,
            can_fire_while_moving: false,
            requires_los: true,
            effects: FiringEffects::default(),
            projectile_template: AsciiString::from(""),
            damage_fx_template: AsciiString::from(""),
            prerequisites: Vec::new(),
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
            ("DamageType", |value| {
                Ok(Box::new(DamageType::from_string(value)) as Box<dyn std::any::Any>)
            }),
            ("AttackType", |value| {
                Ok(Box::new(AttackType::from_string(value)) as Box<dyn std::any::Any>)
            }),
            ("PrimaryDamage", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse primary damage: {}", e))
            }),
            ("SecondaryDamage", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse secondary damage: {}", e))
            }),
            ("DamageRadius", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse damage radius: {}", e))
            }),
            ("Range", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse range: {}", e))
            }),
            ("MinRange", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse min range: {}", e))
            }),
            ("RateOfFire", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse rate of fire: {}", e))
            }),
            ("ReloadTime", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse reload time: {}", e))
            }),
            ("Accuracy", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v.clamp(0.0, 1.0)) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse accuracy: {}", e))
            }),
            ("ProjectileSpeed", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse projectile speed: {}", e))
            }),
            ("ProjectileCount", |value| {
                value
                    .parse::<u32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse projectile count: {}", e))
            }),
            ("AmmoCapacity", |value| {
                value
                    .parse::<u32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse ammo capacity: {}", e))
            }),
            ("Penetration", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse penetration: {}", e))
            }),
            ("ArmorPiercing", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse armor piercing: {}", e))
            }),
            ("CanTargetAir", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse can target air: {}", e))
            }),
            ("CanTargetGround", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse can target ground: {}", e))
            }),
            ("CanTargetWater", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse can target water: {}", e))
            }),
            ("CanTargetStealth", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse can target stealth: {}", e))
            }),
            ("CanFireWhileMoving", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse can fire while moving: {}", e))
            }),
            ("RequiresLOS", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse requires LOS: {}", e))
            }),
            ("MuzzleFlash", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("ProjectileObject", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("HitEffect", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("SoundEffect", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("ProjectileTemplate", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("DamageFXTemplate", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("Prerequisites", |value| {
                let prereqs: Vec<AsciiString> = value
                    .split_whitespace()
                    .map(|s| AsciiString::from(s))
                    .collect();
                Ok(Box::new(prereqs) as Box<dyn std::any::Any>)
            }),
        ]
    }

    /// Update template from properties
    pub fn update_from_properties(&mut self, properties: &HashMap<String, String>) {
        for (key, value) in properties {
            match key.as_str() {
                "DisplayName" => {
                    self.display_name = AsciiString::from(value);
                }
                "DamageType" => {
                    self.damage_type = DamageType::from_string(value);
                }
                "AttackType" => {
                    self.attack_type = AttackType::from_string(value);
                }
                "PrimaryDamage" => {
                    if let Ok(damage) = value.parse::<f32>() {
                        self.primary_damage = damage;
                    }
                }
                "SecondaryDamage" => {
                    if let Ok(damage) = value.parse::<f32>() {
                        self.secondary_damage = damage;
                    }
                }
                "DamageRadius" => {
                    if let Ok(radius) = value.parse::<f32>() {
                        self.damage_radius = radius;
                    }
                }
                "Range" => {
                    if let Ok(range) = value.parse::<f32>() {
                        self.range = range;
                    }
                }
                "MinRange" => {
                    if let Ok(range) = value.parse::<f32>() {
                        self.min_range = range;
                    }
                }
                "RateOfFire" => {
                    if let Ok(rate) = value.parse::<f32>() {
                        self.rate_of_fire = rate;
                    }
                }
                "ReloadTime" => {
                    if let Ok(time) = value.parse::<f32>() {
                        self.reload_time = time;
                    }
                }
                "Accuracy" => {
                    if let Ok(accuracy) = value.parse::<f32>() {
                        self.accuracy = accuracy.clamp(0.0, 1.0);
                    }
                }
                "ProjectileSpeed" => {
                    if let Ok(speed) = value.parse::<f32>() {
                        self.projectile_speed = speed;
                    }
                }
                "ProjectileCount" => {
                    if let Ok(count) = value.parse::<u32>() {
                        self.projectile_count = count;
                    }
                }
                "AmmoCapacity" => {
                    if let Ok(capacity) = value.parse::<u32>() {
                        self.ammo_capacity = capacity;
                    }
                }
                "Penetration" => {
                    if let Ok(penetration) = value.parse::<f32>() {
                        self.penetration = penetration;
                    }
                }
                "ArmorPiercing" => {
                    if let Ok(piercing) = value.parse::<f32>() {
                        self.armor_piercing = piercing;
                    }
                }
                "CanTargetAir" => {
                    if let Ok(can_target) = parse_bool(value) {
                        self.can_target_air = can_target;
                    }
                }
                "CanTargetGround" => {
                    if let Ok(can_target) = parse_bool(value) {
                        self.can_target_ground = can_target;
                    }
                }
                "CanTargetWater" => {
                    if let Ok(can_target) = parse_bool(value) {
                        self.can_target_water = can_target;
                    }
                }
                "CanTargetStealth" => {
                    if let Ok(can_target) = parse_bool(value) {
                        self.can_target_stealth = can_target;
                    }
                }
                "CanFireWhileMoving" => {
                    if let Ok(can_fire) = parse_bool(value) {
                        self.can_fire_while_moving = can_fire;
                    }
                }
                "RequiresLOS" => {
                    if let Ok(requires) = parse_bool(value) {
                        self.requires_los = requires;
                    }
                }
                "MuzzleFlash" => {
                    self.effects.muzzle_flash = AsciiString::from(value);
                }
                "ProjectileObject" => {
                    self.effects.projectile_object = AsciiString::from(value);
                }
                "HitEffect" => {
                    self.effects.hit_effect = AsciiString::from(value);
                }
                "MissEffect" => {
                    self.effects.miss_effect = AsciiString::from(value);
                }
                "SoundEffect" => {
                    self.effects.sound_effect = AsciiString::from(value);
                }
                "TracerEffect" => {
                    self.effects.tracer_effect = AsciiString::from(value);
                }
                "ProjectileTemplate" => {
                    self.projectile_template = AsciiString::from(value);
                }
                "DamageFXTemplate" => {
                    self.damage_fx_template = AsciiString::from(value);
                }
                "Prerequisites" => {
                    self.prerequisites = value
                        .split_whitespace()
                        .map(|s| AsciiString::from(s))
                        .collect();
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
        !self.name.is_empty() && self.primary_damage > 0.0 && self.range > 0.0
    }

    pub fn is_area_weapon(&self) -> bool {
        self.damage_radius > 0.0 || self.attack_type == AttackType::Area
    }

    pub fn is_anti_air(&self) -> bool {
        self.can_target_air && !self.can_target_ground
    }

    pub fn is_anti_ground(&self) -> bool {
        self.can_target_ground && !self.can_target_air
    }

    pub fn is_dual_purpose(&self) -> bool {
        self.can_target_air && self.can_target_ground
    }

    pub fn can_target(&self, target_type: &str) -> bool {
        match target_type.to_lowercase().as_str() {
            "air" | "aircraft" => self.can_target_air,
            "ground" | "land" => self.can_target_ground,
            "water" | "naval" => self.can_target_water,
            "stealth" => self.can_target_stealth,
            _ => false,
        }
    }

    pub fn get_effective_damage(&self, armor: f32) -> f32 {
        let base_damage = self.primary_damage * self.armor_piercing;
        (base_damage - armor).max(0.0)
    }

    pub fn get_dps(&self) -> f32 {
        if self.rate_of_fire > 0.0 {
            self.primary_damage * self.rate_of_fire
        } else {
            0.0
        }
    }
}

/// Weapon store - manages all weapon templates
#[derive(Debug)]
pub struct WeaponStore {
    templates: HashMap<String, WeaponTemplate>,
    template_order: Vec<String>,
}

impl WeaponStore {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            template_order: Vec::new(),
        }
    }

    /// Find a template by name
    pub fn find_template(&self, name: &AsciiString) -> Option<&WeaponTemplate> {
        self.templates.get(name.as_str())
    }

    /// Find a mutable template by name
    pub fn find_template_mut(&mut self, name: &AsciiString) -> Option<&mut WeaponTemplate> {
        self.templates.get_mut(name.as_str())
    }

    /// Create a new template
    pub fn new_template(&mut self, name: AsciiString) -> &mut WeaponTemplate {
        let template = WeaponTemplate::new(name.clone());
        let key = name.as_str().to_string();
        if !self.templates.contains_key(&key) {
            self.template_order.push(key.clone());
        }
        self.templates.insert(key, template);
        self.templates.get_mut(name.as_str()).unwrap()
    }

    /// Get or create a template
    pub fn get_or_create_template(&mut self, name: &AsciiString) -> &mut WeaponTemplate {
        if !self.templates.contains_key(name.as_str()) {
            self.new_template(name.clone());
        }
        self.templates.get_mut(name.as_str()).unwrap()
    }

    /// Register a template
    pub fn register_template(&mut self, template: WeaponTemplate) {
        let name = template.name.as_str().to_string();
        if !self.templates.contains_key(&name) {
            self.template_order.push(name.clone());
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

    /// Get templates by damage type
    pub fn get_templates_by_damage_type(&self, damage_type: &DamageType) -> Vec<&WeaponTemplate> {
        self.template_order
            .iter()
            .filter_map(|name| self.templates.get(name.as_str()))
            .filter(|t| &t.damage_type == damage_type)
            .collect()
    }

    /// Get templates by attack type
    pub fn get_templates_by_attack_type(&self, attack_type: &AttackType) -> Vec<&WeaponTemplate> {
        self.template_order
            .iter()
            .filter_map(|name| self.templates.get(name.as_str()))
            .filter(|t| &t.attack_type == attack_type)
            .collect()
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
    }

    /// Get template count
    pub fn get_template_count(&self) -> usize {
        self.templates.len()
    }

    /// Parse weapon template definition - equivalent to original parseWeaponTemplateDefinition
    pub fn parse_weapon_template_definition(name: AsciiString) -> WeaponResult<()> {
        // In the original C++, this would delegate to WeaponStore::parseWeaponTemplateDefinition
        println!("Parsing weapon template definition for: {}", name.as_str());
        Ok(())
    }
}

impl Default for WeaponStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global weapon store instance
static WEAPON_STORE: OnceCell<RwLock<WeaponStore>> = OnceCell::new();

/// Initialize the global weapon store
pub fn initialize_weapon_store() {
    if WEAPON_STORE.get().is_none() {
        let _ = WEAPON_STORE.set(RwLock::new(WeaponStore::new()));
    } else if let Some(store) = WEAPON_STORE.get() {
        if let Ok(mut guard) = store.write() {
            *guard = WeaponStore::new();
        }
    }
}

/// Get a reference to the global weapon store
pub fn get_weapon_store() -> Option<RwLockWriteGuard<'static, WeaponStore>> {
    WEAPON_STORE
        .get()
        .map(|store| store.write().expect("WeaponStore poisoned"))
}

/// Parse a boolean value from string
pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

/// INI parsing functions for weapons
pub struct IniWeapon;

impl IniWeapon {
    /// Parse weapon template definition - equivalent to INI::parseWeaponTemplateDefinition
    pub fn parse_weapon_template_definition(name: AsciiString) -> WeaponResult<()> {
        // Validate name
        if name.is_empty() {
            return Err(WeaponError::InvalidName);
        }

        // Initialize weapon store if needed
        initialize_weapon_store();

        // Delegate to WeaponStore
        WeaponStore::parse_weapon_template_definition(name)
    }

    /// Parse a complete weapon template block from INI data
    pub fn parse_weapon_template_block(
        name: AsciiString,
        properties: HashMap<String, String>,
    ) -> WeaponResult<WeaponTemplate> {
        // Validate name
        if name.is_empty() {
            return Err(WeaponError::InvalidName);
        }

        // Create template
        let mut template = WeaponTemplate::new(name);

        // Update template from properties
        template.update_from_properties(&properties);

        // Validate template
        if !template.is_valid() {
            return Err(WeaponError::ParseError(
                "Invalid weapon template configuration".to_string(),
            ));
        }

        Ok(template)
    }

    /// Register a weapon template
    pub fn register_template(template: WeaponTemplate) -> WeaponResult<()> {
        initialize_weapon_store();

        let mut store = get_weapon_store()
            .ok_or_else(|| WeaponError::StoreError("Store not initialized".to_string()))?;

        store.register_template(template);
        Ok(())
    }

    /// Find a weapon template by name
    pub fn find_template_by_name(name: &AsciiString) -> Option<WeaponTemplate> {
        if let Some(store) = get_weapon_store() {
            store.find_template(name).cloned()
        } else {
            None
        }
    }

    /// Validate weapon name format
    pub fn validate_name(name: &AsciiString) -> bool {
        !name.is_empty() && name.len() < 128 // Reasonable length limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_type_parsing() {
        assert_eq!(DamageType::from_string("explosive"), DamageType::Explosive);
        assert_eq!(DamageType::from_string("LASER"), DamageType::Laser);
        assert_eq!(
            DamageType::from_string("CustomDamage"),
            DamageType::Custom("CustomDamage".to_string())
        );
    }

    #[test]
    fn test_attack_type_parsing() {
        assert_eq!(AttackType::from_string("area"), AttackType::Area);
        assert_eq!(
            AttackType::from_string("PROJECTILE"),
            AttackType::Projectile
        );
        assert_eq!(
            AttackType::from_string("CustomAttack"),
            AttackType::Custom("CustomAttack".to_string())
        );
    }

    #[test]
    fn test_weapon_template_creation() {
        let name = AsciiString::from("TestWeapon");
        let template = WeaponTemplate::new(name.clone());

        assert_eq!(template.name, name);
        assert_eq!(template.primary_damage, 10.0);
        assert_eq!(template.range, 100.0);
        assert!(template.can_target_air);
        assert!(template.can_target_ground);
        assert!(template.is_valid());
    }

    #[test]
    fn test_weapon_store() {
        let mut store = WeaponStore::new();
        let name = AsciiString::from("TestWeapon");

        // Create new template
        let template = store.new_template(name.clone());
        template.damage_type = DamageType::Explosive;
        template.primary_damage = 50.0;
        template.damage_radius = 20.0;

        // Find template
        let found = store.find_template(&name);
        assert!(found.is_some());
        assert_eq!(found.unwrap().primary_damage, 50.0);
        assert!(matches!(found.unwrap().damage_type, DamageType::Explosive));
        assert!(found.unwrap().is_area_weapon());

        // Count templates
        assert_eq!(store.get_template_count(), 1);
    }

    #[test]
    fn weapon_store_enumerates_in_registration_order() {
        let mut store = WeaponStore::new();

        let mut first = WeaponTemplate::new(AsciiString::from("FirstWeapon"));
        first.damage_type = DamageType::Explosive;
        first.attack_type = AttackType::Projectile;
        let mut second = WeaponTemplate::new(AsciiString::from("SecondWeapon"));
        second.damage_type = DamageType::Laser;
        second.attack_type = AttackType::Projectile;
        let mut third = WeaponTemplate::new(AsciiString::from("ThirdWeapon"));
        third.damage_type = DamageType::Explosive;
        third.attack_type = AttackType::Beam;

        store.register_template(first);
        store.register_template(second);
        store.register_template(third);

        let names: Vec<&str> = store
            .get_template_names()
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(names, vec!["FirstWeapon", "SecondWeapon", "ThirdWeapon"]);

        let explosive_names: Vec<&str> = store
            .get_templates_by_damage_type(&DamageType::Explosive)
            .into_iter()
            .map(|template| template.name.as_str())
            .collect();
        assert_eq!(explosive_names, vec!["FirstWeapon", "ThirdWeapon"]);

        let projectile_names: Vec<&str> = store
            .get_templates_by_attack_type(&AttackType::Projectile)
            .into_iter()
            .map(|template| template.name.as_str())
            .collect();
        assert_eq!(projectile_names, vec!["FirstWeapon", "SecondWeapon"]);
    }

    #[test]
    fn test_weapon_capabilities() {
        let mut template = WeaponTemplate::new(AsciiString::from("TestWeapon"));
        template.can_target_air = true;
        template.can_target_ground = false;
        template.damage_radius = 15.0;
        template.primary_damage = 25.0;
        template.rate_of_fire = 2.0;

        assert!(template.is_anti_air());
        assert!(!template.is_anti_ground());
        assert!(!template.is_dual_purpose());
        assert!(template.is_area_weapon());
        assert!(template.can_target("air"));
        assert!(!template.can_target("ground"));
        assert_eq!(template.get_dps(), 50.0);
    }

    #[test]
    fn test_effective_damage_calculation() {
        let mut template = WeaponTemplate::new(AsciiString::from("TestWeapon"));
        template.primary_damage = 100.0;
        template.armor_piercing = 0.8;

        let damage_vs_light_armor = template.get_effective_damage(10.0);
        let damage_vs_heavy_armor = template.get_effective_damage(90.0);
        let damage_vs_super_armor = template.get_effective_damage(200.0);

        assert_eq!(damage_vs_light_armor, 70.0); // 100 * 0.8 - 10
        assert_eq!(damage_vs_heavy_armor, 0.0); // Max of (80 - 90, 0)
        assert_eq!(damage_vs_super_armor, 0.0); // Max of (80 - 200, 0)
    }

    #[test]
    fn test_template_properties_update() {
        let mut template = WeaponTemplate::new(AsciiString::from("Test"));
        let mut properties = HashMap::new();
        properties.insert("DamageType".to_string(), "Fire".to_string());
        properties.insert("PrimaryDamage".to_string(), "75.0".to_string());
        properties.insert("Range".to_string(), "200.0".to_string());
        properties.insert("CanTargetAir".to_string(), "false".to_string());
        properties.insert("ProjectileCount".to_string(), "3".to_string());

        template.update_from_properties(&properties);

        assert!(matches!(template.damage_type, DamageType::Fire));
        assert_eq!(template.primary_damage, 75.0);
        assert_eq!(template.range, 200.0);
        assert!(!template.can_target_air);
        assert_eq!(template.projectile_count, 3);
    }

    #[test]
    fn test_firing_effects() {
        let mut template = WeaponTemplate::new(AsciiString::from("TestWeapon"));
        template.effects.muzzle_flash = AsciiString::from("MuzzleFlash01");
        template.effects.hit_effect = AsciiString::from("ExplosionSmall");
        template.effects.sound_effect = AsciiString::from("WeaponFire");

        assert_eq!(template.effects.muzzle_flash.as_str(), "MuzzleFlash01");
        assert_eq!(template.effects.hit_effect.as_str(), "ExplosionSmall");
        assert_eq!(template.effects.sound_effect.as_str(), "WeaponFire");
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
        assert!(IniWeapon::validate_name(&AsciiString::from("ValidName")));
        assert!(!IniWeapon::validate_name(&AsciiString::from("")));
    }
}
