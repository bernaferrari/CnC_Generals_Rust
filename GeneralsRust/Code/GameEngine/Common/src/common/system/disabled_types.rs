////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Disabled Types System Implementation
//!
//! Manages runtime disabling of game objects, units, buildings, and other
//! game elements. Used for balancing, debugging, and content control.
//!
//! Rust conversion: 2025

use once_cell::sync::OnceCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::Mutex;

use crate::common::ascii_string::AsciiString;

/// Types of objects that can be disabled
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisableableType {
    Unit,
    Building,
    Upgrade,
    SpecialPower,
    Weapon,
    Module,
    Science,
    CommandButton,
}

impl DisableableType {
    /// Convert from string representation
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "unit" => Some(Self::Unit),
            "building" => Some(Self::Building),
            "upgrade" => Some(Self::Upgrade),
            "specialpower" => Some(Self::SpecialPower),
            "weapon" => Some(Self::Weapon),
            "module" => Some(Self::Module),
            "science" => Some(Self::Science),
            "commandbutton" => Some(Self::CommandButton),
            _ => None,
        }
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unit => "Unit",
            Self::Building => "Building",
            Self::Upgrade => "Upgrade",
            Self::SpecialPower => "SpecialPower",
            Self::Weapon => "Weapon",
            Self::Module => "Module",
            Self::Science => "Science",
            Self::CommandButton => "CommandButton",
        }
    }
}

/// Reason for disabling an object
#[derive(Debug, Clone, PartialEq)]
pub enum DisableReason {
    Balance,        // Game balance adjustment
    Debug,          // Debug/testing purposes
    ContentFilter,  // Content filtering/restriction
    Performance,    // Performance optimization
    Experimental,   // Experimental features
    UserRequest,    // User/admin requested
    Custom(String), // Custom reason
}

impl DisableReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Balance => "Balance",
            Self::Debug => "Debug",
            Self::ContentFilter => "ContentFilter",
            Self::Performance => "Performance",
            Self::Experimental => "Experimental",
            Self::UserRequest => "UserRequest",
            Self::Custom(s) => s,
        }
    }
}

/// Information about a disabled object
#[derive(Debug, Clone)]
pub struct DisableInfo {
    pub name: AsciiString,
    pub object_type: DisableableType,
    pub reason: DisableReason,
    pub description: AsciiString,
    pub disabled_time: u64, // Timestamp when disabled
}

