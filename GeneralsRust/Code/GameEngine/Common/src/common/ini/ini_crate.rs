//! INI Crate parsing module
//! Author: Graham Smallwood Feb 2002
//! Desc: Just passes the parse to the CrateSystem

use super::ini::{INIError, INIResult, INI};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;

/// Crate content types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrateContentType {
    Money,
    Unit,
    Upgrade,
    SpecialPower,
    Veterancy,
    Health,
    Experience,
    Salvage,
}

impl Default for CrateContentType {
    fn default() -> Self {
        CrateContentType::Money
    }
}

/// Crate spawn conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrateSpawnCondition {
    UnitDestroyed,
    BuildingDestroyed,
    Random,
    Scripted,
    Manual,
}

impl Default for CrateSpawnCondition {
    fn default() -> Self {
        CrateSpawnCondition::UnitDestroyed
    }
}

/// Crate rarity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrateRarity {
    Common,
    Uncommon,
    Rare,
    VeryRare,
    Legendary,
}

impl Default for CrateRarity {
    fn default() -> Self {
        CrateRarity::Common
    }
}

/// Crate template structure
#[derive(Debug, Clone)]
pub struct CrateTemplate {
    pub name: String,
    pub content_type: CrateContentType,
    pub spawn_condition: CrateSpawnCondition,
    pub rarity: CrateRarity,
    pub money_amount: i32,
    pub unit_name: String,
    pub upgrade_name: String,
    pub special_power_name: String,
    pub veterancy_level: i32,
    pub health_amount: f32,
    pub experience_amount: i32,
    pub lifetime_frames: u32,
    pub pickup_range: f32,
    pub spawn_probability: f32,
    pub model_name: String,
    pub texture_name: String,
    pub pickup_sound: String,
    pub spawn_sound: String,
    pub expire_sound: String,
    pub glow_effect: String,
    pub particle_effect: String,
    pub animation_name: String,
    pub bounce_height: f32,
    pub bounce_speed: f32,
    pub rotation_speed: f32,
    pub scale_factor: f32,
    pub forbidden_on_modes: Vec<String>,
    pub required_sciences: Vec<String>,
    pub disabled_by_sciences: Vec<String>,
    pub minimum_player_level: u32,
    pub maximum_player_level: u32,
    pub faction_specific: Vec<String>,
    pub weight: f32, // For weighted random selection
}

impl Default for CrateTemplate {
    fn default() -> Self {
        Self {
            name: String::new(),
            content_type: CrateContentType::default(),
            spawn_condition: CrateSpawnCondition::default(),
            rarity: CrateRarity::default(),
            money_amount: 100,
            unit_name: String::new(),
            upgrade_name: String::new(),
            special_power_name: String::new(),
            veterancy_level: 1,
            health_amount: 50.0,
            experience_amount: 100,
            lifetime_frames: 1800, // 60 seconds at 30 FPS
            pickup_range: 50.0,
            spawn_probability: 0.1,
            model_name: "Crate".to_string(),
            texture_name: "CrateTexture".to_string(),
            pickup_sound: "CratePickup".to_string(),
            spawn_sound: "CrateSpawn".to_string(),
            expire_sound: "CrateExpire".to_string(),
            glow_effect: "CrateGlow".to_string(),
            particle_effect: "CrateParticles".to_string(),
            animation_name: String::new(),
            bounce_height: 10.0,
            bounce_speed: 1.0,
            rotation_speed: 45.0, // degrees per second
            scale_factor: 1.0,
            forbidden_on_modes: Vec::new(),
            required_sciences: Vec::new(),
            disabled_by_sciences: Vec::new(),
            minimum_player_level: 0,
            maximum_player_level: 999,
            faction_specific: Vec::new(),
            weight: 1.0,
        }
    }
}

impl CrateTemplate {
    /// Create a new crate template with the given name
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    /// Get the name of this crate template
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Parse crate template from INI
    pub fn parse_from_ini(ini: &mut INI, name: String) -> INIResult<Self> {
        let mut template = Self::new(name);
        template.parse_crate_fields(ini)?;
        Ok(template)
    }

