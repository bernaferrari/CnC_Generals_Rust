////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_science.rs
//! Author: Steven Johnson, Colin Day November 2001 (Converted to Rust)
//! Desc: Science/Technology tree parsing and management
//!
//! Matches C++ Science.h and Science.cpp field parse table

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ascii_string::AsciiString;
use crate::common::name_key_generator::NameKeyGenerator;

// Placeholder for UnicodeString - in real implementation would be from common module
#[derive(Debug, Clone, PartialEq)]
pub struct UnicodeString(String);

impl UnicodeString {
    pub fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Result type for science operations
pub type ScienceResult<T> = Result<T, ScienceError>;

/// Errors that can occur during science parsing
#[derive(Debug, Clone, PartialEq)]
pub enum ScienceError {
    InvalidName,
    InvalidPrerequisite,
    ParseError(String),
    NotFound,
    AlreadyExists,
    NotGrantable,
}

impl std::fmt::Display for ScienceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScienceError::InvalidName => write!(f, "Invalid science name"),
            ScienceError::InvalidPrerequisite => write!(f, "Invalid prerequisite science"),
            ScienceError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ScienceError::NotFound => write!(f, "Science not found"),
            ScienceError::AlreadyExists => write!(f, "Science already exists"),
            ScienceError::NotGrantable => write!(f, "Science is not grantable"),
        }
    }
}

impl std::error::Error for ScienceError {}

/// Science type identifier
/// Matches C++ ScienceType from Science.h line 19-22
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScienceType(pub i32);

impl ScienceType {
    pub const INVALID: ScienceType = ScienceType(-1);

    pub fn is_valid(&self) -> bool {
        self.0 != -1
    }
}

/// Science information structure
/// Matches C++ ScienceInfo from Science.h lines 28-51
/// Field parse table from Science.cpp lines 147-155
#[derive(Debug, Clone)]
pub struct ScienceInfo {
    /// The science identifier (generated from name)
    pub science: ScienceType,

    /// Localized display name
    pub name: UnicodeString,

    /// Localized description
    pub description: UnicodeString,

    /// Root sciences (calculated at runtime, NOT read from INI)
    pub root_sciences: Vec<ScienceType>,

    /// Prerequisite sciences that must be obtained before this one
    pub prereq_sciences: Vec<ScienceType>,

    /// Cost in science purchase points (0 means cannot be purchased)
    pub science_purchase_point_cost: i32,

    /// Whether this science can be granted
    pub grantable: bool,
}

impl ScienceInfo {
    /// Create a new science info with default values
    /// Matches C++ ScienceInfo::ScienceInfo() from Science.h lines 43-47
    pub fn new(science: ScienceType) -> Self {
        Self {
            science,
            name: UnicodeString::from(""),
            description: UnicodeString::from(""),
            root_sciences: Vec::new(),
            prereq_sciences: Vec::new(),
            science_purchase_point_cost: 0, // 0 means "cannot be purchased"
            grantable: true,
        }
    }

    /// Add root sciences to the provided vector
    /// Matches C++ ScienceInfo::addRootSciences from Science.cpp lines 102-120
    pub fn add_root_sciences(&self, v: &mut Vec<ScienceType>, store: &ScienceStore) {
        if self.prereq_sciences.is_empty() {
            // We're a root. Add ourselves if not already present.
            if !v.contains(&self.science) {
                v.push(self.science);
            }
        } else {
            // We're not a root. Add the roots of all our prereqs.
            for prereq in &self.prereq_sciences {
                if let Some(si) = store.find_science_info(*prereq) {
                    si.add_root_sciences(v, store);
                }
            }
        }
    }
}

/// Science store - manages all sciences/technologies
/// Matches C++ ScienceStore from Science.h lines 55-110
pub struct ScienceStore {
    sciences: Vec<ScienceInfo>,
    name_to_science: HashMap<AsciiString, ScienceType>,
}

impl ScienceStore {
    pub fn new() -> Self {
        Self {
            sciences: Vec::new(),
            name_to_science: HashMap::new(),
        }
    }

    /// Initialize the science store
    pub fn init(&mut self) {
        // Calculate root sciences for all sciences after loading
        let sciences_clone: Vec<ScienceInfo> = self.sciences.clone();
        let store_snapshot = ScienceStore {
            sciences: sciences_clone.clone(),
            name_to_science: self.name_to_science.clone(),
        };

        for science_info in &mut self.sciences {
            science_info.root_sciences.clear();
            // Collect roots using an intermediate vector and collect()
            let roots: Vec<ScienceType> = {
                let mut tmp = Vec::new();
                science_info.add_root_sciences(&mut tmp, &store_snapshot);
                tmp
            };
            science_info.root_sciences = roots;
        }
    }

    /// Reset the science store
    pub fn reset(&mut self) {
        self.sciences.clear();
        self.name_to_science.clear();
    }

    /// Check if a science type is valid
    /// Matches C++ ScienceStore::isValidScience
    pub fn is_valid_science(&self, st: ScienceType) -> bool {
        self.find_science_info(st).is_some()
    }