impl DisableInfo {
    pub fn new(name: AsciiString, object_type: DisableableType, reason: DisableReason) -> Self {
        Self {
            name,
            object_type,
            reason,
            description: AsciiString::new(),
            disabled_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn with_description(mut self, description: AsciiString) -> Self {
        self.description = description;
        self
    }
}

/// Manager for disabled types system
pub struct DisabledTypesManager {
    disabled_objects: HashMap<AsciiString, DisableInfo>,
    disabled_by_type: HashMap<DisableableType, HashSet<AsciiString>>,
    enabled: bool,
}

impl Default for DisabledTypesManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DisabledTypesManager {
    /// Create a new disabled types manager
    pub fn new() -> Self {
        Self {
            disabled_objects: HashMap::new(),
            disabled_by_type: HashMap::new(),
            enabled: true,
        }
    }

    /// Enable or disable the entire system
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if the system is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Disable an object
    pub fn disable_object(&mut self, info: DisableInfo) {
        let name = info.name.clone();
        let object_type = info.object_type;

        // Add to main collection
        self.disabled_objects.insert(name.clone(), info);

        // Add to type-based collection
        self.disabled_by_type
            .entry(object_type)
            .or_insert_with(HashSet::new)
            .insert(name);
    }

    /// Enable an object (remove from disabled list)
    pub fn enable_object(&mut self, name: &AsciiString) -> bool {
        if let Some(info) = self.disabled_objects.remove(name) {
            // Remove from type-based collection
            if let Some(set) = self.disabled_by_type.get_mut(&info.object_type) {
                set.remove(name);
                if set.is_empty() {
                    self.disabled_by_type.remove(&info.object_type);
                }
            }
            true
        } else {
            false
        }
    }

    /// Check if an object is disabled
    pub fn is_disabled(&self, name: &AsciiString) -> bool {
        self.enabled && self.disabled_objects.contains_key(name)
    }

    /// Get disable information for an object
    pub fn get_disable_info(&self, name: &AsciiString) -> Option<&DisableInfo> {
        self.disabled_objects.get(name)
    }

    /// Check if a type of object is disabled
    pub fn is_type_disabled(&self, object_type: DisableableType, name: &AsciiString) -> bool {
        if !self.enabled {
            return false;
        }

        self.disabled_by_type
            .get(&object_type)
            .map(|set| set.contains(name))
            .unwrap_or(false)
    }

    /// Get all disabled objects of a specific type
    pub fn get_disabled_objects_by_type(&self, object_type: DisableableType) -> Vec<&DisableInfo> {
        if let Some(names) = self.disabled_by_type.get(&object_type) {
            names
                .iter()
                .filter_map(|name| self.disabled_objects.get(name))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all disabled objects
    pub fn get_all_disabled_objects(&self) -> Vec<&DisableInfo> {
        self.disabled_objects.values().collect()
    }

    /// Get count of disabled objects
    pub fn get_disabled_count(&self) -> usize {
        self.disabled_objects.len()
    }

    /// Get count of disabled objects by type
    pub fn get_disabled_count_by_type(&self, object_type: DisableableType) -> usize {
        self.disabled_by_type
            .get(&object_type)
            .map(|set| set.len())
            .unwrap_or(0)
    }

    /// Clear all disabled objects
    pub fn clear_all(&mut self) {
        self.disabled_objects.clear();
        self.disabled_by_type.clear();
    }

    /// Clear disabled objects of a specific type
    pub fn clear_type(&mut self, object_type: DisableableType) {
        if let Some(names) = self.disabled_by_type.remove(&object_type) {
            for name in names {
                self.disabled_objects.remove(&name);
            }
        }
    }

    /// Clear disabled objects by reason
    pub fn clear_by_reason(&mut self, reason: &DisableReason) {
        let names_to_remove: Vec<AsciiString> = self
            .disabled_objects
            .iter()
            .filter(|(_, info)| &info.reason == reason)
            .map(|(name, _)| name.clone())
            .collect();

        for name in names_to_remove {
            self.enable_object(&name);
        }
    }

    /// Load disabled objects from configuration
    pub fn load_from_config(
        &mut self,
        config_data: &str,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let mut loaded_count = 0;

        for line in config_data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue; // Skip empty lines and comments
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 3 {
                let name = AsciiString::from(parts[0].trim());
                let type_str = parts[1].trim();
                let reason_str = parts[2].trim();

                if let Some(object_type) = DisableableType::from_str(type_str) {
                    let reason = match reason_str {
                        "Balance" => DisableReason::Balance,
                        "Debug" => DisableReason::Debug,
                        "ContentFilter" => DisableReason::ContentFilter,
                        "Performance" => DisableReason::Performance,
                        "Experimental" => DisableReason::Experimental,
                        "UserRequest" => DisableReason::UserRequest,
                        _ => DisableReason::Custom(reason_str.to_string()),
                    };

                    let mut info = DisableInfo::new(name, object_type, reason);

                    // Optional description
                    if parts.len() >= 4 {
                        info = info.with_description(AsciiString::from(parts[3].trim()));
                    }

                    self.disable_object(info);
                    loaded_count += 1;
                }
            }
        }

        Ok(loaded_count)
    }

    /// Save disabled objects to configuration format
    pub fn save_to_config(&self) -> String {
        let mut config = String::new();
        config.push_str("# Disabled Objects Configuration\n");
        config.push_str("# Format: Name, Type, Reason, Description\n\n");

        for info in self.disabled_objects.values() {
            config.push_str(&format!(
                "{}, {}, {}, {}\n",
                info.name.as_str(),
                info.object_type.as_str(),
                info.reason.as_str(),
                info.description.as_str()
            ));
        }

        config
    }

    /// Get statistics about disabled objects
    pub fn get_statistics(&self) -> DisabledTypesStatistics {
        let mut stats = DisabledTypesStatistics::default();

        stats.total_disabled = self.disabled_objects.len();

        for info in self.disabled_objects.values() {
            *stats.by_type.entry(info.object_type).or_insert(0) += 1;

            let reason_key = info.reason.as_str().to_string();
            *stats.by_reason.entry(reason_key).or_insert(0) += 1;
        }

        stats
    }
}

/// Statistics about disabled objects
#[derive(Debug, Default)]
pub struct DisabledTypesStatistics {
    pub total_disabled: usize,
    pub by_type: HashMap<DisableableType, usize>,
    pub by_reason: HashMap<String, usize>,
}

impl DisabledTypesStatistics {
    pub fn format_report(&self) -> String {
        let mut report = String::new();

        report.push_str(&format!("Disabled Objects Report\n"));
        report.push_str(&format!("Total Disabled: {}\n\n", self.total_disabled));

        report.push_str("By Type:\n");
        for (object_type, count) in &self.by_type {
            report.push_str(&format!("  {}: {}\n", object_type.as_str(), count));
        }

        report.push_str("\nBy Reason:\n");
        for (reason, count) in &self.by_reason {
            report.push_str(&format!("  {}: {}\n", reason, count));
        }

        report
    }
}

/// Global disabled types manager
static DISABLED_TYPES_MANAGER: OnceCell<Mutex<DisabledTypesManager>> = OnceCell::new();

/// Initialize the global disabled types manager
pub fn init_disabled_types_manager() {
    let manager = DisabledTypesManager::new();
    if DISABLED_TYPES_MANAGER.set(Mutex::new(manager)).is_err() {
        if let Some(existing) = DISABLED_TYPES_MANAGER.get() {
            if let Ok(mut guard) = existing.lock() {
                *guard = DisabledTypesManager::new();
            }
        }
    }
}

/// C++ parity entrypoint for startup mask initialization (`initDisabledMasks`).
///
/// In the original code this initializes global `DISABLEDMASK_ALL`.
/// Rust uses bitflags `all()` semantics, so this keeps parity as a lightweight
/// startup hook while ensuring the disabled-types manager is reset.
pub fn init_disabled_masks() {
    init_disabled_types_manager();
}

/// Get reference to the global disabled types manager
pub fn get_disabled_types_manager() -> Option<std::sync::MutexGuard<'static, DisabledTypesManager>>
{
    DISABLED_TYPES_MANAGER
        .get()
        .and_then(|manager| manager.lock().ok())
}

/// Convenience functions for checking disabled status
pub fn is_unit_disabled(name: &str) -> bool {
    if let Some(manager) = get_disabled_types_manager() {
        let ascii_name = AsciiString::from(name);
        manager.is_type_disabled(DisableableType::Unit, &ascii_name)
    } else {
        false
    }
}

pub fn is_building_disabled(name: &str) -> bool {
    if let Some(manager) = get_disabled_types_manager() {
        let ascii_name = AsciiString::from(name);
        manager.is_type_disabled(DisableableType::Building, &ascii_name)
    } else {
        false
    }
}

pub fn is_upgrade_disabled(name: &str) -> bool {
    if let Some(manager) = get_disabled_types_manager() {
        let ascii_name = AsciiString::from(name);
        manager.is_type_disabled(DisableableType::Upgrade, &ascii_name)
    } else {
        false
    }
}

pub fn is_special_power_disabled(name: &str) -> bool {
    if let Some(manager) = get_disabled_types_manager() {
        let ascii_name = AsciiString::from(name);
        manager.is_type_disabled(DisableableType::SpecialPower, &ascii_name)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disableable_type_from_str() {
        assert_eq!(
            DisableableType::from_str("unit"),
            Some(DisableableType::Unit)
        );
        assert_eq!(
            DisableableType::from_str("BUILDING"),
            Some(DisableableType::Building)
        );
        assert_eq!(DisableableType::from_str("invalid"), None);
    }

    #[test]
    fn test_disable_info() {
        let name = AsciiString::from("TestUnit");
        let info = DisableInfo::new(name.clone(), DisableableType::Unit, DisableReason::Balance)
            .with_description(AsciiString::from("Too powerful"));

        assert_eq!(info.name, name);
        assert_eq!(info.object_type, DisableableType::Unit);
        assert_eq!(info.reason, DisableReason::Balance);
        assert_eq!(info.description.as_str(), "Too powerful");
    }

    #[test]
    fn test_disabled_types_manager() {
        let mut manager = DisabledTypesManager::new();
        let name = AsciiString::from("TestUnit");

        assert!(!manager.is_disabled(&name));

        let info = DisableInfo::new(name.clone(), DisableableType::Unit, DisableReason::Balance);
        manager.disable_object(info);

        assert!(manager.is_disabled(&name));
        assert!(manager.is_type_disabled(DisableableType::Unit, &name));
        assert_eq!(manager.get_disabled_count(), 1);
        assert_eq!(manager.get_disabled_count_by_type(DisableableType::Unit), 1);

        assert!(manager.enable_object(&name));
        assert!(!manager.is_disabled(&name));
        assert_eq!(manager.get_disabled_count(), 0);
    }

    #[test]
    fn test_config_loading() {
        let mut manager = DisabledTypesManager::new();
        let config_data = r#"
# Test configuration
TestUnit, Unit, Balance, Too powerful
TestBuilding, Building, Debug, Testing purposes
InvalidLine
TestUpgrade, Upgrade, Custom Reason, Custom description
"#;

        let loaded = manager.load_from_config(config_data).unwrap();
        assert_eq!(loaded, 3); // Should load 3 valid entries

        assert!(manager.is_disabled(&AsciiString::from("TestUnit")));
        assert!(manager.is_disabled(&AsciiString::from("TestBuilding")));
        assert!(manager.is_disabled(&AsciiString::from("TestUpgrade")));
    }

    #[test]
    fn test_config_saving() {
        let mut manager = DisabledTypesManager::new();

        let info1 = DisableInfo::new(
            AsciiString::from("TestUnit"),
            DisableableType::Unit,
            DisableReason::Balance,
        )
        .with_description(AsciiString::from("Too powerful"));

        manager.disable_object(info1);

        let config = manager.save_to_config();
        assert!(config.contains("TestUnit"));
        assert!(config.contains("Unit"));
        assert!(config.contains("Balance"));
    }

    #[test]
    fn test_statistics() {
        let mut manager = DisabledTypesManager::new();

        manager.disable_object(DisableInfo::new(
            AsciiString::from("Unit1"),
            DisableableType::Unit,
            DisableReason::Balance,
        ));

        manager.disable_object(DisableInfo::new(
            AsciiString::from("Unit2"),
            DisableableType::Unit,
            DisableReason::Debug,
        ));

        manager.disable_object(DisableInfo::new(
            AsciiString::from("Building1"),
            DisableableType::Building,
            DisableReason::Balance,
        ));

        let stats = manager.get_statistics();
        assert_eq!(stats.total_disabled, 3);
        assert_eq!(stats.by_type.get(&DisableableType::Unit), Some(&2));
        assert_eq!(stats.by_type.get(&DisableableType::Building), Some(&1));
        assert_eq!(stats.by_reason.get("Balance"), Some(&2));
        assert_eq!(stats.by_reason.get("Debug"), Some(&1));
    }

    #[test]
    fn test_clear_operations() {
        let mut manager = DisabledTypesManager::new();

        manager.disable_object(DisableInfo::new(
            AsciiString::from("Unit1"),
            DisableableType::Unit,
            DisableReason::Balance,
        ));

        manager.disable_object(DisableInfo::new(
            AsciiString::from("Building1"),
            DisableableType::Building,
            DisableReason::Debug,
        ));

        assert_eq!(manager.get_disabled_count(), 2);

        manager.clear_type(DisableableType::Unit);
        assert_eq!(manager.get_disabled_count(), 1);
        assert!(!manager.is_disabled(&AsciiString::from("Unit1")));
        assert!(manager.is_disabled(&AsciiString::from("Building1")));

        manager.clear_by_reason(&DisableReason::Debug);
        assert_eq!(manager.get_disabled_count(), 0);
    }
}