    /// Parse crate-specific fields
    fn parse_crate_fields(&mut self, ini: &mut INI) -> INIResult<()> {
        ini.init_from_ini_with_fields_allow_unknown(self, Self::get_field_parse())
    }

    /// Get the field parsing table for crate templates
    pub fn get_field_parse() -> &'static [super::ini::FieldParse<Self>] {
        FIELD_PARSE_TABLE
    }

    /// Check if this crate can spawn for the given player
    pub fn can_spawn_for_player(&self, player_level: u32, faction: &str) -> bool {
        // Check player level requirements
        if player_level < self.minimum_player_level || player_level > self.maximum_player_level {
            return false;
        }

        // Check faction requirements
        if !self.faction_specific.is_empty()
            && !self.faction_specific.contains(&faction.to_string())
        {
            return false;
        }

        true
    }

    /// Check if this crate is forbidden in the given mode
    pub fn is_forbidden_in_mode(&self, mode: &str) -> bool {
        self.forbidden_on_modes.iter().any(|m| m == mode)
    }

    /// Check if this crate requires a specific science
    pub fn requires_science(&self, science: &str) -> bool {
        self.required_sciences.iter().any(|s| s == science)
    }

    /// Check if this crate is disabled by a specific science
    pub fn is_disabled_by_science(&self, science: &str) -> bool {
        self.disabled_by_sciences.iter().any(|s| s == science)
    }

    /// Get the lifetime in seconds
    pub fn get_lifetime_seconds(&self) -> f32 {
        self.lifetime_frames as f32 / 30.0 // Assuming 30 FPS
    }

    /// Get rotation speed in radians per frame
    pub fn get_rotation_speed_radians_per_frame(&self) -> f32 {
        (self.rotation_speed * std::f32::consts::PI / 180.0) / 30.0 // Convert to radians per frame
    }

    /// Validate the crate template configuration
    pub fn validate(&self) -> INIResult<()> {
        // Check that required fields are set based on content type
        match self.content_type {
            CrateContentType::Money => {
                if self.money_amount <= 0 {
                    eprintln!(
                        "CrateTemplate {} with Money content type must have positive MoneyAmount",
                        self.name
                    );
                    return Err(INIError::InvalidData);
                }
            }
            CrateContentType::Unit => {
                if self.unit_name.is_empty() {
                    eprintln!(
                        "CrateTemplate {} with Unit content type must specify UnitName",
                        self.name
                    );
                    return Err(INIError::InvalidData);
                }
            }
            CrateContentType::Upgrade => {
                if self.upgrade_name.is_empty() {
                    eprintln!(
                        "CrateTemplate {} with Upgrade content type must specify UpgradeName",
                        self.name
                    );
                    return Err(INIError::InvalidData);
                }
            }
            CrateContentType::SpecialPower => {
                if self.special_power_name.is_empty() {
                    eprintln!("CrateTemplate {} with SpecialPower content type must specify SpecialPowerName", self.name);
                    return Err(INIError::InvalidData);
                }
            }
            CrateContentType::Veterancy => {
                if self.veterancy_level <= 0 {
                    eprintln!("CrateTemplate {} with Veterancy content type must have positive VeterancyLevel", self.name);
                    return Err(INIError::InvalidData);
                }
            }
            CrateContentType::Health => {
                if self.health_amount <= 0.0 {
                    eprintln!(
                        "CrateTemplate {} with Health content type must have positive HealthAmount",
                        self.name
                    );
                    return Err(INIError::InvalidData);
                }
            }
            CrateContentType::Experience => {
                if self.experience_amount <= 0 {
                    eprintln!("CrateTemplate {} with Experience content type must have positive ExperienceAmount", self.name);
                    return Err(INIError::InvalidData);
                }
            }
            CrateContentType::Salvage => {
                // Salvage crates might not need specific validation
            }
        }

        // Check probability ranges
        if self.spawn_probability < 0.0 || self.spawn_probability > 1.0 {
            eprintln!(
                "CrateTemplate {} has invalid spawn probability: {}",
                self.name, self.spawn_probability
            );
            return Err(INIError::InvalidData);
        }

        // Check weight
        if self.weight <= 0.0 {
            eprintln!("CrateTemplate {} must have positive weight", self.name);
            return Err(INIError::InvalidData);
        }

        Ok(())
    }
}