    /// Check if a science is grantable
    /// Matches C++ ScienceStore::isScienceGrantable
    pub fn is_science_grantable(&self, st: ScienceType) -> bool {
        if let Some(info) = self.find_science_info(st) {
            info.grantable
        } else {
            false
        }
    }

    /// Get the name and description for a science
    /// Matches C++ ScienceStore::getNameAndDescription
    pub fn get_name_and_description(
        &self,
        st: ScienceType,
    ) -> Option<(UnicodeString, UnicodeString)> {
        self.find_science_info(st)
            .map(|info| (info.name.clone(), info.description.clone()))
    }

    /// Get the purchase cost for a science
    /// Matches C++ ScienceStore::getSciencePurchaseCost
    pub fn get_science_purchase_cost(&self, science: ScienceType) -> i32 {
        if let Some(info) = self.find_science_info(science) {
            info.science_purchase_point_cost
        } else {
            0
        }
    }

    /// Get science type from internal name
    /// Matches C++ ScienceStore::getScienceFromInternalName
    pub fn get_science_from_internal_name(&self, name: &str) -> Option<ScienceType> {
        self.name_to_science.get(&AsciiString::from(name)).copied()
    }

    /// Get internal name for a science
    /// Matches C++ ScienceStore::getInternalNameForScience
    pub fn get_internal_name_for_science(&self, science: ScienceType) -> Option<AsciiString> {
        for (name, st) in &self.name_to_science {
            if *st == science {
                return Some(name.clone());
            }
        }
        None
    }

    /// Lookup a science by name (for INI parsing only)
    /// Matches C++ ScienceStore::friend_lookupScience from Science.cpp lines 335-343
    pub fn friend_lookup_science(&self, science_name: &str) -> ScienceResult<ScienceType> {
        let science = self
            .get_science_from_internal_name(science_name)
            .ok_or_else(|| {
                ScienceError::ParseError(format!(
                    "Science name {} not known! (Did you define it in Science.ini?)",
                    science_name
                ))
            })?;

        if !self.is_valid_science(science) {
            return Err(ScienceError::InvalidName);
        }

        Ok(science)
    }

    /// Get all science names (for WorldBuilder only)
    /// Matches C++ ScienceStore::friend_getScienceNames from Science.cpp lines 88-99
    pub fn friend_get_science_names(&self) -> Vec<AsciiString> {
        let mut names = Vec::new();
        for (name, _) in &self.name_to_science {
            names.push(name.clone());
        }
        names
    }

    /// Find science info by science type
    /// Matches C++ ScienceStore::findScienceInfo from Science.cpp lines 124-135
    fn find_science_info(&self, st: ScienceType) -> Option<&ScienceInfo> {
        self.sciences.iter().find(|info| info.science == st)
    }

    /// Find mutable science info by science type
    fn find_science_info_mut(&mut self, st: ScienceType) -> Option<&mut ScienceInfo> {
        self.sciences.iter_mut().find(|info| info.science == st)
    }

    /// Add or update a science
    pub fn add_science(&mut self, name: AsciiString, info: ScienceInfo) -> ScienceResult<()> {
        // Register the name-to-science mapping
        self.name_to_science.insert(name, info.science);

        // Find existing science or add new one
        if let Some(existing) = self.find_science_info_mut(info.science) {
            *existing = info;
        } else {
            self.sciences.push(info);
        }

        Ok(())
    }
}

impl Default for ScienceStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global science store
static SCIENCE_STORE: OnceCell<RwLock<ScienceStore>> = OnceCell::new();

/// Get the global science store
pub fn get_science_store() -> RwLockReadGuard<'static, ScienceStore> {
    SCIENCE_STORE
        .get_or_init(|| RwLock::new(ScienceStore::new()))
        .read()
        .unwrap()
}

/// Get mutable access to the global science store
pub fn get_science_store_mut() -> RwLockWriteGuard<'static, ScienceStore> {
    SCIENCE_STORE
        .get_or_init(|| RwLock::new(ScienceStore::new()))
        .write()
        .unwrap()
}

fn parse_cpp_bool(field_name: &str, value: &str) -> ScienceResult<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(ScienceError::ParseError(format!(
            "{}: invalid boolean token '{}' (expected Yes or No)",
            field_name, value
        ))),
    }
}

