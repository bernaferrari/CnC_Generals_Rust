////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_object.rs
//! Author: Colin Day, November 2001 (Converted to Rust)
//! Desc:   Parsing Object INI entries

use crate::common::ascii_string::AsciiString;
use crate::common::ini::ini::{INIError, INIResult, INI};
use crate::common::thing::thing_factory::{
    get_thing_factory, init_thing_factory, ThingFactory as RuntimeThingFactory,
};
use crate::common::thing::thing_template::{
    parse_bool_field, parse_u32_field, split_weapon_condition_tokens, WeaponSetDefinition,
};
use once_cell::sync::Lazy;
use std::collections::{BTreeMap, HashMap};
use std::sync::RwLock;

/// Result type for object parsing operations
pub type ObjectParseResult<T> = Result<T, ObjectParseError>;

/// Errors that can occur during object parsing
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectParseError {
    InvalidObjectName,
    MissingReskinSource,
    ThingFactoryError(String),
    ParsingError(String),
}

impl std::fmt::Display for ObjectParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectParseError::InvalidObjectName => write!(f, "Invalid object name"),
            ObjectParseError::MissingReskinSource => write!(f, "Missing reskin source"),
            ObjectParseError::ThingFactoryError(msg) => write!(f, "Thing factory error: {}", msg),
            ObjectParseError::ParsingError(msg) => write!(f, "Parsing error: {}", msg),
        }
    }
}

impl std::error::Error for ObjectParseError {}

/// Object definition data
#[derive(Debug, Clone)]
pub struct ObjectDefinition {
    pub name: AsciiString,
    pub reskin_from: Option<AsciiString>,
    pub properties: HashMap<String, String>,
}

impl ObjectDefinition {
    pub fn new(name: AsciiString) -> Self {
        Self {
            name,
            reskin_from: None,
            properties: HashMap::new(),
        }
    }

    pub fn new_with_reskin(name: AsciiString, reskin_from: AsciiString) -> Self {
        Self {
            name,
            reskin_from: Some(reskin_from),
            properties: HashMap::new(),
        }
    }

    pub fn is_reskin(&self) -> bool {
        self.reskin_from.is_some()
    }

    pub fn get_reskin_source(&self) -> Option<&AsciiString> {
        self.reskin_from.as_ref()
    }

    pub fn add_property(&mut self, key: String, value: String) {
        self.properties.insert(key, value);
    }

    pub fn get_property(&self, key: &str) -> Option<&String> {
        self.properties.get(key)
    }

    pub fn weapon_set_definitions(&self) -> Result<Vec<WeaponSetDefinition>, String> {
        let mut sets: BTreeMap<usize, WeaponSetDefinition> = BTreeMap::new();

        for (key, value) in &self.properties {
            if let Some((index, field)) = parse_weapon_set_key(key) {
                let entry = sets.entry(index).or_insert_with(WeaponSetDefinition::new);
                let trimmed_value = value.trim();

                match field {
                    "Conditions" => {
                        for token in split_weapon_condition_tokens(trimmed_value) {
                            entry.add_condition(token);
                        }
                    }
                    "PrimaryWeapon" => {
                        entry.set_weapon_name_str(0, Some(trimmed_value));
                    }
                    "SecondaryWeapon" => {
                        entry.set_weapon_name_str(1, Some(trimmed_value));
                    }
                    "TertiaryWeapon" => {
                        entry.set_weapon_name_str(2, Some(trimmed_value));
                    }
                    "AutoChoosePrimary" | "AutoChooseSourcesPrimary" => {
                        entry.set_auto_choose_mask(0, Some(parse_u32_field(trimmed_value)?));
                    }
                    "AutoChooseSecondary" | "AutoChooseSourcesSecondary" => {
                        entry.set_auto_choose_mask(1, Some(parse_u32_field(trimmed_value)?));
                    }
                    "AutoChooseTertiary" | "AutoChooseSourcesTertiary" => {
                        entry.set_auto_choose_mask(2, Some(parse_u32_field(trimmed_value)?));
                    }
                    "PreferredAgainstPrimary" => {
                        entry.set_preferred_against_mask(
                            0,
                            Some(
                                crate::common::system::kind_of::KindOfMask::from_bits_retain(
                                    parse_u32_field(trimmed_value)? as u128,
                                ),
                            ),
                        );
                    }
                    "PreferredAgainstSecondary" => {
                        entry.set_preferred_against_mask(
                            1,
                            Some(
                                crate::common::system::kind_of::KindOfMask::from_bits_retain(
                                    parse_u32_field(trimmed_value)? as u128,
                                ),
                            ),
                        );
                    }
                    "PreferredAgainstTertiary" => {
                        entry.set_preferred_against_mask(
                            2,
                            Some(
                                crate::common::system::kind_of::KindOfMask::from_bits_retain(
                                    parse_u32_field(trimmed_value)? as u128,
                                ),
                            ),
                        );
                    }
                    "ShareWeaponReloadTime" | "ShareReloadTime" => {
                        entry.set_share_reload_time(Some(parse_bool_field(trimmed_value)?));
                    }
                    "WeaponLockSharedAcrossSets" | "ShareWeaponLock" => {
                        entry.set_share_weapon_lock(Some(parse_bool_field(trimmed_value)?));
                    }
                    _ => {
                        return Err(format!("Unrecognised weapon set field '{}'", field));
                    }
                }
            }
        }

        Ok(sets.into_iter().map(|(_, set)| set).collect())
    }
}