/// Crate system for managing crate templates and spawning
#[derive(Debug)]
pub struct CrateSystem {
    templates: HashMap<String, CrateTemplate>,
    spawn_enabled: bool,
    global_spawn_multiplier: f32,
}

impl CrateSystem {
    /// Create a new crate system
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            spawn_enabled: true,
            global_spawn_multiplier: 1.0,
        }
    }

    /// Find a crate template by name
    pub fn find_template(&self, name: &str) -> Option<&CrateTemplate> {
        self.templates.get(name)
    }

    /// Find a mutable crate template by name
    pub fn find_template_mut(&mut self, name: &str) -> Option<&mut CrateTemplate> {
        self.templates.get_mut(name)
    }

    /// Add a new crate template
    pub fn add_template(&mut self, template: CrateTemplate) {
        self.templates.insert(template.name.clone(), template);
    }

    /// Remove a crate template
    pub fn remove_template(&mut self, name: &str) -> Option<CrateTemplate> {
        self.templates.remove(name)
    }

    /// Get all template names
    pub fn get_template_names(&self) -> Vec<&String> {
        self.templates.keys().collect()
    }

    /// Get templates by content type
    pub fn get_templates_by_type(&self, content_type: CrateContentType) -> Vec<&CrateTemplate> {
        self.templates
            .values()
            .filter(|template| template.content_type == content_type)
            .collect()
    }

    /// Get templates by rarity
    pub fn get_templates_by_rarity(&self, rarity: CrateRarity) -> Vec<&CrateTemplate> {
        self.templates
            .values()
            .filter(|template| template.rarity == rarity)
            .collect()
    }

    /// Get the number of templates
    pub fn count(&self) -> usize {
        self.templates.len()
    }

    /// Check if spawning is enabled
    pub fn is_spawn_enabled(&self) -> bool {
        self.spawn_enabled
    }

    /// Enable or disable crate spawning
    pub fn set_spawn_enabled(&mut self, enabled: bool) {
        self.spawn_enabled = enabled;
    }

    /// Get the global spawn multiplier
    pub fn get_global_spawn_multiplier(&self) -> f32 {
        self.global_spawn_multiplier
    }

    /// Set the global spawn multiplier
    pub fn set_global_spawn_multiplier(&mut self, multiplier: f32) {
        self.global_spawn_multiplier = multiplier.max(0.0);
    }

    /// Clear all templates
    pub fn clear(&mut self) {
        self.templates.clear();
    }

    /// Parse crate template definition from INI
    pub fn parse_crate_template_definition(ini: &mut INI) -> INIResult<()> {
        // Read the template name
        let name = match ini.get_next_value_token() {
            Some(token) => token,
            None => return Err(INIError::InvalidData),
        };

        // Ensure the global crate system exists
        let crate_system_handle = ensure_crate_system();
        let mut crate_system = crate_system_handle.write();

        // Create new crate template
        let template = CrateTemplate::parse_from_ini(ini, name)?;

        // Validate the template
        template.validate()?;

        // Add to the crate system
        crate_system.add_template(template);

        Ok(())
    }

    /// Select a random crate template based on weights and conditions
    pub fn select_random_template(
        &self,
        player_level: u32,
        faction: &str,
        mode: &str,
    ) -> Option<&CrateTemplate> {
        // Filter valid templates
        let valid_templates: Vec<&CrateTemplate> = self
            .templates
            .values()
            .filter(|template| {
                template.can_spawn_for_player(player_level, faction)
                    && !template.is_forbidden_in_mode(mode)
            })
            .collect();

        if valid_templates.is_empty() {
            return None;
        }

        // Calculate total weight
        let total_weight: f32 = valid_templates.iter().map(|template| template.weight).sum();

        if total_weight <= 0.0 {
            return None;
        }

        let mut rng = rand::thread_rng();
        let mut roll = rng.gen_range(0.0..total_weight);
        for template in &valid_templates {
            if roll <= template.weight {
                return Some(*template);
            }
            roll -= template.weight;
        }
        valid_templates.last().copied()
    }
}