/// Parse a science definition from INI
/// Matches C++ ScienceStore::friend_parseScienceDefinition from Science.cpp lines 138-214
/// Field parse table from Science.cpp lines 147-155
pub fn parse_science_definition(
    name: &str,
    properties: &HashMap<String, String>,
) -> ScienceResult<ScienceInfo> {
    // Generate science type from name using NameKey hashing (C++ NameKeyType).
    let science_type = ScienceType(NameKeyGenerator::name_to_key(name) as i32);

    let mut info = ScienceInfo::new(science_type);

    // Parse all fields from the properties map
    // Field parse table from Science.cpp lines 147-155:
    // - PrerequisiteSciences (parseScienceVector)
    // - SciencePurchasePointCost (parseInt)
    // - IsGrantable (parseBool)
    // - DisplayName (parseAndTranslateLabel)
    // - Description (parseAndTranslateLabel)

    for (key, value) in properties {
        match key.as_str() {
            "DisplayName" => {
                info.name = UnicodeString::from(value.as_str());
            }
            "Description" => {
                info.description = UnicodeString::from(value.as_str());
            }
            "SciencePurchasePointCost" => {
                info.science_purchase_point_cost = value.parse().map_err(|e| {
                    ScienceError::ParseError(format!("SciencePurchasePointCost: {}", e))
                })?;
            }
            "IsGrantable" => {
                info.grantable = parse_cpp_bool(key, value)?;
            }
            "PrerequisiteSciences" => {
                // Parse space-separated list of science names
                let prereq_names: Vec<&str> = value.split_whitespace().collect();
                for prereq_name in prereq_names {
                    let prereq_science =
                        ScienceType(NameKeyGenerator::name_to_key(prereq_name) as i32);
                    info.prereq_sciences.push(prereq_science);
                }
            }
            _ => {
                // Unknown field - log warning but don't fail
                eprintln!("Warning: Unknown science field: {}", key);
            }
        }
    }

    Ok(info)
}

/// Helper function to parse a vector of sciences from a string
/// Matches C++ INI::parseScienceVector from INI.cpp lines 674-685
pub fn parse_science_vector(value: &str) -> ScienceResult<Vec<ScienceType>> {
    let mut sciences = Vec::new();
    let tokens: Vec<&str> = value.split_whitespace().collect();

    for token in tokens {
        if token.is_empty() || token.eq_ignore_ascii_case("None") {
            continue;
        }

        let science = ScienceType(NameKeyGenerator::name_to_key(token) as i32);
        sciences.push(science);
    }

    Ok(sciences)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_science_type_validity() {
        let valid = ScienceType(1);
        let invalid = ScienceType::INVALID;

        assert!(valid.is_valid());
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_science_info_creation() {
        let science = ScienceType(42);
        let info = ScienceInfo::new(science);

        assert_eq!(info.science, science);
        assert_eq!(info.science_purchase_point_cost, 0);
        assert!(info.grantable);
        assert!(info.prereq_sciences.is_empty());
    }

    #[test]
    fn test_science_store_operations() {
        let mut store = ScienceStore::new();
        let science = ScienceType(1);
        let info = ScienceInfo::new(science);

        store
            .add_science(AsciiString::from("TestScience"), info)
            .unwrap();

        assert!(store.is_valid_science(science));
        assert!(store.is_science_grantable(science));
    }

    #[test]
    fn test_parse_science_definition() {
        let mut props = HashMap::new();
        props.insert("DisplayName".to_string(), "Test Science".to_string());
        props.insert("Description".to_string(), "A test science".to_string());
        props.insert("SciencePurchasePointCost".to_string(), "100".to_string());
        props.insert("IsGrantable".to_string(), "Yes".to_string());

        let result = parse_science_definition("TestScience", &props);
        assert!(result.is_ok());

        let info = result.unwrap();
        assert_eq!(info.science_purchase_point_cost, 100);
        assert!(info.grantable);

        props.insert("IsGrantable".to_string(), "No".to_string());
        let info = parse_science_definition("TestScienceNo", &props).unwrap();
        assert!(!info.grantable);
    }

    #[test]
    fn science_is_grantable_rejects_invalid_cpp_bool() {
        let mut props = HashMap::new();
        props.insert("IsGrantable".to_string(), "maybe".to_string());

        assert!(parse_science_definition("BadScience", &props).is_err());
    }

    #[test]
    fn test_parse_science_vector() {
        let result = parse_science_vector("Science1 Science2 Science3");
        assert!(result.is_ok());

        let sciences = result.unwrap();
        assert_eq!(sciences.len(), 3);
    }

    #[test]
    fn test_root_sciences_calculation() {
        let mut store = ScienceStore::new();

        // Create a root science (no prerequisites)
        let root_science = ScienceType(1);
        let mut root_info = ScienceInfo::new(root_science);
        store
            .add_science(AsciiString::from("RootScience"), root_info.clone())
            .unwrap();

        // Create a derived science with the root as prerequisite
        let derived_science = ScienceType(2);
        let mut derived_info = ScienceInfo::new(derived_science);
        derived_info.prereq_sciences.push(root_science);

        let mut roots = Vec::new();
        derived_info.add_root_sciences(&mut roots, &store);

        // The root should be in the list
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0], root_science);
    }

    #[test]
    fn test_science_lookup() {
        let mut store = ScienceStore::new();
        let science = ScienceType(42);
        let info = ScienceInfo::new(science);

        store
            .add_science(AsciiString::from("TestScience"), info)
            .unwrap();

        let found = store.get_science_from_internal_name("TestScience");
        assert!(found.is_some());
        assert_eq!(found.unwrap(), science);

        let name = store.get_internal_name_for_science(science);
        assert!(name.is_some());
    }
}