fn parse_weapon_set_key(key: &str) -> Option<(usize, &str)> {
    let remainder = key.strip_prefix("WeaponSet")?;
    let mut parts = remainder.splitn(2, '.');
    let index_str = parts.next()?;
    let field = parts.next()?;
    if index_str.is_empty() || field.is_empty() {
        return None;
    }
    let index = index_str.parse().ok()?;
    Some((index, field))
}

/// Object-definition helper bridge for parsing and lookup.
pub struct ThingFactory;

static OBJECT_DEFINITION_REGISTRY: Lazy<RwLock<HashMap<String, ObjectDefinition>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn with_runtime_factory<F, T>(operation: F) -> ObjectParseResult<T>
where
    F: FnOnce(&mut RuntimeThingFactory) -> Result<T, String>,
{
    let needs_init = {
        let guard = get_thing_factory().map_err(|_| {
            ObjectParseError::ThingFactoryError("Failed to lock thing factory".to_string())
        })?;
        guard.is_none()
    };
    if needs_init {
        init_thing_factory()
            .map_err(|err| ObjectParseError::ThingFactoryError(format!("Init failed: {}", err)))?;
    }

    let mut guard = get_thing_factory().map_err(|_| {
        ObjectParseError::ThingFactoryError("Failed to lock thing factory".to_string())
    })?;
    let factory = guard.as_mut().ok_or_else(|| {
        ObjectParseError::ThingFactoryError("Thing factory unavailable after init".to_string())
    })?;
    operation(factory).map_err(ObjectParseError::ThingFactoryError)
}

impl ThingFactory {
    /// Parse object definition - equivalent to ThingFactory::parseObjectDefinition
    pub fn parse_object_definition(
        name: AsciiString,
        reskin_from: Option<AsciiString>,
    ) -> ObjectParseResult<ObjectDefinition> {
        // Validate object name
        if name.is_empty() {
            return Err(ObjectParseError::InvalidObjectName);
        }

        // Create object definition
        let object_def = if let Some(reskin_source) = reskin_from {
            if reskin_source.is_empty() {
                ObjectDefinition::new(name.clone())
            } else {
                ObjectDefinition::new_with_reskin(name.clone(), reskin_source)
            }
        } else {
            ObjectDefinition::new(name.clone())
        };

        Ok(object_def)
    }

    /// Register an object definition with the factory
    pub fn register_object_definition(definition: ObjectDefinition) -> ObjectParseResult<()> {
        let mut registry = OBJECT_DEFINITION_REGISTRY.write().map_err(|_| {
            ObjectParseError::ThingFactoryError("Object registry lock poisoned".to_string())
        })?;
        registry.insert(definition.name.as_str().to_ascii_lowercase(), definition);
        Ok(())
    }

    /// Find an existing object template by name
    pub fn find_object_template(name: &AsciiString) -> Option<ObjectDefinition> {
        OBJECT_DEFINITION_REGISTRY
            .read()
            .ok()
            .and_then(|registry| registry.get(&name.as_str().to_ascii_lowercase()).cloned())
    }
}

/// INI parsing functions for objects
pub struct IniObject;