fn parse_content_type(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    target.content_type = match token.to_ascii_lowercase().as_str() {
        "money" => CrateContentType::Money,
        "unit" => CrateContentType::Unit,
        "upgrade" => CrateContentType::Upgrade,
        "specialpower" | "special_power" => CrateContentType::SpecialPower,
        "veterancy" => CrateContentType::Veterancy,
        "health" => CrateContentType::Health,
        "experience" => CrateContentType::Experience,
        "salvage" => CrateContentType::Salvage,
        _ => return Err(INIError::InvalidData),
    };
    Ok(())
}

fn parse_spawn_condition(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    target.spawn_condition = match token.to_ascii_lowercase().as_str() {
        "unitdestroyed" | "unit_destroyed" => CrateSpawnCondition::UnitDestroyed,
        "buildingdestroyed" | "building_destroyed" => CrateSpawnCondition::BuildingDestroyed,
        "random" => CrateSpawnCondition::Random,
        "scripted" => CrateSpawnCondition::Scripted,
        "manual" => CrateSpawnCondition::Manual,
        _ => return Err(INIError::InvalidData),
    };
    Ok(())
}

fn parse_rarity(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    target.rarity = match token.to_ascii_lowercase().as_str() {
        "common" => CrateRarity::Common,
        "uncommon" => CrateRarity::Uncommon,
        "rare" => CrateRarity::Rare,
        "veryrare" | "very_rare" => CrateRarity::VeryRare,
        "legendary" => CrateRarity::Legendary,
        _ => return Err(INIError::InvalidData),
    };
    Ok(())
}

fn parse_string_field(target: &mut String, args: &[&str]) -> INIResult<()> {
    if args.is_empty() {
        return Err(INIError::InvalidData);
    }
    *target = INI::parse_ascii_string(&args.join(" "))?;
    Ok(())
}

fn parse_i32_field(target: &mut i32, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    *target = INI::parse_int(token)?;
    Ok(())
}

fn parse_u32_field(target: &mut u32, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    *target = INI::parse_unsigned_int(token)?;
    Ok(())
}

fn parse_f32_field(target: &mut f32, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    *target = INI::parse_real(token)?;
    Ok(())
}

fn parse_string_list(args: &[&str]) -> Vec<String> {
    let mut values = Vec::new();
    for arg in args {
        for piece in arg.split(&[',', '|'][..]) {
            let item = piece.trim();
            if !item.is_empty() {
                values.push(item.to_string());
            }
        }
    }
    values
}

fn parse_money_amount(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_i32_field(&mut target.money_amount, args)
}

fn parse_unit_name(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_string_field(&mut target.unit_name, args)
}

fn parse_upgrade_name(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_string_field(&mut target.upgrade_name, args)
}

fn parse_special_power_name(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    parse_string_field(&mut target.special_power_name, args)
}

fn parse_veterancy_level(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    parse_i32_field(&mut target.veterancy_level, args)
}

fn parse_health_amount(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_f32_field(&mut target.health_amount, args)
}

fn parse_experience_amount(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    parse_i32_field(&mut target.experience_amount, args)
}

fn parse_lifetime_frames(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    parse_u32_field(&mut target.lifetime_frames, args)
}

fn parse_pickup_range(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_f32_field(&mut target.pickup_range, args)
}

fn parse_spawn_probability(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    parse_f32_field(&mut target.spawn_probability, args)
}

fn parse_model_name(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_string_field(&mut target.model_name, args)
}

fn parse_texture_name(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_string_field(&mut target.texture_name, args)
}

fn parse_pickup_sound(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_string_field(&mut target.pickup_sound, args)
}

fn parse_spawn_sound(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_string_field(&mut target.spawn_sound, args)
}

fn parse_expire_sound(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_string_field(&mut target.expire_sound, args)
}

fn parse_glow_effect(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_string_field(&mut target.glow_effect, args)
}

fn parse_particle_effect(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    parse_string_field(&mut target.particle_effect, args)
}

fn parse_animation_name(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    parse_string_field(&mut target.animation_name, args)
}

fn parse_bounce_height(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_f32_field(&mut target.bounce_height, args)
}

fn parse_bounce_speed(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_f32_field(&mut target.bounce_speed, args)
}

fn parse_rotation_speed(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    parse_f32_field(&mut target.rotation_speed, args)
}

fn parse_scale_factor(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_f32_field(&mut target.scale_factor, args)
}

fn parse_forbidden_on_modes(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    target.forbidden_on_modes = parse_string_list(args);
    Ok(())
}

fn parse_required_sciences(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    target.required_sciences = parse_string_list(args);
    Ok(())
}

fn parse_disabled_by_sciences(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    target.disabled_by_sciences = parse_string_list(args);
    Ok(())
}

fn parse_minimum_player_level(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    parse_u32_field(&mut target.minimum_player_level, args)
}

fn parse_maximum_player_level(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    parse_u32_field(&mut target.maximum_player_level, args)
}

fn parse_faction_specific(
    _ini: &mut INI,
    target: &mut CrateTemplate,
    args: &[&str],
) -> INIResult<()> {
    target.faction_specific = parse_string_list(args);
    Ok(())
}

fn parse_weight(_ini: &mut INI, target: &mut CrateTemplate, args: &[&str]) -> INIResult<()> {
    parse_f32_field(&mut target.weight, args)
}

const FIELD_PARSE_TABLE: &[super::ini::FieldParse<CrateTemplate>] = &[
    super::ini::FieldParse {
        token: "ContentType",
        parse: parse_content_type,
    },
    super::ini::FieldParse {
        token: "SpawnCondition",
        parse: parse_spawn_condition,
    },
    super::ini::FieldParse {
        token: "Rarity",
        parse: parse_rarity,
    },
    super::ini::FieldParse {
        token: "MoneyAmount",
        parse: parse_money_amount,
    },
    super::ini::FieldParse {
        token: "UnitName",
        parse: parse_unit_name,
    },
    super::ini::FieldParse {
        token: "UpgradeName",
        parse: parse_upgrade_name,
    },
    super::ini::FieldParse {
        token: "SpecialPowerName",
        parse: parse_special_power_name,
    },
    super::ini::FieldParse {
        token: "VeterancyLevel",
        parse: parse_veterancy_level,
    },
    super::ini::FieldParse {
        token: "HealthAmount",
        parse: parse_health_amount,
    },
    super::ini::FieldParse {
        token: "ExperienceAmount",
        parse: parse_experience_amount,
    },
    super::ini::FieldParse {
        token: "LifetimeFrames",
        parse: parse_lifetime_frames,
    },
    super::ini::FieldParse {
        token: "PickupRange",
        parse: parse_pickup_range,
    },
    super::ini::FieldParse {
        token: "SpawnProbability",
        parse: parse_spawn_probability,
    },
    super::ini::FieldParse {
        token: "ModelName",
        parse: parse_model_name,
    },
    super::ini::FieldParse {
        token: "TextureName",
        parse: parse_texture_name,
    },
    super::ini::FieldParse {
        token: "PickupSound",
        parse: parse_pickup_sound,
    },
    super::ini::FieldParse {
        token: "SpawnSound",
        parse: parse_spawn_sound,
    },
    super::ini::FieldParse {
        token: "ExpireSound",
        parse: parse_expire_sound,
    },
    super::ini::FieldParse {
        token: "GlowEffect",
        parse: parse_glow_effect,
    },
    super::ini::FieldParse {
        token: "ParticleEffect",
        parse: parse_particle_effect,
    },
    super::ini::FieldParse {
        token: "AnimationName",
        parse: parse_animation_name,
    },
    super::ini::FieldParse {
        token: "BounceHeight",
        parse: parse_bounce_height,
    },
    super::ini::FieldParse {
        token: "BounceSpeed",
        parse: parse_bounce_speed,
    },
    super::ini::FieldParse {
        token: "RotationSpeed",
        parse: parse_rotation_speed,
    },
    super::ini::FieldParse {
        token: "ScaleFactor",
        parse: parse_scale_factor,
    },
    super::ini::FieldParse {
        token: "ForbiddenOnModes",
        parse: parse_forbidden_on_modes,
    },
    super::ini::FieldParse {
        token: "RequiredSciences",
        parse: parse_required_sciences,
    },
    super::ini::FieldParse {
        token: "DisabledBySciences",
        parse: parse_disabled_by_sciences,
    },
    super::ini::FieldParse {
        token: "MinimumPlayerLevel",
        parse: parse_minimum_player_level,
    },
    super::ini::FieldParse {
        token: "MaximumPlayerLevel",
        parse: parse_maximum_player_level,
    },
    super::ini::FieldParse {
        token: "FactionSpecific",
        parse: parse_faction_specific,
    },
    super::ini::FieldParse {
        token: "Weight",
        parse: parse_weight,
    },
];