impl IniObject {
    /// Parse Object entry from an INI line - equivalent to INI::parseObjectDefinition.
    pub fn parse_object_definition_from_ini(ini: &mut INI) -> INIResult<()> {
        let tokens = ini.get_line_tokens();
        let name = tokens
            .iter()
            .skip(1)
            .find(|token| **token != "=")
            .ok_or(INIError::InvalidData)?
            .to_string();

        with_runtime_factory(|factory| factory.parse_object_definition(ini, &name, ""))
            .map_err(|_| INIError::InvalidData)
    }

    /// Parse ObjectReskin entry from an INI line - equivalent to INI::parseObjectReskinDefinition.
    pub fn parse_object_reskin_definition_from_ini(ini: &mut INI) -> INIResult<()> {
        let tokens = ini.get_line_tokens();
        let mut args = tokens.iter().skip(1).filter(|token| **token != "=");
        let name = args.next().ok_or(INIError::InvalidData)?.to_string();
        let reskin_from = args.next().ok_or(INIError::InvalidData)?.to_string();

        with_runtime_factory(|factory| factory.parse_object_definition(ini, &name, &reskin_from))
            .map_err(|_| INIError::InvalidData)
    }

    /// Parse Object entry - equivalent to INI::parseObjectDefinition
    pub fn parse_object_definition(name: AsciiString) -> ObjectParseResult<()> {
        let empty_string = AsciiString::from("");
        let object_def = ThingFactory::parse_object_definition(name, Some(empty_string))?;
        ThingFactory::register_object_definition(object_def)?;
        Ok(())
    }

    /// Parse Object reskin entry - equivalent to INI::parseObjectReskinDefinition
    pub fn parse_object_reskin_definition(
        name: AsciiString,
        reskin_from: AsciiString,
    ) -> ObjectParseResult<()> {
        if reskin_from.is_empty() {
            return Err(ObjectParseError::MissingReskinSource);
        }

        let object_def = ThingFactory::parse_object_definition(name, Some(reskin_from))?;
        ThingFactory::register_object_definition(object_def)?;
        Ok(())
    }

    /// Parse a complete object block from INI data
    pub fn parse_object_block(
        name: AsciiString,
        reskin_from: Option<AsciiString>,
        properties: HashMap<String, String>,
    ) -> ObjectParseResult<ObjectDefinition> {
        let mut object_def = ThingFactory::parse_object_definition(name, reskin_from)?;

        // Add all properties from the INI block
        for (key, value) in properties {
            object_def.add_property(key, value);
        }

        Ok(object_def)
    }

    /// Validate object name format
    pub fn validate_object_name(name: &AsciiString) -> bool {
        !name.is_empty() && name.len() < 256 // Reasonable length limit
    }

    /// Check if an object can be reskinned from another
    pub fn can_reskin_from(target: &AsciiString, source: &AsciiString) -> bool {
        if !Self::validate_object_name(target) || !Self::validate_object_name(source) {
            return false;
        }

        if ThingFactory::find_object_template(source).is_some() {
            return true;
        }

        with_runtime_factory(|factory| Ok(factory.find_template(source.as_str(), false).is_some()))
            .unwrap_or(false)
    }
}

/// Token parser for extracting object names from INI tokens
pub struct ObjectTokenParser;