impl Default for CrateSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Global crate system instance (thread-safe)
static CRATE_SYSTEM: OnceCell<Arc<RwLock<CrateSystem>>> = OnceCell::new();

/// Ensure the crate system exists and return a handle to it
pub fn ensure_crate_system() -> Arc<RwLock<CrateSystem>> {
    CRATE_SYSTEM
        .get_or_init(|| Arc::new(RwLock::new(CrateSystem::new())))
        .clone()
}

/// Initialize (or reinitialize) the global crate system
pub fn initialize_crate_system() {
    let crate_system = ensure_crate_system();
    crate_system.write().clear();
}

/// Get a handle to the global crate system if initialized
pub fn get_crate_system() -> Option<Arc<RwLock<CrateSystem>>> {
    CRATE_SYSTEM.get().cloned()
}

/// Parse crate template definition from INI file
/// This is the main entry point called by the INI parser
pub fn parse_crate_template_definition(ini: &mut INI) -> INIResult<()> {
    CrateSystem::parse_crate_template_definition(ini)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_template_creation() {
        let template = CrateTemplate::new("TestCrate".to_string());
        assert_eq!(template.get_name(), "TestCrate");
        assert_eq!(template.content_type, CrateContentType::Money);
        assert_eq!(template.money_amount, 100);
    }

    #[test]
    fn test_crate_template_player_checks() {
        let mut template = CrateTemplate::new("TestCrate".to_string());
        template.minimum_player_level = 5;
        template.maximum_player_level = 10;
        template.faction_specific = vec!["USA".to_string(), "China".to_string()];

        assert!(!template.can_spawn_for_player(3, "USA")); // Level too low
        assert!(!template.can_spawn_for_player(15, "USA")); // Level too high
        assert!(!template.can_spawn_for_player(7, "GLA")); // Wrong faction
        assert!(template.can_spawn_for_player(7, "USA")); // Valid
        assert!(template.can_spawn_for_player(7, "China")); // Valid
    }

    #[test]
    fn test_crate_template_mode_restrictions() {
        let mut template = CrateTemplate::new("TestCrate".to_string());
        template.forbidden_on_modes = vec!["Skirmish".to_string(), "Tournament".to_string()];

        assert!(template.is_forbidden_in_mode("Skirmish"));
        assert!(template.is_forbidden_in_mode("Tournament"));
        assert!(!template.is_forbidden_in_mode("Campaign"));
    }

    #[test]
    fn test_crate_template_science_checks() {
        let mut template = CrateTemplate::new("TestCrate".to_string());
        template.required_sciences = vec!["SCIENCE_ADVANCED_TRAINING".to_string()];
        template.disabled_by_sciences = vec!["SCIENCE_FANATICISM".to_string()];

        assert!(template.requires_science("SCIENCE_ADVANCED_TRAINING"));
        assert!(!template.requires_science("SCIENCE_OTHER"));

        assert!(template.is_disabled_by_science("SCIENCE_FANATICISM"));
        assert!(!template.is_disabled_by_science("SCIENCE_OTHER"));
    }

    #[test]
    fn test_crate_template_validation_money() {
        let mut template = CrateTemplate::new("TestCrate".to_string());
        template.content_type = CrateContentType::Money;
        template.money_amount = 100;
        assert!(template.validate().is_ok());

        template.money_amount = -50;
        assert!(template.validate().is_err());
    }

    #[test]
    fn test_crate_template_validation_unit() {
        let mut template = CrateTemplate::new("TestCrate".to_string());
        template.content_type = CrateContentType::Unit;
        template.unit_name = "Ranger".to_string();
        assert!(template.validate().is_ok());

        template.unit_name = String::new();
        assert!(template.validate().is_err());
    }

    #[test]
    fn test_crate_template_time_conversions() {
        let mut template = CrateTemplate::new("TestCrate".to_string());
        template.lifetime_frames = 900; // 30 seconds at 30 FPS
        template.rotation_speed = 90.0; // 90 degrees per second

        assert_eq!(template.get_lifetime_seconds(), 30.0);

        let rad_per_frame = template.get_rotation_speed_radians_per_frame();
        let expected = (90.0 * std::f32::consts::PI / 180.0) / 30.0;
        assert!((rad_per_frame - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn test_crate_system() {
        let mut system = CrateSystem::new();
        assert_eq!(system.count(), 0);
        assert!(system.is_spawn_enabled());

        // Add a template
        let template = CrateTemplate::new("TestCrate".to_string());
        system.add_template(template);

        assert_eq!(system.count(), 1);

        // Find the template
        let found = system.find_template("TestCrate");
        assert!(found.is_some());
        assert_eq!(found.unwrap().get_name(), "TestCrate");
    }

    #[test]
    fn test_crate_system_filtering() {
        let mut system = CrateSystem::new();

        let mut template1 = CrateTemplate::new("MoneyCrate".to_string());
        template1.content_type = CrateContentType::Money;
        template1.rarity = CrateRarity::Common;

        let mut template2 = CrateTemplate::new("UnitCrate".to_string());
        template2.content_type = CrateContentType::Unit;
        template2.rarity = CrateRarity::Rare;

        let mut template3 = CrateTemplate::new("MoneyRare".to_string());
        template3.content_type = CrateContentType::Money;
        template3.rarity = CrateRarity::Rare;

        system.add_template(template1);
        system.add_template(template2);
        system.add_template(template3);

        // Test filtering by content type
        let money_crates = system.get_templates_by_type(CrateContentType::Money);
        assert_eq!(money_crates.len(), 2);

        let unit_crates = system.get_templates_by_type(CrateContentType::Unit);
        assert_eq!(unit_crates.len(), 1);

        // Test filtering by rarity
        let common_crates = system.get_templates_by_rarity(CrateRarity::Common);
        assert_eq!(common_crates.len(), 1);

        let rare_crates = system.get_templates_by_rarity(CrateRarity::Rare);
        assert_eq!(rare_crates.len(), 2);
    }

    #[test]
    fn test_crate_system_spawn_settings() {
        let mut system = CrateSystem::new();

        assert_eq!(system.get_global_spawn_multiplier(), 1.0);

        system.set_global_spawn_multiplier(2.5);
        assert_eq!(system.get_global_spawn_multiplier(), 2.5);

        system.set_global_spawn_multiplier(-1.0); // Should clamp to 0.0
        assert_eq!(system.get_global_spawn_multiplier(), 0.0);

        system.set_spawn_enabled(false);
        assert!(!system.is_spawn_enabled());
    }
}