impl ObjectTokenParser {
    /// Extract the next object name token
    pub fn get_next_object_name(token: &str) -> ObjectParseResult<AsciiString> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(ObjectParseError::InvalidObjectName);
        }

        Ok(AsciiString::from(trimmed))
    }

    /// Extract reskin source name
    pub fn get_reskin_source(token: &str) -> ObjectParseResult<AsciiString> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(ObjectParseError::MissingReskinSource);
        }

        Ok(AsciiString::from(trimmed))
    }

    /// Parse a property line (key = value)
    pub fn parse_property_line(line: &str) -> ObjectParseResult<(String, String)> {
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            Ok((key, value))
        } else {
            Err(ObjectParseError::ParsingError(format!(
                "Invalid property line format: {}",
                line
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::bit_flags::WeaponSetFlags;
    use crate::common::thing::thing_template::WeaponTemplateSet;

    #[test]
    fn weapon_set_definitions_parse_from_properties() {
        let mut def = ObjectDefinition::new(AsciiString::from("TestObject"));
        def.add_property(
            "WeaponSet1.Conditions".to_string(),
            "HERO | PLAYER_UPGRADE".to_string(),
        );
        def.add_property(
            "WeaponSet1.PrimaryWeapon".to_string(),
            "HeroWeapon".to_string(),
        );
        def.add_property(
            "WeaponSet1.AutoChoosePrimary".to_string(),
            "0x3".to_string(),
        );
        def.add_property(
            "WeaponSet1.ShareWeaponReloadTime".to_string(),
            "true".to_string(),
        );

        let sets = def.weapon_set_definitions().expect("weapon sets");
        assert_eq!(sets.len(), 1);
        let set = &sets[0];

        let mut engine_set = WeaponTemplateSet::new();
        set.apply_to(&mut engine_set).expect("apply definition");
        assert!(engine_set.types().test(WeaponSetFlags::HERO));
        assert!(engine_set.types().test(WeaponSetFlags::PLAYER_UPGRADE));
        assert_eq!(
            engine_set.weapon_template_name(0).map(|name| name.as_str()),
            Some("HeroWeapon"),
        );
        assert_eq!(engine_set.auto_choose_mask(0), 0x3);
        assert!(engine_set.is_reload_time_shared());
    }

    #[test]
    fn test_object_definition_creation() {
        let name = AsciiString::from("TestObject");
        let object_def = ObjectDefinition::new(name.clone());

        assert_eq!(object_def.name, name);
        assert!(!object_def.is_reskin());
        assert!(object_def.get_reskin_source().is_none());
    }

    #[test]
    fn test_object_definition_with_reskin() {
        let name = AsciiString::from("TestObjectReskin");
        let reskin_from = AsciiString::from("BaseObject");
        let object_def = ObjectDefinition::new_with_reskin(name.clone(), reskin_from.clone());

        assert_eq!(object_def.name, name);
        assert!(object_def.is_reskin());
        assert_eq!(object_def.get_reskin_source(), Some(&reskin_from));
    }

    #[test]
    fn test_object_properties() {
        let mut object_def = ObjectDefinition::new(AsciiString::from("TestObject"));
        object_def.add_property("Health".to_string(), "100".to_string());
        object_def.add_property("Armor".to_string(), "TANK_ARMOR".to_string());

        assert_eq!(object_def.get_property("Health"), Some(&"100".to_string()));
        assert_eq!(
            object_def.get_property("Armor"),
            Some(&"TANK_ARMOR".to_string())
        );
        assert_eq!(object_def.get_property("Missing"), None);
    }

    #[test]
    fn test_validate_object_name() {
        assert!(IniObject::validate_object_name(&AsciiString::from(
            "ValidName"
        )));
        assert!(!IniObject::validate_object_name(&AsciiString::from("")));
    }

    #[test]
    fn test_token_parser() {
        assert!(ObjectTokenParser::get_next_object_name("TestObject").is_ok());
        assert!(ObjectTokenParser::get_next_object_name("  SpacedName  ").is_ok());
        assert!(ObjectTokenParser::get_next_object_name("").is_err());
        assert!(ObjectTokenParser::get_next_object_name("   ").is_err());
    }

    #[test]
    fn test_property_line_parsing() {
        let result = ObjectTokenParser::parse_property_line("Health = 100");
        assert!(result.is_ok());
        let (key, value) = result.unwrap();
        assert_eq!(key, "Health");
        assert_eq!(value, "100");

        let result = ObjectTokenParser::parse_property_line("Armor=TANK_ARMOR");
        assert!(result.is_ok());
        let (key, value) = result.unwrap();
        assert_eq!(key, "Armor");
        assert_eq!(value, "TANK_ARMOR");

        assert!(ObjectTokenParser::parse_property_line("InvalidLine").is_err());
    }

    #[test]
    fn test_can_reskin_from() {
        let target = AsciiString::from("ReskinTarget");
        let source = AsciiString::from("ReskinSource");
        let empty = AsciiString::from("");

        let _ = ThingFactory::register_object_definition(ObjectDefinition::new(source.clone()));
        assert!(IniObject::can_reskin_from(&target, &source));
        assert!(!IniObject::can_reskin_from(&empty, &source));
        assert!(!IniObject::can_reskin_from(&target, &empty));
    }
}
